//! Dual Strike's two tag screens, converted at run time from the .nds (the
//! Dual Strike pack) and shown full screen on AW2's map (crate::tag_extras
//! decides when):
//!
//! - **The Tag Power's screen**: Dual Strike's bokeh background
//!   (`ohashi/res_tagbreak`: LZ77 tiles at +0x380, its map at +0x2628, its
//!   palette the file's last 32 bytes), the army's emblem faint over it
//!   (`res_tagbreak_union` for Allied Nations COs, `_black` for Black
//!   Hole's, `_mix` for one of each: tiles, a 32x64 map, two palettes), the
//!   two COs' Dual Strike body art facing each other (the art looks left:
//!   the active CO, on the left, mirrored), the pair's Tag Power name in Dual Strike's tag font
//!   (`res_tagbreakfont`: 32x32 glyphs, A..Z then a..z from 32) and
//!   "POWER 1xx%" in AW2's font on a white plate, as Dual Strike's box.
//! - **CO SWAP**: Dual Strike's red, the incoming CO's body art and the
//!   "CO★SWAP" logo in Dual Strike's change font (`res_changefont`: 16x32
//!   glyphs, A..Z then the star).
//!
//! Dual Strike draws them over both of its 256x192 screens; AW2's 240x160
//! shows the COs at Dual Strike's scale from the head down (the body art's
//! top 160 rows, as AW2's own CO screens do) with the name across them.
//! The picture is composed in colour, then fitted to AW2's layer form
//! ([`crate::ds_story_art::fit`]: 4bpp tiles, nine palettes, a 30x20
//! tilemap) and drawn on BG0 alone (BG1..3 and sprites off): the tiles from
//! BG0's character base, the map at its screen base, BG palettes 6..14.
//! What it covers (the tiles, the map, the BG palettes) is copied to the
//! ROM image's free space ([`BACKUP`]) first and put back after.
//!
//! Each picture depends only on the pack and the COs, so it is made once
//! and kept ([`cache`]); every console makes the same.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use mgba::core::Core;

use crate::ds_story_art::{fit, Picture, HEIGHT, WIDTH};

type Rgb = [i32; 3];

// --- The pack's pieces ------------------------------------------------------------

/// LZ77 (type 0x10) at the start of `b`: the data and the bytes read.
fn lz(b: &[u8]) -> Option<(Vec<u8>, usize)> {
    if b.first() != Some(&0x10) || b.len() < 4 {
        return None;
    }
    let size = u32::from_le_bytes([b[1], b[2], b[3], 0]) as usize;
    let mut out = Vec::with_capacity(size);
    let mut p = 4;
    while out.len() < size {
        let flags = *b.get(p)?;
        p += 1;
        for bit in 0..8 {
            if out.len() >= size {
                break;
            }
            if flags & (0x80 >> bit) != 0 {
                let (b1, b2) = (*b.get(p)? as usize, *b.get(p + 1)? as usize);
                p += 2;
                let disp = ((b1 & 15) << 8 | b2) + 1;
                if disp > out.len() {
                    return None;
                }
                for _ in 0..(b1 >> 4) + 3 {
                    out.push(out[out.len() - disp]);
                }
            } else {
                out.push(*b.get(p)?);
                p += 1;
            }
        }
    }
    Some((out, p))
}

/// An `ohashi/` resource: its LZ77 blocks one after another (each from a
/// word boundary), then the rest (palettes).
fn blocks(raw: &[u8]) -> (Vec<Vec<u8>>, &[u8]) {
    let mut out = Vec::new();
    let mut o = 0;
    while o < raw.len() {
        match lz(&raw[o..]) {
            Some((d, n)) => {
                out.push(d);
                o = (o + n + 3) & !3;
            }
            None => break,
        }
    }
    (out, &raw[o.min(raw.len())..])
}

fn colour(c: u16) -> Rgb {
    [(c & 31) as i32, ((c >> 5) & 31) as i32, ((c >> 10) & 31) as i32]
}

