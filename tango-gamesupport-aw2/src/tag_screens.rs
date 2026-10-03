//! Dual Strike's two tag screens, animated as Dual Strike animates them,
//! from the .nds's own pictures (the Dual Strike pack), converted at run
//! time; crate::tag_extras decides when they play and drives them a frame
//! at a time ([`frame`]).
//!
//! **What Dual Strike does** (recorded frame by frame in melonDS:
//! `tango-backend-melonds/examples/ds_script`, the display registers, OAM and
//! palettes dumped every frame; its frame numbers below count from the
//! menu's Tag / Change, 60 a second):
//!
//! - **The Tag Power** (both screens, about 9.6 s after the first CO's
//!   quote): the map dims and three bolts strike (`SE_TAG_BREAK` at 173, 209
//!   and 219; `BGM_TAG_BREAK_ALLY1` / `_ENEMY1`, sequences 46 / 47, start at
//!   256), the map brightens to white (257..288, master brightness 1..16)
//!   and holds; the screen fades in from white (304..316) on the army's
//!   emblem in full colour on white (`res_tagbreak_union` / `_black` /
//!   `_mix`); the two COs, head to foot, slide in vertically, the active CO
//!   (left, mirrored to look right) down from 94 pixels above, the partner
//!   (right, looking left) up from 94 below, easing to a stop (304..418);
//!   the POWER box rises from below the screen (373..382,
//!   `SE_TAGPT_COUNT01_INIT` at 381), the percentage counts up one a frame
//!   from 384 (`SE_TAGPT_COUNT01` every second frame) while the bar fills a
//!   pixel a percent (its fill colour cycling through `res_tagbreak`'s
//!   16-colour gradient, a step every 4 frames), the box turning orange at
//!   98..100%; meanwhile the bokeh (`res_tagbreak`'s background) blends in
//!   over the white and the emblem (431..456, the bokeh's weight 0 -> 14,
//!   the emblem's 16 -> 4: `BLDALPHA`'s EVA and EVB); at the final value the digits pop one after another
//!   (scaled 2x -> 1x over 8 frames, 495/498/502); 16 frames after the
//!   count, the power's name pops in a letter every 4 frames (each 2x -> 1x
//!   over 8 frames, `SE_TAG_BREAK_TYPE2` each; from 510); the emblem goes
//!   (627), a white burst (`res_tagchange`) blends in additively over the
//!   COs (629..644, EVA 1 -> 16, EVB 16; `SE_TAG_BREAK_EXPLOSE2` at 627),
//!   everything fades to white (655..679), and after a white hold the map
//!   comes back from white (704..716). The times after the count move with
//!   the pair's compatibility (Dual Strike counts to it; the capture is a
//!   110% pair). Not skippable (A, B and START do nothing).
//! - **CO SWAP** (the bottom screen, about 3.2 s after the tag-in line):
//!   the map fades to black (158..170), `SE_SYOGUN_CHANGE` (176), the red
//!   screen (`res_syogunchange`'s last palette, colour 1) fades in from
//!   black (177..201); the incoming CO (looking left, as stored) slides in
//!   from the right and the outgoing CO (mirrored, looking right) from the
//!   left, crossing to stop back to back (209..257), while "CO★SWAP"
//!   (`res_changefont`, 16x32 glyphs) opens a letter every 4 frames from
//!   208, each stretched from a line to its full height over 24 frames;
//!   the COs slide out apart (271..294) under a white burst
//!   (`res_syogunchange`, blended in additively 268..284), the screen fades
//!   to white (295..319) and the map comes back from white (335..347). Not
//!   skippable.
//!
//! **On the GBA** (240x160, one screen) every element keeps Dual Strike's
//! pixels, scale and timing; the two screens' composition is cut to the
//! window the COs' heads are in, with the name and the POWER box moved up
//! into it (the name across the COs' chests, the box at the bottom left
//! with its bar to the right, as on Dual Strike's bottom screen). The
//! screen is the console's own BG layers, all of BG VRAM (kept first in
//! the ROM image's free space, [`BACKUP`], and put back after):
//!
//! - the emblem on white (Dual Strike's tiles and map, an 8-aligned
//!   window), then the burst, raised over the COs and blended additively
//!   as Dual Strike's;
//! - the bokeh, alpha-blended over the emblem with Dual Strike's own
//!   EVA/EVB (the GBA's `BLDALPHA` is the DS's);
//! - the COs, drawn every frame at their places (a tile both COs share
//!   takes a palette made of both);
//! - on top, the name, the POWER box, its digits and bar (or CO★SWAP).
//!
//! Fades to and from white and black (Dual Strike's master brightness) are
//! the palettes' colours moved toward white or black; on the map, before
//! and after, they are the GBA's brightness effect (`BLDY`), and the bolts
//! are flashes of it (the bolts themselves are not drawn: they are a layer
//! over Dual Strike's 3D map). Sound effects are Dual Strike's, converted
//! with the music ([`crate::ds_music::tag_se`], played through AW2's
//! sound-effect call by crate::tag_extras). The tag music stays AW2's power
//! music.
//!
//! Everything a frame shows is a function of the screen (the COs, the
//! name, the compatibility) and its frame count in RAM (crate::tag_extras),
//! so every console and every replay draws the same, and nothing touches
//! the battle's random numbers.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use mgba::core::Core;

use crate::ds_co_art::{FIGURE_H, FIGURE_W};
use crate::ds_music::TagSe;

type Rgb = [i32; 3];

const W: usize = 240;
const H: usize = 160;
const COLS: usize = W / 8;
const ROWS: usize = H / 8;

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

fn rgb(c: u16) -> Rgb {
    [(c & 31) as i32, ((c >> 5) & 31) as i32, ((c >> 10) & 31) as i32]
}

fn bgr(c: Rgb) -> u16 {
    (c[0].clamp(0, 31) | (c[1].clamp(0, 31) << 5) | (c[2].clamp(0, 31) << 10)) as u16
}

fn d2(a: Rgb, b: Rgb) -> i32 {
    (0..3).map(|k| (a[k] - b[k]) * (a[k] - b[k])).sum()
}

fn pal16(b: &[u8]) -> Option<[u16; 16]> {
    let b = b.get(..32)?;
    let mut p = [0u16; 16];
    for (k, c) in p.iter_mut().enumerate() {
        *c = u16::from_le_bytes([b[2 * k], b[2 * k + 1]]);
    }
    Some(p)
}

fn pals(b: &[u8]) -> Vec<[u16; 16]> {
    b.chunks(32).filter_map(pal16).collect()
}

/// A 4bpp tile's pixel (x, y).
fn tile_px(tiles: &[u8], k: usize, x: usize, y: usize) -> u8 {
    tiles.get(32 * k + 4 * y + x / 2).map_or(0, |b| (b >> (4 * (x & 1))) & 15)
}

/// A Dual Strike BG layer: 4bpp tiles, a map `cols` entries wide, its
/// palettes.
struct Bg {
    tiles: Vec<u8>,
    map: Vec<u16>,
    cols: usize,
    pals: Vec<[u16; 16]>,
}

fn map16(b: &[u8]) -> Vec<u16> {
    b.chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect()
}

/// The POWER box: `res_tagbreak`'s first block (the box, the digits, the
/// bar), its four palettes (the box white, then orange in three steps) and
/// the bar's 16-colour gradient (raw, after the block).
struct Meter {
    tiles: Vec<u8>,
    pals: [[u16; 16]; 4],
    gradient: [u16; 16],
}

/// Tiles of the POWER box's block, as Dual Strike's sprites use them: the
/// bar's left cap filled 0..5 pixels (tiles 0, 2 .. 10), a middle piece
/// filled 0..8 (12 .. 28), the right cap 0..5 (30 .. 40); the digits 0..9
/// (16x16, from 42, 4 tiles each); the box (32x32 from 82, 16x32 from 98).
const BAR_LEFT: usize = 0;
const BAR_MID: usize = 12;
const BAR_RIGHT: usize = 30;
const DIGITS: usize = 42;
const BOX_LEFT: usize = 82;
const BOX_RIGHT: usize = 98;
/// The bar's pieces: a cap, 15 middles, a cap (130 pixels, one a percent).
const BAR_MIDDLES: usize = 15;
/// Colour 6 of the box's palette: the bar's fill (cycling).
const BAR_FILL: usize = 6;

