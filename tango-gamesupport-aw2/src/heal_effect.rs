//! Dual Strike's heal effect for the Black Crystal and Black Obelisk
//! ([`crate::obelisk`]): when they heal Black Hole's units at its turn
//! start, each structure plays its own animation, as in Dual Strike.
//!
//! What Dual Strike does (its code, traced in melonDS): at a Black Hole
//! turn start (0x020BA610, army style 5) a proc runs per structure
//! (script 0x02167FF0: camera to it, the animation, 40 frames later the
//! heal, then it waits for the animation to end). The animation is one
//! object anchored on the structure, nothing is drawn on the units:
//! - Crystal: anim 0x0213E078, pixels `bmap/06c`, palette `bmap/06e`,
//!   anchored at the bottom centre of its square (x*16+8, y*16+16): the
//!   outline flashes (4 cells of 16x32 for 2, 4, 6 and 8 frames), a
//!   5-frame pause, then pillars of light rise on the four squares around
//!   it and burst into sparkles (15 cells, 3-5 frames each): 74 frames.
//! - Obelisk: anim 0x0213E2A0, pixels `bmap/06f`, palette `bmap/071`,
//!   anchored at the centre of its 3x3 squares (x*16+24, y*16+24 from the
//!   top left): the Obelisk glows (2 cells), then a ring widens, pillars
//!   rise on its four sides and fall into sparkles (22 cells, 3-7 frames
//!   each): 109 frames.
//!
//! Anim format (Dual Strike arm9): u16 offset of the cell table, u16 offset
//! of the sequence table; the cell table holds u16 offsets (from the table)
//! of the cells; a cell is a u16 piece count, then per piece its three OAM
//! attributes (y, x, shape and size, flips, tile) relative to the anchor.
//! The tile number counts 32-byte units into the pixels, which are stored
//! in rows of the piece's width (not as 8x8 tiles). The sequence table's
//! first u16 leads to (duration, cell) u16 pairs, ended by a zero duration.
//!
//! The pieces are cut into 16x16 and 8x8 sprites (empty ones dropped,
//! repeats and mirror images shared through the sprite flips) and drawn on
//! the map from the frame's effects pass (the sandstorm's hook,
//! [`crate::sandstorm`]). Their tiles go to OBJ tiles no map screen uses
//! (0x1F9-0x209, 0x2D2-0x2DA, 0x2E4, 0x2EC, 0x2F4, 0x2FC, 0x309-0x311: 51
//! tiles; the Obelisk's two busiest frames need 52 and leave out one 8x8
//! sparkle), and the colours to OBJ palette 8, which no map sprite uses:
//! it is saved at the start and put back at the end, and the tiles are
//! cleared again. Dual Strike blends the effect half-transparent; here it
//! is drawn opaque.
//!
//! What to draw is kept in RAM (when, which structure, where, the saved
//! palette), so a rollback redraws the same frames.

use mgba::core::Core;
use std::collections::HashMap;
use std::sync::OnceLock;

const OBJ_VRAM: u32 = 0x0601_0000;
/// Free OBJ tiles: (first, count).
const TILE_RUNS: [(u16, u16); 7] = [(0x1F9, 17), (0x2D2, 9), (0x2E4, 4), (0x2EC, 4), (0x2F4, 4), (0x2FC, 4), (0x309, 9)];
const PALETTE: u16 = 8;
const PAL_BUFFER: u32 = 0x0300_20C0;
const PAL_RAM: u32 = 0x0500_0000;

const CRYSTAL_ANIM: u32 = 0x0213_E078;
const OBELISK_ANIM: u32 = 0x0213_E2A0;

/// RAM tangoAW2 keeps: start clock (u32), structure (0 none, 1 Crystal,
/// 2 Obelisk), its x, y, OBJ palette 8 as it was, and the map's scroll
/// last frame (the animation starts once the camera has stopped).
const STATE: u32 = 0x0203_FD80;
const START: u32 = STATE;
const KIND: u32 = STATE + 4;
const POS_X: u32 = STATE + 5;
const POS_Y: u32 = STATE + 6;
const SAVED_PALETTE: u32 = STATE + 8; // 32 bytes
const LAST_SCROLL: u32 = STATE + 0x28; // x, y (u16)
#[cfg(test)]
const STATE_END: u32 = LAST_SCROLL + 4;
const GAME_CLOCK: u32 = 0x0300_4008;
const OAM_NEXT: u32 = 0x0300_141C;
/// The frame's sprite list (`sub_0801BC08`: base at +0, flushed to OAM):
/// 128 entries.
const SPRITE_LIST: u32 = 0x0300_0278;
const SPRITE_LIST_SIZE: u32 = 0x400;
const MAP_POINTER: u32 = 0x0849_9590;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Crystal = 1,
    Obelisk = 2,
}

