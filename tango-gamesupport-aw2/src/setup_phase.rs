//! Dual Strike's Setup phase in the DS Campaign: before day 1 of a mission
//! whose COs the player picks, the map is open for scouting and the battle
//! starts when the player chooses Deploy.
//!
//! Dual Strike (melonDS): a mission with a CO for the player to pick (its
//! record's 0x1C; Jake's Trial, whose COs are set, has none) opens on its
//! map with a "Setup" title and no day yet (funds 0). The cursor moves
//! freely; the menu (its Y button) is Setup ("Select a CO.": the Select CO
//! screen), CO, Intel, Options, Save and Deploy ("Begin battle with the
//! current settings."): Deploy brings day 1 (bank 0xC0, texts 86, 87, 753,
//! 758; "You know you can scout the map before taking the field ... When
//! you're ready to start fighting, tap the menu button and choose Deploy",
//! bank 0x3A text 19).
//!
//! Here, AW2's way: the COs are picked on AW2's CO screen before the map
//! (as before), so the menu has no Setup item. At the battle's first turn
//! start (`MapState_TurnStart` `0x08034DCC`, state 5, day 1, army 1: before
//! day 1's title, its funds and its opening events) the map goes to its
//! cursor state instead, with "Setup" at the top (AW2's font,
//! crate::two_front's sprites). A, wherever the cursor is (`HandleMapCursorA` `0x0802E4B4`,
//! trapped), opens the map menu (`0x0802D458`, as an empty cell does): our
//! copy with CO, Intel, Options, Front (a two-front mission's, to look at
//! the second front) and Deploy, its help line Dual Strike's. Deploy closes
//! the menu and the turn starts (day 1's title, funds, opening events). B, L,
//! R and SELECT keep AW2's (ranges, unit info, the next unit). No Save.

use mgba::core::Core;

/// The phase (0 none, [`PENDING`] from the mission's start to its first
/// handover, [`ACTIVE`] until Deploy): a byte of the DS Campaign's mission
/// state (crate::ds_campaign_rules, cleared when a mission starts).
pub const SETUP: u32 = 0x0203_F706;
const PENDING: u8 = 1;
const ACTIVE: u8 = 2;

const MAP_STATE: u32 = 0x0300_32D8;
const STATE_TURN_START: u16 = 5;
/// `MapState_TurnStart` (state 5).
const TURN_START: u32 = 0x0803_4DCC;
const STATE_CURSOR: u16 = 0xD;
/// The cursor state's own step (0: waiting for the pad).
const CURSOR_STEP: u32 = 0x0300_3334;
const DAY: u32 = 0x0300_4080;
const CURRENT_ARMY: u32 = 0x0300_33EC;

/// `HandleMapCursorA(x, y)`, and the map menu's opening (no arguments).
const CURSOR_A: u32 = 0x0802_E4B4;
const OPEN_MAP_MENU: u32 = 0x0802_D458;
const MAP_MENU_POOL: u32 = 0x0802_D49C;
const MAP_MENU: u32 = 0x0849_AAC0;
const MENU_ENTRY: u32 = 0x20;
const CLOSE_MENU: u32 = 0x0801_A168;
const START_SLOT_SCRIPT: u32 = 0x0801_52EC;

/// The game's map menu items the Setup menu keeps (CO, Intel, Options), the
/// one whose looks Deploy takes (End), and the list's end.
const KEEP: [u32; 3] = [0, 1, 4];
const END_AT: u32 = 6;
const LIST_END: u32 = 7;

/// ROM (free space after crate::two_front's): the mark, the stubs, the
/// script, the menu, the label.
pub const ROM: u32 = 0x08E7_4000;
const ROM_END: u32 = 0x08E7_4400;
const ROM_MAGIC: u32 = 0x5055_5453; // "STUP"
/// Magic ids of this module's stubs (r3 at crate::campaign_model::LANDING).
pub const MAGIC: u32 = 0x2E00_0000;
const STUB_CHOSEN: u32 = 1;
const STUB_GO: u32 = 2;
const STUBS_AT: u32 = ROM + 0x10;
const SCRIPT: u32 = ROM + 0x40;
pub const MENU: u32 = ROM + 0x80;
const TEXTS: u32 = ROM + 0x200;
/// Deploy's label (a text id from the text table's free tail; the DS
/// Campaign's run from 0x7400, crate::two_front's are 0x7FFD and 0x7FFE).
const TEXT_DEPLOY: u16 = 0x7FFC;
const LABEL_DEPLOY: &[u8] = b"\x09\x8bDeploy\0";
const TEXT_TABLE: u32 = crate::campaign_model::TEXT_TABLE;

/// The help line under the menu while Deploy is highlighted (Dual Strike's).
pub const DEPLOY_HELP: &str = "Begin battle with the current settings.";
/// The title while the phase lasts.
pub const TITLE: &str = "Setup";

fn stub_at(n: u32) -> u32 {
    STUBS_AT + 16 * (n - 1)
}

fn op(ptr: u32, imm: u16, op: u16) -> [u8; 8] {
    let mut r = [0u8; 8];
    r[0..4].copy_from_slice(&ptr.to_le_bytes());
    r[4..6].copy_from_slice(&imm.to_le_bytes());
    r[6..8].copy_from_slice(&op.to_le_bytes());
    r
}

