//! The DS Campaign: Dual Strike's story campaign played in AW2's campaign
//! engine, with the Dual Strike pack ([`crate::ds_campaign_data`] converts
//! it from the player's ROM).
//!
//! How it runs:
//!
//! - **Data.** The converted campaign is written once into the ROM image's
//!   free space at [`DATA`] (maps, deployments, AW2 event scripts and
//!   trigger lists, texts, the magic stubs), its texts into the text
//!   table's free tail ([`crate::ds_campaign_data::TEXT_FIRST`]..).
//! - **Map table.** AW2 reads every map header from its map table (tangoAW2
//!   moved it, [`crate::five_map`]). While a DS Campaign session is on
//!   ([`ACTIVE`]), the table's 37 literal-pool words point at a copy with
//!   the campaign's maps at ids [`crate::ds_campaign_data::MAP_ID_BASE`]..;
//!   otherwise at the usual table.
//! - **Start.** The Select Mode menu's DS Campaign entry
//!   ([`crate::campaign_menu`]) sets [`REQUEST`]; the game then calls its
//!   own Campaign New/Continue handler (`sub_0803BA4C` / `sub_0803BA88`),
//!   whose first instruction is trapped: with a request it starts the DS
//!   session instead (campaign mode, the mission's map id) and runs
//!   [`START_PROC`]: AW2's own mission start (`ResetRulesAfterCampaignMap`,
//!   the mission title, the battle).
//! - **Events.** AW2's event engine runs the converted scripts. Dual
//!   Strike's own conditions and actions (code in its overlay) are Rust
//!   here: their pointers in the scripts are small Thumb stubs that load an
//!   id into r3 and jump to [`crate::ds_campaign_data::LANDING`], dead code
//!   that is trapped ([`landing`]).
//! - **Flags.** The campaign's own flags 0x20.. (AW2 keeps its campaign
//!   progress there) are kept in [`FLAGS`] while a session is on (traps on
//!   AW2's flag get/set).
//! - **End.** The campaign's end-of-battle handler (`sub_08038484`) is
//!   trapped: a win records the mission and starts the next one, a loss
//!   restarts it; progress is saved to Flash in AW2's own save slot system
//!   (slot [`SAVE_SLOT`], separate from AW2's profile).

use mgba::core::Core;
use std::sync::OnceLock;

use crate::ds_campaign_data as data;

/// The campaign's ROM blob: past the Dual Strike music (0x08800000..
/// 0x08D2FFFF), past the 8 MB cartridge (mGBA grows the image).
pub const DATA: u32 = 0x08E0_0000;
const DATA_END: u32 = 0x08F0_0000;
/// The map table copy used during a session: 0x100 entries.
const DS_MAP_TABLE: u32 = 0x08DF_0000;
const ENTRY: u32 = 0x5C;
const MAGIC_WORD: u32 = 0x4443_5344; // "DSCD"
const MAGIC_AT: u32 = DS_MAP_TABLE - 4;
/// AW2's font widths (for re-wrapping Dual Strike's dialogue).
const FONT_WIDTHS: u32 = 0x084C_36E4;

// --- RAM (EWRAM the game never touches; 0x0203FA00..0x0203FBFF) ------------------

/// 1 while a DS Campaign session is on (from its start to the menu).
pub const ACTIVE: u32 = 0x0203_FA00;
/// The Select Mode menu's request: 0 none, 1 New, 2 Continue.
pub const REQUEST: u32 = 0x0203_FA01;
/// The mission being played (index 0..27).
pub const MISSION: u32 = 0x0203_FA02;
/// Real-time countdown (frames), Dual Strike's op 0x5A; 0 off.
const COUNTDOWN: u32 = 0x0203_FA04;
/// The campaign's flags 0x20..0x9F (16 bytes).
const FLAGS: u32 = 0x0203_FA10;
/// The progress record saved to Flash ([`SAVE_SIZE`] bytes).
pub const PROGRESS: u32 = 0x0203_FA40;
const SAVE_SIZE: u32 = 0x40;
/// Progress layout: magic, version, next mission, missions won (bits), the
/// flags 0x20..0x9F.
const P_MAGIC: u32 = PROGRESS;
const P_NEXT: u32 = PROGRESS + 4;
const P_WON: u32 = PROGRESS + 8;
const P_FLAGS: u32 = PROGRESS + 0x10;
const PROGRESS_MAGIC: u32 = 0x4344_5741; // "AWDC"
/// AW2's save slot for the progress (AW2 uses 0 profile, 2..4 suspends,
/// 5..7 design maps).
pub const SAVE_SLOT: u8 = 15;