struct Sprite {
    dx: i16,
    dy: i16,
    /// 16x16 (4 tiles in a row) or 8x8.
    big: bool,
    tile: u16,
    hflip: bool,
    vflip: bool,
}

struct Frame {
    tiles: Vec<(u16, [u8; 32])>,
    sprites: Vec<Sprite>,
}

struct Anim {
    frames: Vec<Frame>,
    /// (duration in frames, cell), in order.
    sequence: Vec<(u32, usize)>,
    palette: [u16; 16],
    anchor: (i32, i32),
}

impl Anim {
    #[cfg(test)]
    fn length(&self) -> u32 {
        self.sequence.iter().map(|s| s.0).sum()
    }

    fn cell_at(&self, t: u32) -> Option<usize> {
        let mut end = 0;
        for &(d, c) in &self.sequence {
            end += d;
            if t < end {
                return Some(c);
            }
        }
        None
    }
}

struct Art {
    crystal: Anim,
    obelisk: Anim,
}

static ART: OnceLock<Option<Art>> = OnceLock::new();

fn art() -> Option<&'static Art> {
    ART.get_or_init(|| {
        let pack = crate::ds_pack::pack()?;
        let hw = |a: u32| pack.arm9_at(a, 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as u32);
        let load = |anim: u32, pixels: &str, pal: &str, anchor| -> Option<Anim> {
            let pixels = crate::ds_art::lz10(pack.file(pixels)?)?;
            let pal = pack.file(pal)?;
            let (cells, sequence) = parse(&hw, anim)?;
            let mut palette = [0u16; 16];
            for (i, c) in palette.iter_mut().enumerate() {
                *c = u16::from_le_bytes([*pal.get(2 * i)?, *pal.get(2 * i + 1)?]);
            }
            let frames = cells.iter().map(|cell| compile(&pixels, cell)).collect::<Vec<_>>();
            if sequence.iter().any(|&(_, c)| c >= frames.len()) {
                return None;
            }
            Some(Anim { frames, sequence, palette, anchor })
        };
        Some(Art {
            crystal: load(CRYSTAL_ANIM, "bmap/06c", "bmap/06e", (8, 16))?,
            obelisk: load(OBELISK_ANIM, "bmap/06f", "bmap/071", (24, 24))?,
        })
    })
    .as_ref()
}

/// A piece of a cell: position from the anchor, size, first 32-byte unit
/// of its pixels, flips.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Piece {
    x: i32,
    y: i32,
    w: usize,
    h: usize,
    at: usize,
    hflip: bool,
    vflip: bool,
}

const OBJ_SIZES: [[(usize, usize); 4]; 3] = [
    [(8, 8), (16, 16), (32, 32), (64, 64)],
    [(16, 8), (32, 8), (32, 16), (64, 32)],
    [(8, 16), (8, 32), (16, 32), (32, 64)],
];

type Cells = Vec<Vec<Piece>>;

/// A Dual Strike anim: its cells' pieces and its sequence.
fn parse(hw: &dyn Fn(u32) -> Option<u32>, base: u32) -> Option<(Cells, Vec<(u32, usize)>)> {
    let (cells_at, seq_at) = (base + hw(base)?, base + hw(base + 2)?);
    let count = seq_at.checked_sub(cells_at)? / 2;
    if count == 0 || count > 64 {
        return None;
    }
    let mut cells = Vec::new();
    for i in 0..count {
        let c = cells_at + hw(cells_at + 2 * i)?;
        let n = hw(c)?;
        if n > 32 {
            return None;
        }
        let mut pieces = Vec::new();
        for k in 0..n {
            let (a0, a1, a2) = (hw(c + 2 + 6 * k)?, hw(c + 4 + 6 * k)?, hw(c + 6 + 6 * k)?);
            let (shape, size) = ((a0 >> 14) as usize, (a1 >> 14) as usize);
            let (w, h) = *OBJ_SIZES.get(shape)?.get(size)?;
            pieces.push(Piece {
                x: ((a1 & 0x1FF) as i32) << 23 >> 23,
                y: (a0 & 0xFF) as i8 as i32,
                w,
                h,
                at: (a2 & 0x3FF) as usize,
                hflip: a1 & 0x1000 != 0,
                vflip: a1 & 0x2000 != 0,
            });
        }
        cells.push(pieces);
    }
    let mut s = seq_at + hw(seq_at)?;
    let mut sequence = Vec::new();
    loop {
        let (d, c) = (hw(s)?, hw(s + 2)?);
        if d == 0 {
            break;
        }
        sequence.push((d, c as usize));
        s += 4;
        if sequence.len() > 64 {
            return None;
        }
    }
    Some((cells, sequence))
}

