//! Black Hole's inventions drawn in the Design Room with the game's own
//! battle graphics.
//!
//! The editor never shows inventions: their map tiles draw as plain grass,
//! and in battle everything you see of them is sprites that the battle's
//! own code places (tiles decompressed from the ROM, Black Hole's unit
//! palette, priority 3). tangoAW2 does the same in the editor: it loads
//! those tiles and palettes into sprite memory the editor never uses, and
//! appends each placed invention's sprites, laid out exactly as a battle
//! lays them out, to the frame's sprite list at the game's VBlank flush
//! (the same trap as the title badge). While an invention is picked on the
//! Silo entry of the tool bar, its picture shows above the bar.
//!
//! Everything comes from the ROM at run time (tangoAW2 ships no game art),
//! and runs inside the emulated frame, so it is deterministic.

use mgba::core::Core;

/// Where a piece's tiles come from.
#[derive(Clone, Copy)]
enum Src {
    /// Uncompressed 4bpp tiles in the ROM.
    Raw(u32),
    /// LZ77-compressed block in the ROM, and the first tile within it.
    Lz(u32, u32),
    /// The Black Crystal (`None`) or a piece of the Black Obelisk, from
    /// [`crate::ds_art`].
    Art(Option<usize>),
}

/// One sprite of an invention: offset from the footprint's top-left
/// corner (pixels), GBA shape/size, tile count and source.
#[derive(Clone, Copy)]
struct Piece {
    dx: i32,
    dy: i32,
    shape: u16,
    size: u16,
    tiles: u32,
    src: Src,
}

const fn piece(dx: i32, dy: i32, w: u32, h: u32, src: Src) -> Piece {
    // GBA sprite shapes: 0 square, 1 wide, 2 tall; size 0..3.
    let (shape, size) = match (w, h) {
        (8, 8) => (0, 0),
        (16, 16) => (0, 1),
        (32, 32) => (0, 2),
        (64, 64) => (0, 3),
        (16, 8) => (1, 0),
        (32, 8) => (1, 1),
        (32, 16) => (1, 2),
        (64, 32) => (1, 3),
        (8, 16) => (2, 0),
        (8, 32) => (2, 1),
        (16, 32) => (2, 2),
        (32, 64) => (2, 3),
        _ => (0, 0),
    };
    Piece {
        dx,
        dy,
        shape,
        size,
        tiles: w / 8 * (h / 8),
        src,
    }
}

// The battle's sources (see `0x0803FD80`, the invention graphics loader).
const BLACK_CANNON_DOWN: u32 = 0x080D_24E0;
const BLACK_CANNON_UP: u32 = 0x080D_2AE8;
const BLACK_FACTORY: u32 = 0x080D_22C4;
const VOLCANO: u32 = 0x080D_3268;

const MINI_S: &[Piece] = &[piece(0, -16, 16, 32, Src::Raw(0x080D_06C4))];
const MINI_N: &[Piece] = &[piece(0, -16, 16, 32, Src::Raw(0x080D_07C4))];
const MINI_W: &[Piece] = &[piece(0, -16, 16, 32, Src::Raw(0x080D_04C4))];
const MINI_E: &[Piece] = &[piece(0, -16, 16, 32, Src::Raw(0x080D_05C4))];
const LASER: &[Piece] = &[piece(0, -16, 16, 32, Src::Raw(0x080D_02C4))];
const CANNON_S: &[Piece] = &[
    piece(0, 0, 32, 32, Src::Lz(BLACK_CANNON_DOWN, 36)),
    piece(32, 0, 16, 32, Src::Lz(BLACK_CANNON_DOWN, 52)),
    piece(0, 32, 32, 16, Src::Lz(BLACK_CANNON_DOWN, 60)),
    piece(32, 32, 16, 16, Src::Lz(BLACK_CANNON_DOWN, 68)),
];
/// The battle draws the Deathray with the upward Black Cannon's dish.
const CANNON_N: &[Piece] = &[
    piece(0, 0, 32, 32, Src::Lz(BLACK_CANNON_UP, 0)),
    piece(32, 0, 16, 32, Src::Lz(BLACK_CANNON_UP, 16)),
    piece(0, 32, 32, 16, Src::Lz(BLACK_CANNON_UP, 24)),
    piece(32, 32, 16, 16, Src::Lz(BLACK_CANNON_UP, 32)),
];
const FACTORY: &[Piece] = &[
    piece(0, 0, 32, 64, Src::Lz(BLACK_FACTORY, 0)),
    piece(32, 0, 16, 32, Src::Lz(BLACK_FACTORY, 32)),
    piece(32, 32, 16, 32, Src::Lz(BLACK_FACTORY, 40)),
];
const VOLCANO_ART: &[Piece] = &[piece(0, 0, 64, 64, Src::Lz(VOLCANO, 0))];
/// Drawn as the battle draws them (`crate::obelisk`'s sprite definitions).
const CRYSTAL: &[Piece] = &[piece(0, -16, 16, 32, Src::Art(None))];
const OBELISK: &[Piece] = &[
    piece(0, 0, 32, 32, Src::Art(Some(0))),
    piece(32, 0, 16, 32, Src::Art(Some(1))),
    piece(0, 32, 32, 16, Src::Art(Some(2))),
    piece(32, 32, 16, 16, Src::Art(Some(3))),
];