fn palette(b: &[u8]) -> Vec<Rgb> {
    b.chunks(2).filter(|c| c.len() == 2).map(|c| colour(u16::from_le_bytes([c[0], c[1]]))).collect()
}

/// A 4bpp tile's pixel (x, y), flips applied.
fn tile_px(tiles: &[u8], k: usize, x: usize, y: usize, hf: bool, vf: bool) -> u8 {
    let (sx, sy) = (if hf { 7 - x } else { x }, if vf { 7 - y } else { y });
    tiles.get(32 * k + 4 * sy + sx / 2).map_or(0, |b| (b >> (4 * (sx & 1))) & 15)
}

/// A layer: `w` x `h` pixels, None where transparent.
struct Layer {
    w: usize,
    h: usize,
    px: Vec<Option<Rgb>>,
}

impl Layer {
    fn from_map(tiles: &[u8], map: &[u8], pals: &[Rgb], cols: usize) -> Layer {
        let rows = map.len() / 2 / cols;
        let (w, h) = (8 * cols, 8 * rows);
        let mut px = vec![None; w * h];
        for c in 0..cols * rows {
            let e = u16::from_le_bytes([map[2 * c], map[2 * c + 1]]);
            let (k, hf, vf, p) = ((e & 0x3FF) as usize, e & 0x400 != 0, e & 0x800 != 0, (e >> 12) as usize);
            for y in 0..8 {
                for x in 0..8 {
                    let v = tile_px(tiles, k, x, y, hf, vf) as usize;
                    if v != 0 {
                        px[w * (8 * (c / cols) + y) + 8 * (c % cols) + x] = pals.get(16 * p + v).copied();
                    }
                }
            }
        }
        Layer { w, h, px }
    }

    fn at(&self, x: i32, y: i32) -> Option<Rgb> {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return None;
        }
        self.px[self.w * y as usize + x as usize]
    }
}

/// The bokeh: `res_tagbreak`'s second picture (its first part is the
/// sprites of Dual Strike's power meter).
fn bokeh() -> Option<Layer> {
    let raw = crate::ds_pack::pack()?.file("ohashi/res_tagbreak")?;
    let (tiles, _) = lz(raw.get(0x380..)?)?;
    let (map, _) = lz(raw.get(0x2628..)?)?;
    if tiles.len() != 0x4000 || map.len() != 0x800 || raw.len() < 32 {
        return None;
    }
    let pals = palette(&raw[raw.len() - 32..]);
    Some(Layer::from_map(&tiles, &map, &pals, 32))
}

/// Which emblem: Allied Nations, Black Hole, or the mix.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Side {
    Union,
    Black,
    Mix,
}

fn emblem(side: Side) -> Option<Layer> {
    let name = match side {
        Side::Union => "ohashi/res_tagbreak_union",
        Side::Black => "ohashi/res_tagbreak_black",
        Side::Mix => "ohashi/res_tagbreak_mix",
    };
    let (b, tail) = blocks(crate::ds_pack::pack()?.file(name)?);
    let (tiles, map) = (b.first()?, b.get(1)?);
    if map.len() != 0x1000 {
        return None;
    }
    Some(Layer::from_map(tiles, map, &palette(tail), 32))
}

/// Black Hole's COs by Dual Strike id: Von Bolt, Jugger, Lash, Koal,
/// Hawke, Kindle, Flak, Adder (and AW2's Sturm, who has none).
fn black_hole(co: u8) -> bool {
    match crate::tag::ds_id(co) {
        Some(d) => matches!(d, 11 | 12 | 13 | 14 | 15 | 25 | 26 | 27),
        None => true,
    }
}

/// A font: glyphs of `gw` x `gh` pixels (tiles in rows), colour indices.
struct Font {
    tiles: Vec<u8>,
    gw: usize,
    gh: usize,
    pals: Vec<Rgb>,
}