/// A square of screen pixels (colour indices), `n` x `n`, in rows.
type Square = Vec<u8>;

fn flipped(sq: &Square, n: usize, h: bool, v: bool) -> Square {
    let mut out = vec![0; n * n];
    for y in 0..n {
        for x in 0..n {
            let (sx, sy) = (if h { n - 1 - x } else { x }, if v { n - 1 - y } else { y });
            out[n * y + x] = sq[n * sy + sx];
        }
    }
    out
}

/// The orientation to store (the least of the four) and the flips that
/// draw `sq` from it.
fn canonical(sq: &Square, n: usize) -> (Square, bool, bool) {
    let mut best: Option<(Square, bool, bool)> = None;
    for (h, v) in [(false, false), (true, false), (false, true), (true, true)] {
        let f = flipped(sq, n, h, v);
        if best.as_ref().is_none_or(|b| f < b.0) {
            best = Some((f, h, v));
        }
    }
    best.unwrap()
}

/// 8x8 pixels of a square, as a 4bpp GBA tile.
fn tile_bytes(sq: &Square, n: usize, tx: usize, ty: usize) -> [u8; 32] {
    let mut t = [0u8; 32];
    for y in 0..8 {
        for x in 0..8 {
            let v = sq[n * (8 * ty + y) + 8 * tx + x] & 15;
            t[4 * y + x / 2] |= v << (4 * (x & 1));
        }
    }
    t
}

/// A cell as sprites (in the cell's order, the first on top) and the tiles
/// they use, placed in the free tiles.
fn compile(pixels: &[u8], cell: &[Piece]) -> Frame {
    // Every piece as screen pixels (its flips applied), cut into squares:
    // (order, x, y, pixels, 16x16?).
    let mut squares: Vec<(i32, i32, Square, bool)> = Vec::new();
    for p in cell {
        let px = |x: usize, y: usize| -> u8 {
            let (sx, sy) = (if p.hflip { p.w - 1 - x } else { x }, if p.vflip { p.h - 1 - y } else { y });
            let i = 64 * p.at + p.w * sy + sx;
            pixels.get(i / 2).map_or(0, |b| (b >> (4 * (i & 1))) & 15)
        };
        let square = |x0: usize, y0: usize, n: usize| -> Square { (0..n * n).map(|i| px(x0 + i % n, y0 + i / n)).collect() };
        let empty = |sq: &Square| sq.iter().all(|&v| v == 0);
        let big = p.w >= 16 && p.h >= 16;
        let step = if big { 16 } else { 8 };
        for by in (0..p.h).step_by(step) {
            for bx in (0..p.w).step_by(step) {
                let (x, y) = (p.x + bx as i32, p.y + by as i32);
                if !big {
                    let s = square(bx, by, 8);
                    if !empty(&s) {
                        squares.push((x, y, s, false));
                    }
                    continue;
                }
                let quads: Vec<Square> = (0..4).map(|q| square(bx + 8 * (q % 2), by + 8 * (q / 2), 8)).collect();
                if quads.iter().all(|q| !empty(q)) {
                    squares.push((x, y, square(bx, by, 16), true));
                } else {
                    for (q, s) in quads.into_iter().enumerate() {
                        if !empty(&s) {
                            squares.push((x + 8 * (q % 2) as i32, y + 8 * (q / 2) as i32, s, false));
                        }
                    }
                }
            }
        }
    }
    // Free tiles: 16x16 slots (4 in a row) first, then what is left.
    let mut big_slots: Vec<u16> = Vec::new();
    let mut small_slots: Vec<u16> = Vec::new();
    for (first, n) in TILE_RUNS {
        let blocks = n / 4;
        big_slots.extend((0..blocks).map(|b| first + 4 * b));
        small_slots.extend(first + 4 * blocks..first + n);
    }
    let mut tiles = Vec::new();
    let mut shared: HashMap<(bool, Square), u16> = HashMap::new();
    let mut placed: Vec<Option<Sprite>> = (0..squares.len()).map(|_| None).collect();
    // The 16x16 squares take their slots first, then the 8x8 ones.
    for pass_big in [true, false] {
        for (i, (x, y, sq, big)) in squares.iter().enumerate() {
            if *big != pass_big {
                continue;
            }
            let n = if *big { 16 } else { 8 };
            let (c, h, v) = canonical(sq, n);
            let tile = match shared.get(&(*big, c.clone())) {
                Some(&t) => t,
                None => {
                    let t = if *big {
                        if big_slots.is_empty() {
                            continue;
                        }
                        big_slots.remove(0)
                    } else if !small_slots.is_empty() {
                        small_slots.remove(0)
                    } else if let Some(b) = big_slots.pop() {
                        // A 16x16 slot nothing needs: four 8x8 ones.
                        small_slots.extend(b + 1..b + 4);
                        b
                    } else {
                        continue;
                    };
                    for q in 0..n * n / 64 {
                        tiles.push((t + q as u16, tile_bytes(&c, n, q % (n / 8), q / (n / 8))));
                    }
                    shared.insert((*big, c), t);
                    t
                }
            };
            placed[i] = Some(Sprite { dx: *x as i16, dy: *y as i16, big: *big, tile, hflip: h, vflip: v });
        }
    }
    Frame { tiles, sprites: placed.into_iter().flatten().collect() }
}