// --- AW2 ------------------------------------------------------------------------

const GAME_MODE: u32 = 0x0300_3FC1;
const MAP_ID: u32 = 0x0300_3FC2;
const CAMPAIGN: u8 = 1;
const PROC_START: u32 = 0x0801_C8F4;
const RESET_RULES: u32 = 0x0803_46FD;
const MISSION_PROC: u32 = 0x0849_EBFC;
/// Campaign New / Continue handlers (Select Mode's leaves 1 and 0).
pub const CAMPAIGN_NEW: u32 = 0x0803_BA4C;
pub const CAMPAIGN_CONTINUE: u32 = 0x0803_BA88;
/// The campaign's end-of-battle handler.
pub const CAMPAIGN_END: u32 = 0x0803_8484;
/// "Was the battle won" (army 1's team alive).
const BATTLE_WON: u32 = 0x0803_861C;
/// AW2's campaign flags: set (id, value), is set (id).
pub const SET_FLAG: u32 = 0x0803_CBA0;
pub const IS_FLAG: u32 = 0x0803_CBD8;
/// Return to the Select Mode menu.
const RETURN_TO_MENU: u32 = 0x0803_B83D;

/// The proc script that starts a mission: [`START_PROC`] in the blob.
fn start_proc_script() -> [u8; 24] {
    let mut s = [0u8; 24];
    // CALL ResetRulesAfterCampaignMap; GOTO_SCR mission title + battle; END
    s[0..2].copy_from_slice(&2u16.to_le_bytes());
    s[4..8].copy_from_slice(&RESET_RULES.to_le_bytes());
    s[8..10].copy_from_slice(&0x0Du16.to_le_bytes());
    s[12..16].copy_from_slice(&MISSION_PROC.to_le_bytes());
    s
}

pub struct Campaign {
    pub built: data::Built,
    pub start_proc: u32,
}

static BUILT: OnceLock<Option<Campaign>> = OnceLock::new();

/// The converted campaign (built once from the pack and the AW2 ROM).
pub fn campaign(core: &Core) -> Option<&'static Campaign> {
    BUILT
        .get_or_init(|| {
            let pack = crate::ds_pack::pack()?;
            let ds = data::Ds::from_pack(pack)?;
            let mut widths = vec![0u8; 256];
            core.raw_read_range(FONT_WIDTHS, -1, &mut widths);
            let mut built = data::build(&ds, DATA + 0x100, &widths)?;
            let start_proc = built.base + built.blob.len() as u32;
            let start_proc = (start_proc + 3) & !3;
            while (built.base + built.blob.len() as u32) < start_proc {
                built.blob.push(0);
            }
            built.blob.extend_from_slice(&start_proc_script());
            assert!(built.base + (built.blob.len() as u32) < DATA_END);
            Some(Campaign { built, start_proc })
        })
        .as_ref()
}

fn installed(core: &Core) -> bool {
    core.raw_read_32(MAGIC_AT, -1) == MAGIC_WORD
}

/// Writes the campaign into the ROM image (once).
fn install(core: &mut Core) -> bool {
    if installed(core) {
        return true;
    }
    let Some(c) = campaign(core) else { return false };
    let b = &c.built;
    core.raw_write_range(b.base, -1, &b.blob);
    for &(id, at) in &b.texts {
        core.raw_write_32(data::TEXT_TABLE + 4 * id as u32, -1, at);
    }
    // The session's map table: the usual one, then the campaign's maps.
    let mut table = vec![0u8; (0x100 * ENTRY) as usize];
    let n = (crate::five_map::MAP_IDS * ENTRY) as usize;
    core.raw_read_range(crate::five_map::MAP_TABLE, -1, &mut table[..n]);
    for (id, h) in &b.headers {
        let o = (*id as u32 * ENTRY) as usize;
        table[o..o + ENTRY as usize].copy_from_slice(h);
    }
    core.raw_write_range(DS_MAP_TABLE, -1, &table);
    core.raw_write_32(MAGIC_AT, -1, MAGIC_WORD);
    true
}