/// A Dual Strike font: glyphs of `gw` x `gh` (tiles in rows), its palette.
struct Font {
    tiles: Vec<u8>,
    gw: usize,
    gh: usize,
    pal: [u16; 16],
}

impl Font {
    fn load(name: &str, gw: usize, gh: usize) -> Option<Font> {
        let (b, tail) = blocks(crate::ds_pack::pack()?.file(name)?);
        let tiles: Vec<u8> = b.concat();
        (tiles.len() >= 32 * (gw / 8) * (gh / 8) * 27).then_some(Font { tiles, gw, gh, pal: pal16(tail)? })
    }

    fn glyphs(&self) -> usize {
        self.tiles.len() / (32 * (self.gw / 8) * (self.gh / 8))
    }

    fn px(&self, g: usize, x: usize, y: usize) -> u8 {
        let tw = self.gw / 8;
        let k = g * tw * (self.gh / 8) + (y / 8) * tw + x / 8;
        tile_px(&self.tiles, k, x % 8, y % 8)
    }

    /// The glyph's drawn columns (first, last), None if blank.
    fn extent(&self, g: usize) -> Option<(usize, usize)> {
        let cols: Vec<usize> = (0..self.gw).filter(|&x| (0..self.gh).any(|y| self.px(g, x, y) != 0)).collect();
        Some((*cols.first()?, *cols.last()?))
    }
}

struct Art {
    bokeh: Bg,
    emblems: [Bg; 3],
    tag_burst: Bg,
    swap_burst: Bg,
    /// CO SWAP's red (`res_syogunchange`'s last palette, colour 1).
    red: u16,
    meter: Meter,
    tag_font: Font,
    swap_font: Font,
}

fn load() -> Option<Art> {
    let pack = crate::ds_pack::pack()?;
    // res_tagbreak: the POWER box's block, its palettes and gradient, then
    // the bokeh's tiles and map (LZ77), its palette the file's last 32
    // bytes.
    let raw = pack.file("ohashi/res_tagbreak")?;
    let (meter_tiles, n) = lz(raw)?;
    let p = (n + 3) & !3;
    let mp = pals(raw.get(p..p + 0xA0)?);
    let meter = Meter { tiles: meter_tiles, pals: [mp[0], mp[1], mp[2], mp[3]], gradient: mp[4] };
    let (bt, bn) = lz(raw.get(p + 0xA0..)?)?;
    let (bm, _) = lz(raw.get((p + 0xA0 + bn + 3) & !3..)?)?;
    if bt.len() != 0x4000 || bm.len() != 0x800 || meter.tiles.len() < 32 * (BOX_RIGHT + 8) {
        return None;
    }
    let bokeh = Bg { tiles: bt, map: map16(&bm), cols: 32, pals: vec![pal16(&raw[raw.len() - 32..])?] };
    let emblem = |name: &str| -> Option<Bg> {
        let (b, tail) = blocks(pack.file(name)?);
        let (tiles, map) = (b.first()?.clone(), b.get(1)?);
        (map.len() == 0x1000).then(|| Bg { tiles, map: map16(map), cols: 32, pals: pals(tail) })
    };
    let emblems = [emblem("ohashi/res_tagbreak_union")?, emblem("ohashi/res_tagbreak_black")?, emblem("ohashi/res_tagbreak_mix")?];
    // res_tagchange: the Tag Power's burst, a map for each screen.
    let (b, tail) = blocks(pack.file("ohashi/res_tagchange")?);
    let (tiles, top, bottom) = (b.first()?.clone(), b.get(1)?, b.get(2)?);
    let tag_burst = Bg { tiles, map: [map16(top), map16(bottom)].concat(), cols: 32, pals: pals(tail.get(..96)?) };
    // res_syogunchange: CO SWAP's burst; after its three palettes a block
    // and the red's palette.
    let (b, tail) = blocks(pack.file("ohashi/res_syogunchange")?);
    let swap_burst = Bg { tiles: b.first()?.clone(), map: map16(b.get(1)?), cols: 32, pals: pals(tail.get(..96)?) };
    let red = pal16(tail.get(tail.len().checked_sub(32)?..)?)?[1];
    Some(Art {
        bokeh,
        emblems,
        tag_burst,
        swap_burst,
        red,
        meter,
        tag_font: Font::load("ohashi/res_tagbreakfont", 32, 32)?,
        swap_font: Font::load("ohashi/res_changefont", 16, 32)?,
    })
}

fn art() -> Option<&'static Art> {
    static ART: OnceLock<Option<Art>> = OnceLock::new();
    ART.get_or_init(load).as_ref()
}

/// Black Hole's COs by Dual Strike id: Von Bolt, Jugger, Lash, Koal,
/// Hawke, Kindle, Flak, Adder (and AW2's Sturm, who has none).
fn black_hole(co: u8) -> bool {
    match crate::tag::ds_id(co) {
        Some(d) => matches!(d, 11 | 12 | 13 | 14 | 15 | 25 | 26 | 27),
        None => true,
    }
}

// --- The COs ------------------------------------------------------------------------

/// A CO head to foot ([`FIGURE_W`] x [`FIGURE_H`] colour indices, looking
/// left as stored) and its colours.
struct Figure {
    px: Vec<u8>,
    pal: [u16; 16],
}

const PRESENTATION_POOL: u32 = 0x0803_9B7C;
const PRESENTATION_ROW: u32 = 0x44;

/// Dual Strike's figure, or for a CO it lacks (AW2's Sturm) AW2's body
/// (128x160, the head at the top, to the waist; its last row carried on
/// down).
fn figure(core: &Core, co: u8) -> Option<Arc<Figure>> {
    static CACHE: OnceLock<Mutex<HashMap<u8, Option<Arc<Figure>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(f) = cache.lock().unwrap().get(&co) {
        return f.clone();
    }
    let f = match crate::tag::ds_id(co).and_then(crate::ds_co_art::full_figure) {
        Some((px, pal)) => Some(Figure { px, pal }),
        None => aw2_figure(core, co),
    }
    .map(Arc::new);
    cache.lock().unwrap().insert(co, f.clone());
    f
}

fn aw2_figure(core: &Core, co: u8) -> Option<Figure> {
    let row = core.raw_read_32(PRESENTATION_POOL, -1) + PRESENTATION_ROW * co as u32;
    let pair = core.raw_read_32(row, -1);
    let top = crate::invention_art::lz77(core, core.raw_read_32(pair, -1));
    let bottom = crate::invention_art::lz77(core, core.raw_read_32(pair + 4, -1));
    let mut pb = [0u8; 32];
    core.raw_read_range(core.raw_read_32(row + 0x08, -1), -1, &mut pb);
    if top.len() < 128 * 32 || bottom.len() < 192 * 32 {
        return None;
    }
    let mut px = vec![0u8; FIGURE_W * FIGURE_H];
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
                    px[FIGURE_W * (y0 + 8 * (k / 8) + y) + x0 + 8 * (k % 8) + x] = tile_px(tiles, k, x, y);
                }
            }
        }
    }
    // Below the waist AW2 draws nothing: the last row carried on down (a
    // long coat), so the figure slides in whole as Dual Strike's do.
    let last: Vec<u8> = px[FIGURE_W * 159..FIGURE_W * 160].to_vec();
    for y in 160..FIGURE_H {
        px[FIGURE_W * y..FIGURE_W * (y + 1)].copy_from_slice(&last);
    }
    Some(Figure { px, pal: pal16(&pb)? })
}

// --- Dual Strike's timing --------------------------------------------------------------
//
// Frame numbers are Dual Strike's, from the menu's choice (see the module's
// docs); [`TAG_START`] and [`SWAP_START`] are the frames the screens start
// at here (the quote just closed).

const TAG_START: i32 = 141;
const SWAP_START: i32 = 157;