fn anim(kind: u8) -> Option<&'static Anim> {
    let art = art()?;
    match kind {
        1 => Some(&art.crystal),
        2 => Some(&art.obelisk),
        _ => None,
    }
}

fn palette_at(base: u32) -> u32 {
    base + 0x200 + 32 * PALETTE as u32
}

/// A structure shows its heal now (the camera is on it): its animation
/// starts at (x, y), its square (the Obelisk's top left).
pub fn start(core: &mut Core, kind: Kind, x: u8, y: u8) {
    if art().is_none() {
        return;
    }
    if core.raw_read_8(KIND, -1) == 0 {
        let mut p = [0u8; 32];
        core.raw_read_range(palette_at(PAL_BUFFER), -1, &mut p);
        core.raw_write_range(SAVED_PALETTE, -1, &p);
    }
    let map = core.raw_read_32(MAP_POINTER, -1);
    let scroll = core.raw_read_32(map + 4, -1);
    core.raw_write_32(LAST_SCROLL, -1, scroll);
    let clock = core.raw_read_32(GAME_CLOCK, -1);
    core.raw_write_32(START, -1, clock);
    core.raw_write_8(KIND, -1, kind as u8);
    core.raw_write_8(POS_X, -1, x);
    core.raw_write_8(POS_Y, -1, y);
}

fn write_if_changed(core: &mut Core, at: u32, bytes: &[u8]) {
    let mut now = vec![0u8; bytes.len()];
    core.raw_read_range(at, -1, &mut now);
    if now != bytes {
        core.raw_write_range(at, -1, bytes);
    }
}

/// The animation is over: palette 8 as it was, the tiles cleared, and
/// the turn goes on ([`playing_fn`]).
fn finish(core: &mut Core) {
    let mut p = [0u8; 32];
    core.raw_read_range(SAVED_PALETTE, -1, &mut p);
    for base in [PAL_BUFFER, PAL_RAM] {
        write_if_changed(core, palette_at(base), &p);
    }
    for (first, n) in TILE_RUNS {
        write_if_changed(core, OBJ_VRAM + 32 * first as u32, &vec![0u8; 32 * n as usize]);
    }
    core.raw_write_8(KIND, -1, 0);
}