impl Font {
    fn load(name: &str, gw: usize, gh: usize) -> Option<Font> {
        let (b, tail) = blocks(crate::ds_pack::pack()?.file(name)?);
        let tiles: Vec<u8> = b.concat();
        let pals = palette(tail);
        (tiles.len() >= 32 * (gw / 8) * (gh / 8) * 27 && pals.len() >= 16).then_some(Font { tiles, gw, gh, pals })
    }

    fn glyphs(&self) -> usize {
        self.tiles.len() / (32 * (self.gw / 8) * (self.gh / 8))
    }

    fn px(&self, g: usize, x: usize, y: usize) -> u8 {
        let tw = self.gw / 8;
        let k = g * tw * (self.gh / 8) + (y / 8) * tw + x / 8;
        tile_px(&self.tiles, k, x % 8, y % 8, false, false)
    }

    /// The glyph's drawn columns (first, last), None if blank.
    fn extent(&self, g: usize) -> Option<(usize, usize)> {
        let cols: Vec<usize> = (0..self.gw).filter(|&x| (0..self.gh).any(|y| self.px(g, x, y) != 0)).collect();
        Some((*cols.first()?, *cols.last()?))
    }
}

/// Text in a Dual Strike font, its glyphs side by side (each its drawn
/// width, `gap` apart; a space `space` wide; characters the font lacks
/// skipped), as a layer `gh` high. `glyph_of` gives a glyph and how many
/// of its rows to draw (an apostrophe: the top of an `l`).
fn set_text(font: &Font, text: &[u8], glyph_of: impl Fn(u8) -> Option<(usize, usize)>, gap: usize, space: usize) -> Layer {
    let mut cols: Vec<Vec<Option<Rgb>>> = Vec::new();
    for &c in text {
        if c == b' ' {
            cols.extend((0..space).map(|_| vec![None; font.gh]));
            continue;
        }
        let Some((g, rows)) = glyph_of(c).filter(|&(g, _)| g < font.glyphs()) else { continue };
        let Some((x0, x1)) = font.extent(g) else { continue };
        if !cols.is_empty() {
            cols.extend((0..gap).map(|_| vec![None; font.gh]));
        }
        for x in x0..=x1 {
            let px = |y: usize| Some(font.px(g, x, y)).filter(|&v| v != 0 && y < rows).map(|v| font.pals[v as usize]);
            cols.push((0..font.gh).map(px).collect());
        }
    }
    let (w, h) = (cols.len(), font.gh);
    let mut px = vec![None; w * h];
    for (x, col) in cols.iter().enumerate() {
        for (y, p) in col.iter().enumerate() {
            px[w * y + x] = *p;
        }
    }
    Layer { w, h, px }
}

/// A layer squeezed (nearest pixel) to at most `max_w` wide.
fn squeeze(l: Layer, max_w: usize) -> Layer {
    if l.w <= max_w {
        return l;
    }
    let mut px = vec![None; max_w * l.h];
    for y in 0..l.h {
        for x in 0..max_w {
            px[max_w * y + x] = l.px[l.w * y + x * l.w / max_w];
        }
    }
    Layer { w: max_w, h: l.h, px }
}

