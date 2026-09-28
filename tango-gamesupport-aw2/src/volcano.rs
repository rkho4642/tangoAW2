//! The Volcano's colours in battle. The game loads them into sprite
//! palette 12 (`0x0803FE0A`), which is also the fourth army's buildings'
//! palette: no campaign map has both, but a Versus map with a Volcano and
//! Yellow Comet drew Yellow Comet's HQ, cities and bases in the Volcano's
//! colours. In Versus the Volcano gets sprite palette 2 instead, which the
//! battle map neither uses nor loads (5 is the day banner's).

use mgba::core::Core;

/// The map's structure graphics are loaded (`sub_0803FD80`, with the
/// sprite tile base in r1).
pub const STRUCTURES: u32 = 0x0803_FD80;
/// Its Volcano branch, about to copy the Volcano's palette (r0 source, r1
/// palette offset 0x380 = sprite palette 12, r2 size); r7 is the tile base.
pub const VOLCANO_PALETTE: u32 = 0x0803_FE14;
const GAME_PALETTE: u32 = 12;
const PALETTE: u32 = 2;
/// The Volcano's colours (the game's source for the copy).
const COLOURS: u32 = 0x080D_3FC4;
const PAL_BUFFER: u32 = 0x0300_20C0;
const PAL_RAM: u32 = 0x0500_0000;
/// The Volcano's first sprite tile while it uses [`PALETTE`], plus one (0
/// = not moved on this map).
const MOVED: u32 = 0x0203_FFA2;
/// One 64 x 64 sprite.
const TILES: u32 = 64;

pub fn structures(core: &mut Core) {
    core.raw_write_16(MOVED, -1, 0);
}

pub fn volcano_palette(core: &mut Core) {
    if !crate::pvp::in_versus(core) || core.gba().cpu().gpr(1) as u32 != 0x200 + GAME_PALETTE * 32 {
        return;
    }
    let base = (core.gba().cpu().gpr(7) as u32 + 0xE8) & 0x3FF;
    core.raw_write_16(MOVED, -1, base as u16 + 1);
    core.gba_mut().cpu_mut().set_gpr(1, (0x200 + PALETTE * 32) as i32);
}

/// At the sprite flush: the Volcano's sprites with its own palette.
pub fn recolour(core: &mut Core, start: u32, at: u32) {
    let moved = core.raw_read_16(MOVED, -1) as u32;
    if moved == 0 || crate::design::in_map_editor(core) {
        return;
    }
    let base = moved - 1;
    let mut drawn = false;
    let mut p = start;
    while p + 8 <= at {
        let a2 = core.raw_read_16(p + 4, -1);
        let tile = (a2 & 0x3FF) as u32;
        if (a2 >> 12) as u32 == GAME_PALETTE && (base..base + TILES).contains(&tile) {
            core.raw_write_16(p + 4, -1, (a2 & 0x0FFF) | ((PALETTE as u16) << 12));
            drawn = true;
        }
        p += 8;
    }
    // Its colours, whenever it is drawn (kept, should anything have loaded
    // other colours there meanwhile).
    if drawn {
        let mut pal = [0u8; 32];
        core.raw_read_range(COLOURS, -1, &mut pal);
        for base in [PAL_BUFFER, PAL_RAM] {
            core.raw_write_range(base + 0x200 + PALETTE * 32, -1, &pal);
        }
    }
}