/// Every map frame, from the effects pass: the sprites of a heal in
/// progress.
pub fn draw(core: &mut Core) {
    let kind = core.raw_read_8(KIND, -1);
    if kind == 0 {
        return;
    }
    let Some(anim) = anim(kind) else { return };
    // While the camera moves to the structure, wait: Dual Strike starts the
    // animation when it is there.
    let map = core.raw_read_32(MAP_POINTER, -1);
    let scroll = core.raw_read_32(map + 4, -1);
    let clock = core.raw_read_32(GAME_CLOCK, -1);
    if scroll != core.raw_read_32(LAST_SCROLL, -1) {
        core.raw_write_32(LAST_SCROLL, -1, scroll);
        core.raw_write_32(START, -1, clock.wrapping_add(1));
        return;
    }
    let t = clock.wrapping_sub(core.raw_read_32(START, -1));
    if t > u32::MAX / 2 {
        return;
    }
    let Some(cell) = anim.cell_at(t) else {
        finish(core);
        return;
    };
    let colours: Vec<u8> = anim.palette.iter().flat_map(|c| c.to_le_bytes()).collect();
    for base in [PAL_BUFFER, PAL_RAM] {
        write_if_changed(core, palette_at(base), &colours);
    }
    let frame = &anim.frames[cell];
    for (tile, bytes) in &frame.tiles {
        write_if_changed(core, OBJ_VRAM + 32 * *tile as u32, bytes);
    }
    let (sx, sy) = (core.raw_read_16(map + 4, -1) as i32, core.raw_read_16(map + 6, -1) as i32);
    let (x, y) = (core.raw_read_8(POS_X, -1) as i32, core.raw_read_8(POS_Y, -1) as i32);
    let (ax, ay) = (16 * x + anim.anchor.0 - sx, 16 * y + anim.anchor.1 - sy);
    // The map's own sprites are already in the list, from entry 16 on (the
    // first 16 are left for this pass): the effect takes the entries still
    // parked this frame (y = 160) and leaves the others alone. The list
    // pointer moves past the first 16 it fills, as before.
    let base = core.raw_read_32(SPRITE_LIST, -1);
    let (reserved_end, list_end) = (base + 8 * RESERVED, base + SPRITE_LIST_SIZE);
    let mut oam = core.raw_read_32(OAM_NEXT, -1);
    let mut next = oam;
    for s in &frame.sprites {
        let (px, py) = (ax + s.dx as i32, ay + s.dy as i32);
        let n = if s.big { 16 } else { 8 };
        if !(-n..240).contains(&px) || !(-n..160).contains(&py) {
            continue;
        }
        while oam + 8 <= list_end && oam >= reserved_end && core.raw_read_16(oam, -1) & 0xFF != PARKED_Y {
            oam += 8;
        }
        if oam + 8 > list_end {
            break;
        }
        let size = if s.big { 0x4000 } else { 0 };
        let flips = (s.hflip as u16) << 12 | (s.vflip as u16) << 13;
        core.raw_write_16(oam, -1, (py & 0xFF) as u16);
        core.raw_write_16(oam + 2, -1, size | flips | (px & 0x1FF) as u16);
        core.raw_write_16(oam + 4, -1, s.tile | PALETTE << 12);
        oam += 8;
        if oam <= reserved_end {
            next = oam;
        }
    }
    core.raw_write_32(OAM_NEXT, -1, next);
}

/// Sprite-list entries before the map's own sprites, and the y of an entry
/// nothing uses this frame.
const RESERVED: u32 = 16;
const PARKED_Y: u16 = 0xA0;

// --- Holding the turn while it plays -------------------------------------------

/// `show(x, y, parent, script)`: `Proc_StartBlocking(script, parent)` (the
/// turn-start loop waits for it, as for a cannon's shot) and
/// `sub_0802909C(x, y)` (the camera goes there, as for a shot), written
/// to free ROM. `bl` does not reach from there: far calls through r3.
const ROM: u32 = 0x0874_9A00;
pub const SHOW_FN: u32 = ROM;
/// The proc the turn-start loop waits for: `PROC_WHILE(playing)`, then
/// the end.
pub const WAIT: u32 = ROM + 0x40;
const PLAYING_FN: u32 = ROM + 0x60;
const ROM_SENTINEL: u32 = ROM + 0x1FC;
const ROM_MAGIC: u32 = 0x3948_5344; // "DSH9"
const PROC_START_BLOCKING: u32 = 0x0801_C95D;
const CAMERA_TO: u32 = 0x0802_909D;
const PROC_WHILE: u16 = 0x14;

