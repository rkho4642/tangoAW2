//! Dual Strike's heal effect for the Black Crystal and Black Obelisk
//! ([`crate::obelisk`]): when they heal Black Hole's units at its turn
//! start, each Crystal flashes its outline and every unit healed shows a
//! droplet that splashes into sparkles, as in Dual Strike.
//!
//! The pictures are Dual Strike's own (`bmap/06c`, 26 frames of 16x16, rows
//! of pixels, palette `bmap/06e`), taken from the pack at run time. They
//! are drawn as sprites on the map from the frame's effects pass (the
//! sandstorm's hook, [`crate::sandstorm`]): 8 tiles no map screen uses
//! (0x1F9: the structure's frame, then the units'), and 3 colours of OBJ
//! palette 7 nothing on the map uses (8, 9, 15; the sand has 5-7).
//!
//! What to draw is kept in RAM (when, and where), so a rollback redraws
//! the same frames.

use mgba::core::Core;
use std::sync::OnceLock;

const OBJ_VRAM: u32 = 0x0601_0000;
const FIRST_TILE: u32 = 0x1F9;
const STRUCTURE_TILES: u32 = FIRST_TILE;
const UNIT_TILES: u32 = FIRST_TILE + 4;
const PALETTE: u16 = 7;
/// The effect's colours 1..3 go to these colours of the palette.
const COLOUR_SLOTS: [usize; 3] = [8, 9, 15];
const PAL_BUFFER: u32 = 0x0300_20C0;
const PAL_RAM: u32 = 0x0500_0000;

const FRAMES: usize = 26;
const OUTLINE_FRAMES: usize = 4;
const TICKS_PER_FRAME: u32 = 3;
/// The Crystal's outline flashes twice, then the units' splash plays.
const OUTLINE_TICKS: u32 = 2 * OUTLINE_FRAMES as u32 * TICKS_PER_FRAME;
const SPLASH_TICKS: u32 = (FRAMES - OUTLINE_FRAMES) as u32 * TICKS_PER_FRAME;
const DURATION: u32 = OUTLINE_TICKS + SPLASH_TICKS;

/// RAM tangoAW2 keeps: start clock (u32), counts, then positions.
const STATE: u32 = 0x0203_FD80;
const START: u32 = STATE;
const CRYSTAL_COUNT: u32 = STATE + 4;
const UNIT_COUNT: u32 = STATE + 5;
const CRYSTALS: u32 = STATE + 8; // 8 x (x, y)
const UNITS: u32 = STATE + 0x18; // 48 x (x, y)
const MAX_CRYSTALS: usize = 8;
const MAX_UNITS: usize = 48;
const GAME_CLOCK: u32 = 0x0300_4008;
const OAM_NEXT: u32 = 0x0300_141C;
/// The frame's sprite list (`sub_0801BC08`: base at +0, flushed to OAM):
/// 128 entries.
const SPRITE_LIST: u32 = 0x0300_0278;
const SPRITE_LIST_SIZE: u32 = 0x400;
const MAP_POINTER: u32 = 0x0849_9590;

struct Art {
    /// Per frame, 4 tiles (2x2, in rows), colours already moved.
    frames: Vec<[u8; 128]>,
    colours: [u16; 3],
}

static ART: OnceLock<Option<Art>> = OnceLock::new();

fn art() -> Option<&'static Art> {
    ART.get_or_init(|| {
        let pack = crate::ds_pack::pack()?;
        let pixels = crate::ds_art::lz10(pack.file("bmap/06c")?)?;
        let pal = pack.file("bmap/06e")?;
        if pixels.len() < FRAMES * 128 || pal.len() < 8 {
            return None;
        }
        let colour = |i: usize| u16::from_le_bytes([pal[2 * i], pal[2 * i + 1]]);
        let mut frames = Vec::new();
        for f in 0..FRAMES {
            let mut t = [0u8; 128];
            for y in 0..16 {
                for x in 0..16 {
                    let i = 256 * f + 16 * y + x;
                    let v = (pixels[i / 2] >> (4 * (i & 1))) & 15;
                    let v = match v {
                        1..=3 => COLOUR_SLOTS[v as usize - 1] as u8,
                        _ => 0,
                    };
                    let tile = (y / 8) * 2 + x / 8;
                    t[32 * tile + 4 * (y % 8) + (x % 8) / 2] |= v << (4 * (x & 1));
                }
            }
            frames.push(t);
        }
        Some(Art { frames, colours: [colour(1), colour(2), colour(3)] })
    })
    .as_ref()
}

/// A structure shows its heal now (the camera is on it): a Crystal's
/// outline flash first, then the splash on the units it healed.
pub fn start(core: &mut Core, crystals: &[(u8, u8)], units: &[(u8, u8)]) {
    if art().is_none() {
        return;
    }
    let clock = core.raw_read_32(GAME_CLOCK, -1);
    let start = if crystals.is_empty() { clock.wrapping_sub(OUTLINE_TICKS) } else { clock };
    core.raw_write_32(START, -1, start);
    let c = crystals.len().min(MAX_CRYSTALS);
    let u = units.len().min(MAX_UNITS);
    core.raw_write_8(CRYSTAL_COUNT, -1, c as u8);
    core.raw_write_8(UNIT_COUNT, -1, u as u8);
    for (k, &(x, y)) in crystals[..c].iter().enumerate() {
        core.raw_write_8(CRYSTALS + 2 * k as u32, -1, x);
        core.raw_write_8(CRYSTALS + 2 * k as u32 + 1, -1, y);
    }
    for (k, &(x, y)) in units[..u].iter().enumerate() {
        core.raw_write_8(UNITS + 2 * k as u32, -1, x);
        core.raw_write_8(UNITS + 2 * k as u32 + 1, -1, y);
    }
}

