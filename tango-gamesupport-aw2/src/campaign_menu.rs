//! The Select Mode menu's Campaign entry with the Dual Strike pack: a small
//! sub-menu, "AW2 Campaign" (AW2's own, then its Continue / New) or "DS
//! Campaign" ([`crate::ds_campaign`]).

use mgba::core::Core;

/// The game's process pool (`sProcArray`): 0x6C bytes each, script first.
const PROCS: u32 = 0x0200_D610;
const PROCS_END: u32 = 0x0200_E418;
const PROC_SIZE: u32 = 0x6C;
/// `ProcScr_MainMenu`: the SELECT MODE menu.
const MAIN_MENU: u32 = 0x0849_E818;

fn running(core: &Core, script: u32) -> bool {
    (PROCS..PROCS_END).step_by(PROC_SIZE as usize).any(|p| core.raw_read_32(p, -1) == script)
}

/// The Select Mode menu is up.
pub fn on_select_mode(core: &Core) -> bool {
    running(core, MAIN_MENU)
}

/// Every frame, before the game runs.
pub fn tick(core: &mut Core, ds: bool, keys: u32, prev: u32) -> u32 {
    let _ = (core, ds, prev);
    keys
}