/// A CO's body, 128x160 (the head at the top), from Dual Strike's art (or
/// AW2's for a CO it lacks), colour scheme 0. Both face left as stored
/// (measured against Dual Strike's captures: its CO SWAP and the tag
/// screen's right-hand CO draw the art as stored).
fn body(core: &Core, co: u8) -> Option<Layer> {
    let ds = crate::tag::ds_id(co).and_then(crate::ds_co_art::co_art);
    let (top, bottom, pal) = match ds {
        Some(a) => (a.body_top, a.body_bottom, a.palette[..32].to_vec()),
        None => {
            let row = core.raw_read_32(PRESENTATION_POOL, -1) + PRESENTATION_ROW * co as u32;
            let pair = core.raw_read_32(row, -1);
            let top = crate::invention_art::lz77(core, core.raw_read_32(pair, -1));
            let bottom = crate::invention_art::lz77(core, core.raw_read_32(pair + 4, -1));
            let mut pal = vec![0u8; 32];
            core.raw_read_range(core.raw_read_32(row + 0x08, -1), -1, &mut pal);
            (top, bottom, pal)
        }
    };
    if top.len() < 128 * 32 || bottom.len() < 192 * 32 {
        return None;
    }
    let pals = palette(&pal);
    let mut px = vec![None; 128 * 160];
    // Six sprites: 64x64 at (0, 0) and (64, 0) (the top file), 64x64 at
    // (0, 64) and (64, 64), 64x32 at (0, 128) and (64, 128) (the bottom).
    let sprites: [(&[u8], usize, usize, usize); 6] = [
        (&top[..], 0, 0, 64),
        (&top[64 * 32..], 64, 0, 64),
        (&bottom[..], 0, 64, 64),
        (&bottom[64 * 32..], 64, 64, 64),
        (&bottom[128 * 32..], 0, 128, 32),
        (&bottom[160 * 32..], 64, 128, 32),
    ];
    for (tiles, x0, y0, h) in sprites {
        for k in 0..8 * (h / 8) {
            for y in 0..8 {
                for x in 0..8 {
                    let v = tile_px(tiles, k, x, y, false, false) as usize;
                    if v != 0 {
                        px[128 * (y0 + 8 * (k / 8) + y) + x0 + 8 * (k % 8) + x] = Some(pals[v]);
                    }
                }
            }
        }
    }
    Some(Layer { w: 128, h: 160, px })
}

const PRESENTATION_POOL: u32 = 0x0803_9B7C;
const PRESENTATION_ROW: u32 = 0x44;

// --- Composing -------------------------------------------------------------------------

struct Canvas(Vec<Rgb>);

impl Canvas {
    fn new(c: Rgb) -> Canvas {
        Canvas(vec![c; WIDTH * HEIGHT])
    }

    /// `l` drawn with its top left at (x0, y0), mirrored or not, at
    /// `alpha`/16 over what is there.
    fn draw(&mut self, l: &Layer, x0: i32, y0: i32, mirror: bool, alpha: i32) {
        for y in 0..HEIGHT as i32 {
            for x in 0..WIDTH as i32 {
                let lx = if mirror { l.w as i32 - 1 - (x - x0) } else { x - x0 };
                if let Some(c) = l.at(lx, y - y0) {
                    let o = &mut self.0[WIDTH * y as usize + x as usize];
                    for k in 0..3 {
                        o[k] = (c[k] * alpha + o[k] * (16 - alpha)) / 16;
                    }
                }
            }
        }
    }

    fn rect(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, c: Rgb) {
        for y in y0..y1.min(HEIGHT) {
            for x in x0..x1.min(WIDTH) {
                self.0[WIDTH * y + x] = c;
            }
        }
    }
}

/// Text in AW2's font (its glyphs at `0x084C32E4`, widths at
/// `0x084C36E4`), a layer 16 high.
fn aw2_text(core: &Core, s: &[u8], ink: Rgb) -> Layer {
    const GLYPHS: u32 = 0x084C_32E4;
    const WIDTHS: u32 = 0x084C_36E4;
    let w: usize = s.iter().map(|&c| if c == b' ' { 4 } else { core.raw_read_8(WIDTHS + c as u32, -1) as usize + 1 }).sum();
    let mut px = vec![None; w.max(1) * 16];
    let mut x = 0;
    for &c in s {
        if c == b' ' {
            x += 4;
            continue;
        }
        let cw = core.raw_read_8(WIDTHS + c as u32, -1) as usize;
        let at = core.raw_read_32(GLYPHS + 4 * c as u32, -1);
        if (0x0800_0000..0x0A00_0000).contains(&at) {
            let stride = cw.div_ceil(2);
            for r in 0..12usize {
                for cx in 0..cw {
                    let b = core.raw_read_8(at + (stride * (3 + r) + cx / 2) as u32, -1);
                    if (b >> (4 * (cx & 1))) & 15 == 0xA && x + cx < w {
                        px[w * (2 + r) + x + cx] = Some(ink);
                    }
                }
            }
        }
        x += cw + 1;
    }
    Layer { w: w.max(1), h: 16, px }
}

