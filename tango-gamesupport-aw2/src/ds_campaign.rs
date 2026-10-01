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
//! - **Map table.** AW2 reads every map header from its map table; with
//!   the pack that is tangoAW2's copy with room for 0x100 ids
//!   ([`crate::survival::TABLE`]). A mission is played on map id
//!   [`crate::ds_campaign_data::MAP_ID`]: its header is written into that
//!   entry when the mission starts.
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
/// 0x08D2FFFF) and Survival (0x08E00000..0x08E4FFFF), past the 8 MB
/// cartridge (mGBA grows the image). A mark first, the blob from +0x100.
pub const DATA: u32 = 0x08F0_0000;
const DATA_END: u32 = crate::ds_worldmap::BASE;
const ENTRY: u32 = 0x5C;
const MAGIC_WORD: u32 = 0x4443_5344; // "DSCD"
const MAGIC_AT: u32 = DATA;
/// AW2's font widths (for re-wrapping Dual Strike's dialogue).
const FONT_WIDTHS: u32 = 0x084C_36E4;

// --- RAM (EWRAM the game never touches; 0x0203FD10..0x0203FD5F) ------------------

/// 1 while a DS Campaign session is on (from its start to the menu).
pub const ACTIVE: u32 = 0x0203_FD10;
/// The Select Mode menu's request: 0 none, 1 New, 2 Continue.
pub const REQUEST: u32 = 0x0203_FD11;
/// The mission being played (index 0..27).
pub const MISSION: u32 = 0x0203_FD12;
/// The Select Mode sub-menu's level and row ([`crate::campaign_menu`]).
pub const MENU_LEVEL: u32 = 0x0203_FD13;
pub const MENU_CHOICE: u32 = 0x0203_FD14;
/// Real-time countdown (frames), Dual Strike's op 0x5A; 0 off.
const COUNTDOWN: u32 = 0x0203_FD18;
/// 1 once [`MISSION`]'s header is in the map table (the world map's sync).
const MISSION_SET: u32 = 0x0203_FD15;
/// The last mission's outcome (1 won, 2 lost), its index and day (u16).
pub const LAST_RESULT: u32 = 0x0203_FD1C;
/// The last Dual Strike condition that held (its address) and the day.
pub const LAST_CONDITION: u32 = 0x0203_FD50;
/// What ended the last mission through its events: the condition that
/// held when an event script declared the winner (AW2's op 0x40), and the
/// day (u16). Zero when the battle ended by AW2's own rules (no units left,
/// an HQ taken). For the tests and logs.
pub const WIN_CAUSE: u32 = 0x0203_FD58;
/// `EventOp_DefeatOtherTeamsAndEndMatch` (AW2's script op 0x40).
const SCRIPT_END_MATCH: u32 = 0x0801_8FB4;
/// The campaign's flags 0x20..0x9F (16 bytes).
const FLAGS: u32 = 0x0203_FD20;
/// The progress record saved to Flash ([`SAVE_SIZE`] bytes).
pub const PROGRESS: u32 = 0x0203_FD30;
const SAVE_SIZE: u32 = 0x20;
#[cfg(test)]
const RAM_END: u32 = 0x0203_FD60;
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
/// The campaign's end-of-battle handler (`EndOfGame_FinishCampaignMap`),
/// trapped past its prologue (its `bl IsPlayer1TeamAlive`), and its
/// `bl ResetRulesAfterCampaignMap` (with r4 = 0 it then starts AW2's
/// after-mission campaign proc and returns).
pub const CAMPAIGN_END: u32 = 0x0803_8488;
const CAMPAIGN_END_RESET: u32 = 0x0803_84F4;
/// AW2's campaign proc that opens the world map and starts the battle the
/// player picks there (`gUnknown_0849EB7C`, AW2's Continue).
const WORLD_MAP_PROC: u32 = 0x0849_EB7C;
/// "Was the battle won" (army 1's team alive).
const BATTLE_WON: u32 = 0x0803_861C;
/// AW2's campaign flags: set (id, value), is set (id).
pub const SET_FLAG: u32 = 0x0803_CBA0;
pub const IS_FLAG: u32 = 0x0803_CBD8;