/// Per invention, in `design::INVENTIONS` order: its pieces and whether it
/// uses the Volcano's palette.
fn art(i: usize) -> (&'static [Piece], bool) {
    match i {
        0 => (MINI_S, false),
        1 => (MINI_N, false),
        2 => (MINI_W, false),
        3 => (MINI_E, false),
        4 => (LASER, false),
        5 => (CANNON_S, false),
        6 => (CANNON_N, false),
        7 => (FACTORY, false),
        8 => (VOLCANO_ART, true),
        10 => (CRYSTAL, false),
        11 => (OBELISK, false),
        _ => (CANNON_N, false),
    }
}

const INVENTION_COUNT: usize = 12;
const FACTORY_INDEX: usize = 7;
const VOLCANO_INDEX: usize = 8;

fn key(s: &Src) -> (usize, u32) {
    match *s {
        Src::Raw(a) => (a as usize, 0),
        Src::Lz(a, t) => (a as usize, t),
        Src::Art(p) => (usize::MAX, p.map_or(u32::MAX, |p| p as u32)),
    }
}

/// Sprite tiles (1D) the editor never uses, found by watching its sprite
/// lists through a whole editing session: 289..=535.
const FIRST_TILE: u32 = 289;
const LAST_TILE: u32 = 535;
const OBJ_VRAM: u32 = 0x0601_0000;
const PAL_BUFFER: u32 = 0x0300_20C0;
const PAL_RAM: u32 = 0x0500_0000;
/// The only sprite palette the editor never uses.
const FREE_PALETTE: u32 = 2;
/// Slot 4's unit palette (Black Hole's while slot 4 is Black Hole).
const SLOT4_UNIT_PALETTE: u32 = 12;
const BLACK_HOLE_PALETTE: u32 = 0x080D_3E84;
const VOLCANO_PALETTE: u32 = 0x080D_3EC4;

/// The sprite palette holding Black Hole's colours in the editor: the free
/// one, or slot 4's unit palette on a Volcano map.
pub fn black_hole_palette(core: &Core) -> u32 {
    if crate::design::volcano_on_map(core) {
        SLOT4_UNIT_PALETTE
    } else {
        FREE_PALETTE
    }
}

/// Distinct sources, each loaded once: (source, tile count), in sprite
/// memory order from FIRST_TILE. All of them do not fit, but a map has
/// either the Black Factory or the Volcano (`design::stamp`), so only that
/// one's tiles are loaded.
fn blocks(volcano_on_map: bool) -> Vec<(Src, u32)> {
    let mut out: Vec<(Src, u32)> = Vec::new();
    for i in 0..INVENTION_COUNT {
        if i == if volcano_on_map { FACTORY_INDEX } else { VOLCANO_INDEX } {
            continue;
        }
        for p in art(i).0 {
            if !out.iter().any(|(s, _)| key(s) == key(&p.src)) {
                out.push((p.src, p.tiles));
            }
        }
    }
    out
}

/// Where a piece's tiles sit in sprite memory.
fn tile_of(src: Src, volcano_on_map: bool) -> u32 {
    let mut at = FIRST_TILE;
    for (s, n) in blocks(volcano_on_map) {
        if key(&s) == key(&src) {
            return at;
        }
        at += n;
    }
    at
}

/// GBA BIOS-style LZ77 (type 0x10) decompression, from the ROM.
fn lz77(core: &Core, addr: u32) -> Vec<u8> {
    let header = core.raw_read_32(addr, -1);
    let size = (header >> 8) as usize;
    let mut out = Vec::with_capacity(size);
    let mut p = addr + 4;
    while out.len() < size {
        let flags = core.raw_read_8(p, -1);
        p += 1;
        for bit in 0..8 {
            if out.len() >= size {
                break;
            }
            if flags & (0x80 >> bit) != 0 {
                let b1 = core.raw_read_8(p, -1) as usize;
                let b2 = core.raw_read_8(p + 1, -1) as usize;
                p += 2;
                let len = (b1 >> 4) + 3;
                let disp = ((b1 & 0xF) << 8 | b2) + 1;
                for _ in 0..len {
                    let b = out[out.len() - disp];
                    out.push(b);
                }
            } else {
                out.push(core.raw_read_8(p, -1));
                p += 1;
            }
        }
    }
    out
}

fn palette_bytes(core: &Core, addr: u32) -> [u8; 32] {
    let mut p = [0u8; 32];
    core.raw_read_range(addr, -1, &mut p);
    p
}