fn show_fn() -> Vec<u8> {
    let h: [u16; 20] = [
        0xB530, // push {r4, r5, lr}
        0x1C04, // adds r4, r0, #0 (x)
        0x1C0D, // adds r5, r1, #0 (y)
        0x1C11, // adds r1, r2, #0 (parent)
        0x1C18, // adds r0, r3, #0 (script)
        0x4B07, // ldr r3, =Proc_StartBlocking
        0x467A, // mov r2, pc
        0x3205, // adds r2, #5
        0x4696, // mov lr, r2
        0x4718, // bx r3
        0x1C20, // adds r0, r4, #0
        0x1C29, // adds r1, r5, #0
        0x4B04, // ldr r3, =sub_0802909C
        0x467A, // mov r2, pc
        0x3205, // adds r2, #5
        0x4696, // mov lr, r2
        0x4718, // bx r3
        0xBC30, // pop {r4, r5}
        0xBC01, // pop {r0}
        0x4700, // bx r0
    ];
    let mut b: Vec<u8> = h.iter().flat_map(|v| v.to_le_bytes()).collect();
    b.extend_from_slice(&PROC_START_BLOCKING.to_le_bytes());
    b.extend_from_slice(&CAMERA_TO.to_le_bytes());
    b
}

fn wait_script() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&PROC_WHILE.to_le_bytes());
    b.extend_from_slice(&[0u8; 2]);
    b.extend_from_slice(&(PLAYING_FN | 1).to_le_bytes());
    b.extend_from_slice(&[0u8; 8]);
    b
}

/// `playing()`: 1 while a heal animation plays (its structure in RAM),
/// else 0.
fn playing_fn() -> Vec<u8> {
    let h: [u16; 6] = [
        0x4802, // ldr r0, =KIND
        0x7800, // ldrb r0, [r0]
        0x2800, // cmp r0, #0
        0xD000, // beq (return 0)
        0x2001, // movs r0, #1
        0x4770, // bx lr
    ];
    let mut b: Vec<u8> = h.iter().flat_map(|v| v.to_le_bytes()).collect();
    b.extend_from_slice(&KIND.to_le_bytes());
    b
}

/// Writes the functions and the wait once (the same bytes on every peer).
pub fn install(core: &mut Core) {
    if art().is_none() || core.raw_read_32(ROM_SENTINEL, -1) == ROM_MAGIC {
        return;
    }
    core.raw_write_range(SHOW_FN, -1, &show_fn());
    core.raw_write_range(WAIT, -1, &wait_script());
    core.raw_write_range(PLAYING_FN, -1, &playing_fn());
    core.raw_write_32(ROM_SENTINEL, -1, ROM_MAGIC);
}

pub fn available() -> bool {
    art().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room() {
        assert!(STATE_END <= 0x0203_FDC8);
        assert_eq!(TILE_RUNS.iter().map(|r| r.1).sum::<u16>(), 51);
        assert_eq!(show_fn().len(), 48);
        assert!(SHOW_FN + 48 <= WAIT);
        assert_eq!(wait_script().len(), 16);
        assert!(WAIT + 16 <= PLAYING_FN && PLAYING_FN % 4 == 0);
        assert!(PLAYING_FN + playing_fn().len() as u32 <= ROM_SENTINEL);
    }

    /// The anim format, on a made-up anim: two cells and a sequence.
    #[test]
    fn parses_an_anim() {
        let words: [u16; 16] = [
            4, 8, // cell table at +4, sequence table at +8
            6, 14, // the cells at table+6 (+10) and table+14 (+18)
            12, // the pairs at sequence table+12 (+20)
            1, 0x80E0, 0x81F8, 0x0008, // one 16x32 piece at (-8, -32), unit 8
            0, // an empty cell
            3, 0, 5, 1, 0, 0,
        ];
        let base = 0x0200_0000;
        let hw = |a: u32| words.get(((a - base) / 2) as usize).map(|&v| v as u32);
        let (cells, seq) = parse(&hw, base).unwrap();
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0], vec![Piece { x: -8, y: -32, w: 16, h: 32, at: 8, hflip: false, vflip: false }]);
        assert!(cells[1].is_empty());
        assert_eq!(seq, vec![(3, 0), (5, 1)]);
    }

    #[test]
    fn shares_mirror_images() {
        // A 16x16 piece and the same piece mirrored: one set of tiles.
        let mut pixels = vec![0u8; 128];
        for (i, b) in pixels.iter_mut().enumerate() {
            *b = (i % 7) as u8 | 0x10;
        }
        let p = Piece { x: 0, y: 0, w: 16, h: 16, at: 0, hflip: false, vflip: false };
        let f = compile(&pixels, &[p, Piece { x: 16, hflip: true, ..p }]);
        assert_eq!(f.sprites.len(), 2);
        assert_eq!(f.tiles.len(), 4);
        assert_eq!(f.sprites[0].tile, f.sprites[1].tile);
        assert_ne!(f.sprites[0].hflip, f.sprites[1].hflip);
    }
}