/// AW2's save slot writer `sub_0801A7D8(slot, buffer, size)`.
const SLOT_WRITER: u32 = 0x0801_A7D8;
/// The CO select screen (War Room's, also the campaign's).
const CO_SELECT_PROC: u32 = 0x0861_65C0;
/// Its lists: CO ids (u8, by group), each group's count and country (u8),
/// the group count (u32), and each group's "may switch" flag (u32 x 5).
const CO_LIST: u32 = 0x0300_58E0;
const CO_GROUP_COUNTS: u32 = 0x0300_5948;
const CO_GROUP_COUNTRY: u32 = 0x0300_5958;
const CO_GROUPS: u32 = 0x0300_5944;
const CO_GROUP_SWITCH: u32 = 0x0300_59C0;

/// Magic flow ids ([`data::Magic::Flow`]).
pub const FLOW_CO_SETUP: u8 = 1;
pub const FLOW_SAVE: u8 = 2;
pub const FLOW_HIDE: u8 = 3;
pub const FLOW_CLEAR: u8 = 4;
pub const FLOW_PROLOGUE: u8 = 5;

/// AW2's world map entered from the menu (`gUnknown_0861485C`: fade,
/// music, `SetupWorldMapForResume`, fade in, the cursor's procs, then the
/// main loop): the DS session's copy calls the prologue's magic stub before
/// the main loop. The words that point at AW2's script point at the copy
/// during a session ([`crate::ds_worldmap::tick`]).
const AW2_MAP_SCRIPT: u32 = 0x0861_485C;
fn map_script(prologue: u32, save: u32) -> Vec<u8> {
    [
        proc_cmd(0x1D, 0x1E, 0),
        proc_cmd(0x1B, 0x1A8, 0),
        proc_cmd(0x02, 0, 0x0807_6ADD),
        proc_cmd(0x1E, 0x1E, 0),
        proc_cmd(0x02, 0, 0x0807_67C1),
        proc_cmd(0x02, 0, prologue),
        // (the record keeps the prologue's flag)
        proc_cmd(0x02, 0, save | 1),
        proc_cmd(0x0D, 0, 0x0861_4614),
        proc_cmd(0x00, 0, 0),
    ]
    .concat()
}
/// `StartBlockingEventScript(script, proc)`.
const START_BLOCKING_SCRIPT: u32 = 0x0807_8540;
/// The prologue has been shown (a flag of the record, past Dual Strike's).
const PROLOGUE_FLAG: u32 = 0x9E;

/// On the world map from the menu, before the player has a turn: a new
/// campaign (nothing won) that has not seen it gets Dual Strike's prologue
/// (tail-calls `StartBlockingEventScript`, which returns to the proc).
fn prologue(core: &mut Core) {
    let Some(c) = campaign(core) else { return return_to(core, 0) };
    let (at, bit) = ((PROLOGUE_FLAG - 0x20) / 8, 1u8 << ((PROLOGUE_FLAG - 0x20) % 8));
    let seen = core.raw_read_8(P_FLAGS + at, -1) & bit != 0;
    if !active(core) || core.raw_read_32(P_WON, -1) != 0 || seen || c.built.story.prologue == 0 {
        return return_to(core, 0);
    }
    for base in [P_FLAGS, FLAGS] {
        let v = core.raw_read_8(base + at, -1);
        core.raw_write_8(base + at, -1, v | bit);
    }
    let proc = core.gba().cpu().gpr(0);
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, c.built.story.prologue as i32);
    cpu.set_gpr(1, proc);
    cpu.set_thumb_pc(START_BLOCKING_SCRIPT);
}

/// The BG0 tilemap buffer's pointer word (`gBG0TilemapBuffer`) and
/// `BG_EnableSyncBG0` (copies it to VRAM at the next VBlank).
const BG0_BUFFER_PTR: u32 = 0x0849_9578;
const BG0_SYNC: u32 = 0x0801_3AEC;

/// The map menu's Save item: its hide test (menu table entry 0x0849AB64,
/// `sub_0802C644`). During a session it points at a magic stub that hides
/// the item: a suspended DS mission would resume as an AW2 one.
const SAVE_ITEM_TEST: u32 = 0x0849_AB64;
const SAVE_ITEM_AW2: u32 = 0x0802_C645;
/// `GetCampaignResultCountPlusOne`: the mission title's number.
pub const MISSION_NUMBER: u32 = 0x0803_840C;