/// Dual Strike's 13-step fade out of white or black (master brightness
/// 14, 12 .. 0), and its 25-step fade into white (1 .. 16).
const FADE_IN: [i8; 13] = [14, 12, 10, 9, 7, 6, 5, 4, 3, 2, 1, 1, 0];
const FADE_OUT: [i8; 25] = [1, 2, 3, 4, 5, 6, 7, 7, 8, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14, 14, 15, 15, 15, 15, 16];
/// The map's brightening to white before the Tag Power's screen (257..288).
const TAG_MAP_WHITE: [i8; 32] = [1, 1, 1, 1, 1, 1, 1, 1, 2, 3, 4, 5, 6, 7, 7, 8, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14, 14, 15, 15, 15, 15, 16];
/// The map's fade to black before CO SWAP (158..170).
const SWAP_MAP_BLACK: [i8; 13] = [2, 4, 6, 7, 9, 10, 11, 12, 13, 14, 15, 15, 16];
/// CO SWAP's fade in from black (177..201).
const SWAP_IN: [i8; 25] = [15, 14, 13, 12, 11, 10, 9, 9, 8, 7, 6, 6, 5, 5, 4, 4, 3, 3, 2, 2, 1, 1, 1, 1, 0];
/// The left CO's place above its stop (304..418; the right CO's is the
/// same below).
const TAG_SLIDE: [i8; 115] = [
    -94, -93, -91, -90, -88, -87, -85, -84, -82, -81, -80, -78, -77, -76, -74, -73, -72, -70, -69, -68, -67, -65, -64, -63, -62, -60, -59, -58, -57, -56, -55,
    -54, -52, -51, -50, -49, -48, -47, -46, -45, -44, -43, -42, -41, -40, -39, -38, -37, -36, -35, -34, -33, -32, -32, -31, -30, -29, -28, -27, -27, -26,
    -25, -24, -24, -23, -22, -21, -21, -20, -19, -19, -18, -17, -17, -16, -15, -15, -14, -14, -13, -12, -12, -11, -11, -10, -10, -9, -9, -8, -8, -8, -7, -7,
    -6, -6, -6, -5, -5, -4, -4, -4, -3, -3, -3, -3, -2, -2, -2, -2, -1, -1, -1, -1, -1, 0,
];
/// The POWER box's place below its stop as it rises (373..383).
const TAG_BOX_RISE: [u8; 11] = [46, 37, 30, 22, 16, 11, 7, 4, 1, 0, 0];
/// The bokeh blending in over the white and the emblem (431..456: EVA
/// the bokeh's weight, EVB the emblem's).
const TAG_BLEND: [(u8, u8); 26] = [
    (0, 16),
    (1, 16),
    (2, 16),
    (3, 15),
    (4, 14),
    (5, 13),
    (5, 13),
    (6, 12),
    (7, 11),
    (7, 11),
    (8, 10),
    (8, 10),
    (9, 9),
    (10, 8),
    (10, 8),
    (10, 8),
    (11, 7),
    (11, 7),
    (12, 6),
    (12, 6),
    (12, 6),
    (13, 5),
    (13, 5),
    (13, 5),
    (13, 5),
    (14, 4),
];
/// A letter's or a digit's pop: the affine scale's inverse (256 = 1x) for
/// its first 8 frames.
const POP: [i32; 8] = [128, 144, 163, 184, 204, 224, 240, 252];
/// The digits' pops after the count (Dual Strike's 495, 498, 502).
const DIGIT_POPS: [i32; 3] = [0, 3, 7];
/// CO SWAP's letters, stretched from a line: the affine's vertical inverse
/// (256 = full height) for their first 24 frames.
const STRETCH: [i32; 24] = [4096, 1820, 1191, 897, 728, 618, 541, 481, 436, 402, 374, 352, 334, 318, 304, 293, 284, 277, 271, 266, 262, 259, 257, 256];
/// The outgoing CO's place (Dual Strike's x, 209..294; the incoming CO's
/// is 112 - it).
const SWAP_X: [i16; 86] = [
    -134, -124, -115, -106, -97, -88, -79, -71, -63, -55, -47, -39, -32, -24, -17, -10, -3, 3, 9, 16, 22, 27, 33, 38, 43, 48, 53, 58, 62, 67, 71, 75, 78, 82,
    85, 88, 91, 94, 96, 99, 101, 103, 105, 106, 108, 109, 110, 111, 112, 112, 112, 112, 112, 112, 112, 112, 112, 112, 112, 112, 112, 112, 113, 115, 116, 118,
    121, 124, 127, 130, 134, 139, 144, 149, 154, 160, 166, 173, 180, 187, 195, 203, 212, 221, 230, 240,
];
/// The thunder's three bolts (`SE_TAG_BREAK`).
const BOLTS: [i32; 3] = [173, 209, 219];
/// Dual Strike's capture is a 110% pair; what follows the count moves with
/// the compatibility.
const CAPTURED_POWER: i32 = 110;

/// The two screens: the Tag Power's (the active CO, its partner, the
/// power's name, the compatibility) and CO SWAP (outgoing, incoming).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Screen {
    Tag(u8, u8, Vec<u8>, u8),
    Swap(u8, u8),
}

/// Frames of each screen, from its start (the quote closing) to the map
/// back in place.
pub fn length(s: &Screen) -> u16 {
    match s {
        Screen::Tag(.., power) => (717 + shift(*power) - TAG_START) as u16,
        Screen::Swap(..) => (348 - SWAP_START) as u16,
    }
}

/// The Tag Power's frame (Dual Strike's numbering) its screen's frame `t`
/// is; CO SWAP's.
pub fn tag_frame(t: u16) -> i32 {
    TAG_START + t as i32
}

pub fn swap_frame(t: u16) -> i32 {
    SWAP_START + t as i32
}

fn shift(power: u8) -> i32 {
    power as i32 - CAPTURED_POWER
}

/// The fade's value at `f` of a table starting at `at` (before: `before`,
/// after: its last).
fn ramp(table: &[i8], at: i32, f: i32, before: i8) -> i8 {
    if f < at {
        before
    } else {
        table[((f - at) as usize).min(table.len() - 1)]
    }
}

// --- Composing --------------------------------------------------------------------------

/// BG palettes: the bokeh, the emblem or burst (three), the two COs and
/// both together, the name (or CO★SWAP), the POWER box.
const PAL_BOKEH: u8 = 6;
const PAL_BACK: u8 = 7;
const PAL_LEFT: u8 = 10;
const PAL_RIGHT: u8 = 11;
const PAL_MIX: u8 = 12;
const PAL_FONT: u8 = 13;
const PAL_BOX: u8 = 14;

/// A layer being drawn: per pixel its palette and colour (palette << 4 |
/// colour; 0 nothing).
struct Canvas(Vec<u16>);

impl Canvas {
    fn new() -> Canvas {
        Canvas(vec![0; W * H])
    }

    fn set(&mut self, x: i32, y: i32, pal: u8, v: u8) {
        if v != 0 && (0..W as i32).contains(&x) && (0..H as i32).contains(&y) {
            self.0[W * y as usize + x as usize] = (pal as u16) << 4 | v as u16;
        }
    }

    /// A figure with its box's top left at (x0, y0), mirrored or not.
    fn figure(&mut self, f: &Figure, x0: i32, y0: i32, mirror: bool, pal: u8) {
        for y in 0.max(-y0)..(H as i32 - y0).min(FIGURE_H as i32) {
            for x in 0.max(-x0)..(W as i32 - x0).min(FIGURE_W as i32) {
                let sx = if mirror { FIGURE_W as i32 - 1 - x } else { x };
                self.set(x0 + x, y0 + y, pal, f.px[FIGURE_W * y as usize + sx as usize]);
            }
        }
    }

    /// `w` x `h` pixels from `src(x, y)` with their centre at (cx, cy),
    /// scaled by 256 / `sx` across and 256 / `sy` down (nearest pixel, as
    /// the DS's affine sprites).
    #[allow(clippy::too_many_arguments)]
    fn scaled(&mut self, w: i32, h: i32, cx: i32, cy: i32, sx: i32, sy: i32, pal: u8, src: impl Fn(i32, i32) -> u8) {
        let (hw, hh) = ((w * 256 / sx + 1) / 2 + 1, (h * 256 / sy + 1) / 2 + 1);
        for dy in -hh..=hh {
            for dx in -hw..=hw {
                let (u, v) = (((dx * sx) >> 8) + w / 2, ((dy * sy) >> 8) + h / 2);
                if (0..w).contains(&u) && (0..h).contains(&v) {
                    self.set(cx + dx, cy + dy, pal, src(u, v));
                }
            }
        }
    }
}

