//! The CO screen's unit grid (map menu > CO, its last page), with the Dual
//! Strike pack: every unit, AW2's and the seven new ones, on two pages.
//!
//! How AW2 draws it: the CO screen (`CoInfo`, proc script 0x08616BE4) keeps
//! its page in [`PAGE`] (0..3 texts, 4 the grid); each frame the page's
//! handler (jump table at 0x08084C3A) runs, page 4's `sub_08085244` calls
//! `sub_08085708(proc, army)`, which walks 20 slots of a unit list (`u16`
//! ids, 0 = empty; 0x08616B22, or 0x08616B4A without the Neotank when
//! `sub_080261E8` says so), 4 to a row, 54 px apart, 24 px a row, and per
//! unit draws
//! - its map picture as a 16x16 sprite (`sub_080859A0`): OBJ tile 0x100 +
//!   4 * the unit's map slot (`sub_080261A4(army, type)`, slot per CO
//!   country), from the map sheet `sub_08085950` copies to OBJ tile 0x100
//!   (27 slots, 0x6C tiles) with the army's colours in OBJ palette 0;
//! - a firepower bar (`sub_08085410`: the CO's firepower bonus picks one of
//!   13 bars, for exactly -30, -20, -10, 0, 10, 15, 20, 30, 40, 50, 60, 75
//!   and 80; any other value draws the plain bar);
//! - a range change, or else a move change, on the line below.
//!
//! With the pack the grid has two pages in the build menus' order: page 4
//! ground units, page 5 air and naval units ([`GROUND`], [`AIR_SEA`]). The
//! changes, all traps that do nothing with the pack off:
//! - DOWN on page 4 goes on to page 5 ([`DOWN`]); page 5 runs page 4's
//!   handler ([`HANDLER`]), R (the legend) works there ([`LEGEND`]); UP
//!   from 5 goes back to 4 (the game's own code; the panel stays).
//! - The arrows: page 4 shows up and down, page 5 up ([`ARROWS`]).
//! - The list is tangoAW2's for the page ([`LIST`]).
//! - The new units' pictures: the map sheet copied for the grid is
//!   tangoAW2's copy per CO country ([`SHEETS`]), whose slots for the other
//!   countries' Infantry and Mech (17..26, two per country; the viewed
//!   army uses its own two) hold the new units' map pictures; their slot
//!   (59.., [`crate::roster`]'s) is moved there ([`ICON`]). The palette is
//!   the army's, as for every unit on the grid.
//! - The bar: Dual Strike's bonuses (and the Com Towers', Kindle's) take
//!   the nearest bar ([`BAR`]; away from 0 between two).

use mgba::core::Core;

use crate::ds_weather::is_on;
use crate::roster::{ds_id, NEW_UNITS};

/// The CO screen's page (u32; 0..3 texts, 4 the grid, 5 the second grid).
pub const PAGE: u32 = 0x0300_5940;
const GRID: u32 = 4;
const GRID2: u32 = 5;

/// Build menu order, 4 to a row; each class starts a row.
pub const GROUND: [u16; 20] = [1, 2, 6, 5, 3, 8, 4, 7, 10, 11, 14, 15, 9, 27, 0, 0, 0, 0, 0, 0];
pub const AIR_SEA: [u16; 20] = [16, 17, 19, 20, 12, 13, 0, 0, 21, 22, 23, 24, 18, 26, 0, 0, 0, 0, 0, 0];
const NEOTANK: u16 = 8;
/// AW2's list without the Neotank.
const AW2_LIST_NO_NEOTANK: u32 = 0x0861_6B4A;

// --- Free ROM: 0x087F0000..0x087F4FFF ------------------------------------------

const DATA: u32 = 0x087F_0000;
/// The map sheet per CO country (1..5), 0x6C tiles each.
pub const SHEETS: u32 = DATA;
const SHEET_SIZE: u32 = 0x6C * 32;
/// Four lists of 20 u16: ground, air/sea, each then without the Neotank.
const LISTS: u32 = DATA + 0x4400;
const SENTINEL: u32 = DATA + 0x4FFC;
const MAGIC: u32 = 0x4744_5343; // "CSDG"
pub const DATA_END: u32 = DATA + 0x5000;

/// The game's map sheet (frame 0 is what the CO screen copies).
const AW2_SHEET: u32 = 0x0810_BE60;
/// [`crate::roster`]'s map slot of the first new unit (`FIRST_SLOT`).
const NEW_SLOT: u32 = 59;
/// The sheet's per-country Infantry and Mech slots: 17 + 2 * (country - 1).
const COUNTRY_SLOTS: u32 = 17;
const COUNTRIES: u32 = 5;

/// The sheet slot of new unit `k` (0..6, [`NEW_UNITS`]' order) for CO
/// country `country` (1..5): the other countries' Infantry/Mech slots.
pub fn free_slot(country: u32, k: usize) -> u32 {
    let own = COUNTRY_SLOTS + 2 * (country.clamp(1, COUNTRIES) - 1);
    (COUNTRY_SLOTS..COUNTRY_SLOTS + 2 * COUNTRIES)
        .filter(|&s| s != own && s != own + 1)
        .nth(k)
        .unwrap()
}