fn write_if_changed(core: &mut Core, at: u32, bytes: &[u8]) {
    let mut now = vec![0u8; bytes.len()];
    core.raw_read_range(at, -1, &mut now);
    if now != bytes {
        core.raw_write_range(at, -1, bytes);
    }
}

/// Every map frame, from the effects pass: the sprites of a heal in
/// progress.
pub fn draw(core: &mut Core) {
    let Some(art) = art() else { return };
    let units = core.raw_read_8(UNIT_COUNT, -1) as u32;
    let crystals = core.raw_read_8(CRYSTAL_COUNT, -1) as u32;
    if units == 0 && crystals == 0 {
        return;
    }
    let t = core.raw_read_32(GAME_CLOCK, -1).wrapping_sub(core.raw_read_32(START, -1));
    if t >= DURATION {
        core.raw_write_8(UNIT_COUNT, -1, 0);
        core.raw_write_8(CRYSTAL_COUNT, -1, 0);
        return;
    }
    for base in [PAL_BUFFER, PAL_RAM] {
        for (i, c) in art.colours.iter().enumerate() {
            let at = base + 0x200 + 32 * PALETTE as u32 + 2 * COLOUR_SLOTS[i] as u32;
            if core.raw_read_16(at, -1) != *c {
                core.raw_write_16(at, -1, *c);
            }
        }
    }
    let map = core.raw_read_32(MAP_POINTER, -1);
    let (sx, sy) = (core.raw_read_16(map + 4, -1) as i32, core.raw_read_16(map + 6, -1) as i32);
    let mut oam = core.raw_read_32(OAM_NEXT, -1);
    let oam_end = core.raw_read_32(SPRITE_LIST, -1) + SPRITE_LIST_SIZE;
    let mut put = |core: &mut Core, x: u8, y: u8, tiles: u32| {
        let (px, py) = (16 * x as i32 - sx, 16 * y as i32 - sy);
        if !(-16..240).contains(&px) || !(-16..160).contains(&py) || oam + 8 > oam_end {
            return;
        }
        core.raw_write_16(oam, -1, (py & 0xFF) as u16);
        core.raw_write_16(oam + 2, -1, 0x4000 | (px & 0x1FF) as u16);
        core.raw_write_16(oam + 4, -1, tiles as u16 | PALETTE << 12);
        oam += 8;
    };
    if t < OUTLINE_TICKS {
        let f = (t / TICKS_PER_FRAME) as usize % OUTLINE_FRAMES;
        write_if_changed(core, OBJ_VRAM + 32 * STRUCTURE_TILES, &art.frames[f]);
        for k in 0..crystals {
            let (x, y) = (core.raw_read_8(CRYSTALS + 2 * k, -1), core.raw_read_8(CRYSTALS + 2 * k + 1, -1));
            put(core, x, y, STRUCTURE_TILES);
        }
    } else {
        let f = OUTLINE_FRAMES + ((t - OUTLINE_TICKS) / TICKS_PER_FRAME) as usize;
        write_if_changed(core, OBJ_VRAM + 32 * UNIT_TILES, &art.frames[f.min(FRAMES - 1)]);
        for k in 0..units {
            let (x, y) = (core.raw_read_8(UNITS + 2 * k, -1), core.raw_read_8(UNITS + 2 * k + 1, -1));
            put(core, x, y, UNIT_TILES);
        }
    }
    core.raw_write_32(OAM_NEXT, -1, oam);
}

// --- Holding the turn while it plays -------------------------------------------

/// `show(x, y, parent, script)`: `Proc_StartBlocking(script, parent)` (the
/// turn-start loop waits for it, as for a cannon's shot) and
/// `sub_0802909C(x, y)` (the camera goes there, as for a shot), written
/// to free ROM. `bl` does not reach from there: far calls through r3.
const ROM: u32 = 0x0874_9A00;
pub const SHOW_FN: u32 = ROM;
pub const CRYSTAL_WAIT: u32 = ROM + 0x40;
pub const OBELISK_WAIT: u32 = ROM + 0x50;
const ROM_SENTINEL: u32 = ROM + 0x1FC;
const ROM_MAGIC: u32 = 0x3848_5344; // "DSH8"
const PROC_START_BLOCKING: u32 = 0x0801_C95D;
const CAMERA_TO: u32 = 0x0802_909D;
const PROC_SLEEP: u16 = 0x0E;

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

fn wait_script(frames: u32) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&PROC_SLEEP.to_le_bytes());
    b.extend_from_slice(&(frames as u16).to_le_bytes());
    b.extend_from_slice(&[0u8; 12]);
    b
}

/// Writes the function and the waits once (the same bytes on every peer).
pub fn install(core: &mut Core) {
    if core.raw_read_32(ROM_SENTINEL, -1) == ROM_MAGIC {
        return;
    }
    core.raw_write_range(SHOW_FN, -1, &show_fn());
    core.raw_write_range(CRYSTAL_WAIT, -1, &wait_script(DURATION));
    core.raw_write_range(OBELISK_WAIT, -1, &wait_script(SPLASH_TICKS));
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
        assert!(UNITS + 2 * MAX_UNITS as u32 <= 0x0203_FE00);
        assert!(UNIT_TILES + 4 <= 0x1F9 + 17);
        assert_eq!(show_fn().len(), 48);
        assert!(SHOW_FN + 48 <= CRYSTAL_WAIT);
    }
}
