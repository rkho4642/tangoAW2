//! The Volcano's colours in battle. The game loads them into sprite
//! palette 12 (`0x0803FE0A`), which is also the fourth army's buildings'
//! palette: no campaign map has both, but a Versus map with a Volcano and
//! Yellow Comet drew Yellow Comet's HQ, cities and bases in the Volcano's
//! colours. In Versus the Volcano gets sprite palette 2 instead, which the
//! battle map neither uses nor loads (5 is the day banner's).
//!
//! In the DS Campaign (Ring of Fire) the Volcano takes Dual Strike's colours,
//! read from the pack: Dual Strike's map sprites' palette file `bmap/00e`,
//! sub-palette 13 (its Volcano's, drawn at `bmap/015` +0x4700). Dual Strike's
//! palette is AW2's own with its greens made yellow-green (indices 1..5, the
//! lava, are the same), so it colours AW2's Volcano index for index. AW2's
//! maps keep AW2's colours.

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

const DS_PALETTES: &str = "bmap/00e";
const DS_VOLCANO_PALETTE: usize = 13;
/// AW2's colour 0 of the Volcano's palette (clear either way).
const AW2_CLEAR: u16 = 0x3DEF;

/// Dual Strike's Volcano colours.
fn ds_colours() -> Option<&'static [u8; 32]> {
    static C: std::sync::OnceLock<Option<[u8; 32]>> = std::sync::OnceLock::new();
    C.get_or_init(|| {
        let pal = crate::ds_pack::pack()?.file(DS_PALETTES)?;
        let mut c: [u8; 32] = pal.get(32 * DS_VOLCANO_PALETTE..32 * DS_VOLCANO_PALETTE + 32)?.try_into().ok()?;
        c[0..2].copy_from_slice(&AW2_CLEAR.to_le_bytes());
        Some(c)
    })
    .as_ref()
}

pub fn structures(core: &mut Core) {
    core.raw_write_16(MOVED, -1, 0);
}

pub fn volcano_palette(core: &mut Core) {
    if core.gba().cpu().gpr(1) as u32 != 0x200 + GAME_PALETTE * 32 {
        return;
    }
    if crate::ds_campaign::active(core) {
        if let Some(colours) = ds_colours() {
            // The copy's source made its destination, which holds Dual
            // Strike's colours (palette RAM too, until the next upload).
            let dest = 0x200 + GAME_PALETTE * 32;
            for base in [PAL_BUFFER, PAL_RAM] {
                core.raw_write_range(base + dest, -1, colours);
            }
            core.gba_mut().cpu_mut().set_gpr(0, (PAL_BUFFER + dest) as i32);
        }
        return;
    }
    if !crate::pvp::in_versus(core) {
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