fn install(core: &mut Core) -> bool {
    if core.raw_read_32(SENTINEL, -1) == MAGIC {
        return true;
    }
    let mut art = Vec::new();
    for &t in NEW_UNITS.iter() {
        let Some(tiles) = ds_id(t).and_then(|d| crate::ds_unit_art::map_tiles(d, 0)) else {
            return false;
        };
        art.push(tiles);
    }
    let mut sheet = vec![0u8; SHEET_SIZE as usize];
    core.raw_read_range(AW2_SHEET, -1, &mut sheet);
    for country in 1..=COUNTRIES {
        let mut s = sheet.clone();
        for (k, tiles) in art.iter().enumerate() {
            let at = 128 * free_slot(country, k) as usize;
            s[at..at + 128].copy_from_slice(tiles);
        }
        core.raw_write_range(SHEETS + SHEET_SIZE * (country - 1), -1, &s);
    }
    for (k, list) in [GROUND, AIR_SEA].iter().enumerate() {
        for (v, hide) in [false, true].iter().enumerate() {
            let b: Vec<u8> = list
                .iter()
                .map(|&u| if *hide && u == NEOTANK { 0 } else { u })
                .flat_map(|u| u.to_le_bytes())
                .collect();
            core.raw_write_range(LISTS + 40 * (2 * k + v) as u32, -1, &b);
        }
    }
    core.raw_write_32(SENTINEL, -1, MAGIC);
    true
}

fn ready(core: &Core) -> bool {
    is_on(core) && core.raw_read_32(SENTINEL, -1) == MAGIC
}

/// Every frame: writes the ROM data once the pack is on (nothing without).
pub fn tick(core: &mut Core, on: bool) {
    if on {
        install(core);
    }
}

fn page(core: &Core) -> u32 {
    core.raw_read_32(PAGE, -1)
}

/// An army's CO country (1..5), as `sub_08042DE0` works it out.
fn country(core: &Core, army: u32) -> u32 {
    let co = core.raw_read_8(crate::five::players(core) + 0x3C * army + 0x1D, -1) as u32;
    let table = core.raw_read_32(0x0804_2DDC, -1);
    core.raw_read_8(table + co * 0x104 + 0x15, -1) as u32 + 1
}

// --- Traps ------------------------------------------------------------------------

/// DOWN: `cmp r0, #3; ble` (r0 the page) lets pages 0..3 go on; 4 too.
pub const DOWN: u32 = 0x0808_4D12;
const DOWN_GO: u32 = 0x0808_4D18;
fn down(core: &mut Core) {
    if ready(core) && core.gba().cpu().gpr(0) as u32 == GRID {
        core.gba_mut().cpu_mut().set_thumb_pc(DOWN_GO);
    }
}

/// The page handlers' jump table (`cmp r0, #4; bhi`): page 5 runs page 4's.
pub const HANDLER: u32 = 0x0808_4C3A;
const GRID_HANDLER: u32 = 0x0808_4C7C;
fn handler(core: &mut Core) {
    if ready(core) && core.gba().cpu().gpr(0) as u32 == GRID2 {
        core.gba_mut().cpu_mut().set_thumb_pc(GRID_HANDLER);
    }
}

/// R opens the legend on page 4 only (`cmp r0, #4`): on page 5 too.
pub const LEGEND: u32 = 0x0808_4F28;
fn legend(core: &mut Core) {
    if ready(core) && core.gba().cpu().gpr(0) as u32 == GRID2 {
        core.gba_mut().cpu_mut().set_gpr(0, GRID as i32);
    }
}

/// The page arrows (`sub_08085044`, `cmp r0, #4` with r0 the page): page 4
/// has only the up arrow at the grid's corner (0xDC, 0x20); the text pages
/// both, at the text panel's edge (0x64). With the pack page 4 takes the
/// text pages' branch with the grid's corner for the up arrow and the down
/// arrow left of the R legend button; page 5 takes page 4's.
pub const ARROWS: u32 = 0x0808_50EE;
const BOTH_ARROWS: u32 = 0x0808_50F2;
pub const UP_ARROW: u32 = 0x0808_50FA;
pub const DOWN_ARROW: u32 = 0x0808_5126;
const DOWN_ARROW_SPRITE: u32 = 0x44;
const GRID_ARROW_X: i32 = 0xDC;
pub const GRID_DOWN_X: i32 = 0xAC;
const GRID_DOWN_Y: i32 = 0x98;
fn arrows(core: &mut Core) {
    if !ready(core) {
        return;
    }
    match core.gba().cpu().gpr(0) as u32 {
        GRID => core.gba_mut().cpu_mut().set_thumb_pc(BOTH_ARROWS),
        GRID2 => core.gba_mut().cpu_mut().set_gpr(0, GRID as i32),
        _ => {}
    }
}
fn up_arrow(core: &mut Core) {
    if ready(core) && page(core) == GRID {
        core.gba_mut().cpu_mut().set_gpr(1, GRID_ARROW_X);
    }
}
fn down_arrow(core: &mut Core) {
    if ready(core) && page(core) == GRID && core.gba().cpu().gpr(0) as u32 == DOWN_ARROW_SPRITE {
        let cpu = core.gba_mut().cpu_mut();
        cpu.set_gpr(1, GRID_DOWN_X);
        cpu.set_gpr(2, GRID_DOWN_Y);
    }
}