pub fn active(core: &Core) -> bool {
    core.raw_read_8(ACTIVE, -1) != 0
}

/// Every frame, before the game runs.
pub fn tick(core: &mut Core, ds: bool) {
    // Without the pack (and nothing installed) nothing here runs.
    if !ds && !installed(core) {
        return;
    }
    let on = ds && install(core);
    let session = on && active(core);
    if !on && core.raw_read_8(ACTIVE, -1) != 0 {
        core.raw_write_8(ACTIVE, -1, 0);
    }
    // The map table's pool words: the session's copy or the usual one.
    {
        let want = if session { DS_MAP_TABLE } else { crate::five_map::MAP_TABLE };
        for (at, field) in crate::five_map::TABLE_POINTERS {
            let now = core.raw_read_32(at, -1);
            if (now == crate::five_map::MAP_TABLE + field || now == DS_MAP_TABLE + field) && now != want + field {
                core.raw_write_32(at, -1, want + field);
            }
        }
    }
    if session {
        // Back on the Select Mode menu: the session is over.
        if crate::campaign_menu::on_select_mode(core) {
            core.raw_write_8(ACTIVE, -1, 0);
        }
        let n = core.raw_read_32(COUNTDOWN, -1);
        if n > 1 && in_battle(core) {
            core.raw_write_32(COUNTDOWN, -1, n - 1);
        }
    }
}

fn in_battle(core: &Core) -> bool {
    core.raw_read_32(0x0300_0004, -1) != 0
}

/// The mission's AW2 map id.
pub fn map_id(index: u8) -> u8 {
    data::MAP_ID_BASE + index
}

// --- Progress --------------------------------------------------------------------

fn progress_valid(core: &Core) -> bool {
    core.raw_read_32(P_MAGIC, -1) == PROGRESS_MAGIC
}

fn new_progress(core: &mut Core) {
    for a in (PROGRESS..PROGRESS + SAVE_SIZE).step_by(4) {
        core.raw_write_32(a, -1, 0);
    }
    core.raw_write_32(P_MAGIC, -1, PROGRESS_MAGIC);
    core.raw_write_8(P_NEXT, -1, 0);
}

/// The next mission to play from the progress record.
pub fn next_mission(core: &Core) -> u8 {
    if progress_valid(core) {
        core.raw_read_8(P_NEXT, -1).min(data::MISSIONS as u8 - 1)
    } else {
        0
    }
}

pub fn won(core: &Core, index: u8) -> bool {
    core.raw_read_32(P_WON, -1) & (1 << index) != 0
}

// --- Traps -----------------------------------------------------------------------

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![
        (data::LANDING, Box::new(landing)),
        (CAMPAIGN_NEW, Box::new(|core: &mut Core| start(core, true))),
        (CAMPAIGN_CONTINUE, Box::new(|core: &mut Core| start(core, false))),
        (CAMPAIGN_END, Box::new(end_of_battle)),
        (SET_FLAG, Box::new(set_flag)),
        (IS_FLAG, Box::new(is_flag)),
    ]
}

fn return_to(core: &mut Core, r0: u32) {
    let cpu = core.gba_mut().cpu_mut();
    let lr = cpu.gpr(14) as u32;
    cpu.set_gpr(0, r0 as i32);
    cpu.set_thumb_pc(lr & !1);
}

/// Tail-call `Proc_Start(script, 3)` from a trapped function's first
/// instruction (returns to its caller).
fn proc_start_instead(core: &mut Core, script: u32) {
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, script as i32);
    cpu.set_gpr(1, 3);
    cpu.set_thumb_pc(PROC_START);
}

/// Starts mission `index` in a DS session: campaign mode, its map, and
/// AW2's mission start.
fn begin_mission(core: &mut Core, index: u8) {
    let Some(c) = campaign(core) else { return };
    core.raw_write_8(ACTIVE, -1, 1);
    core.raw_write_8(MISSION, -1, index);
    core.raw_write_32(COUNTDOWN, -1, 0);
    core.raw_write_8(GAME_MODE, -1, CAMPAIGN);
    core.raw_write_8(MAP_ID, -1, map_id(index));
    // Point the map table at the session's copy right away (the battle
    // loads in this frame's procs).
    for (at, field) in crate::five_map::TABLE_POINTERS {
        let now = core.raw_read_32(at, -1);
        if now == crate::five_map::MAP_TABLE + field {
            core.raw_write_32(at, -1, DS_MAP_TABLE + field);
        }
    }
    proc_start_instead(core, c.start_proc);
}