/// Every frame in the editor: keep the tiles and palette in place (the
/// editor's other screens may have used that memory meanwhile).
pub fn tick(core: &mut Core, volcano_on_map: bool) {
    let mut at = FIRST_TILE;
    let mut lz_cache: Vec<(u32, Vec<u8>)> = Vec::new();
    for (src, n) in blocks(volcano_on_map) {
        let bytes = match src {
            Src::Art(None) => crate::ds_art::art().map_or(vec![0; 256], |a| a.crystal.clone()),
            Src::Art(Some(p)) => {
                let (a, b) = crate::ds_art::OBELISK_PIECES[p];
                crate::ds_art::art().map_or(vec![0; (b - a) * 32], |art| art.obelisk[a * 32..b * 32].to_vec())
            }
            Src::Raw(a) => {
                let mut b = vec![0u8; n as usize * 32];
                core.raw_read_range(a, -1, &mut b);
                b
            }
            Src::Lz(a, first) => {
                if !lz_cache.iter().any(|(k, _)| *k == a) {
                    let d = lz77(core, a);
                    lz_cache.push((a, d));
                }
                let d = &lz_cache.iter().find(|(k, _)| *k == a).unwrap().1;
                let from = (first * 32) as usize;
                d[from..from + n as usize * 32].to_vec()
            }
        };
        if at + n > LAST_TILE + 1 {
            break;
        }
        let dest = OBJ_VRAM + at * 32;
        let mut now = vec![0u8; bytes.len()];
        core.raw_read_range(dest, -1, &mut now);
        if now != bytes {
            core.raw_write_range(dest, -1, &bytes);
        }
        at += n;
    }
    let pal = palette_bytes(
        core,
        if volcano_on_map {
            VOLCANO_PALETTE
        } else {
            BLACK_HOLE_PALETTE
        },
    );
    for base in [PAL_BUFFER, PAL_RAM] {
        core.raw_write_range(base + 0x200 + FREE_PALETTE * 32, -1, &pal);
    }
}

/// A sprite to append: the invention `i` with its footprint's top-left
/// corner at screen (x, y).
pub struct Placed {
    pub i: usize,
    pub x: i32,
    pub y: i32,
    /// Sprite priority: 3 on the map (under units, as in battle), 0 for the
    /// tool bar's preview (over the bar).
    pub priority: u16,
}

/// Appends the placed inventions' sprites to the frame's sprite list (at
/// the VBlank flush), between `at` and `end`; returns the new `at`.
pub fn append(core: &mut Core, list: &[Placed], volcano_on_map: bool, bottom: i32, mut at: u32, end: u32) -> u32 {
    for p in list {
        let (pieces, volcano) = art(p.i);
        // The free palette holds the Volcano's colours on a Volcano map;
        // the rest then use slot 4's unit palette (Black Hole's own while
        // slot 4 is Black Hole).
        let palette = if volcano || !volcano_on_map {
            FREE_PALETTE
        } else {
            SLOT4_UNIT_PALETTE
        };
        for piece in pieces {
            let (x, y) = (p.x + piece.dx, p.y + piece.dy);
            let (w, h) = dims(piece);
            // Nothing below `bottom` (the top of an open tool bar, which
            // the sprites would otherwise draw over).
            if x + w <= 0 || x >= 240 || y + h <= 0 || y >= 160 || y + h > bottom {
                continue;
            }
            if at + 8 > end {
                return at;
            }
            let attr0 = (y as u16 & 0xFF) | (piece.shape << 14);
            let attr1 = (x as u16 & 0x1FF) | (piece.size << 14);
            let attr2 = tile_of(piece.src, volcano_on_map) as u16 | (p.priority << 10) | ((palette as u16) << 12);
            core.raw_write_16(at, -1, attr0);
            core.raw_write_16(at + 2, -1, attr1);
            core.raw_write_16(at + 4, -1, attr2);
            at += 8;
        }
    }
    at
}

fn dims(p: &Piece) -> (i32, i32) {
    match (p.shape, p.size) {
        (0, s) => (8 << s, 8 << s),
        (1, 0) => (16, 8),
        (1, 1) => (32, 8),
        (1, 2) => (32, 16),
        (1, _) => (64, 32),
        (2, 0) => (8, 16),
        (2, 1) => (8, 32),
        (2, 2) => (16, 32),
        _ => (32, 64),
    }
}

/// The size of an invention's picture, for centring a preview.
pub fn extent(i: usize) -> (i32, i32, i32, i32) {
    let (pieces, _) = art(i);
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for p in pieces {
        let (w, h) = dims(p);
        x0 = x0.min(p.dx);
        y0 = y0.min(p.dy);
        x1 = x1.max(p.dx + w);
        y1 = y1.max(p.dy + h);
    }
    (x0, y0, x1, y1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_fit_in_the_free_range() {
        for volcano in [false, true] {
            let n: u32 = blocks(volcano).iter().map(|&(_, n)| n).sum();
            assert!(FIRST_TILE + n <= LAST_TILE + 1, "{volcano}: {n} tiles");
        }
    }
}