/// The name, set in the tag font: its pixels (colour indices, `w` x 32) and
/// each character's columns (None: a space or a character the font lacks).
struct Name {
    w: usize,
    px: Vec<u8>,
    spans: Vec<Option<(usize, usize)>>,
}

/// The tag font has letters only: an apostrophe ("Viper's Nest") is the
/// top of its `l`.
const APOSTROPHE_ROWS: usize = 13;
const NAME_MAX: usize = W - 8;
const SPACE: usize = 10;

fn set_name(font: &Font, text: &[u8]) -> Name {
    let glyph = |c: u8| match c {
        b'A'..=b'Z' => Some(((c - b'A') as usize, 32)),
        b'a'..=b'z' => Some((32 + (c - b'a') as usize, 32)),
        b'\'' => Some((32 + (b'l' - b'a') as usize, APOSTROPHE_ROWS)),
        _ => None,
    };
    let mut cols: Vec<[u8; 32]> = Vec::new();
    let mut spans = Vec::new();
    for &c in text {
        let g = glyph(c).filter(|&(g, _)| g < font.glyphs()).and_then(|(g, rows)| Some((g, rows, font.extent(g)?)));
        let Some((g, rows, (x0, x1))) = g else {
            if c == b' ' {
                cols.extend((0..SPACE).map(|_| [0u8; 32]));
            }
            spans.push(None);
            continue;
        };
        let a = cols.len();
        for x in x0..=x1 {
            let mut col = [0u8; 32];
            for (y, v) in col.iter_mut().enumerate().take(rows.min(font.gh)) {
                *v = font.px(g, x, y);
            }
            cols.push(col);
        }
        spans.push(Some((a, cols.len())));
    }
    // Squeezed (nearest column) to fit the screen.
    let n = cols.len().max(1);
    let w = n.min(NAME_MAX);
    let mut px = vec![0u8; w * 32];
    for x in 0..w {
        if let Some(col) = cols.get(x * n / w) {
            for y in 0..32 {
                px[w * y + x] = col[y];
            }
        }
    }
    let at = |x: usize| x * w / n;
    let spans = spans.into_iter().map(|s| s.map(|(a, b)| (at(a), at(b).max(at(a) + 1)))).collect();
    Name { w, px, spans }
}

/// What the screen shows at a frame (its layers and registers).
struct Shown {
    text: Canvas,
    cos: Canvas,
    /// The BG palettes 6..14 before the fade.
    pals: [[u16; 16]; 9],
    backdrop: u16,
    /// Master brightness (Dual Strike's): + toward white, - toward black.
    bright: i8,
    /// The back layer: 0 none, 1 the emblem, 2 the burst.
    back: u8,
    /// The Tag Power's bokeh is up.
    bokeh: bool,
    blend: Option<(u16, u8, u8)>,
}

/// A static layer: its tiles (4bpp, numbered from where they are loaded)
/// and its 30x20 window's map entries.
struct Static {
    tiles: Vec<u8>,
    map: Vec<u16>,
    pals: Vec<[u16; 16]>,
}

/// A screen's pieces that do not change with the frame.
struct Still {
    /// The Tag Power's bokeh (tiles from 1) and emblem or burst (tiles from
    /// [`STATIC_SECOND`]); CO SWAP's burst (tiles from 1).
    bokeh: Option<Static>,
    emblem: Option<Static>,
    burst: Static,
    name: Option<Name>,
    mix: [u16; 16],
    /// The Tag Power's active CO and partner; CO SWAP's incoming and
    /// outgoing COs.
    first: Arc<Figure>,
    second: Option<Arc<Figure>>,
}

/// The static tiles' halves: the bokeh from tile 1, the emblem or the
/// burst from [`STATIC_SECOND`].
const STATIC_SECOND: u16 = 512;
const STATIC_TILES: u16 = 1024;

/// The 30x20 window at (ox, oy) (multiples of 8) of a Dual Strike layer,
/// its tiles numbered from `base`, its palettes from `pal`; `opaque`: the
/// layer's transparent pixels drawn in its palette's white (the emblem's
/// white ground).
fn window(bg: &Bg, ox: usize, oy: usize, base: u16, limit: u16, pal: u8, opaque: bool) -> Static {
    let rows = bg.map.len() / bg.cols;
    let mut tiles = Vec::new();
    let mut index: HashMap<(u16, u16), u16> = HashMap::new();
    let mut map = vec![0u16; COLS * ROWS];
    for r in 0..ROWS {
        for c in 0..COLS {
            let (mr, mc) = (r + oy / 8, c + ox / 8);
            let e = if mr < rows && mc < bg.cols { bg.map[bg.cols * mr + mc] } else { 0 };
            let (k, p) = (e & 0x3FF, e >> 12);
            let n = match index.get(&(k, p)) {
                Some(&n) => n,
                None => {
                    let n = base + (tiles.len() / 32) as u16;
                    if n >= limit {
                        continue;
                    }
                    let mut t: [u8; 32] = bg.tiles.get(32 * k as usize..32 * k as usize + 32).map_or([0u8; 32], |s| s.try_into().unwrap());
                    if opaque {
                        let pl = bg.pals.get(p as usize).copied().unwrap_or([0x7FFF; 16]);
                        let white = (1..16).min_by_key(|&i| d2(rgb(pl[i]), [31, 31, 31])).unwrap() as u8;
                        for b in t.iter_mut() {
                            let (lo, hi) = (*b & 15, *b >> 4);
                            *b = (if lo == 0 { white } else { lo }) | (if hi == 0 { white } else { hi }) << 4;
                        }
                    }
                    tiles.extend_from_slice(&t);
                    index.insert((k, p), n);
                    n
                }
            };
            map[COLS * r + c] = n | (e & 0x0C00) | ((pal as u16 + p) << 12);
        }
    }
    Static { tiles, map, pals: bg.pals.clone() }
}

/// The two COs' colours in one palette (for a tile they share): both
/// palettes' colours, the closest merged until 15 are left.
fn mix(a: &[u16; 16], b: &[u16; 16]) -> [u16; 16] {
    let mut cs: Vec<Rgb> = Vec::new();
    for c in a[1..].iter().chain(b[1..].iter()).map(|&c| rgb(c)) {
        if !cs.contains(&c) {
            cs.push(c);
        }
    }
    while cs.len() > 15 {
        let mut best = (i32::MAX, 0, 0);
        for i in 0..cs.len() {
            for j in i + 1..cs.len() {
                let d = d2(cs[i], cs[j]);
                if d < best.0 {
                    best = (d, i, j);
                }
            }
        }
        let (i, j) = (best.1, best.2);
        cs[i] = [(cs[i][0] + cs[j][0]) / 2, (cs[i][1] + cs[j][1]) / 2, (cs[i][2] + cs[j][2]) / 2];
        cs.remove(j);
    }
    let mut p = [0u16; 16];
    for (k, c) in cs.iter().enumerate() {
        p[k + 1] = bgr(*c);
    }
    p
}

fn still(core: &Core, s: &Screen) -> Option<Arc<Still>> {
    static CACHE: OnceLock<Mutex<HashMap<Screen, Option<Arc<Still>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(v) = cache.lock().unwrap().get(s) {
        return v.clone();
    }
    let v = make_still(core, s).map(Arc::new);
    cache.lock().unwrap().insert(s.clone(), v.clone());
    v
}