/// `sub_08085708`: the list's address in r1, about to be indexed.
pub const LIST: u32 = 0x0808_5732;
fn list(core: &mut Core) {
    if !ready(core) {
        return;
    }
    let second = (page(core) == GRID2) as u32;
    let hide = (core.gba().cpu().gpr(1) as u32 == AW2_LIST_NO_NEOTANK) as u32;
    core.gba_mut().cpu_mut().set_gpr(1, (LISTS + 40 * (2 * second + hide)) as i32);
}

/// `sub_08085950(pal, army)`: the map sheet's address in r0 (army in r8).
pub const SHEET: u32 = 0x0808_595E;
fn sheet(core: &mut Core) {
    if !ready(core) {
        return;
    }
    let army = core.gba().cpu().gpr(8) as u32;
    let c = country(core, army).clamp(1, COUNTRIES);
    core.gba_mut().cpu_mut().set_gpr(0, (SHEETS + SHEET_SIZE * (c - 1)) as i32);
}

/// `sub_080859A0`: the unit's sheet tile (4 * slot) in r0, the army in r9
/// (`sub_08085708`'s): a new unit's slot becomes its place in [`SHEETS`].
pub const ICON: u32 = 0x0808_59BC;
fn icon(core: &mut Core) {
    if !ready(core) {
        return;
    }
    let slot = core.gba().cpu().gpr(0) as u32 / 4;
    if !(NEW_SLOT..NEW_SLOT + NEW_UNITS.len() as u32).contains(&slot) {
        return;
    }
    let army = core.gba().cpu().gpr(9) as u32;
    let s = free_slot(country(core, army), (slot - NEW_SLOT) as usize);
    core.gba_mut().cpu_mut().set_gpr(0, (4 * s) as i32);
}

/// `sub_08085410`: the firepower bonus + 0x1E in r0, before the bar is
/// picked (`cmp r0, #0x6E`).
pub const BAR: u32 = 0x0808_542A;
/// The bonuses AW2 has a bar for.
pub const BAR_LEVELS: [i32; 13] = [-30, -20, -10, 0, 10, 15, 20, 30, 40, 50, 60, 75, 80];

/// The bar for a bonus: the nearest level, away from 0 between two.
pub fn bar_level(v: i32) -> i32 {
    *BAR_LEVELS
        .iter()
        .min_by_key(|&&l| ((l - v).abs(), -(l.abs())))
        .unwrap()
}

fn bar(core: &mut Core) {
    if !ready(core) {
        return;
    }
    let v = core.gba().cpu().gpr(0) - 0x1E;
    core.gba_mut().cpu_mut().set_gpr(0, bar_level(v) + 0x1E);
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![
        (DOWN, Box::new(down)),
        (HANDLER, Box::new(handler)),
        (LEGEND, Box::new(legend)),
        (ARROWS, Box::new(arrows)),
        (UP_ARROW, Box::new(up_arrow)),
        (DOWN_ARROW, Box::new(down_arrow)),
        (LIST, Box::new(list)),
        (SHEET, Box::new(sheet)),
        (ICON, Box::new(icon)),
        (BAR, Box::new(bar)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_unit_once() {
        let mut all: Vec<u16> = GROUND.iter().chain(AIR_SEA.iter()).copied().filter(|&u| u != 0).collect();
        all.sort();
        let want: Vec<u16> = (1..=24).chain([26, 27]).collect();
        assert_eq!(all, want);
    }

    #[test]
    fn free_slots() {
        for c in 1..=COUNTRIES {
            let own = [COUNTRY_SLOTS + 2 * (c - 1), COUNTRY_SLOTS + 2 * (c - 1) + 1];
            let s: Vec<u32> = (0..NEW_UNITS.len()).map(|k| free_slot(c, k)).collect();
            for (i, x) in s.iter().enumerate() {
                assert!(!own.contains(x) && (17..27).contains(x));
                assert!(!s[..i].contains(x));
            }
        }
    }

    #[test]
    fn bars() {
        assert_eq!(bar_level(0), 0);
        assert_eq!(bar_level(5), 10);
        assert_eq!(bar_level(-5), -10);
        assert_eq!(bar_level(25), 30);
        assert_eq!(bar_level(65), 60);
        assert_eq!(bar_level(70), 75);
        assert_eq!(bar_level(120), 80);
        assert_eq!(bar_level(-45), -30);
        for l in BAR_LEVELS {
            assert_eq!(bar_level(l), l);
        }
        assert!(DATA_END <= 0x0880_0000);
    }
}