/// AW2's save staging buffer: the slot writer copies its record from here
/// (the pointer word 0x0200CC2C holds it).
const STAGING: u32 = 0x0200_0000;

/// The start proc's save (a proc CALL to a magic stub): the progress record
/// is copied to the staging buffer and the call goes on into AW2's slot
/// writer, `sub_0801A7D8(SAVE_SLOT, buffer, SAVE_SIZE)`, which returns to
/// the proc.
fn save(core: &mut Core) {
    let mut b = vec![0u8; SAVE_SIZE as usize];
    core.raw_read_range(PROGRESS, -1, &mut b);
    core.raw_write_range(STAGING, -1, &b);
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, SAVE_SLOT as i32);
    cpu.set_gpr(1, STAGING as i32);
    cpu.set_gpr(2, SAVE_SIZE as i32);
    cpu.set_thumb_pc(SLOT_WRITER);
}

/// Empties the BG0 tilemap buffer, then tail-calls `BG_EnableSyncBG0`
/// (which returns to the proc).
fn clear_bg0(core: &mut Core) {
    let buffer = core.raw_read_32(BG0_BUFFER_PTR, -1);
    if (0x0200_0000..0x0204_0000).contains(&buffer) {
        core.raw_write_range(buffer, -1, &[0u8; 0x800]);
    }
    core.gba_mut().cpu_mut().set_thumb_pc(BG0_SYNC);
}

fn proc_cmd(op: u16, arg: i16, ptr: u32) -> [u8; 8] {
    let mut c = [0u8; 8];
    c[0..2].copy_from_slice(&op.to_le_bytes());
    c[2..4].copy_from_slice(&arg.to_le_bytes());
    c[4..8].copy_from_slice(&ptr.to_le_bytes());
    c
}

/// The proc that opens the DS Campaign: save the progress, then AW2's own
/// world map ([`crate::ds_worldmap`]: the mission picked there, its CO
/// screen, `ResetRulesAfterCampaignMap`, the mission title, the battle).
fn start_proc_script(save: u32) -> Vec<u8> {
    let _ = (CO_SELECT_PROC, RESET_RULES, MISSION_PROC);
    [proc_cmd(0x02, 0, save | 1), proc_cmd(0x0D, 0, WORLD_MAP_PROC), proc_cmd(0x00, 0, 0)].concat()
}

pub struct Campaign {
    pub built: data::Built,
    pub start_proc: u32,
    /// The DS session's copy of AW2's world map script ([`map_script`]).
    pub map_script: u32,
    pub hide_stub: u32,
    /// The CO screen's setup (a mission's `coSelect` on the world map).
    pub co_setup: u32,
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
            let save = built.add_magic(data::Magic::Flow(FLOW_SAVE)) & !1;
            let co_setup = built.add_magic(data::Magic::Flow(FLOW_CO_SETUP));
            let hide_stub = built.add_magic(data::Magic::Flow(FLOW_HIDE));
            let clear = built.add_magic(data::Magic::Flow(FLOW_CLEAR));
            let _ = clear;
            let prologue = built.add_magic(data::Magic::Flow(FLOW_PROLOGUE));
            let start_proc = built.add(&start_proc_script(save));
            let map_script = built.add(&map_script(prologue, save));
            assert!(built.base + (built.blob.len() as u32) < DATA_END);
            Some(Campaign { built, start_proc, map_script, hide_stub, co_setup })
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
    core.raw_write_32(MAGIC_AT, -1, MAGIC_WORD);
    true
}

/// The world map's data (built on the first DS session: it takes a moment).
fn install_world_map(core: &mut Core) -> bool {
    let Some(c) = campaign(core) else { return false };
    let picks: Vec<bool> =
        c.built.missions.iter().take(data::MISSIONS).map(|m| m.cos.iter().take(m.armies as usize).any(|&(co, _)| co == 0x1C)).collect();
    let texts: Vec<u16> = c.built.missions.iter().take(data::MISSIONS).map(|m| m.info_text).collect();
    crate::ds_worldmap::install(core, &picks, c.co_setup, &texts, &c.built.story.after_win)
}