fn make_still(core: &Core, s: &Screen) -> Option<Still> {
    let art = art()?;
    match s {
        Screen::Tag(a, b, name, _) => {
            let side = match (black_hole(*a), black_hole(*b)) {
                (false, false) => 0,
                (true, true) => 1,
                _ => 2,
            };
            let (first, second) = (figure(core, *a)?, figure(core, *b)?);
            let mix = mix(&first.pal, &second.pal);
            Some(Still {
                // The window Dual Strike's COs' heads are in: its two
                // screens' x 8.., y 64.. (the bokeh's map is the top
                // screen's, 40 rows up; the emblem's circle is centred).
                bokeh: Some(window(&art.bokeh, 8, 40, 1, STATIC_SECOND, PAL_BOKEH, false)),
                emblem: Some(window(&art.emblems[side], 8, 112, STATIC_SECOND, STATIC_TILES, PAL_BACK, true)),
                burst: window(&art.tag_burst, 8, 64, STATIC_SECOND, STATIC_TILES, PAL_BACK, false),
                name: Some(set_name(&art.tag_font, name)),
                mix,
                first,
                second: Some(second),
            })
        }
        Screen::Swap(from, to) => {
            let (first, second) = (figure(core, *to)?, figure(core, *from));
            let mix = second.as_ref().map_or(first.pal, |o| mix(&first.pal, &o.pal));
            Some(Still {
                bokeh: None,
                emblem: None,
                // The window: Dual Strike's bottom screen's x 8.., y 8...
                // (No bokeh: the burst takes all the still tiles.)
                burst: window(&art.swap_burst, 8, 8, 1, STATIC_TILES, PAL_BACK, false),
                name: None,
                mix,
                first,
                second,
            })
        }
    }
}

/// The POWER box's place when up, its digits' and bar's from it (Dual
/// Strike's: the box at (34, 140) of the bottom screen, the digits at
/// (-1, 12), the bar at (52, 16)).
const BOX_X: i32 = 26;
const BOX_Y: i32 = 122;
const NAME_Y: i32 = 80;

/// The Tag Power's screen at Dual Strike's frame `f`.
fn compose_tag(art: &Art, st: &Still, power: u8, f: i32) -> Shown {
    let sh = shift(power);
    let (left, right) = (&st.first, st.second.as_ref().unwrap());
    let mut cos = Canvas::new();
    let dy = TAG_SLIDE[((f - 304).max(0) as usize).min(TAG_SLIDE.len() - 1)] as i32;
    // The partner (right, as stored), then the active CO over it (left,
    // mirrored), at Dual Strike's places less the window's 8.
    cos.figure(right, 112, -dy, false, PAL_RIGHT);
    cos.figure(left, -16, dy, true, PAL_LEFT);

    let mut text = Canvas::new();
    let m = &art.meter;
    let count = (f - 384).clamp(0, power as i32);
    if f >= 373 {
        let rise = TAG_BOX_RISE[((f - 373) as usize).min(TAG_BOX_RISE.len() - 1)] as i32;
        let (bx, by) = (BOX_X, BOX_Y + rise);
        let tiles = |k: usize, w: i32, h: i32, x: i32, y: i32, text: &mut Canvas| {
            let tw = (w / 8) as usize;
            for ty in 0..h / 8 {
                for tx in 0..w / 8 {
                    let t = k + ty as usize * tw + tx as usize;
                    for yy in 0..8 {
                        for xx in 0..8 {
                            text.set(x + 8 * tx + xx, y + 8 * ty + yy, PAL_BOX, tile_px(&m.tiles, t, xx as usize, yy as usize));
                        }
                    }
                }
            }
        };
        tiles(BOX_LEFT, 32, 32, bx, by, &mut text);
        tiles(BOX_RIGHT, 16, 32, bx + 32, by, &mut text);
        // The bar: a pixel a percent.
        for k in 0..BAR_MIDDLES + 2 {
            let t = if k == 0 {
                BAR_LEFT + 2 * count.min(5) as usize
            } else if k == BAR_MIDDLES + 1 {
                BAR_RIGHT + 2 * (count - 125).clamp(0, 5) as usize
            } else {
                BAR_MID + 2 * (count - 5 - 8 * (k as i32 - 1)).clamp(0, 8) as usize
            };
            tiles(t, 8, 8, bx + 52 + 8 * k as i32, by + 16, &mut text);
        }
        // The digits, each popping once the count is done.
        let digits = [count / 100 % 10, count / 10 % 10, count % 10];
        for (k, &d) in digits.iter().enumerate() {
            let (x, y) = (bx - 1 + 12 * k as i32, by + 12);
            let age = f - (384 + power as i32 + 1 + DIGIT_POPS[k]);
            let base = DIGITS + 4 * d as usize;
            let src = |u: i32, v: i32| tile_px(&m.tiles, base + (v / 8 * 2 + u / 8) as usize, (u % 8) as usize, (v % 8) as usize);
            if (0..POP.len() as i32).contains(&age) {
                let s = POP[age as usize];
                text.scaled(16, 16, x + 8, y + 8, s, s, PAL_BOX, src);
            } else {
                for v in 0..16 {
                    for u in 0..16 {
                        text.set(x + u, y + v, PAL_BOX, src(u, v));
                    }
                }
            }
        }
    }
    // The name, a letter every 4 frames from 16 after the count; the newest
    // over the others.
    if let Some(n) = &st.name {
        let start = 384 + power as i32 + 16;
        let x0 = (W as i32 - n.w as i32) / 2;
        for (k, span) in n.spans.iter().enumerate() {
            let age = f - (start + 4 * k as i32);
            let Some((a, b)) = *span else { continue };
            if age < 0 {
                continue;
            }
            let src = |u: i32, v: i32| n.px[n.w * v as usize + a + u as usize];
            let w = (b - a) as i32;
            if (age as usize) < POP.len() {
                let s = POP[age as usize];
                text.scaled(w, 32, x0 + a as i32 + w / 2, NAME_Y + 16, s, s, PAL_FONT, src);
            } else {
                for v in 0..32 {
                    for u in 0..w {
                        text.set(x0 + a as i32 + u, NAME_Y + v, PAL_FONT, src(u, v));
                    }
                }
            }
        }
    }

    // Palettes.
    let mut pals = [[0u16; 16]; 9];
    pals[(PAL_BOKEH - 6) as usize] = st.bokeh.as_ref().unwrap().pals[0];
    let burst = f >= 629 + sh;
    let back = if burst { &st.burst } else { st.emblem.as_ref().unwrap() };
    for (k, p) in back.pals.iter().take(3).enumerate() {
        pals[(PAL_BACK - 6) as usize + k] = *p;
    }
    pals[(PAL_LEFT - 6) as usize] = left.pal;
    pals[(PAL_RIGHT - 6) as usize] = right.pal;
    pals[(PAL_MIX - 6) as usize] = st.mix;
    pals[(PAL_FONT - 6) as usize] = art.tag_font.pal;
    // The box white, orange from 98%; the bar's colour cycling.
    let mut boxp = m.pals[(count - 97).clamp(0, 3) as usize];
    boxp[BAR_FILL] = m.gradient[((12 + (f - 365).max(0) / 4) % 16) as usize];
    pals[(PAL_BOX - 6) as usize] = boxp;

    // Brightness: in from white at 304, out to white from 655.
    let bright = if f < 655 + sh { ramp(&FADE_IN, 304, f, 16) } else { ramp(&FADE_OUT, 655 + sh, f, 0) };
    // The emblem on white alone; then the bokeh blended in over it (Dual
    // Strike's EVA the bokeh's, EVB the emblem's); the bokeh alone; the
    // burst blended over everything.
    let (back, blend) = if f < 431 {
        (1, None)
    } else if f < 627 + sh {
        let (a, b) = TAG_BLEND[((f - 431) as usize).min(TAG_BLEND.len() - 1)];
        (1, Some((BLEND_BOKEH, a, b)))
    } else if f < 629 + sh {
        (0, None)
    } else {
        (2, Some((BLEND_TAG_BURST, (f - 628 - sh).clamp(1, 16) as u8, 16)))
    };
    Shown { text, cos, pals, backdrop: 0x7FFF, bright, back, bokeh: f >= 431, blend }
}

/// The logo's letters (CO★SWAP: the font's 27th glyph is the star).
const LOGO: &[u8] = b"CO*SWAP";
const LOGO_X: i32 = 64;
const LOGO_Y: i32 = 72;