/// Campaign New / Continue: with a DS Campaign request, start the DS
/// session instead of AW2's campaign.
fn start(core: &mut Core, new: bool) {
    let req = core.raw_read_8(REQUEST, -1);
    if req == 0 || !crate::ds_weather::is_on(core) || campaign(core).is_none() {
        return;
    }
    let _ = new;
    core.raw_write_8(REQUEST, -1, 0);
    if req == 1 || !progress_valid(core) {
        new_progress(core);
    }
    // Flags of the session come from the progress record.
    for k in 0..16 {
        let v = core.raw_read_8(P_FLAGS + k, -1);
        core.raw_write_8(FLAGS + k, -1, v);
    }
    let index = next_mission(core);
    begin_mission(core, index);
}

/// The campaign's end of battle: record a win and go on to the next
/// mission; after a loss, play the mission again.
fn end_of_battle(core: &mut Core) {
    if !active(core) {
        return;
    }
    let index = core.raw_read_8(MISSION, -1);
    let won = battle_won(core);
    if won {
        let w = core.raw_read_32(P_WON, -1) | (1 << index);
        core.raw_write_32(P_WON, -1, w);
        let next = (index + 1).min(data::MISSIONS as u8);
        core.raw_write_8(P_NEXT, -1, next);
        for k in 0..16 {
            let v = core.raw_read_8(FLAGS + k, -1);
            core.raw_write_8(P_FLAGS + k, -1, v);
        }
        if next as usize >= data::MISSIONS {
            core.raw_write_8(ACTIVE, -1, 0);
            let cpu = core.gba_mut().cpu_mut();
            cpu.set_thumb_pc(RETURN_TO_MENU & !1);
            return;
        }
        begin_mission(core, next);
    } else {
        begin_mission(core, index);
    }
}

/// The game's own test (`sub_0803861C`): an army of army 1's team is still
/// in the battle (player +0x1B set, +0x14 clear).
fn battle_won(core: &Core) -> bool {
    let _ = BATTLE_WON;
    let players = core.raw_read_32(0x0849_9598, -1);
    let team = core.raw_read_8(players + 0x3C + 0x2A, -1);
    (1..=4u32).any(|a| {
        let p = players + 0x3C * a;
        core.raw_read_8(p + 0x1B, -1) != 0 && core.raw_read_16(p + 0x14, -1) == 0 && core.raw_read_8(p + 0x2A, -1) == team
    })
}

fn flag_bit(id: u32) -> Option<(u32, u8)> {
    (0x20..0xA0).contains(&id).then(|| (FLAGS + (id - 0x20) / 8, 1u8 << ((id - 0x20) % 8)))
}

fn set_flag(core: &mut Core) {
    if !active(core) {
        return;
    }
    let cpu = core.gba().cpu();
    let (id, v) = (cpu.gpr(0) as u32, cpu.gpr(1));
    let Some((at, bit)) = flag_bit(id) else { return };
    let old = core.raw_read_8(at, -1);
    core.raw_write_8(at, -1, if v != 0 { old | bit } else { old & !bit });
    return_to(core, 0);
}

fn is_flag(core: &mut Core) {
    if !active(core) {
        return;
    }
    let id = core.gba().cpu().gpr(0) as u32;
    let Some((at, bit)) = flag_bit(id) else { return };
    let v = core.raw_read_8(at, -1) & bit != 0;
    return_to(core, v as u32);
}

// --- Magic functions ---------------------------------------------------------------

/// The landing of every magic stub: r3 = the magic id.
fn landing(core: &mut Core) {
    let id = core.gba().cpu().gpr(3) as u32;
    let r = match campaign(core).and_then(|c| c.built.magic.get(id as usize)).cloned() {
        Some(m) => crate::ds_campaign_rules::run(core, &m),
        None => 0,
    };
    return_to(core, r);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ram_fits() {
        assert!(PROGRESS + SAVE_SIZE <= 0x0203_FC00);
        assert!(FLAGS + 16 <= PROGRESS);
        assert!(DS_MAP_TABLE + 0x100 * ENTRY <= DATA);
        assert!(map_id(data::MISSIONS as u8 + data::SECOND_FRONTS as u8 - 1) as u32 <= 0xFF);
    }
}