/// On the world map, the mission under the cursor is the one played: its
/// header is written into [`data::MAP_ID`]'s entry (the map's mission panel
/// reads it, and the battle starts on it).
fn sync_mission(core: &mut Core) {
    let m = core.raw_read_32(crate::ds_worldmap::S_MISSION, -1);
    if m as usize >= data::MISSIONS || core.raw_read_8(MISSION, -1) as u32 == m && core.raw_read_8(MISSION_SET, -1) == 1 {
        return;
    }
    let Some(c) = campaign(core) else { return };
    let (Some(table), Some((_, header))) = (big_table(core), c.built.headers.iter().find(|h| h.0 as u32 == m)) else { return };
    core.raw_write_range(table + ENTRY * data::MAP_ID as u32, -1, header);
    core.raw_write_8(MISSION, -1, m as u8);
    core.raw_write_8(MISSION_SET, -1, 1);
    core.raw_write_32(COUNTDOWN, -1, 0);
}

/// The missions open on the world map: the first story mission not won
/// (in [`ORDER`]), and each lab mission whose flag is set and which is
/// not won.
pub fn available(core: &Core) -> Vec<u8> {
    let mut out = Vec::new();
    if let Some(&m) = ORDER.iter().find(|&&m| !SIDE_MISSIONS.iter().any(|s| s.0 == m) && !won(core, m)) {
        out.push(m);
    }
    for &(m, flag) in &SIDE_MISSIONS {
        if campaign_flag(core, flag) && !won(core, m) {
            out.push(m);
        }
    }
    out
}

/// The missions won so far.
fn won_list(core: &Core) -> Vec<u8> {
    (0..data::MISSIONS as u8).filter(|&m| won(core, m)).collect()
}

/// The map table the game reads, when it has room for [`data::MAP_ID`]
/// (tangoAW2's 0x100-id copy, in use with the pack).
fn big_table(core: &Core) -> Option<u32> {
    let t = crate::five_map::table(core);
    (t == crate::survival::TABLE).then_some(t)
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
    {
        // The map menu's Save item, hidden during a session.
        if let Some(c) = campaign(core) {
            let want = if session { c.hide_stub } else { SAVE_ITEM_AW2 };
            let now = core.raw_read_32(SAVE_ITEM_TEST, -1);
            if (now == c.hide_stub || now == SAVE_ITEM_AW2) && now != want {
                core.raw_write_32(SAVE_ITEM_TEST, -1, want);
            }
        }
    }
    if session {
        // ACTIVE is 1 from the start (the menu is still closing), 2 once the
        // menu has gone; back on the Select Mode menu, the session is over.
        let menu = crate::campaign_menu::on_select_mode(core);
        match core.raw_read_8(ACTIVE, -1) {
            1 if !menu => core.raw_write_8(ACTIVE, -1, 2),
            2 if menu => core.raw_write_8(ACTIVE, -1, 0),
            _ => {}
        }
        let n = core.raw_read_32(COUNTDOWN, -1);
        if n > 1 && in_battle(core) {
            core.raw_write_32(COUNTDOWN, -1, n - 1);
        }
        if !in_battle(core) {
            sync_mission(core);
        }
    }
    let map_script = campaign(core).map_or(AW2_MAP_SCRIPT, |c| c.map_script);
    crate::ds_worldmap::tick(core, on && active(core), AW2_MAP_SCRIPT, map_script);
}

fn in_battle(core: &Core) -> bool {
    core.raw_read_32(0x0300_0004, -1) != 0
}