/// CO SWAP at Dual Strike's frame `f`.
fn compose_swap(art: &Art, st: &Still, f: i32) -> Shown {
    let mut cos = Canvas::new();
    if (209..295).contains(&f) {
        let ox = SWAP_X[(f - 209) as usize] as i32;
        // The outgoing CO (mirrored), then the incoming one over it, at
        // Dual Strike's places less the window's (8, 8).
        if let Some(out) = &st.second {
            cos.figure(out, ox - 8, -8, true, PAL_RIGHT);
        }
        cos.figure(&st.first, 112 - ox - 8, -8, false, PAL_LEFT);
    }
    let mut text = Canvas::new();
    let font = &art.swap_font;
    for (k, &c) in LOGO.iter().enumerate() {
        let age = f - (208 + 4 * k as i32);
        if age < 0 {
            continue;
        }
        let g = if c == b'*' { 26 } else { (c - b'A') as usize };
        let sy = STRETCH[(age as usize).min(STRETCH.len() - 1)];
        text.scaled(16, 32, LOGO_X + 16 * k as i32 + 8, LOGO_Y + 16, 256, sy, PAL_FONT, |u, v| font.px(g, u as usize, v as usize));
    }
    let mut pals = [[0u16; 16]; 9];
    for (k, p) in st.burst.pals.iter().take(3).enumerate() {
        pals[(PAL_BACK - 6) as usize + k] = *p;
    }
    pals[(PAL_LEFT - 6) as usize] = st.first.pal;
    if let Some(o) = &st.second {
        pals[(PAL_RIGHT - 6) as usize] = o.pal;
    }
    pals[(PAL_MIX - 6) as usize] = st.mix;
    pals[(PAL_FONT - 6) as usize] = font.pal;
    let bright = if f < 295 { -ramp(&SWAP_IN, 177, f, 16) } else { ramp(&FADE_OUT, 295, f, 0) };
    let (back, blend) = if f >= 268 { (2, Some((BLEND_SWAP_BURST, (f - 268).clamp(0, 16) as u8, 16))) } else { (0, None) };
    Shown { text, cos, pals, backdrop: art.red, bright, back, bokeh: false, blend }
}

// --- Tiles ------------------------------------------------------------------------------

/// A canvas in BG form, its tiles added to `tiles` (deduplicated, `index`;
/// at most `limit`) and its map returned; a tile of several palettes takes
/// `mix` (or the palette most of it is in), its colours the nearest there.
fn to_tiles(cv: &Canvas, pals: &[[u16; 16]; 9], mix: Option<u8>, tiles: &mut Vec<u8>, index: &mut HashMap<[u8; 32], u16>, limit: usize) -> Vec<u16> {
    let mut map = vec![0u16; COLS * ROWS];
    for r in 0..ROWS {
        for c in 0..COLS {
            let mut px = [0u16; 64];
            let mut any = false;
            for y in 0..8 {
                for x in 0..8 {
                    let v = cv.0[W * (8 * r + y) + 8 * c + x];
                    px[8 * y + x] = v;
                    any |= v != 0;
                }
            }
            if !any {
                continue;
            }
            let mut counts: Vec<(u8, usize)> = Vec::new();
            for &v in px.iter().filter(|&&v| v != 0) {
                let p = (v >> 4) as u8;
                match counts.iter_mut().find(|e| e.0 == p) {
                    Some(e) => e.1 += 1,
                    None => counts.push((p, 1)),
                }
            }
            let pal = if counts.len() > 1 { mix.unwrap_or_else(|| counts.iter().max_by_key(|e| e.1).unwrap().0) } else { counts[0].0 };
            let target = pals[(pal - 6) as usize];
            let mut t = [0u8; 32];
            for (i, &v) in px.iter().enumerate() {
                if v == 0 {
                    continue;
                }
                let (p, k) = ((v >> 4) as u8, (v & 15) as usize);
                let idx = if p == pal {
                    k as u8
                } else {
                    let col = rgb(pals[(p - 6) as usize][k]);
                    (1..16u8).min_by_key(|&j| d2(col, rgb(target[j as usize]))).unwrap()
                };
                t[i / 2] |= idx << (4 * (i & 1));
            }
            let n = match index.get(&t) {
                Some(&n) => n,
                None => {
                    let n = (tiles.len() / 32) as u16;
                    if n as usize >= limit {
                        continue;
                    }
                    tiles.extend_from_slice(&t);
                    index.insert(t, n);
                    n
                }
            };
            map[COLS * r + c] = n | (pal as u16) << 12;
        }
    }
    map
}

// --- On the screen -------------------------------------------------------------------

const VRAM: u32 = 0x0600_0000;
const VRAM_LEN: u32 = 0x1_0000;
/// Char block 0: the static tiles; char block 2: each frame's; screen
/// blocks 28..31: BG0..3's maps.
const STATIC_CHARS: u32 = VRAM;
const FRAME_CHARS: u32 = VRAM + 0x8000;
const FRAME_TILES: usize = 0x6000 / 32;
const MAPS: u32 = VRAM + 0xE000;
const PAL_BUFFER: u32 = 0x0300_20C0;
const PAL_RAM: u32 = 0x0500_0000;
/// The display shadows AW2 copies to the registers at VBlank
/// (`sub_08012420`): DISPCNT, BG0..3CNT, the BGs' scroll, the scroll
/// offset every BG takes, BLDCNT, EVA, EVB, BLDY.
const DISPCNT: u32 = 0x0300_30CC;
const BGCNT: [u32; 4] = [0x0300_2B6C, 0x0300_1FE8, 0x0300_30B4, 0x0300_251C];
const BGOFS: [u32; 8] = [0x0300_1FF8, 0x0300_1418, 0x0300_2B34, 0x0300_2F18, 0x0300_30A0, 0x0300_1400, 0x0300_200C, 0x0300_2000];
const SHIFT: [u32; 2] = [0x0300_30D0, 0x0300_2B20];
const BLDCNT: u32 = 0x0300_30E0;
const EVA: u32 = 0x0300_2020;
const EVB: u32 = 0x0300_2B28;
const BLDY: u32 = 0x0300_1FFC;
const SHADOWS: [u32; 19] = [
    DISPCNT, BGCNT[0], BGCNT[1], BGCNT[2], BGCNT[3], BGOFS[0], BGOFS[1], BGOFS[2], BGOFS[3], BGOFS[4], BGOFS[5], BGOFS[6], BGOFS[7], SHIFT[0], SHIFT[1], BLDCNT,
    EVA, EVB, BLDY,
];
const IO: u32 = 0x0400_0000;
/// The layer and window bits (BG0..3, sprites, windows).
const LAYERS: u16 = 0xFF00;
/// Blends: the bokeh (BG2) over the emblem (BG3); the Tag Power's burst
/// (BG3, raised) over the COs (BG1), the bokeh and the backdrop; CO SWAP's
/// burst (BG1) over the COs (BG2) and the red backdrop.
const BLEND_BOKEH: u16 = 0x0844;
const BLEND_TAG_BURST: u16 = 0x2648;
const BLEND_SWAP_BURST: u16 = 0x2442;
/// The map's brightening and darkening (every layer, the backdrop).
const BRIGHTEN: u16 = 0x00BF;
const DARKEN: u16 = 0x00FF;

/// What the screens cover while they show, in the ROM image's free space
/// (`0x08790000..0x087A042F`): all of BG VRAM, then the BG palettes (the
/// buffer and the RAM), then the display shadows.
pub const BACKUP: u32 = 0x0879_0000;
const BACKUP_PAL: u32 = BACKUP + VRAM_LEN;
const BACKUP_SHADOWS: u32 = BACKUP_PAL + 0x400;
pub const BACKUP_END: u32 = BACKUP_SHADOWS + 2 * SHADOWS.len() as u32;

