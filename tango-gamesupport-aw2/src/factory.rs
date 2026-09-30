//! The Black Factory for human Black Hole armies and on design maps.
//!
//! The game's factory spawner (0x080607E8) deploys up to three units on the
//! row under the factory each turn, reading the unit types from a per-map
//! table (32 days x 3 slots) at the pointer in [`SPAWN_TABLE_PTR`]. Only the
//! AI's turn setup calls it (0x08061900, for a Black Hole army), and it takes
//! the table from the campaign map header, which design maps don't have: a
//! computer Black Hole there got garbage "units", and a human one got none.
//!
//! Two traps fix both, without touching the ROM file:
//! - after the AI setup stores the table pointer, design maps get Factory
//!   Blues' table (ground units, the campaign's schedule);
//! - in the start-of-turn code, a human Black Hole army's turn runs through
//!   the same spawner once, then carries on where it left off.
//!
//! Both run inside the emulated frame and keep their state in RAM, so
//! rollback and both netplay peers see the same thing.

use mgba::core::Core;

/// Where the spawner reads its unit table from.
const SPAWN_TABLE_PTR: u32 = 0x0300_46B4;
/// Factory Blues' unit table (map 0x99, normal difficulty).
const FACTORY_BLUES_TABLE: u32 = 0x0857_6F23;
/// The spawner: walks the factory's three door tiles for today.
const SPAWNER: u32 = 0x0806_07E8;
/// The instruction right after the AI setup stores the table pointer (a
/// trap's handler runs before its instruction).
const AI_TABLE_STORED: u32 = 0x0806_18AE;
/// Start of turn, just after the per-player turn-start call returns: the
/// scratch registers are dead and the next instruction reloads r0.
const TURN_START: u32 = 0x0802_6810;
/// Set while our detour through the spawner is in flight, so the trap
/// doesn't fire a second time when the spawner returns to [`TURN_START`].
const DETOUR: u32 = 0x0203_FFFC;

const CURRENT_ARMY: u32 = 0x0300_33EC;
/// Player blocks, indexed by the 1-based army number in [`CURRENT_ARMY`].
const PLAYER_SIZE: u32 = 0x3C;
const BLACK_HOLE: u8 = 5;
const HUMAN: u8 = 1;
const MAP_ID: u32 = 0x0300_3FC2;
const DESIGN_MAPS: std::ops::RangeInclusive<u8> = 0xB4..=0xB7;

fn on_design_map(core: &Core) -> bool {
    let id = core.raw_read_8(MAP_ID, -1);
    DESIGN_MAPS.contains(&id) || crate::five::is_five_map(id)
}

fn after_ai_table_stored(core: &mut Core) {
    if on_design_map(core) {
        core.raw_write_32(SPAWN_TABLE_PTR, -1, FACTORY_BLUES_TABLE);
    }
}

fn at_turn_start(core: &mut Core) {
    if core.raw_read_8(DETOUR, -1) != 0 {
        // Back from the spawner.
        core.raw_write_8(DETOUR, -1, 0);
        return;
    }
    let army = core.raw_read_16(CURRENT_ARMY, -1) as u32;
    let block = crate::five::players(core) + PLAYER_SIZE * army;
    if core.raw_read_8(block + 0x1A, -1) != BLACK_HOLE || core.raw_read_8(block + 0x1B, -1) != HUMAN {
        return;
    }
    // A human Black Hole army's turn: the computer's turn setup would have
    // run the spawner here. Detour through it and come back to this
    // instruction (the handler runs before it, so it hasn't run yet; the
    // detour flag lets it through the second time).
    let table = core.raw_read_32(SPAWN_TABLE_PTR, -1);
    if on_design_map(core) || !(0x0800_0000..0x0A00_0000).contains(&table) {
        core.raw_write_32(SPAWN_TABLE_PTR, -1, FACTORY_BLUES_TABLE);
    }
    core.raw_write_8(DETOUR, -1, 1);
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(14, (TURN_START | 1) as i32);
    cpu.set_thumb_pc(SPAWNER);
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    let mut traps: Vec<(u32, Box<dyn Fn(&mut Core)>)> = vec![
        (AI_TABLE_STORED, Box::new(after_ai_table_stored)),
        (TURN_START, Box::new(at_turn_start)),
        (crate::branding::SPRITE_FLUSH, Box::new(crate::branding::flush)),
        (crate::volcano::STRUCTURES, Box::new(crate::volcano::structures)),
        (
            crate::volcano::VOLCANO_PALETTE,
            Box::new(crate::volcano::volcano_palette),
        ),
        (crate::five::MAP_PICKED, Box::new(crate::five::map_picked)),
        (crate::five::RESUME, Box::new(crate::five::before_resume)),
        (crate::design_bar::LIST_BUILT, Box::new(crate::design_bar::list_built)),
        (crate::design_bar::ICON_LOADER, Box::new(crate::design_bar::icon_loader)),
        (crate::design_bar::ICON_LOADED, Box::new(crate::design_bar::icon_loaded)),
        (crate::design_bar::BAR_NAME, Box::new(crate::design_bar::bar_name)),
        (
            crate::design_bar::ICON_PALETTE,
            Box::new(crate::design_bar::icon_palette),
        ),
    ];
    traps.extend(crate::five::traps());
    traps.extend(crate::obelisk::traps());
    traps.extend(crate::design5::traps());
    traps.extend(crate::ds_weather::traps());
    traps.extend(crate::sandstorm::traps());
    traps.extend(crate::wasteland::traps());
    traps.extend(crate::com_tower::traps());
    traps.extend(crate::roster::traps());
    traps.extend(crate::co_roster::traps());
    traps.extend(crate::co_new::traps());
    traps.extend(crate::co_grid::traps());
    traps.extend(crate::co_powers::traps());
    traps.extend(crate::power_anim::traps());
    traps.extend(crate::map_anim::traps());
    traps.extend(crate::unit_actions::traps());
    traps.extend(crate::oozium::traps());
    traps.extend(crate::ds_battle::traps());
    traps.extend(crate::cpu_tactics::traps());
    traps
}

#[cfg(test)]
mod trap_tests {
    /// Two traps at one address would silently replace each other.
    #[test]
    fn no_two_traps_share_an_address() {
        let mut seen = std::collections::BTreeMap::new();
        for (at, _) in super::traps() {
            *seen.entry(at).or_insert(0) += 1;
        }
        let twice: Vec<String> = seen.iter().filter(|(_, n)| **n > 1).map(|(a, _)| format!("{a:08x}")).collect();
        assert!(twice.is_empty(), "trapped twice: {twice:?}");
    }
}