/// `GetCampaignResultCountPlusOne` (the mission title's "MISSION n"):
/// during a session, the DS missions won so far plus one.
fn mission_number(core: &mut Core) {
    if !active(core) {
        return;
    }
    let mission = core.raw_read_8(MISSION, -1);
    let won = core.raw_read_32(P_WON, -1) & !(1 << mission);
    return_to(core, won.count_ones() + 1);
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

/// The campaign's order: Dual Strike's 25 story missions, with its three
/// research-lab side missions (records 25..27). Dual Strike opens a side
/// mission on its world map when the player captures the city hiding the
/// lab's map in the mission before it (campaign flags 0x60..0x62, set by
/// the mission's own script): The Long March after Black Boats Ahoy! (flag
/// 0x60), Lash's Test after Frozen Fortress (0x61), Spiral Garden after
/// Snow Hunters (0x62). Here a side mission is played next when its flag
/// is set, and skipped otherwise ([`SIDE_MISSIONS`]).
pub const ORDER: [u8; data::MISSIONS] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 25, 10, 11, 26, 12, 13, 27, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
];
/// The side missions and the campaign flag that opens each.
pub const SIDE_MISSIONS: [(u8, u32); 3] = [(25, 0x90), (26, 0x91), (27, 0x92)];

/// A campaign flag (0x20..0x9F) of the session.
pub fn campaign_flag(core: &Core, id: u32) -> bool {
    flag_bit(id).is_some_and(|(at, bit)| core.raw_read_8(at, -1) & bit != 0)
}

pub fn set_campaign_flag(core: &mut Core, id: u32) {
    if let Some((at, bit)) = flag_bit(id) {
        let v = core.raw_read_8(at, -1);
        core.raw_write_8(at, -1, v | bit);
    }
}

/// The next mission to play from the progress record (its place in
/// [`ORDER`]).
pub fn next_step(core: &Core) -> u8 {
    if progress_valid(core) {
        core.raw_read_8(P_NEXT, -1).min(data::MISSIONS as u8 - 1)
    } else {
        0
    }
}

/// Whether there is a DS Campaign to continue (read from Flash once).
pub fn has_save(core: &mut Core) -> bool {
    if !progress_valid(core) {
        load_from_flash(core);
    }
    progress_valid(core)
}

/// AW2's Flash sectors: "2ars", ..., +0x08 generation, +0x0D slot id,
/// +0x0E offset, +0x50 length, +0x52 payload.
const FLASH: u32 = 0x0E00_0000;
const SECTOR: u32 = 0x1000;
const SECTOR_MAGIC: u32 = 0x7372_6132;

/// Reads the progress record from the newest sector of [`SAVE_SLOT`].
fn load_from_flash(core: &mut Core) {
    let mut best: Option<(u32, u32)> = None;
    for s in 0..16 {
        let at = FLASH + SECTOR * s;
        if core.raw_read_32(at, -1) != SECTOR_MAGIC || core.raw_read_8(at + 0x0D, -1) != SAVE_SLOT {
            continue;
        }
        let generation = core.raw_read_32(at + 8, -1);
        if best.is_none_or(|(g, _)| generation >= g) {
            best = Some((generation, at));
        }
    }
    let Some((_, at)) = best else { return };
    let len = (core.raw_read_16(at + 0x50, -1) as u32).min(SAVE_SIZE);
    let mut b = vec![0u8; len as usize];
    for (k, v) in b.iter_mut().enumerate() {
        *v = core.raw_read_8(at + 0x52 + k as u32, -1);
    }
    if b.len() >= 4 && u32::from_le_bytes(b[0..4].try_into().unwrap()) == PROGRESS_MAGIC {
        core.raw_write_range(PROGRESS, -1, &b);
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
        (crate::campaign_menu::SAVE_FLAG, Box::new(crate::campaign_menu::save_flag)),
        (MISSION_NUMBER, Box::new(mission_number)),
        (crate::ds_worldmap::SAVE_PROMPT, Box::new(save_prompt)),
        (BEST_SCORE, Box::new(best_score)),
        (SCRIPT_END_MATCH, Box::new(script_end_match)),
    ]
}