/// RAM (crate::tag_extras's): the screen has the display (1); the shadows
/// are kept (1); which back layer is up (0 none, 1 the emblem, 2 the
/// burst).
pub struct Ram {
    pub up: u32,
    pub kept: u32,
    pub back: u32,
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

fn set16(core: &mut Core, at: u32, v: u16) {
    if core.raw_read_16(at, -1) != v {
        core.raw_write_16(at, -1, v);
    }
}

/// The shadows kept (once, as a screen starts).
fn keep_shadows(core: &mut Core, ram: &Ram) {
    if core.raw_read_8(ram.kept, -1) == 1 {
        return;
    }
    for (k, &a) in SHADOWS.iter().enumerate() {
        let v = core.raw_read_16(a, -1);
        core.raw_write_16(BACKUP_SHADOWS + 2 * k as u32, -1, v);
    }
    core.raw_write_8(ram.kept, -1, 1);
}

fn saved(core: &Core, a: u32) -> u16 {
    let k = SHADOWS.iter().position(|&s| s == a).unwrap() as u32;
    core.raw_read_16(BACKUP_SHADOWS + 2 * k, -1)
}

/// The shadows (and the registers they feed) as they were.
fn restore_shadows(core: &mut Core, ram: &Ram) {
    if core.raw_read_8(ram.kept, -1) != 1 {
        return;
    }
    for &a in SHADOWS.iter() {
        let v = saved(core, a);
        set16(core, a, v);
    }
    let d = saved(core, DISPCNT);
    set16(core, IO, d);
    for (k, &a) in BGCNT.iter().enumerate() {
        let v = saved(core, a);
        set16(core, IO + 8 + 2 * k as u32, v);
    }
    let (c, a, b, y) = (saved(core, BLDCNT), saved(core, EVA), saved(core, EVB), saved(core, BLDY));
    set16(core, IO + 0x50, c);
    set16(core, IO + 0x52, a | b << 8);
    set16(core, IO + 0x54, y);
    core.raw_write_8(ram.kept, -1, 0);
}

/// A brightness effect on AW2's own display (the map): + brighten, -
/// darken, 0 as it was.
fn map_brightness(core: &mut Core, v: i8) {
    let (c, y) = match v {
        0 => (saved(core, BLDCNT), saved(core, BLDY)),
        v if v > 0 => (BRIGHTEN, v as u16),
        v => (DARKEN, (-v) as u16),
    };
    set16(core, BLDCNT, c);
    set16(core, BLDY, y);
    set16(core, IO + 0x50, c);
    set16(core, IO + 0x54, y);
}

/// The display taken: what it covers kept first.
fn take(core: &mut Core, ram: &Ram) {
    if core.raw_read_8(ram.up, -1) == 1 {
        return;
    }
    copy(core, VRAM, BACKUP, VRAM_LEN);
    copy(core, PAL_BUFFER, BACKUP_PAL, 0x200);
    copy(core, PAL_RAM, BACKUP_PAL + 0x200, 0x200);
    core.raw_write_8(ram.up, -1, 1);
    core.raw_write_8(ram.back, -1, 0);
}

/// The display given back (VRAM and the palettes as they were; the
/// layers' shadows too).
fn give_back(core: &mut Core, ram: &Ram) {
    if core.raw_read_8(ram.up, -1) != 1 {
        return;
    }
    copy(core, BACKUP, VRAM, VRAM_LEN);
    copy(core, BACKUP_PAL, PAL_BUFFER, 0x200);
    copy(core, BACKUP_PAL + 0x200, PAL_RAM, 0x200);
    for &a in [DISPCNT].iter().chain(BGCNT.iter()).chain(BGOFS.iter()).chain(SHIFT.iter()) {
        let v = saved(core, a);
        set16(core, a, v);
    }
    let d = saved(core, DISPCNT);
    set16(core, IO, d);
    for (k, &a) in BGCNT.iter().enumerate() {
        let v = saved(core, a);
        set16(core, IO + 8 + 2 * k as u32, v);
    }
    core.raw_write_8(ram.up, -1, 0);
    core.raw_write_8(ram.back, -1, 0);
}

/// Everything back as it was (a screen over, or cut short).
pub fn hide(core: &mut Core, ram: &Ram) {
    give_back(core, ram);
    restore_shadows(core, ram);
}

fn faded(p: &[u16], bright: i8) -> Vec<u8> {
    let t = bright.unsigned_abs() as i32;
    let target = if bright > 0 { 31 } else { 0 };
    p.iter().flat_map(|&c| bgr(rgb(c).map(|v| v + (target - v) * t / 16)).to_le_bytes()).collect()
}

fn entries(m: &[u16]) -> Vec<u8> {
    m.iter().flat_map(|e| e.to_le_bytes()).collect()
}

/// BG `n`'s map (screen block 28 + n): the 30x20 window in its 32x32.
fn write_map(core: &mut Core, n: u32, m: &[u16]) {
    let mut b = vec![0u8; 0x800];
    let e = entries(m);
    for r in 0..ROWS {
        b[64 * r..64 * r + 60].copy_from_slice(&e[60 * r..60 * r + 60]);
    }
    write_if_changed(core, MAPS + 0x800 * n, &b);
}

fn show(core: &mut Core, ram: &Ram, st: &Still, s: &Shown) {
    take(core, ram);
    // The Tag Power: the text BG0, the COs BG1, the bokeh BG2, the back
    // layer BG3 (the burst raised over the COs). CO SWAP: the text BG0, the
    // burst BG1, the COs BG2.
    let tag = st.bokeh.is_some();
    let (co_bg, back_bg) = if tag { (1u32, 3u32) } else { (2, 1) };
    // Static tile 0 blank (a map entry 0 is nothing).
    write_if_changed(core, STATIC_CHARS, &[0u8; 32]);
    if let Some(b) = &st.bokeh {
        write_if_changed(core, STATIC_CHARS + 32, &b.tiles);
        write_map(core, 2, &b.map);
    }
    let back = match s.back {
        1 => st.emblem.as_ref(),
        2 => Some(&st.burst),
        _ => None,
    };
    if let Some(b) = back {
        if core.raw_read_8(ram.back, -1) != s.back {
            core.raw_write_8(ram.back, -1, s.back);
        }
        let base = if tag { STATIC_SECOND as u32 } else { 1 };
        write_if_changed(core, STATIC_CHARS + 32 * base, &b.tiles);
        write_map(core, back_bg, &b.map);
    }
    // This frame's tiles: the text's, then the COs'.
    let mut tiles = vec![0u8; 32];
    let mut index = HashMap::new();
    index.insert([0u8; 32], 0u16);
    let tm = to_tiles(&s.text, &s.pals, None, &mut tiles, &mut index, FRAME_TILES);
    let cm = to_tiles(&s.cos, &s.pals, Some(PAL_MIX), &mut tiles, &mut index, FRAME_TILES);
    write_if_changed(core, FRAME_CHARS, &tiles);
    write_map(core, 0, &tm);
    write_map(core, co_bg, &cm);
    // Palettes 6..14 faded, and the backdrop.
    let flat: Vec<u16> = s.pals.iter().flatten().copied().collect();
    let p = faded(&flat, s.bright);
    let bd = faded(&[s.backdrop], s.bright);
    for base in [PAL_BUFFER, PAL_RAM] {
        write_if_changed(core, base + 32 * 6, &p);
        write_if_changed(core, base, &bd);
    }
    let cnt = |prio: u16, chars: u16, n: u16| prio | chars << 2 | (28 + n) << 8;
    let burst_up = s.back == 2;
    let cnts: [u16; 4] = if tag {
        [cnt(0, 2, 0), cnt(if burst_up { 2 } else { 1 }, 2, 1), cnt(if burst_up { 3 } else { 2 }, 0, 2), cnt(if burst_up { 1 } else { 3 }, 0, 3)]
    } else {
        [cnt(0, 2, 0), cnt(1, 0, 1), cnt(2, 2, 2), cnt(3, 0, 3)]
    };
    let mut on: u16 = 0x0100 | 1 << (8 + co_bg);
    if s.bokeh {
        on |= 1 << (8 + 2);
    }
    if back.is_some() {
        on |= 1 << (8 + back_bg);
    }
    for (k, &c) in cnts.iter().enumerate() {
        set16(core, BGCNT[k], c);
        set16(core, IO + 8 + 2 * k as u32, c);
    }
    for &a in BGOFS.iter().chain(SHIFT.iter()) {
        set16(core, a, 0);
    }
    for k in 0..8 {
        set16(core, IO + 0x10 + 2 * k, 0);
    }
    for reg in [DISPCNT, IO] {
        let d = core.raw_read_16(reg, -1);
        set16(core, reg, (d & !LAYERS) | on);
    }
    let (c, a, b) = s.blend.unwrap_or((0, 0, 0));
    set16(core, BLDCNT, c);
    set16(core, EVA, a as u16);
    set16(core, EVB, b as u16);
    set16(core, BLDY, 0);
    set16(core, IO + 0x50, c);
    set16(core, IO + 0x52, a as u16 | (b as u16) << 8);
    set16(core, IO + 0x54, 0);
}

/// One frame of screen `s` (`t` frames from its start): the sound effect
/// to play now (a song id), if any, and whether the screen is over
/// (everything put back). No pictures (no pack): over at once.
pub fn frame(core: &mut Core, ram: &Ram, s: &Screen, t: u16) -> (Option<u16>, bool) {
    let (Some(art), Some(st)) = (art(), still(core, s)) else {
        hide(core, ram);
        return (None, true);
    };
    if t >= length(s) {
        hide(core, ram);
        return (None, true);
    }
    keep_shadows(core, ram);
    let se = crate::ds_music::tag_se;
    match s {
        Screen::Tag(_, _, _, power) => {
            let (sh, p) = (shift(*power), *power as i32);
            let f = tag_frame(t);
            let start = 384 + p + 16;
            let letter = (f - start - 2) / 4;
            let sound = if BOLTS.contains(&f) {
                se(TagSe::Thunder)
            } else if f == 381 {
                se(TagSe::MeterUp)
            } else if f >= 382 && (f - 382) % 2 == 0 && (f - 382) / 2 < p / 2 {
                se(TagSe::Count)
            } else if f >= start + 2 && (f - start - 2) % 4 == 0 && st.name.as_ref().is_some_and(|n| n.spans.get(letter as usize).is_some_and(|s| s.is_some())) {
                se(TagSe::Letter)
            } else if f == 627 + sh {
                se(TagSe::Burst)
            } else {
                None
            };
            if f < 297 {
                // The map: dims, flashes at the bolts, then whitens.
                let v = if f >= 257 {
                    ramp(&TAG_MAP_WHITE, 257, f, 0)
                } else if let Some(&b) = BOLTS.iter().rev().find(|&&b| f >= b && f < b + 8) {
                    (12 - 3 * (f - b) / 2) as i8
                } else {
                    -((f - TAG_START) / 5).min(6) as i8
                };
                give_back(core, ram);
                map_brightness(core, v);
            } else if f < 687 + sh {
                let shown = compose_tag(art, &st, *power, f);
                show(core, ram, &st, &shown);
            } else {
                give_back(core, ram);
                map_brightness(core, ramp(&FADE_IN, 704 + sh, f, 16));
            }
            (sound, false)
        }
        Screen::Swap(..) => {
            let f = swap_frame(t);
            let sound = if f == 176 { se(TagSe::Swap) } else { None };
            if f < 171 {
                give_back(core, ram);
                map_brightness(core, -ramp(&SWAP_MAP_BLACK, 158, f, 0));
            } else if f < 335 {
                let shown = compose_swap(art, &st, f);
                show(core, ram, &st, &shown);
            } else {
                give_back(core, ram);
                map_brightness(core, ramp(&FADE_IN, 335, f, 16));
            }
            (sound, false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_in_its_range() {
        assert!(BACKUP_END <= 0x087A_1000);
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

    #[test]
    fn tables() {
        assert_eq!(SWAP_X.len(), 295 - 209);
        assert_eq!(TAG_SLIDE.len(), 419 - 304);
        // The screens' lengths: the Tag Power's 9.6 s at 110%, less at 100%.
        assert_eq!(length(&Screen::Tag(0, 1, b"x".to_vec(), 110)), 576);
        assert_eq!(length(&Screen::Tag(0, 1, b"x".to_vec(), 100)), 566);
        assert_eq!(length(&Screen::Swap(0, 1)), 191);
    }

    #[test]
    fn scaled_keeps_its_centre() {
        let mut c = Canvas::new();
        c.scaled(4, 4, 20, 20, 128, 128, 13, |_, _| 1);
        let xs: Vec<usize> = (0..W).filter(|&x| (0..H).any(|y| c.0[W * y + x] != 0)).collect();
        assert_eq!((xs[0], *xs.last().unwrap()), (16, 23));
        // A line, stretched from nothing: two rows at the centre.
        let mut c = Canvas::new();
        c.scaled(16, 32, 50, 50, 256, 4096, 13, |_, _| 1);
        let ys: Vec<usize> = (0..H).filter(|&y| (0..W).any(|x| c.0[W * y + x] != 0)).collect();
        assert!(ys.len() <= 3 && ys.contains(&50), "{ys:?}");
    }

    #[test]
    fn a_shared_tile_takes_the_mix() {
        let mut cv = Canvas::new();
        cv.set(0, 0, PAL_LEFT, 1);
        cv.set(1, 0, PAL_RIGHT, 1);
        let mut pals = [[0u16; 16]; 9];
        pals[(PAL_LEFT - 6) as usize][1] = 0x001F;
        pals[(PAL_RIGHT - 6) as usize][1] = 0x7C00;
        pals[(PAL_MIX - 6) as usize] = mix(&pals[(PAL_LEFT - 6) as usize], &pals[(PAL_RIGHT - 6) as usize]);
        let (mut tiles, mut index) = (vec![0u8; 32], HashMap::new());
        index.insert([0u8; 32], 0);
        let m = to_tiles(&cv, &pals, Some(PAL_MIX), &mut tiles, &mut index, 10);
        assert_eq!(m[0] >> 12, PAL_MIX as u16);
        assert_eq!(tiles.len(), 64);
    }

    /// With `TANGOAW2_DS_ROM`: every still layer fits its tiles, and every
    /// frame of Andy and Max's screens (the widest pair of the capture)
    /// fits the frame's tiles.
    #[test]
    #[ignore]
    fn everything_fits() {
        crate::ds_pack::pack().expect("TANGOAW2_DS_ROM");
        let art = art().expect("the screens' pictures");
        let fits = |st: &Static, base: u16| st.map.iter().all(|&e| e & 0x3FF != 0) && st.tiles.len() / 32 + base as usize <= STATIC_TILES as usize;
        assert!(fits(&window(&art.bokeh, 8, 40, 1, STATIC_SECOND, PAL_BOKEH, false), 1));
        assert!(window(&art.bokeh, 8, 40, 1, STATIC_SECOND, PAL_BOKEH, false).tiles.len() / 32 < STATIC_SECOND as usize);
        for e in &art.emblems {
            assert!(fits(&window(e, 8, 112, STATIC_SECOND, STATIC_TILES, PAL_BACK, true), STATIC_SECOND));
        }
        assert!(fits(&window(&art.tag_burst, 8, 64, STATIC_SECOND, STATIC_TILES, PAL_BACK, false), STATIC_SECOND));
        assert!(fits(&window(&art.swap_burst, 8, 8, 1, STATIC_TILES, PAL_BACK, false), 1));
        let fig = |d: u8| {
            let (px, pal) = crate::ds_co_art::full_figure(d).unwrap();
            Arc::new(Figure { px, pal })
        };
        let (andy, max) = (fig(2), fig(3));
        let st = Still {
            bokeh: Some(window(&art.bokeh, 8, 40, 1, STATIC_SECOND, PAL_BOKEH, false)),
            emblem: Some(window(&art.emblems[0], 8, 112, STATIC_SECOND, STATIC_TILES, PAL_BACK, true)),
            burst: window(&art.tag_burst, 8, 64, STATIC_SECOND, STATIC_TILES, PAL_BACK, false),
            name: Some(set_name(&art.tag_font, b"Power Wrench")),
            mix: mix(&andy.pal, &max.pal),
            first: andy.clone(),
            second: Some(max.clone()),
        };
        let swap = Still {
            bokeh: None,
            emblem: None,
            burst: window(&art.swap_burst, 8, 8, 1, STATIC_TILES, PAL_BACK, false),
            name: None,
            mix: mix(&max.pal, &andy.pal),
            first: max,
            second: Some(andy),
        };
        let count = |s: &Shown| {
            let (mut tiles, mut index) = (vec![0u8; 32], HashMap::new());
            index.insert([0u8; 32], 0u16);
            to_tiles(&s.text, &s.pals, None, &mut tiles, &mut index, usize::MAX);
            to_tiles(&s.cos, &s.pals, Some(PAL_MIX), &mut tiles, &mut index, usize::MAX);
            tiles.len() / 32
        };
        let most = (297..687).map(|f| count(&compose_tag(art, &st, 110, f))).max().unwrap();
        let most_swap = (171..335).map(|f| count(&compose_swap(art, &swap, f))).max().unwrap();
        eprintln!("frame tiles: the Tag Power's at most {most}, CO SWAP's {most_swap} (room {FRAME_TILES})");
        assert!(most <= FRAME_TILES && most_swap <= FRAME_TILES);
    }
}