/// The tag font has letters only: an apostrophe ("Viper's Nest") is the
/// top of its `l`.
const APOSTROPHE_ROWS: usize = 13;

/// The Tag Power's screen for COs `a` (the active one, left) and `b`.
fn compose_tag(core: &Core, a: u8, b: u8, name: &[u8], power: u8) -> Option<Vec<Rgb>> {
    let mut cv = Canvas::new([28, 28, 31]);
    cv.draw(&bokeh()?, -8, -40, false, 16);
    let side = match (black_hole(a), black_hole(b)) {
        (false, false) => Side::Union,
        (true, true) => Side::Black,
        _ => Side::Mix,
    };
    // The emblem's circle (its centre at (128, 190) of its 256x512) at
    // the screen's centre, faint.
    cv.draw(&emblem(side)?, 120 - 128, 80 - 190, false, 5);
    // The COs facing each other, as Dual Strike's: the active CO on the
    // left, mirrored to look right; the partner on the right as stored,
    // looking left.
    cv.draw(&body(core, a)?, -6, 0, true, 16);
    cv.draw(&body(core, b)?, WIDTH as i32 - 122, 0, false, 16);
    let font = Font::load("ohashi/res_tagbreakfont", 32, 32)?;
    let glyph = |c: u8| match c {
        b'A'..=b'Z' => Some(((c - b'A') as usize, 32)),
        b'a'..=b'z' => Some((32 + (c - b'a') as usize, 32)),
        b'\'' => Some((32 + (b'l' - b'a') as usize, APOSTROPHE_ROWS)),
        _ => None,
    };
    let title = squeeze(set_text(&font, name, glyph, 0, 10), WIDTH - 8);
    cv.draw(&title, (WIDTH as i32 - title.w as i32) / 2, 92, false, 16);
    // "POWER 1xx%": a white plate at the bottom left, as Dual Strike's.
    let text = aw2_text(core, format!("POWER {power}%").as_bytes(), [4, 4, 8]);
    let (pw, py) = (text.w + 10, 142usize);
    cv.rect(3, py - 1, 3 + pw + 2, py + 17, [6, 6, 10]);
    cv.rect(4, py, 4 + pw, py + 16, [31, 31, 31]);
    cv.draw(&text, 9, py as i32, false, 16);
    Some(cv.0)
}

/// CO SWAP for incoming CO `to`.
fn compose_swap(core: &Core, to: u8) -> Option<Vec<Rgb>> {
    let mut cv = Canvas::new([31, 5, 0]);
    // As stored, as Dual Strike's.
    cv.draw(&body(core, to)?, (WIDTH as i32 - 128) / 2, 0, false, 16);
    let font = Font::load("ohashi/res_changefont", 16, 32)?;
    let glyph = |c: u8| match c {
        b'A'..=b'Z' => Some(((c - b'A') as usize, 32)),
        b'*' => Some((26, 32)),
        _ => None,
    };
    let logo = set_text(&font, b"CO*SWAP", glyph, 0, 6);
    cv.draw(&logo, (WIDTH as i32 - logo.w as i32) / 2, 88, false, 16);
    Some(cv.0)
}

// --- Cache ---------------------------------------------------------------------------------

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Screen {
    /// The active CO, its partner, the power's name, the compatibility.
    Tag(u8, u8, Vec<u8>, u8),
    /// The incoming CO.
    Swap(u8),
}