/// Writes the ROM area (once; the image keeps it).
fn install(core: &mut Core) {
    if core.raw_read_32(ROM, -1) == ROM_MAGIC {
        return;
    }
    for n in [STUB_CHOSEN, STUB_GO] {
        core.raw_write_range(stub_at(n), -1, &crate::campaign_model::stub(MAGIC | n));
    }
    let script = [op(CLOSE_MENU | 1, 0, 2), op(stub_at(STUB_GO) | 1, 0, 2), op(0, 0, 7)].concat();
    core.raw_write_range(SCRIPT, -1, &script);
    let entry = |core: &Core, k: u32| {
        let mut e = vec![0u8; MENU_ENTRY as usize];
        core.raw_read_range(MAP_MENU + MENU_ENTRY * k, -1, &mut e);
        e
    };
    let mut menu = Vec::new();
    for k in KEEP {
        menu.extend(entry(core, k));
    }
    // Front (crate::two_front's, shown only in a two-front battle).
    menu.extend(crate::two_front::front_entry(core));
    let mut deploy = entry(core, END_AT);
    deploy[0x14..0x18].copy_from_slice(&(stub_at(STUB_CHOSEN) | 1).to_le_bytes());
    deploy[0x1C..0x20].copy_from_slice(&(TEXT_DEPLOY as u32).to_le_bytes());
    menu.extend(deploy);
    menu.extend(entry(core, LIST_END));
    core.raw_write_range(MENU, -1, &menu);
    core.raw_write_range(TEXTS, -1, LABEL_DEPLOY);
    core.raw_write_32(ROM, -1, ROM_MAGIC);
    assert!(MENU + menu.len() as u32 <= TEXTS && TEXTS + 0x10 <= ROM_END);
}

/// The menu's items, in order (for the help lines and the tests).
pub const FRONT_AT: u32 = 3;
pub const DEPLOY_AT: u32 = 4;

/// The DS Campaign mission being started has a Setup phase: the player
/// picks a CO for it (crate::ds_campaign::player_picks). From
/// crate::ds_campaign::map_start at a battle's start (not a second front's
/// set-up, crate::two_front).
pub fn map_start(core: &mut Core, picks: bool) {
    if crate::two_front::rebuilding(core) {
        return;
    }
    install(core);
    core.raw_write_8(SETUP, -1, if picks { PENDING } else { 0 });
}

/// The phase is on (the map open for scouting).
pub fn active(core: &Core) -> bool {
    core.raw_read_8(SETUP, -1) == ACTIVE && crate::ds_campaign::active(core) && crate::ds_campaign::in_battle(core)
}

/// `MapState_TurnStart`: day 1's first turn start of a mission with a
/// Setup phase goes to the map's cursor instead.
fn turn_start(core: &mut Core) {
    if core.raw_read_8(SETUP, -1) != PENDING || !crate::ds_campaign::active(core) || crate::two_front::rebuilding(core) {
        return;
    }
    if core.raw_read_16(DAY, -1) != 1 || core.raw_read_16(CURRENT_ARMY, -1) != 1 {
        core.raw_write_8(SETUP, -1, 0);
        return;
    }
    core.raw_write_8(SETUP, -1, ACTIVE);
    core.raw_write_16(MAP_STATE, -1, STATE_CURSOR);
    core.raw_write_16(CURSOR_STEP, -1, 0);
    let cpu = core.gba_mut().cpu_mut();
    let lr = cpu.gpr(14) as u32;
    cpu.set_thumb_pc(lr & !1);
}

/// `HandleMapCursorA`: during the phase, A opens the menu wherever the
/// cursor is (no unit is moved, nothing built).
fn cursor_a(core: &mut Core) {
    if active(core) {
        core.gba_mut().cpu_mut().set_thumb_pc(OPEN_MAP_MENU);
    }
}

/// A stub of this module (from crate::ds_campaign's landing).
pub fn magic(core: &mut Core, id: u32) {
    if id & 0xFF == STUB_CHOSEN {
        // Deploy chosen: the menu closes, then [`STUB_GO`].
        let cpu = core.gba_mut().cpu_mut();
        cpu.set_gpr(0, SCRIPT as i32);
        cpu.set_gpr(1, 0);
        cpu.set_thumb_pc(START_SLOT_SCRIPT);
        return;
    }
    // The phase ends: day 1's turn starts.
    if core.raw_read_8(SETUP, -1) == ACTIVE {
        core.raw_write_8(SETUP, -1, 0);
        core.raw_write_16(CURSOR_STEP, -1, 0);
        core.raw_write_16(MAP_STATE, -1, STATE_TURN_START);
    }
    let cpu = core.gba_mut().cpu_mut();
    let lr = cpu.gpr(14) as u32;
    cpu.set_gpr(0, 0);
    cpu.set_thumb_pc(lr & !1);
}

/// Every frame, after crate::two_front's and crate::tag's menus: the Setup
/// menu in the map menu's place while the phase lasts; Deploy's label.
pub fn menus(core: &mut Core, on: bool) {
    if !on {
        return;
    }
    // Deploy's help line (crate::two_front's panels, outside its battles).
    crate::two_front::panels_outside(core);
    let now = core.raw_read_32(MAP_MENU_POOL, -1);
    if on && active(core) {
        install(core);
        if now != MENU {
            core.raw_write_32(MAP_MENU_POOL, -1, MENU);
        }
        let entry = TEXT_TABLE + 4 * TEXT_DEPLOY as u32;
        if core.raw_read_32(entry, -1) != TEXTS {
            core.raw_write_32(entry, -1, TEXTS);
        }
    } else if now == MENU {
        core.raw_write_32(MAP_MENU_POOL, -1, MAP_MENU);
    }
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![(CURSOR_A, Box::new(cursor_a)), (TURN_START, Box::new(turn_start))]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_and_rom_fit() {
        // In the DS Campaign's mission state, after Means to an End's.
        assert!(SETUP >= 0x0203_F700 + crate::ds_campaign_rules::MTE_LEN && SETUP < 0x0203_F708);
        assert!(ROM >= crate::two_front::ROM + 0x4000);
        assert!(TEXT_DEPLOY > crate::campaign_model::TEXT_FIRST);
    }
}