fn script_end_match(core: &mut Core) {
    if active(core) {
        let c = core.raw_read_32(LAST_CONDITION, -1);
        let day = core.raw_read_16(0x0300_4080, -1);
        core.raw_write_32(WIN_CAUSE, -1, c);
        core.raw_write_16(WIN_CAUSE + 4, -1, day);
    }
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

/// Campaign New / Continue: with a DS Campaign request, start the DS
/// session instead of AW2's campaign: its world map, from the progress.
fn start(core: &mut Core, new: bool) {
    let req = core.raw_read_8(REQUEST, -1);
    if req == 0 || !crate::ds_weather::is_on(core) || campaign(core).is_none() || big_table(core).is_none() {
        return;
    }
    let _ = new;
    if !install_world_map(core) {
        return;
    }
    core.raw_write_8(REQUEST, -1, 0);
    if req == 1 || !progress_valid(core) {
        new_progress(core);
    }
    // A record saved by 0.4.0 kept the lab missions' flags at 0x60..0x62
    // (AW2's Hard Campaign flag among them): they move to 0x90..0x92.
    for k in 0..3u32 {
        let (old, new) = (P_FLAGS + (0x60 + k - 0x20) / 8, P_FLAGS + (0x90 + k - 0x20) / 8);
        let (ob, nb) = (1u8 << ((0x60 + k - 0x20) % 8), 1u8 << ((0x90 + k - 0x20) % 8));
        if core.raw_read_8(old, -1) & ob != 0 {
            let v = core.raw_read_8(old, -1);
            core.raw_write_8(old, -1, v & !ob);
            let v = core.raw_read_8(new, -1);
            core.raw_write_8(new, -1, v | nb);
        }
    }
    // Flags of the session come from the progress record.
    for k in 0..16 {
        let v = core.raw_read_8(P_FLAGS + k, -1);
        core.raw_write_8(FLAGS + k, -1, v);
    }
    crate::ds_worldmap::backup_aw2_state(core);
    let open = available(core);
    // The cursor on the mission at the progress's step if it is open (a
    // lab mission), else the first open one.
    let at_step = ORDER[next_step(core) as usize];
    let focus = if open.contains(&at_step) { at_step } else { open.first().copied().unwrap_or(0) };
    crate::ds_worldmap::write_state(core, &open, &won_list(core), focus);
    core.raw_write_8(ACTIVE, -1, 1);
    core.raw_write_8(MISSION_SET, -1, 0);
    core.raw_write_8(GAME_MODE, -1, CAMPAIGN);
    core.raw_write_8(MAP_ID, -1, data::MAP_ID);
    sync_mission(core);
    let Some(c) = campaign(core) else { return };
    proc_start_instead(core, c.start_proc);
}

/// The campaign's end of battle, past `EndOfGame_FinishCampaignMap`'s
/// prologue: record the outcome, write which missions the win opens, and
/// carry on into the function's own tail (`ResetRulesAfterCampaignMap`,
/// then `StartCampaignAfterMap`: AW2's world map after a mission, which
/// marks a won mission cleared and reveals the newly opened ones).
fn end_of_battle(core: &mut Core) {
    if !active(core) {
        return;
    }
    let index = core.raw_read_8(MISSION, -1);
    let won = battle_won(core);
    // The outcome, for the tests and logs: 1 won, 2 lost, and the day.
    core.raw_write_8(LAST_RESULT, -1, if won { 1 } else { 2 });
    core.raw_write_8(LAST_RESULT + 1, -1, index);
    core.raw_write_16(LAST_RESULT + 2, -1, core.raw_read_16(0x0300_4080, -1));
    let before = available(core);
    if won {
        let w = core.raw_read_32(P_WON, -1) | (1 << index);
        core.raw_write_32(P_WON, -1, w);
        for k in 0..16 {
            let v = core.raw_read_8(FLAGS + k, -1);
            core.raw_write_8(P_FLAGS + k, -1, v);
        }
    } else {
        // A lost mission leaves the flags as they were before it.
        for k in 0..16 {
            let v = core.raw_read_8(P_FLAGS + k, -1);
            core.raw_write_8(FLAGS + k, -1, v);
        }
    }
    let after = available(core);
    let newly: Vec<u8> = after.iter().copied().filter(|m| !before.contains(m)).collect();
    // The progress's step: the next story mission (or, when every mission
    // is won, the campaign is over).
    let next = after.iter().filter_map(|m| ORDER.iter().position(|o| o == m)).min();
    match next {
        Some(s) => core.raw_write_8(P_NEXT, -1, s as u8),
        None => core.raw_write_8(P_NEXT + 1, -1, 1),
    }
    crate::ds_worldmap::write_reveal(core, index, &newly);
    // Back on the map, the cursor waits on the mission just opened (else
    // the next one open, else the one just played).
    if let Some(&m) = newly.first().or(after.first()) {
        crate::ds_worldmap::point_at(core, m);
    }
    core.raw_write_32(crate::ds_worldmap::S_MISSION, -1, index as u32);
    core.raw_write_8(crate::ds_worldmap::S_WON, -1, won as u8);
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(4, 0);
    cpu.set_thumb_pc(CAMPAIGN_END_RESET);
}

/// `InsertBestScoreRecord`: in campaign mode it keeps a mission's best
/// score at AW2's results `[mapID - 0x8A]` (`gUnknown_0200C2D0`, 8 bytes
/// each); a DS mission's id (0xF0) would land on the event script slots
/// (0x0200C600: a slot that never ends, which held the map's save
/// prompt). The DS Campaign keeps no scores.
const BEST_SCORE: u32 = 0x0801_7720;
fn best_score(core: &mut Core) {
    if active(core) && core.raw_read_8(MAP_ID, -1) == data::MAP_ID {
        let cpu = core.gba_mut().cpu_mut();
        let lr = cpu.gpr(14) as u32;
        cpu.set_thumb_pc(lr & !1);
    }
}

/// AW2's save prompt after a mission (`SaveScreenCampaign_StartMessage`):
/// in a DS session the DS Campaign saves its own record instead.
fn save_prompt(core: &mut Core) {
    if active(core) {
        save(core);
    }
}

/// In a DS mission, the map's Com Towers are Com Towers ([`crate::com_tower`]:
/// they add firepower, earn nothing, and capturing one does not defeat its
/// owner); Dual Strike's research labs share their tiles and stay Labs
/// ([`is_lab_cell`]).
pub fn towers_active(core: &Core) -> bool {
    active(core) && core.raw_read_8(MAP_ID, -1) == data::MAP_ID && core.raw_read_8(GAME_MODE, -1) == CAMPAIGN
}

/// Whether (x, y) is one of the mission's research labs (capturing one
/// defeats its owner, as AW2's Lab).
pub fn is_lab_cell(core: &Core, x: u32, y: u32) -> bool {
    towers_active(core)
        && campaign(core)
            .and_then(|c| c.built.missions.get(core.raw_read_8(MISSION, -1) as usize))
            .is_some_and(|m| m.labs.contains(&(x as u8, y as u8)))
}

/// gPlaySt's fog and weather (now, mode, default, next).
const FOG: u32 = 0x0300_3FCD;
const WEATHER: u32 = 0x0300_3FEC;
const WEATHER_MODE: u32 = 0x0300_3FED;
const NEXT_WEATHER: u32 = 0x0300_3FEE;
const DEFAULT_WEATHER: u32 = 0x0300_3FEF;

/// At every map start (`crate::sandstorm`'s trap on
/// `CalcRandomWeatherChances`), in a DS mission: its fog, its weather as
/// fixed weather (snow, rain, or tangoAW2's sandstorm: fixed with clear as
/// the default, as Survival's maps), and Dual Strike's look (Normal is AW2's;
/// Snow, Desert and Wasteland are drawn with Dual Strike's own terrain,
/// [`crate::wasteland::set_ds_look`]).
pub fn map_start(core: &mut Core) {
    if !active(core) || core.raw_read_8(MAP_ID, -1) != data::MAP_ID {
        return;
    }
    let Some(m) = campaign(core).and_then(|c| c.built.missions.get(core.raw_read_8(MISSION, -1) as usize)) else { return };
    let (mode, w) = match m.weather {
        1 => (3, 1),
        2 => (3, 2),
        3 => (3, 0),
        _ => (0, 0),
    };
    core.raw_write_32(WIN_CAUSE, -1, 0);
    core.raw_write_16(WIN_CAUSE + 4, -1, 0);
    core.raw_write_8(WEATHER_MODE, -1, mode);
    core.raw_write_8(DEFAULT_WEATHER, -1, w);
    core.raw_write_8(WEATHER, -1, w);
    core.raw_write_8(NEXT_WEATHER, -1, w);
    core.raw_write_8(FOG, -1, m.fog as u8);
    crate::wasteland::set_ds_look(core, m.look);
    set_controllers(core, m);
}

/// Who plays each army (player +0x1B: 1 the player, 2 the computer): in
/// Dual Strike the player has army 1 and every army whose CO the player
/// picks (0x1C); the others (Jake's Trial's Rachel, every Black Hole army)
/// are the computer's. AW2's campaign would give the player every army
/// not in Black Hole's colours.
fn set_controllers(core: &mut Core, m: &data::MissionInfo) {
    let players = core.raw_read_32(0x0849_9598, -1);
    for a in 1..=4u32 {
        let p = players + 0x3C * a + 0x1B;
        if a > m.armies as u32 || core.raw_read_8(p, -1) == 0 {
            continue;
        }
        let human = a == 1 || m.cos[a as usize - 1].0 == 0x1C;
        core.raw_write_8(p, -1, if human { 1 } else { 2 });
    }
}

/// The mission's armies the player picks a CO for (Dual Strike's 0x1C), and
/// the COs to pick from; fills the CO select screen's lists. 1 if there is
/// a pick to make.
fn co_setup(core: &mut Core) -> u32 {
    let Some(c) = campaign(core) else { return 0 };
    let index = core.raw_read_8(MISSION, -1) as usize;
    let Some(m) = c.built.missions.get(index) else { return 0 };
    if !m.cos.iter().take(m.armies as usize).any(|&(co, _)| co == 0x1C) {
        return 0;
    }
    // Dual Strike's pool, by country (AW2's tabs: Orange Star, Blue Moon,
    // Green Earth, Yellow Comet, then Black Hole).
    let mut groups: Vec<(u8, Vec<u8>)> = Vec::new();
    let pool: Vec<u8> = if m.pool.is_empty() { vec![0x14, 0x15, 0x03] } else { m.pool.clone() };
    for &ds in &pool {
        let Some(co) = data::aw2_co(ds) else { continue };
        let country = crate::ds_campaign_rules::country(ds);
        match groups.iter_mut().find(|g| g.0 == country) {
            Some(g) => g.1.push(co),
            None => groups.push((country, vec![co])),
        }
    }
    groups.sort_by_key(|g| g.0);
    let mut k = 0;
    for (g, (country, cos)) in groups.iter().enumerate() {
        core.raw_write_8(CO_GROUP_COUNTRY + g as u32, -1, *country);
        core.raw_write_8(CO_GROUP_COUNTS + g as u32, -1, cos.len() as u8);
        for &co in cos {
            core.raw_write_8(CO_LIST + k, -1, co);
            k += 1;
        }
    }
    core.raw_write_32(CO_GROUPS, -1, groups.len() as u32);
    for g in 0..5 {
        core.raw_write_32(CO_GROUP_SWITCH + 4 * g, -1, 1);
    }
    1
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
        Some(data::Magic::Flow(FLOW_CO_SETUP)) => co_setup(core),
        Some(data::Magic::Flow(FLOW_SAVE)) => return save(core),
        Some(data::Magic::Flow(FLOW_HIDE)) => 1,
        Some(data::Magic::Flow(FLOW_CLEAR)) => return clear_bg0(core),
        Some(data::Magic::Flow(FLOW_PROLOGUE)) => return prologue(core),
        Some(m) => match crate::ds_campaign_rules::run(core, &m) {
            crate::ds_campaign_rules::TAIL_CALLED => return,
            r => r,
        },
        None => 0,
    };
    return_to(core, r);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ram_fits() {
        assert!(PROGRESS + SAVE_SIZE <= RAM_END);
        assert!(FLAGS + 16 <= PROGRESS);
        assert!(P_FLAGS + 16 <= PROGRESS + SAVE_SIZE);
        assert!(COUNTDOWN + 4 <= FLAGS);
        // Past Survival's RAM (0x0203FA00..0x0203FD0F) and before the CPU
        // tactics' (0x0203FD60..).
        assert!(ACTIVE >= 0x0203_FD10 && RAM_END <= 0x0203_FD60);
        // Past Survival's ids and inside its table.
        assert!(data::MAP_ID > 0xEC);
        assert!(DATA >= 0x08E5_0000);
    }
}