fn cache() -> &'static Mutex<HashMap<Screen, Option<Arc<Picture>>>> {
    static C: OnceLock<Mutex<HashMap<Screen, Option<Arc<Picture>>>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// The screen's picture in AW2's layer form (None without the pack's art).
pub fn picture(core: &Core, s: &Screen) -> Option<Arc<Picture>> {
    if let Some(p) = cache().lock().unwrap().get(s) {
        return p.clone();
    }
    let img = match s {
        Screen::Tag(a, b, name, power) => compose_tag(core, *a, *b, name, *power),
        Screen::Swap(to) => compose_swap(core, *to),
    };
    let pic = img.map(|i| Arc::new(fit(&i)));
    cache().lock().unwrap().insert(s.clone(), pic.clone());
    pic
}

/// The composed colours (for the review images).
pub fn colours(core: &Core, s: &Screen) -> Option<Vec<Rgb>> {
    match s {
        Screen::Tag(a, b, name, power) => compose_tag(core, *a, *b, name, *power),
        Screen::Swap(to) => compose_swap(core, *to),
    }
}

// --- On the screen -------------------------------------------------------------------

const VRAM: u32 = 0x0600_0000;
const BG0CNT: u32 = 0x0300_2B6C;
const DISPCNT: u32 = 0x0300_30CC;
const DISPCNT_IO: u32 = 0x0400_0000;
const BG0HOFS: u32 = 0x0300_1FF8;
const BG0VOFS: u32 = 0x0300_1418;
const PAL_BUFFER: u32 = 0x0300_20C0;
const PAL_RAM: u32 = 0x0500_0000;
/// The layer and window bits (BG0..3, sprites, windows).
const LAYERS: u16 = 0xFF00;
const BG0_ONLY: u16 = 0x0100;
/// What the screen covers, while it is up (ROM image free space,
/// `0x08790000..0x0879FFFF`): the tiles, then the map, then the BG
/// palettes (buffer and RAM).
pub const BACKUP: u32 = 0x0879_0000;
const BACKUP_TILES: u32 = 0x8000;
const BACKUP_MAP: u32 = BACKUP + BACKUP_TILES;
const BACKUP_PAL: u32 = BACKUP_MAP + 0x800;
pub const BACKUP_END: u32 = BACKUP_PAL + 0x400;
/// Frames of the fade in from white.
const FADE: u16 = 10;

/// RAM (crate::tag_extras's): the screen is up (1), the display's bits
/// before it, BG0's scroll before it.
pub struct Ram {
    pub up: u32,
    pub disp: u32,
    pub hofs: u32,
    pub vofs: u32,
}

fn bg0(core: &Core) -> (u32, u32) {
    let c = core.raw_read_16(BG0CNT, -1) as u32;
    (VRAM + 0x4000 * ((c >> 2) & 3), VRAM + 0x800 * ((c >> 8) & 31))
}

/// The tiles BG0 can take below its map (or the end of BG VRAM).
fn room(core: &Core) -> usize {
    let (chars, map) = bg0(core);
    let end = if map > chars { map } else { VRAM + 0x1_0000 };
    (((end - chars) / 32) as usize).min(1024).min(BACKUP_TILES as usize / 32)
}

fn copy(core: &mut Core, from: u32, to: u32, len: u32) {
    let mut b = vec![0u8; len as usize];
    core.raw_read_range(from, -1, &mut b);
    core.raw_write_range(to, -1, &b);
}

fn write_if_changed(core: &mut Core, at: u32, b: &[u8]) {
    let mut now = vec![0u8; b.len()];
    core.raw_read_range(at, -1, &mut now);
    if now != b {
        core.raw_write_range(at, -1, b);
    }
}

/// Whether the picture fits BG0 here.
pub fn fits(core: &Core, pic: &Picture) -> bool {
    pic.tiles.len() / 32 <= room(core)
}

/// Frame `frame` of the picture (the first copies what it covers aside).
pub fn show(core: &mut Core, ram: &Ram, pic: &Picture, frame: u16) {
    let (chars, map) = bg0(core);
    let n = room(core) as u32 * 32;
    if core.raw_read_8(ram.up, -1) != 1 {
        copy(core, chars, BACKUP, n);
        copy(core, map, BACKUP_MAP, 0x800);
        copy(core, PAL_BUFFER, BACKUP_PAL, 0x200);
        copy(core, PAL_RAM, BACKUP_PAL + 0x200, 0x200);
        core.raw_write_16(ram.disp, -1, core.raw_read_16(DISPCNT, -1) & LAYERS);
        core.raw_write_16(ram.hofs, -1, core.raw_read_16(BG0HOFS, -1));
        core.raw_write_16(ram.vofs, -1, core.raw_read_16(BG0VOFS, -1));
        core.raw_write_8(ram.up, -1, 1);
    }
    write_if_changed(core, chars, &pic.tiles);
    let mut m = vec![0u8; 0x800];
    for (i, e) in pic.tilemap.iter().enumerate() {
        let at = 2 * (32 * (i / 30) + i % 30);
        m[at..at + 2].copy_from_slice(&e.to_le_bytes());
    }
    write_if_changed(core, map, &m);
    // BG palettes 6..14, faded in from white.
    let t = FADE.saturating_sub(frame) as i32;
    let mut p = Vec::with_capacity(pic.palettes.len() * 2);
    for &c in &pic.palettes {
        let c = colour(c).map(|v| v + (31 - v) * t / FADE as i32);
        p.extend_from_slice(&((c[0] | c[1] << 5 | c[2] << 10) as u16).to_le_bytes());
    }
    for base in [PAL_BUFFER, PAL_RAM] {
        write_if_changed(core, base + 32 * 6, &p);
        // The backdrop (palette 0 colour 0) black behind the picture.
        write_if_changed(core, base, &[0, 0]);
    }
    core.raw_write_16(BG0HOFS, -1, 0);
    core.raw_write_16(BG0VOFS, -1, 0);
    for reg in [DISPCNT, DISPCNT_IO] {
        let d = core.raw_read_16(reg, -1);
        if d & LAYERS != BG0_ONLY {
            core.raw_write_16(reg, -1, (d & !LAYERS) | BG0_ONLY);
        }
    }
}

/// The picture taken away: what it covered put back.
pub fn hide(core: &mut Core, ram: &Ram) {
    if core.raw_read_8(ram.up, -1) != 1 {
        return;
    }
    let (chars, map) = bg0(core);
    let n = room(core) as u32 * 32;
    copy(core, BACKUP, chars, n);
    copy(core, BACKUP_MAP, map, 0x800);
    copy(core, BACKUP_PAL, PAL_BUFFER, 0x200);
    copy(core, BACKUP_PAL + 0x200, PAL_RAM, 0x200);
    core.raw_write_16(BG0HOFS, -1, core.raw_read_16(ram.hofs, -1));
    core.raw_write_16(BG0VOFS, -1, core.raw_read_16(ram.vofs, -1));
    let saved = core.raw_read_16(ram.disp, -1);
    for reg in [DISPCNT, DISPCNT_IO] {
        let d = core.raw_read_16(reg, -1);
        core.raw_write_16(reg, -1, (d & !LAYERS) | saved);
    }
    core.raw_write_8(ram.up, -1, 0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_in_its_range() {
        assert!(BACKUP_END <= 0x087A_0000);
        assert!(BACKUP >= crate::tag::ROM + 0x1_0000);
    }

    #[test]
    fn lz_blocks() {
        // One literal block of 4 bytes, padded, then a tail.
        let raw = [0x10, 4, 0, 0, 0x00, 1, 2, 3, 4, 0, 0, 0, 0xAA, 0xBB];
        let (b, tail) = blocks(&raw);
        assert_eq!(b, vec![vec![1, 2, 3, 4]]);
        assert_eq!(tail, &[0xAA, 0xBB]);
    }
}
