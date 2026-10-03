//! Battles on two fronts (Dual Strike's dual-screen battles), with the
//! Dual Strike pack: docs/AW2.md "Two fronts".
//!
//! A two-front battle is described by a [`TwoFront`]
//! (crate::campaign_model): the battle's own map header is its main front,
//! the description names the second front's header and its rules. AW2 plays
//! one battle at a time, so both fronts' full state is kept and one of them
//! is on the console:
//!
//! - **The store.** A front not on the screen is kept as AW2's own suspend
//!   block (`CaptureBattleSaveState`, `sub_08016F38`: day, army, gPlaySt,
//!   weather, players, units, the tiles changed from its map, inventions),
//!   [`BLOCK_LEN`] bytes, plus tangoAW2's own battle state that lasts past a
//!   turn ([`EXTRA_LEN`]: the rain's fog rule, CO skills, Ex Machina's stun)
//!   at [`STORE`], in EWRAM the game never touches. Swapping fronts captures
//!   the live front into AW2's staging buffer (`0x02000000`), exchanges it
//!   with the store, writes the other front's header into the battle's map
//!   table entry (its map, deployment and events) and rebuilds the scene as
//!   AW2's Continue does (`ResumeScript_LoadSuspendSave`, `sub_08017658`:
//!   `RestoreBattleSaveState`, the terrain plane, the units, the map's
//!   graphics and frame callbacks). A front never played yet starts as a
//!   battle does (`InitMapGameState`, `sub_08034890`, on its header).
//! - **Rounds.** When every army on the live front has had its turn (the
//!   handover state, `MapState_TurnHandoverPrompt`, with no army after the
//!   current one), the screen fades to black, the fronts swap and the other
//!   front's armies play their round; then back, and the day goes on. Dual
//!   Strike's order (checked in melonDS): day 1 main front (the player, then
//!   Black Hole), day 1 second front (its armies in order), day 2 main...
//! - **The swap** is a script of AW2's script slots (`gUnknown_03001470`, the
//!   kind its Continue runs, [`swap_script`]): fade to black, capture,
//!   exchange, rebuild, fade in. Its steps are magic stubs (Rust,
//!   [`magic`]) that tail-call the game's own functions.
//! - **The Front item** (the map menu: CO / Intel / Options / Front / Save /
//!   End, with a help line) shows the other front, display only: the same
//!   swap, the map cursor free to look around and every other button held
//!   back from the game, then B swaps back; the front left was captured
//!   mid-turn and comes back as AW2's Continue brings a saved turn back.
//! - **Send** (a unit's command, in the slot of the "Capt" command no
//!   sendable unit ever has): the unit leaves the main front once its move
//!   ends and is written into the second front's state, by its army's HQ
//!   there (or where the army stands), on a cell it can enter.
//! - **Who plays a second-front turn**: per army, by the description's
//!   [`FrontControl`] and the army's owner (its controller on the main
//!   front: a human, local or a netplay peer, or the computer), never by
//!   "the player". Intel > Auto CO (Dual Strike's, where the description is
//!   `AutoCo`) lets an army's owner give its second-front turns to the
//!   computer or take them; off, the owner plays them with that front's
//!   CO, funds and units, and may save there.
//! - **The second front's end.** The second front's own events (Dual
//!   Strike's records for it) end its battle as AW2's events end a battle;
//!   then, instead of AW2's results, the front's result shows and the main
//!   front comes back for good. Won: the surviving units' value charges the
//!   army's power meter (Dual Strike: "the surviving units will be added to
//!   the power meter") and the second front's CO joins the main front's
//!   army ([`second_front_over`], the hook for tag pairs). Lost: the
//!   battle goes on.
//!
//! The one gate ([`battle`]) decides where any of this is on: today the DS
//! Campaign's two-front missions. Everywhere else nothing here runs or
//! writes, and every menu is the game's own.

use mgba::core::Core;

use crate::campaign_model::{FrontControl, MissionInfo, SendRule, TwoFront, PICK};

// --- The game ------------------------------------------------------------------------

/// gPlaySt (0x48 bytes): +0x01 game mode, +0x02 map id, +0x07 powers on,
/// +0x30 turn limit, +0x33 colours, +0x38 controllers, +0x3D COs, +0x42 teams.
const PLAY_ST: u32 = 0x0300_3FC0;
const GAME_MODE: u32 = PLAY_ST + 0x01;
const MAP_ID: u32 = PLAY_ST + 0x02;
const POWERS_ON: u32 = PLAY_ST + 0x07;
const TURN_LIMIT: u32 = PLAY_ST + 0x30;
const COLOURS: u32 = PLAY_ST + 0x33;
const CONTROLLERS: u32 = PLAY_ST + 0x38;
const COS: u32 = PLAY_ST + 0x3D;
const TEAMS: u32 = PLAY_ST + 0x42;
const CAMPAIGN: u8 = 1;
/// The map state machine's state (`gUnknown_030032D8`, `RunMapStateMachine`):
/// 0 idle, 3 the handover, 5 a turn's start, 0xC a turn by its controller,
/// 0xD the player's cursor, 0x12 the match's end.
const MAP_STATE: u32 = 0x0300_32D8;
const STATE_IDLE: u16 = 0;
const STATE_HANDOVER: u16 = 3;
const STATE_DISPATCH: u16 = 0xC;
const STATE_CURSOR: u16 = 0xD;
const CURRENT_ARMY: u32 = 0x0300_33EC;
const PLAYERS_PTR: u32 = 0x0849_9598;
const UNITS_PTR: u32 = 0x0849_9594;
const PLAYER: u32 = 0x3C;
/// The battle scene's function table: nonzero while a battle is loaded.
const BATTLE: u32 = 0x0300_0004;
/// gMap: size, unit plane +0x12, tiles +0xA22, terrain +0x1432, rows +0x417A.
const MAP: u32 = 0x0201_E450;
/// The tile -> terrain class table the game reads (its RAM copy).
const CLASS_TABLE_PTR: u32 = 0x0849_959C;
/// The inventions list (16 entries of 8 bytes).
const INVENTIONS: u32 = 0x0202_8360;

/// `MapState_TurnHandoverPrompt` (state 3): every army on the front has had
/// its turn when no army after the current one is in the battle.
pub const HANDOVER: u32 = 0x0803_4AF8;
/// `MapState_EndOfGame` (state 0x12): the match is over once its scripts
/// have ended (`sub_08019260`).
pub const END_OF_GAME: u32 = 0x0803_4EF0;
const SCRIPTS_RUNNING: u32 = 0x0801_9260;
/// `sub_0803BD14` (the CO screen's number of picks: the map header's
/// leading 0xFF COs) and `SetArmyCoIdsFromList` (the picks into gPlaySt).
pub const PICK_COUNT: u32 = 0x0803_BD14;
pub const SET_PICKS: u32 = 0x0803_BCDC;
/// `RunMapEventsAfterUnitAction`: a sent unit leaves here (its move done).
pub const AFTER_ACTION: u32 = 0x0807_43E8;

const CAPTURE: u32 = 0x0801_6F38;
const RESTORE: u32 = 0x0801_7208;
const INIT_SETTINGS: u32 = 0x0802_150C;
const INIT_MAP_STATE: u32 = 0x0803_4890;
/// The map header's deployment (`+0x34`, Hard `+0x38`) placed
/// (`ApplyUnitSpawnTable` on it).
const SPAWN_UNITS: u32 = 0x0801_96C0;
const TERRAIN_PLANE: u32 = 0x0801_759C;
const RESET_UNITS: u32 = 0x0802_6798;
const MAP_GRAPHICS: u32 = 0x0802_3348;
const FRAME_CALLBACKS: u32 = 0x0803_662C;
const CO_MUSIC: u32 = 0x0804_3DAC;
const LOCK_MAP: u32 = 0x0803_4F7C;
const UNLOCK_MAP: u32 = 0x0803_4F8C;
const FADES_RUNNING: u32 = 0x0801_16A0;
const START_SLOT_SCRIPT: u32 = 0x0801_52EC;
const CLOSE_MENU: u32 = 0x0801_A168;
const REBUILD_UNITS: u32 = 0x0802_4268;
const ADD_CHARGE: u32 = 0x0804_40E0;
/// The staging buffer the suspend block is captured into and restored from.
pub const STAGING: u32 = 0x0200_0000;
pub const BLOCK_LEN: u32 = 0xE28;
/// The block: day, army, gPlaySt (+0x140), players (+0x14, five of 0x3C),
/// units (+0x188, 4 armies x 51 of 12 bytes), changed tiles (+0xBB8: x, y,
/// tile; tile 0xFFFF ends), inventions (+0xD28).
const B_DAY: u32 = 0x00;
const B_ARMY: u32 = 0x02;
const B_PLAY_ST: u32 = 0x140;
const B_PLAYERS: u32 = 0x14;
const B_UNITS: u32 = 0x188;
const B_TILES: u32 = 0xBB8;
const B_TILES_END: u32 = 0xD28;
const B_INVENTIONS: u32 = 0xD28;
const SLOTS: u32 = 51;

/// The map menu and the unit command menu (their tables and the
/// literal-pool words `CreateMenu`'s callers read them from).
const MAP_MENU: u32 = 0x0849_AAC0;
const MAP_MENU_POOL: u32 = 0x0802_D49C;
const UNIT_MENU: u32 = 0x0849_AE28;
const UNIT_MENU_POOL: u32 = 0x0802_D59C;
const MENU_ENTRY: u32 = 0x20;
/// The map menu's Options (index 4: Front comes after it) and Save.
const OPTIONS_AT: u32 = 4;
const SAVE_AT: u32 = 5;
const MAP_MENU_LEN: u32 = 7;
/// The unit menu's "Capt" with a star (index 3): no unit that can be sent
/// ever has it (an air unit never captures; a unit on its own HQ or base
/// has nothing to capture), so Send takes its slot (the menu has room for
/// 13 commands, all used).
const SEND_AT: u32 = 3;
const UNIT_MENU_LEN: u32 = 13;
/// The map menu's builder (a script slot: +0x20 table, +0x31 shown
/// entries, +0x42 the cursor) and the slot array.
const SLOT_ARRAY: u32 = 0x0300_1470;
const SLOT_SIZE: u32 = 0x60;
const MENU_SCRIPT: u32 = 0x0848_A42C;
/// AW2's key state (`gpKeySt`: +2 pressed this frame).
const KEY_A: u32 = 1;
const KEY_B: u32 = 2;
const KEY_SELECT: u32 = 4;
const KEY_START: u32 = 8;
const KEY_R: u32 = 1 << 8;
const KEY_L: u32 = 1 << 9;

// --- ROM: scripts, stubs, menus (free space) -----------------------------------------

/// The ROM image's free space for this module (between Survival's
/// 0x08E00000..0x08E4FFFF and the looks' 0x08E80000..).
pub const ROM: u32 = 0x08E7_0000;
const ROM_END: u32 = 0x08E7_4000;
const ROM_MAGIC: u32 = 0x5446_5232; // "2RFT"
/// Magic ids for this module's stubs (r3 at crate::campaign_model::LANDING):
/// `MAGIC | n`.
pub const MAGIC: u32 = 0x2F00_0000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
enum Stub {
    BeginRound = 1,
    BeginViewIn,
    BeginViewOut,
    BeginOver,
    Capture,
    Stash,
    Load,
    Restore,
    Terrain,
    Arrive,
    Music,
    State,
    End,
    ViewWait,
    FrontUsable,
    FrontChosen,
    SaveUsable,
    SendUsable,
    SendChosen,
    Bonus,
    AutoOnUsable,
    AutoOffUsable,
    AutoChosen,
}

const STUBS: [Stub; 23] = [
    Stub::BeginRound,
    Stub::BeginViewIn,
    Stub::BeginViewOut,
    Stub::BeginOver,
    Stub::Capture,
    Stub::Stash,
    Stub::Load,
    Stub::Restore,
    Stub::Terrain,
    Stub::Arrive,
    Stub::Music,
    Stub::State,
    Stub::End,
    Stub::ViewWait,
    Stub::FrontUsable,
    Stub::FrontChosen,
    Stub::SaveUsable,
    Stub::SendUsable,
    Stub::SendChosen,
    Stub::Bonus,
    Stub::AutoOnUsable,
    Stub::AutoOffUsable,
    Stub::AutoChosen,
];

/// The ROM area: the mark, the stubs (16 bytes each), the scripts, the
/// menus.
const STUBS_AT: u32 = ROM + 0x10;
fn stub_at(s: Stub) -> u32 {
    STUBS_AT + 16 * (s as u32 - 1)
}
const SCRIPT_ROUND: u32 = ROM + 0x400;
const SCRIPT_VIEW: u32 = ROM + 0x600;
const SCRIPT_OVER: u32 = ROM + 0x900;
const MAP_MENU_COPY: u32 = ROM + 0xC00;
const UNIT_MENU_COPY: u32 = ROM + 0xE00;
const TEXTS: u32 = ROM + 0x1000;
const INTEL_MENU_COPY: u32 = ROM + 0x1100;
/// The menu items' labels (AW2 text ids read from the text table's free
/// tail, crate::campaign_model::TEXT_TABLE; the DS Campaign's texts use
/// 0x7400..: these are the last ones, crate::setup_phase's Deploy 0x7FFC
/// among them).
const TEXT_FRONT: u16 = 0x7FFE;
const TEXT_SEND: u16 = 0x7FFD;
const TEXT_AUTO_ON: u16 = 0x7FFB;
const TEXT_AUTO_OFF: u16 = 0x7FFA;
/// The first of the menus' own text ids (a campaign's texts stay below).
pub const TEXT_IDS_FROM: u16 = TEXT_AUTO_OFF;
const TEXT_TABLE: u32 = crate::campaign_model::TEXT_TABLE;
/// The game's menu glyphs: the icon then the name, as its own items.
const LABEL_FRONT: &[u8] = b"\x09\x86Front\0";
const LABEL_SEND: &[u8] = b"\x09\x8bSend\0";
/// Dual Strike's Auto CO items (bank 0xC0 texts 102, 103: no icon; the
/// `\x1C` pads "On" to "Off"'s width, as AW2's own "Music On").
const LABEL_AUTO_ON: &[u8] = b"Auto CO On\x1c\0";
const LABEL_AUTO_OFF: &[u8] = b"Auto CO Off\0";

/// The Intel menu (Status, Terms, Unit, Rules; `0x0802D504` opens it, its
/// table from the literal pool word), and Auto CO's two items in the copy
/// after them, one shown at a time, as the Options menu's Music On / Music
/// Off: chosen, the setting flips and the menu is redrawn in place
/// (`0x08019E68`, what Music's handler calls).
const INTEL_MENU: u32 = 0x0849_ABC0;
const INTEL_MENU_POOL: u32 = 0x0802_D550;
const INTEL_MENU_LEN: u32 = 4;
/// Terms: the item whose event id (0: no help line of the game's) and B
/// handler Auto CO's take.
const TERMS_AT: u32 = 1;
pub const AUTO_ON_AT: u32 = 4;
pub const AUTO_OFF_AT: u32 = 5;
const REFRESH_MENU: u32 = 0x0801_9E68;

/// A script slot record: {pointer, immediate, op}. Ops (`StepSlotScript`'s
/// table `0x0848A160`): 2 call, 0x17 wait for 1, 0x18 wait for 0, 0x1E
/// fade to black, 0x1F fade from black, 7 end.
fn op(ptr: u32, imm: u16, op: u16) -> [u8; 8] {
    let mut r = [0u8; 8];
    r[0..4].copy_from_slice(&ptr.to_le_bytes());
    r[4..6].copy_from_slice(&imm.to_le_bytes());
    r[6..8].copy_from_slice(&op.to_le_bytes());
    r
}

/// One swap: fade to black, capture the live front, exchange it with the
/// store, rebuild the scene, fade in.
fn swap_ops(begin: Stub) -> Vec<[u8; 8]> {
    let call = |s: Stub| op(stub_at(s) | 1, 0, 2);
    vec![
        call(begin),
        op(0, 0x10, 0x1E),
        op(FADES_RUNNING | 1, 0, 0x18),
        call(Stub::Capture),
        call(Stub::Stash),
        call(Stub::Load),
        call(Stub::Restore),
        call(Stub::Terrain),
        call(Stub::Arrive),
        op(RESET_UNITS | 1, 0, 2),
        op(MAP_GRAPHICS | 1, 0, 2),
        op(FRAME_CALLBACKS | 1, 0, 2),
        call(Stub::Music),
        call(Stub::State),
        op(LOCK_MAP | 1, 0, 2),
        op(0, 0x10, 0x1F),
        op(FADES_RUNNING | 1, 0, 0x18),
        op(UNLOCK_MAP | 1, 0, 2),
        call(Stub::End),
    ]
}

fn swap_script(kind: Stub) -> Vec<u8> {
    let mut v: Vec<[u8; 8]> = Vec::new();
    match kind {
        Stub::BeginViewIn => {
            v.push(op(CLOSE_MENU | 1, 0, 2));
            v.extend(swap_ops(Stub::BeginViewIn));
            v.push(op(stub_at(Stub::ViewWait) | 1, 0, 0x17));
            v.extend(swap_ops(Stub::BeginViewOut));
        }
        Stub::BeginOver => {
            v.extend(swap_ops(Stub::BeginOver));
            v.push(op(stub_at(Stub::Bonus) | 1, 0, 2));
        }
        k => v.extend(swap_ops(k)),
    }
    v.push(op(0, 0, 7));
    v.concat()
}

fn stub_code(n: u32) -> [u8; 16] {
    crate::campaign_model::stub(MAGIC | n)
}

/// Writes the module's ROM area (once; the image keeps it).
fn install(core: &mut Core) {
    if core.raw_read_32(ROM, -1) == ROM_MAGIC {
        return;
    }
    for s in STUBS {
        core.raw_write_range(stub_at(s), -1, &stub_code(s as u32));
    }
    for (at, kind) in [(SCRIPT_ROUND, Stub::BeginRound), (SCRIPT_VIEW, Stub::BeginViewIn), (SCRIPT_OVER, Stub::BeginOver)] {
        core.raw_write_range(at, -1, &swap_script(kind));
    }
    // The map menu: the game's seven items, Front after Options, Save's
    // test ours (no Save while the second front is on the screen).
    let mut menu = Vec::new();
    for k in 0..MAP_MENU_LEN {
        let mut e = vec![0u8; MENU_ENTRY as usize];
        core.raw_read_range(MAP_MENU + MENU_ENTRY * k, -1, &mut e);
        if k == SAVE_AT {
            e[4..8].copy_from_slice(&(stub_at(Stub::SaveUsable) | 1).to_le_bytes());
        }
        menu.extend_from_slice(&e);
        if k == OPTIONS_AT {
            menu.extend_from_slice(&menu_entry(core, OPTIONS_AT, Stub::FrontUsable, Stub::FrontChosen, TEXT_FRONT));
        }
    }
    let mut end = vec![0u8; MENU_ENTRY as usize];
    core.raw_read_range(MAP_MENU + MENU_ENTRY * MAP_MENU_LEN, -1, &mut end);
    menu.extend_from_slice(&end);
    core.raw_write_range(MAP_MENU_COPY, -1, &menu);
    // The unit menu: Send in the starred Capt's slot.
    let mut units = vec![0u8; (MENU_ENTRY * (UNIT_MENU_LEN + 1)) as usize];
    core.raw_read_range(UNIT_MENU, -1, &mut units);
    let send = menu_entry_from(core, UNIT_MENU + MENU_ENTRY * SEND_AT, Stub::SendUsable, Stub::SendChosen, TEXT_SEND);
    units[(MENU_ENTRY * SEND_AT) as usize..(MENU_ENTRY * (SEND_AT + 1)) as usize].copy_from_slice(&send);
    core.raw_write_range(UNIT_MENU_COPY, -1, &units);
    // The Intel menu: the game's four items, then Auto CO On / Auto CO Off.
    let mut intel = vec![0u8; (MENU_ENTRY * INTEL_MENU_LEN) as usize];
    core.raw_read_range(INTEL_MENU, -1, &mut intel);
    for (usable, text) in [(Stub::AutoOnUsable, TEXT_AUTO_ON), (Stub::AutoOffUsable, TEXT_AUTO_OFF)] {
        intel.extend(menu_entry_from(core, INTEL_MENU + MENU_ENTRY * TERMS_AT, usable, Stub::AutoChosen, text));
    }
    let mut end = vec![0u8; MENU_ENTRY as usize];
    core.raw_read_range(INTEL_MENU + MENU_ENTRY * INTEL_MENU_LEN, -1, &mut end);
    intel.extend(end);
    core.raw_write_range(INTEL_MENU_COPY, -1, &intel);
    // Labels and the help line.
    core.raw_write_range(TEXTS, -1, LABEL_FRONT);
    core.raw_write_range(TEXTS + 0x10, -1, LABEL_SEND);
    core.raw_write_range(TEXTS + 0x20, -1, LABEL_AUTO_ON);
    core.raw_write_range(TEXTS + 0x30, -1, LABEL_AUTO_OFF);
    core.raw_write_32(ROM, -1, ROM_MAGIC);
    assert!(UNIT_MENU_COPY + MENU_ENTRY * (UNIT_MENU_LEN + 1) <= TEXTS && TEXTS + 0x40 <= INTEL_MENU_COPY);
    assert!(INTEL_MENU_COPY + intel.len() as u32 <= ROM_END);
}

/// Front's menu entry (for crate::setup_phase's menu): this module's ROM
/// written first (its stubs).
pub fn front_entry(core: &mut Core) -> Vec<u8> {
    install(core);
    menu_entry(core, OPTIONS_AT, Stub::FrontUsable, Stub::FrontChosen, TEXT_FRONT)
}

/// A front is being set up or brought back (a swap's steps run).
pub fn rebuilding(core: &Core) -> bool {
    core.raw_read_8(BUSY, -1) != 0
}

/// A menu entry like `like`'s (its event id, its B handler), with our
/// usability test, A handler and label.
fn menu_entry(core: &Core, like: u32, usable: Stub, chosen: Stub, text: u16) -> Vec<u8> {
    menu_entry_from(core, MAP_MENU + MENU_ENTRY * like, usable, chosen, text)
}

fn menu_entry_from(core: &Core, like: u32, usable: Stub, chosen: Stub, text: u16) -> Vec<u8> {
    let mut e = vec![0u8; MENU_ENTRY as usize];
    core.raw_read_range(like, -1, &mut e);
    e[4..8].copy_from_slice(&(stub_at(usable) | 1).to_le_bytes());
    e[0x14..0x18].copy_from_slice(&(stub_at(chosen) | 1).to_le_bytes());
    e[0x1C..0x20].copy_from_slice(&(text as u32).to_le_bytes());
    e
}

// --- RAM (EWRAM the game never touches: 0x0203E400..0x0203F5FF) ----------------------

/// The module's state ([`STATE_LEN`] bytes).
const STATE: u32 = 0x0203_E400;
/// The front on the screen: 0 main, 1 second.
const LIVE: u32 = STATE;
/// The second front: 0 not started, 1 being fought, 2 won, 3 lost.
const SECOND: u32 = STATE + 1;
const SECOND_IDLE: u8 = 0;
const SECOND_ON: u8 = 1;
const SECOND_WON: u8 = 2;
const SECOND_LOST: u8 = 3;
/// The store holds a front (1) or nothing (0).
const STORE_KIND: u32 = STATE + 2;
/// The swap running: its begin stub (0 none).
const BUSY: u32 = STATE + 3;
/// The next handover passes (the round's swap has just brought it back).
const PASS: u32 = STATE + 4;
/// The other front is being looked at (1), B pressed to go back (2).
const VIEW: u32 = STATE + 5;
/// The swap loads a front never played (set by the stash).
const FRESH: u32 = STATE + 6;
/// The second front's result banner: frames left.
const BANNER: u32 = STATE + 7;
/// The unit being sent (its record), until its move has ended.
const SENDING: u32 = STATE + 0x08;
/// The staging buffer's checksum while a front is looked at.
const CHECK: u32 = STATE + 0x0C;
/// The power the second front's survivors bring (its value).
const BONUS: u32 = STATE + 0x10;
/// Units sent before the second front's first round (12-byte records,
/// their armies), placed when it starts; their count.
const QUEUED: u32 = STATE + 0x14;
/// The army that won the second front (for the hook and the banner).
const WINNER: u32 = STATE + 0x15;
/// The second front has had its first round (1).
const STARTED: u32 = STATE + 0x16;
/// The looked-at front's current army's controller, while it shows the
/// cursor (put back before it is captured).
const VIEW_CTL: u32 = STATE + 0x17;
/// Intel > Auto CO, per army (bit `army - 1`): set, Auto CO is off and the
/// army's owner plays its second-front turns ([`FrontControl::AutoCo`]).
/// (Set means off: a battle saved before the setting existed reads 0, Dual
/// Strike's default, on.)
const MANUAL: u32 = STATE + 0x1B;
/// A Continue is bringing the second front back (1, until the battle is on
/// the screen).
const CONTINUED: u32 = STATE + 0x1C;
const QUEUE: u32 = STATE + 0x20;
const QUEUE_LEN: u32 = 8;
const QUEUE_ARMY: u32 = QUEUE + 12 * QUEUE_LEN;
const AFTER_LR: u32 = QUEUE_ARMY + QUEUE_LEN;
const AFTER_R0: u32 = AFTER_LR + 4;
/// The battle's own flags (AW2's mission flags 0..0x7F, `0x030033F4`: its
/// event records' "once" flags among them) during a swap: both fronts share
/// them, as Dual Strike's events do (Victory or Death!'s Black Arc stops
/// dropping its bombs on the main front once the second front's record
/// that destroys it has fired, flag 2).
const LOCAL_KEEP: u32 = AFTER_R0 + 4;
const LOCAL_FLAGS: u32 = 0x0300_33F4;
const LOCAL_LEN: u32 = 16;
pub const STATE_LEN: u32 = LOCAL_KEEP + LOCAL_LEN - STATE;
/// Each army's CO on the second front (the player's picks), and the
/// mission they were picked for (its index + 1): kept between battles (the
/// CO screen comes before the battle).
const SECOND_COS: u32 = STATE + STATE_LEN;
const PICKED_FOR: u32 = SECOND_COS + 4;
const PICKS_LEN: u32 = 5;
/// crate::sky_front's state (a borrowed OBJ palette: 1 byte, then 32 at +4).
pub const SKY_STATE: u32 = STATE + 0xC0;
const SKY_LEN: u32 = 0x24;
/// The store: the block, then [`EXTRA_LEN`] bytes of tangoAW2's state.
const STORE: u32 = 0x0203_E500;
const EXTRA: u32 = STORE + BLOCK_LEN;
/// The rain's fog rule (1), CO skills (30), Ex Machina's stun.
const EX_FOG: u32 = 0;
const EX_SKILLS: u32 = 1;
const SKILLS_LEN: u32 = 30;
const EX_STUN: u32 = EX_SKILLS + SKILLS_LEN;
pub const EXTRA_LEN: u32 = EX_STUN + crate::co_powers::STUN_STATE_LEN as u32;
pub const STORE_LEN: u32 = BLOCK_LEN + EXTRA_LEN;
#[cfg(test)]
const RAM_END: u32 = 0x0203_F600;

// --- The gate ------------------------------------------------------------------------

/// A two-front battle in progress (or about to start): the main front's
/// mission and its description.
#[derive(Clone, Copy)]
pub struct Battle {
    pub main: &'static MissionInfo,
    pub fronts: &'static TwoFront,
}

/// The one gate: whether the battle being played (or set up) is on two
/// fronts. Today: a DS Campaign mission whose description has a second
/// front (Dual Strike's five). Everything in this module is off without it.
pub fn battle(core: &Core) -> Option<Battle> {
    if !crate::ds_campaign::active(core)
        || core.raw_read_8(MAP_ID, -1) != crate::campaign_model::MAP_ID
        || core.raw_read_8(GAME_MODE, -1) != CAMPAIGN
    {
        return None;
    }
    let c = crate::ds_campaign::campaign(core)?;
    let main = c.model.built.missions.get(crate::ds_campaign::mission(core) as usize)?;
    main.two_front.as_ref().map(|fronts| Battle { main, fronts })
}

/// A battle is loaded (its scene's function table), or being rebuilt by a
/// swap (which clears it until the frame callbacks are back).
fn in_battle(core: &Core) -> bool {
    core.raw_read_32(BATTLE, -1) != 0 || core.raw_read_8(BUSY, -1) != 0
}

/// A two-front battle on the screen.
pub fn on(core: &Core) -> Option<Battle> {
    battle(core).filter(|_| in_battle(core))
}

/// The front on the screen (0 main, 1 second), in a two-front battle.
pub fn live(core: &Core) -> Option<u8> {
    on(core).map(|_| core.raw_read_8(LIVE, -1))
}

/// The second front is on the screen.
pub fn second_live(core: &Core) -> bool {
    live(core) == Some(1)
}

/// The fronts are being swapped (the live front's RAM is being exchanged):
/// what is on the screen is neither front's yet.
pub fn swapping(core: &Core) -> bool {
    on(core).is_some() && core.raw_read_8(BUSY, -1) != 0 && core.raw_read_8(VIEW, -1) != 1
}

/// The live front's own mission info (its look, weather, fog, armies).
pub fn live_info(core: &Core) -> Option<&'static MissionInfo> {
    let b = battle(core)?;
    if core.raw_read_8(LIVE, -1) == 1 {
        crate::ds_campaign::campaign(core)?.model.built.missions.get(b.fronts.second as usize)
    } else {
        Some(b.main)
    }
}

fn header(core: &Core, index: u8) -> Option<[u8; 0x5C]> {
    crate::ds_campaign::campaign(core)?.model.built.headers.iter().find(|h| h.0 == index).map(|h| h.1)
}

/// The map table entry the battle is played on (tangoAW2's table with
/// room for 0x100 ids).
fn table_entry(core: &Core) -> Option<u32> {
    let t = crate::five_map::table(core);
    (t == crate::survival::TABLE).then_some(t + 0x5C * crate::campaign_model::MAP_ID as u32)
}

/// The live front's header in the battle's map table entry (the game reads
/// the map, the deployment and the events from it).
fn sync_header(core: &mut Core, b: &Battle) {
    let index = if core.raw_read_8(LIVE, -1) == 1 { b.fronts.second } else { b.main.index as u8 };
    let (Some(at), Some(h)) = (table_entry(core), header(core, index)) else { return };
    let mut now = [0u8; 0x5C];
    core.raw_read_range(at, -1, &mut now);
    if now != h {
        core.raw_write_range(at, -1, &h);
    }
}

/// The state cleared (but the CO screen's picks).
fn reset(core: &mut Core) {
    let z = vec![0u8; STATE_LEN as usize];
    let mut now = vec![0u8; STATE_LEN as usize];
    core.raw_read_range(STATE, -1, &mut now);
    if now != z {
        core.raw_write_range(STATE, -1, &z);
    }
    if core.raw_read_8(SKY_STATE, -1) != 0 {
        core.raw_write_8(SKY_STATE, -1, 0);
    }
}

/// Every frame, before the game runs (crate::pvp, with the pack offline).
pub fn tick(core: &mut Core, on_: bool) {
    if !on_ {
        return;
    }
    let Some(b) = battle(core) else { return };
    install(core);
    if !in_battle(core) {
        // Between battles (the world map, the CO screen, the mission card,
        // a Continue on its way): a battle starts on its main front (its
        // state is cleared when it starts, [`map_start`], or comes with a
        // mission saved halfway, [`restore_saved`]).
        // (but a Continue on the second front, between its block restored
        // and the battle's frame callbacks, [`restore_saved`])
        if core.raw_read_8(LIVE, -1) != 0 && core.raw_read_8(CONTINUED, -1) == 0 {
            core.raw_write_8(LIVE, -1, 0);
        }
        return;
    }
    if core.raw_read_8(CONTINUED, -1) != 0 {
        core.raw_write_8(CONTINUED, -1, 0);
    }
    if core.raw_read_8(SECOND, -1) == SECOND_IDLE && core.raw_read_8(STORE_KIND, -1) == 0 {
        // (the battle's first frame: the second front is to be fought)
        core.raw_write_8(SECOND, -1, SECOND_ON);
        fill_second_cos(core, &b);
        // Auto CO as the description starts it, per army.
        let manual = (0..4).filter(|&k| b.fronts.control[k] == FrontControl::AutoCo { on: false }).fold(0u8, |m, k| m | 1 << k);
        core.raw_write_8(MANUAL, -1, manual);
    }
    sync_header(core, &b);
    // Means to an End: the crystals on the second front guard the main
    // front's weak points (crate::ds_campaign_rules::crystal_alive).
    crate::ds_campaign_rules::mte_crystals_tick(core);
    // Omens and Signs: the Black Arc's barrier round the main front's
    // fortress.
    crate::ds_campaign_rules::omens_barrier_tick(core);
    // The panels (help line, view, result, which front).
    panel_tick(core);
    // A front in the sky: its clouds and the Black Arc (crate::sky_front).
    crate::sky_front::tick(core);
    let banner = core.raw_read_8(BANNER, -1);
    // (shown on the player's turn, [`want_panel`])
    if banner > 0 && core.raw_read_8(BUSY, -1) == 0 && core.raw_read_16(MAP_STATE, -1) == STATE_CURSOR {
        core.raw_write_8(BANNER, -1, banner - 1);
    }
}

/// The menus (their pool words, every frame): ours while a two-front
/// battle is on, the game's otherwise.
pub fn menus(core: &mut Core, on_: bool) {
    let ours = on_ && on(core).is_some() && core.raw_read_32(ROM, -1) == ROM_MAGIC;
    for (pool, game, copy) in [
        (MAP_MENU_POOL, MAP_MENU, MAP_MENU_COPY),
        (UNIT_MENU_POOL, UNIT_MENU, UNIT_MENU_COPY),
        (INTEL_MENU_POOL, INTEL_MENU, INTEL_MENU_COPY),
    ] {
        let want = if ours { copy } else { game };
        if core.raw_read_32(pool, -1) != want {
            core.raw_write_32(pool, -1, want);
        }
    }
    // The labels: text ids from the free tail of the text table.
    for (id, at) in [(TEXT_FRONT, TEXTS), (TEXT_SEND, TEXTS + 0x10), (TEXT_AUTO_ON, TEXTS + 0x20), (TEXT_AUTO_OFF, TEXTS + 0x30)] {
        let entry = TEXT_TABLE + 4 * id as u32;
        if ours && core.raw_read_32(entry, -1) != at {
            core.raw_write_32(entry, -1, at);
        }
    }
}

/// The keys while the other front is looked at: the map cursor's D-pad
/// reaches the game, everything else is ours (B goes back).
pub fn keys(core: &mut Core, keys: u32, prev: u32) -> u32 {
    if core.raw_read_8(VIEW, -1) == 0 || on(core).is_none() {
        return keys;
    }
    if core.raw_read_8(BUSY, -1) != Stub::BeginViewIn as u8 || core.raw_read_8(VIEW, -1) != 1 {
        return keys & !(KEY_A | KEY_B | KEY_SELECT | KEY_START | KEY_L | KEY_R);
    }
    if keys & !prev & KEY_B != 0 {
        core.raw_write_8(VIEW, -1, 2);
    }
    keys & !(KEY_A | KEY_B | KEY_SELECT | KEY_START | KEY_L | KEY_R)
}

/// Each army's CO on the second front, the player's picks filled in (picks
/// not made for this mission: the main front's CO).
fn fill_second_cos(core: &mut Core, b: &Battle) {
    let picked = core.raw_read_8(PICKED_FOR, -1) == b.main.index as u8 + 1;
    for k in 0..4u32 {
        let c = b.fronts.cos[k as usize];
        let v = match c {
            PICK if picked => continue,
            PICK => core.raw_read_8(COS + 1 + k, -1),
            c => c,
        };
        core.raw_write_8(SECOND_COS + k, -1, v);
    }
    core.raw_write_8(PICKED_FOR, -1, b.main.index as u8 + 1);
}

// --- The CO screen: the second front's picks -----------------------------------------

/// The picks the CO screen asks for: the main front's (the header's leading
/// "player picks"), then the second front's.
fn picks(b: &Battle) -> (u32, u32) {
    let main = b.main.cos.iter().take(b.main.armies as usize).take_while(|c| c.0 == 0x1C).count() as u32;
    let second = b.fronts.cos.iter().take_while(|&&c| c == PICK).count() as u32;
    (main, second)
}

fn co_screen(core: &Core) -> bool {
    const CO_SCREEN: u32 = 0x0861_6638;
    (0..32).any(|k| core.raw_read_32(0x0200_D610 + 0x6C * k, -1) == CO_SCREEN)
}

fn return_to(core: &mut Core, r0: u32) {
    let cpu = core.gba_mut().cpu_mut();
    let lr = cpu.gpr(14) as u32;
    cpu.set_gpr(0, r0 as i32);
    cpu.set_thumb_pc(lr & !1);
}

/// `sub_0803BD14` on the CO screen of a two-front battle: the main front's
/// picks and the second front's.
fn pick_count(core: &mut Core) {
    let Some(b) = battle(core) else { return };
    if in_battle(core) || !co_screen(core) {
        return;
    }
    let (m, s) = picks(&b);
    if s > 0 {
        return_to(core, m + s);
    }
}

/// `SetArmyCoIdsFromList(list)`: the main front's picks to gPlaySt, the
/// second front's to [`SECOND_COS`].
fn set_picks(core: &mut Core) {
    // The DS Campaign's tag partners picked on the same screen (crate::tag
    // shares this trap; nothing there in a two-front mission).
    crate::tag::set_cos(core);
    let Some(b) = battle(core) else { return };
    let (m, s) = picks(&b);
    if s == 0 {
        return;
    }
    let list = core.gba().cpu().gpr(0) as u32;
    for k in 0..m {
        let v = core.raw_read_8(list + k, -1);
        core.raw_write_8(COS + 1 + k, -1, v);
    }
    core.raw_write_8(COS, -1, 0);
    let mut n = m;
    for k in 0..4u32 {
        let c = b.fronts.cos[k as usize];
        let v = if c == PICK {
            n += 1;
            core.raw_read_8(list + n - 1, -1)
        } else {
            c
        };
        core.raw_write_8(SECOND_COS + k, -1, v);
    }
    core.raw_write_8(PICKED_FOR, -1, b.main.index as u8 + 1);
    return_to(core, 0);
}

// --- Rounds --------------------------------------------------------------------------

fn players(core: &Core) -> u32 {
    core.raw_read_32(PLAYERS_PTR, -1)
}

/// `IsPlayerAliveAndActive`: in the battle and not defeated.
fn alive(core: &Core, army: u32) -> bool {
    let p = players(core) + PLAYER * army;
    core.raw_read_8(p + 0x1B, -1) != 0 && core.raw_read_16(p + 0x14, -1) == 0
}

/// Every army on the front has had its turn: none after the current one.
fn round_over(core: &Core) -> bool {
    let army = core.raw_read_16(CURRENT_ARMY, -1) as u32;
    !(army + 1..=4).any(|a| alive(core, a))
}

/// Starts a script of ours in a free script slot (`sub_080152EC(script,
/// 0)`), tail-called (it returns to the trapped function's caller).
fn start_script_instead(core: &mut Core, script: u32) {
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, script as i32);
    cpu.set_gpr(1, 0);
    cpu.set_thumb_pc(START_SLOT_SCRIPT);
}

/// `MapState_TurnHandoverPrompt`: at the end of a round on one front, the
/// other front's round (while the second front is fought).
fn handover(core: &mut Core) {
    let Some(_) = on(core) else { return };
    if core.raw_read_8(PASS, -1) != 0 {
        core.raw_write_8(PASS, -1, 0);
        return;
    }
    if core.raw_read_8(SECOND, -1) != SECOND_ON || core.raw_read_8(BUSY, -1) != 0 || !round_over(core) {
        return;
    }
    core.raw_write_16(MAP_STATE, -1, STATE_IDLE);
    core.raw_write_8(BUSY, -1, Stub::BeginRound as u8);
    start_script_instead(core, SCRIPT_ROUND);
}

/// `MapState_EndOfGame` on the second front: its battle is over (its events
/// or AW2's rules ended it). Its result shows and the main front comes back
/// for good.
fn end_of_game(core: &mut Core) {
    let Some(b) = on(core) else { return };
    if core.raw_read_8(LIVE, -1) != 1 || core.raw_read_8(BUSY, -1) != 0 {
        return;
    }
    // (AW2's own state waits for the match's scripts first, `sub_08019260`)
    let _ = SCRIPTS_RUNNING;
    if events_running(core) {
        return;
    }
    let won = crate::ds_campaign_rules::player_won(core);
    core.raw_write_8(SECOND, -1, if won { SECOND_WON } else { SECOND_LOST });
    let winner = (1..=4u32).find(|&a| alive(core, a)).unwrap_or(0);
    core.raw_write_8(WINNER, -1, winner as u8);
    // The survivors' value, for the main front's power meter (Dual Strike:
    // "the surviving units will be added to the power meter").
    let mut value = 0u32;
    if won {
        for (_, u) in live_units(core, 1) {
            value += unit_value(core, u);
        }
    }
    core.raw_write_32(BONUS, -1, value);
    let _ = b;
    core.raw_write_16(MAP_STATE, -1, STATE_IDLE);
    core.raw_write_8(BUSY, -1, Stub::BeginOver as u8);
    start_script_instead(core, SCRIPT_OVER);
}

/// An event script is running (`gUnknown_0200C528`: 10 slots of 0x18
/// bytes, the script's pointer first).
pub(crate) fn events_running(core: &Core) -> bool {
    (0..11).any(|k| core.raw_read_32(0x0200_C510 + 0x18 * k, -1) != 0)
}

fn live_units(core: &Core, army: u32) -> Vec<(u32, u32)> {
    let base = core.raw_read_32(UNITS_PTR, -1);
    (1..SLOTS)
        .map(|j| (j, base + 12 * ((army - 1) * 64 + j)))
        .filter(|&(_, u)| core.raw_read_8(u, -1) != 0)
        .collect()
}

/// A unit's value: its price x its bars (the power meter's measure,
/// crate::oozium).
fn unit_value(core: &Core, u: u32) -> u32 {
    let t = core.raw_read_8(u, -1) as u32;
    let price = core.raw_read_16(crate::roster::table(core) + 0x5C * t + 6, -1) as u32 * 10;
    let hp = (core.raw_read_16(u + 4, -1) & 0x7F) as u32;
    price * hp.div_ceil(10)
}

/// The second front is over (won or lost) and the main front is back on
/// the screen (the over swap's last step, before the survivors' power).
///
/// **The hook for tag pairs** (worked on separately): Dual Strike brings
/// the winning side's second-front CO to the main front as its army's tag
/// partner (won by the player: "The second front has been secured. The CO
/// will now report back to the main front."; by Black Hole: "Return to the
/// main front for tag battle."). [`second_front_result`] gives the side,
/// the winning army and its second-front CO; a tag-pair module joins that
/// CO to that army's main-front CO here. Until then the main front's armies
/// keep their one CO.
fn second_front_over(core: &mut Core) {
    // The winning side's second-front CO joins its army's main-front CO as
    // its tag partner (crate::tag; an empty meter, as Dual Strike's), for
    // the player's army and Black Hole's alike.
    if let Some((_, army, co)) = second_front_result(core) {
        let main = crate::tag::army_co_of(core, army);
        if co != 0xFF && co != main && crate::tag::partner(core, army).is_none() {
            crate::tag::form_pair(core, army, co, 0);
        }
    }
}

/// The second front's outcome, once it is over: (the player's side won,
/// the winning army, that army's second-front CO).
pub fn second_front_result(core: &Core) -> Option<(bool, u32, u8)> {
    battle(core)?;
    let s = core.raw_read_8(SECOND, -1);
    if s != SECOND_WON && s != SECOND_LOST {
        return None;
    }
    let winner = core.raw_read_8(WINNER, -1) as u32;
    let co = if (1..=4).contains(&winner) { core.raw_read_8(SECOND_COS + winner - 1, -1) } else { 0xFF };
    Some((s == SECOND_WON, winner, co))
}

// --- The stubs -----------------------------------------------------------------------

/// A magic stub of this module ([`MAGIC`] | n), from the landing
/// (crate::ds_campaign). Returns r0 (or does its own tail call).
pub fn magic(core: &mut Core, id: u32) {
    let n = id & 0xFF;
    let Some(&s) = STUBS.iter().find(|s| **s as u32 == n) else { return return_to(core, 0) };
    let b = battle(core);
    if std::env::var_os("TWO_FRONT_DEBUG").is_some() {
        eprintln!("two_front magic {s:?} battle {} busy {} r2 {:x}", b.is_some(), core.raw_read_8(BUSY, -1), core.gba().cpu().gpr(2));
    }
    match (s, b) {
        (Stub::FrontUsable, Some(_)) => {
            // Shown while the second front is fought, else hidden.
            let shown = core.raw_read_8(SECOND, -1) == SECOND_ON && core.raw_read_8(BUSY, -1) == 0;
            return_to(core, if shown { 0 } else { 1 })
        }
        (Stub::FrontChosen, Some(_)) => {
            // (a stub's r2 and r3 are its own: the item's flags are not
            // passed on; it is never greyed)
            if core.raw_read_8(SECOND, -1) != SECOND_ON || core.raw_read_8(BUSY, -1) != 0 {
                return return_to(core, 0);
            }
            core.raw_write_8(BUSY, -1, Stub::BeginViewIn as u8);
            start_script_instead(core, SCRIPT_VIEW)
        }
        (Stub::SaveUsable, Some(_)) => {
            // On either front (Dual Strike's menu has Save on a player's
            // second-front turn too); the game's own test.
            if core.raw_read_8(BUSY, -1) != 0 {
                return return_to(core, 1);
            }
            core.gba_mut().cpu_mut().set_thumb_pc(0x0802_C644)
        }
        (Stub::SendUsable, Some(b)) => {
            let r = send_usable(core, &b);
            return_to(core, r)
        }
        (Stub::SendChosen, Some(b)) => send_chosen(core, &b),
        (Stub::AutoOnUsable | Stub::AutoOffUsable, Some(b)) => {
            // Shown to the army whose setting it is, on its main-front turn:
            // On while its Auto CO is on, Off while it is off.
            let shown = auto_co_army(core, &b).is_some_and(|a| auto_co(core, a) == (s == Stub::AutoOnUsable));
            return_to(core, if shown { 0 } else { 1 })
        }
        (Stub::AutoChosen, Some(b)) => {
            if let Some(a) = auto_co_army(core, &b) {
                let m = core.raw_read_8(MANUAL, -1) ^ (1 << (a - 1));
                core.raw_write_8(MANUAL, -1, m);
            }
            // The menu redrawn in place (its other item shown), as Music
            // On / Off's handler does.
            core.gba_mut().cpu_mut().set_thumb_pc(REFRESH_MENU)
        }
        (_, None) => return_to(
            core,
            if matches!(s, Stub::FrontUsable | Stub::SendUsable | Stub::SaveUsable | Stub::AutoOnUsable | Stub::AutoOffUsable) { 1 } else { 0 },
        ),
        (s, Some(b)) => swap_step(core, s, &b),
    }
}

/// The swap's steps (see [`swap_ops`]).
fn swap_step(core: &mut Core, s: Stub, b: &Battle) {
    let kind = core.raw_read_8(BUSY, -1) as u32;
    match s {
        Stub::BeginRound | Stub::BeginViewIn | Stub::BeginViewOut | Stub::BeginOver => {
            core.raw_write_8(BUSY, -1, s as u8);
            core.raw_write_16(MAP_STATE, -1, STATE_IDLE);
            if s == Stub::BeginViewIn {
                core.raw_write_8(VIEW, -1, 0);
            }
            if s == Stub::BeginViewOut {
                // The looked-at front's army as it was.
                let army = core.raw_read_8(CURRENT_ARMY, -1) as u32;
                if (1..=4).contains(&army) {
                    let ctl = core.raw_read_8(VIEW_CTL, -1);
                    core.raw_write_8(players(core) + PLAYER * army + 0x1B, -1, ctl);
                }
            }
            return_to(core, 0)
        }
        Stub::Capture => {
            // Back from looking at a front: the front looked at is as it was
            // restored (its block still in the staging buffer), unless it
            // was set up only to be looked at.
            if kind == Stub::BeginViewOut as u32
                && core.raw_read_8(FRESH, -1) == 0
                && core.raw_read_32(CHECK, -1) == checksum(core, STAGING, BLOCK_LEN)
            {
                return return_to(core, 0);
            }
            // `CaptureBattleSaveState(0)`: resumed, a front goes on where it
            // was (its state set by [`Stub::State`]).
            let cpu = core.gba_mut().cpu_mut();
            cpu.set_gpr(0, 0);
            cpu.set_thumb_pc(CAPTURE)
        }
        Stub::Stash => {
            stash(core, b, kind);
            return_to(core, 0)
        }
        Stub::Load => {
            if core.raw_read_8(FRESH, -1) == 1 {
                // A front never played: started as a battle starts.
                core.gba_mut().cpu_mut().set_thumb_pc(INIT_MAP_STATE)
            } else {
                core.gba_mut().cpu_mut().set_thumb_pc(INIT_SETTINGS)
            }
        }
        Stub::Restore => {
            if core.raw_read_8(FRESH, -1) == 1 {
                // Its deployment, as a battle's start places it (after
                // `InitMapGameAndGraphics`, the start's script `0x0849D134`).
                return core.gba_mut().cpu_mut().set_thumb_pc(SPAWN_UNITS);
            }
            core.gba_mut().cpu_mut().set_thumb_pc(RESTORE)
        }
        Stub::Terrain => {
            if core.raw_read_8(FRESH, -1) == 1 {
                return return_to(core, 0);
            }
            core.gba_mut().cpu_mut().set_thumb_pc(TERRAIN_PLANE)
        }
        Stub::Arrive => {
            arrive(core, b);
            return_to(core, 0)
        }
        Stub::Music => {
            // A round's swap: the next turn starts its CO's music itself.
            if kind == Stub::BeginRound as u32 && core.raw_read_8(FRESH, -1) != 1 {
                return return_to(core, 0);
            }
            let army = core.raw_read_8(CURRENT_ARMY, -1) as u32;
            let cpu = core.gba_mut().cpu_mut();
            cpu.set_gpr(0, army.max(1) as i32);
            cpu.set_thumb_pc(CO_MUSIC)
        }
        Stub::State => {
            let fresh = core.raw_read_8(FRESH, -1) == 1;
            let state = match kind {
                // The other front's round: its handover, then its armies.
                k if k == Stub::BeginRound as u32 || k == Stub::BeginOver as u32 => {
                    if fresh {
                        5
                    } else {
                        core.raw_write_8(PASS, -1, 1);
                        STATE_HANDOVER
                    }
                }
                // Looked at: the cursor only (its army made the player's
                // for as long, as the game's dispatch gives a player's army
                // the cursor; put back before it leaves).
                k if k == Stub::BeginViewIn as u32 => {
                    let army = (core.raw_read_8(CURRENT_ARMY, -1) as u32).max(1);
                    let at = players(core) + PLAYER * army + 0x1B;
                    core.raw_write_8(VIEW_CTL, -1, core.raw_read_8(at, -1));
                    core.raw_write_8(at, -1, 1);
                    STATE_DISPATCH
                }
                // Back: the turn goes on as AW2's Continue brings it back.
                _ => STATE_DISPATCH,
            };
            let _ = STATE_CURSOR;
            core.raw_write_16(MAP_STATE, -1, state);
            if kind == Stub::BeginViewIn as u32 {
                core.raw_write_8(VIEW, -1, 1);
            }
            if kind == Stub::BeginOver as u32 {
                core.raw_write_8(BANNER, -1, 150);
            }
            return_to(core, 0)
        }
        Stub::End => {
            core.raw_write_8(BUSY, -1, if kind == Stub::BeginViewIn as u32 { kind as u8 } else { 0 });
            if kind != Stub::BeginViewIn as u32 {
                core.raw_write_8(VIEW, -1, 0);
                core.raw_write_8(FRESH, -1, 0);
            }
            return_to(core, 0)
        }
        Stub::ViewWait => {
            let back = core.raw_read_8(VIEW, -1) == 2;
            if back {
                core.raw_write_8(VIEW, -1, 3);
            }
            return_to(core, back as u32)
        }
        Stub::Bonus => {
            second_front_over(core);
            let value = core.raw_read_32(BONUS, -1);
            core.raw_write_32(BONUS, -1, 0);
            if value == 0 || core.raw_read_8(SECOND, -1) != SECOND_WON {
                return return_to(core, 0);
            }
            let cpu = core.gba_mut().cpu_mut();
            cpu.set_gpr(0, 1);
            cpu.set_gpr(1, value as i32);
            cpu.set_thumb_pc(ADD_CHARGE)
        }
        _ => return_to(core, 0),
    }
}

fn checksum(core: &Core, at: u32, len: u32) -> u32 {
    let mut b = vec![0u8; len as usize];
    core.raw_read_range(at, -1, &mut b);
    b.iter().fold(0x811C_9DC5u32, |h, &x| (h ^ x as u32).wrapping_mul(0x0100_0193))
}

fn extras(core: &Core) -> Vec<u8> {
    let mut e = vec![0u8; EXTRA_LEN as usize];
    e[EX_FOG as usize] = crate::ds_weather::rule_fog(core);
    core.raw_read_range(crate::co_skills::ACTIVE, -1, &mut e[EX_SKILLS as usize..(EX_SKILLS + SKILLS_LEN) as usize]);
    e[EX_STUN as usize..].copy_from_slice(&crate::co_powers::stun_state(core));
    e
}

fn set_extras(core: &mut Core, e: &[u8]) {
    crate::ds_weather::set_rule_fog(core, e[EX_FOG as usize]);
    core.raw_write_range(crate::co_skills::ACTIVE, -1, &e[EX_SKILLS as usize..(EX_SKILLS + SKILLS_LEN) as usize]);
    let army = core.raw_read_8(CURRENT_ARMY, -1);
    crate::co_powers::set_stun_state(core, Some(&e[EX_STUN as usize..]), army);
}

/// The live front (just captured into the staging buffer) and the store
/// change places; the other front's header goes into the map table entry.
/// A front never played is set up instead (its armies, COs and rules in
/// gPlaySt, as the campaign sets a mission's).
fn stash(core: &mut Core, b: &Battle, kind: u32) {
    let live = core.raw_read_8(LIVE, -1);
    let mut captured = vec![0u8; BLOCK_LEN as usize];
    core.raw_read_range(STAGING, -1, &mut captured);
    let live_extras = extras(core);
    // (the battle's flags go on to the other front, [`arrive`])
    let mut flags = [0u8; LOCAL_LEN as usize];
    core.raw_read_range(LOCAL_FLAGS, -1, &mut flags);
    core.raw_write_range(LOCAL_KEEP, -1, &flags);
    let mut stored = vec![0u8; STORE_LEN as usize];
    core.raw_read_range(STORE, -1, &mut stored);
    let to = 1 - live;
    // The second front starts afresh at its first round (whatever was set
    // up to be looked at before), and when it is looked at before any.
    let started = core.raw_read_8(STARTED, -1) == 1;
    let fresh = to == 1 && (core.raw_read_8(STORE_KIND, -1) == 0 || (!started && kind != Stub::BeginViewIn as u32));
    if to == 1 && kind == Stub::BeginRound as u32 {
        core.raw_write_8(STARTED, -1, 1);
    }
    core.raw_write_range(STORE, -1, &captured);
    core.raw_write_range(EXTRA, -1, &live_extras);
    core.raw_write_8(STORE_KIND, -1, 1);
    core.raw_write_8(LIVE, -1, to);
    sync_header(core, b);
    if fresh {
        core.raw_write_8(FRESH, -1, 1);
        setup_second(core, b);
    } else {
        core.raw_write_8(FRESH, -1, 0);
        core.raw_write_range(STAGING, -1, &stored[..BLOCK_LEN as usize]);
        // (applied once the front is restored, [`arrive`])
        core.raw_write_range(EXTRA_PENDING, -1, &stored[BLOCK_LEN as usize..]);
        if kind == Stub::BeginViewIn as u32 {
            core.raw_write_32(CHECK, -1, checksum(core, STAGING, BLOCK_LEN));
        }
    }
}

/// The incoming front's own tangoAW2 state, between the stash and the
/// restore (in the staging buffer's tail, which the block does not use).
const EXTRA_PENDING: u32 = STAGING + 0x1F90;

/// gPlaySt for the second front's first round: its armies' colours, teams,
/// COs (the player's pick), who directs them and its rules.
fn setup_second(core: &mut Core, b: &Battle) {
    let Some(h) = header(core, b.fronts.second) else { return };
    let Some(info) = crate::ds_campaign::campaign(core).and_then(|c| c.model.built.missions.get(b.fronts.second as usize)) else { return };
    for k in 0..4u32 {
        let present = k < info.armies as u32;
        core.raw_write_8(COLOURS + 1 + k, -1, h[0x40 + k as usize]);
        core.raw_write_8(TEAMS + 1 + k, -1, h[0x44 + k as usize]);
        let co = core.raw_read_8(SECOND_COS + k, -1);
        core.raw_write_8(COS + 1 + k, -1, if present { co } else { 0 });
        let ctl = if present { second_controller(core, b, k + 1) } else { 0 };
        core.raw_write_8(CONTROLLERS + 1 + k, -1, ctl);
    }
    core.raw_write_8(TURN_LIMIT, -1, 0);
    core.raw_write_8(POWERS_ON, -1, b.fronts.powers as u8);
}

/// The front has been restored (or set up): its own tangoAW2 state, its
/// controllers and the units sent to it.
fn arrive(core: &mut Core, b: &Battle) {
    // Its look (before the map's graphics are loaded, the swap's next
    // steps): each front its own (crate::ds_campaign::set_look).
    if let Some(m) = live_info(core) {
        crate::ds_campaign::set_look(core, m);
    }
    // The battle's flags, shared by both fronts.
    let mut flags = [0u8; LOCAL_LEN as usize];
    core.raw_read_range(LOCAL_KEEP, -1, &mut flags);
    core.raw_write_range(LOCAL_FLAGS, -1, &flags);
    if core.raw_read_8(FRESH, -1) == 1 {
        // (set up afresh: no tangoAW2 state of its own yet; its armies as
        // the description says)
        let mut e = vec![0u8; EXTRA_LEN as usize];
        e[EX_STUN as usize..].copy_from_slice(&vec![0u8; crate::co_powers::STUN_STATE_LEN]);
        set_extras(core, &e);
        controllers(core, b);
        // The units sent before it started (kept for its first round when
        // it is only looked at).
        let n = core.raw_read_8(QUEUED, -1) as u32;
        for k in 0..n.min(QUEUE_LEN) {
            let mut u = [0u8; 12];
            core.raw_read_range(QUEUE + 12 * k, -1, &mut u);
            let army = core.raw_read_8(QUEUE_ARMY + k, -1) as u32;
            place_live(core, army, &u);
        }
        if core.raw_read_8(BUSY, -1) != Stub::BeginViewIn as u8 {
            core.raw_write_8(QUEUED, -1, 0);
        }
    } else {
        let mut e = vec![0u8; EXTRA_LEN as usize];
        core.raw_read_range(EXTRA_PENDING, -1, &mut e);
        set_extras(core, &e);
        // Who plays each army's turns here now (Auto CO may have changed
        // since this front's last round).
        set_controllers(core, b);
    }
}

/// The army's owner: its controller on the main front (1 a human, local or
/// a netplay peer's by its seat; 2 the computer), read from the main front's
/// block in the store while the second front is on the screen.
fn owner(core: &Core, army: u32) -> u8 {
    let at = if core.raw_read_8(LIVE, -1) == 1 && core.raw_read_8(STORE_KIND, -1) == 1 {
        STORE + B_PLAYERS + PLAYER * army + 0x1B
    } else {
        players(core) + PLAYER * army + 0x1B
    };
    match core.raw_read_8(at, -1) {
        0 => 2,
        c => c,
    }
}

/// The army's Auto CO is on (bit clear in [`MANUAL`]).
fn auto_co(core: &Core, army: u32) -> bool {
    (1..=4).contains(&army) && core.raw_read_8(MANUAL, -1) & (1 << (army - 1)) == 0
}

/// The army whose Intel > Auto CO the menu shows now: the current army, on
/// its main-front turn, while the second front is fought, when the
/// description gives it the choice.
fn auto_co_army(core: &Core, b: &Battle) -> Option<u32> {
    if core.raw_read_8(LIVE, -1) != 0 || core.raw_read_8(SECOND, -1) != SECOND_ON || core.raw_read_8(BUSY, -1) != 0 {
        return None;
    }
    let army = core.raw_read_16(CURRENT_ARMY, -1) as u32;
    (1..=4).contains(&army).then_some(army).filter(|&a| matches!(b.fronts.control[a as usize - 1], FrontControl::AutoCo { .. }))
}

/// Who plays `army`'s turns on the second front (its controller byte
/// there): by the description's [`FrontControl`] and the army's owner.
fn second_controller(core: &Core, b: &Battle, army: u32) -> u8 {
    match b.fronts.control[army as usize - 1] {
        FrontControl::Cpu => 2,
        FrontControl::Owner => owner(core, army),
        FrontControl::AutoCo { .. } if auto_co(core, army) => 2,
        FrontControl::AutoCo { .. } => owner(core, army),
    }
}

/// The second front's armies' controllers, by [`second_controller`] (in
/// its players and gPlaySt), whenever it comes on the screen.
fn set_controllers(core: &mut Core, b: &Battle) {
    if core.raw_read_8(LIVE, -1) != 1 {
        return;
    }
    let p = players(core);
    for a in 1..=4u32 {
        let at = p + PLAYER * a + 0x1B;
        if core.raw_read_8(at, -1) == 0 {
            continue;
        }
        let ctl = second_controller(core, b, a);
        if core.raw_read_8(at, -1) != ctl {
            core.raw_write_8(at, -1, ctl);
        }
        if core.raw_read_8(CONTROLLERS + a, -1) != ctl {
            core.raw_write_8(CONTROLLERS + a, -1, ctl);
        }
    }
}

/// The second front's armies when it starts: their controllers (AW2's
/// campaign start would make every army not Black Hole's the player's), no
/// CO skills yet, the front's own colours and teams.
fn controllers(core: &mut Core, b: &Battle) {
    if core.raw_read_8(LIVE, -1) != 1 {
        return;
    }
    set_controllers(core, b);
    let p = players(core);
    let h = header(core, b.fronts.second);
    for a in 1..=4u32 {
        if core.raw_read_8(p + PLAYER * a + 0x1B, -1) == 0 {
            continue;
        }
        crate::co_skills::set(core, a, &[]);
        // Its colours and teams: the front's own (the campaign's start would
        // colour an army by its CO's country).
        if let Some(h) = h {
            core.raw_write_8(p + PLAYER * a + 0x1A, -1, h[0x40 + a as usize - 1]);
            core.raw_write_8(p + PLAYER * a + 0x2A, -1, h[0x44 + a as usize - 1]);
        }
    }
}

/// At a map's start (crate::ds_campaign::map_start, after its own): the
/// second front's armies' controllers.
pub fn map_start(core: &mut Core) {
    let Some(b) = battle(core) else { return };
    if core.raw_read_8(BUSY, -1) == 0 {
        // A battle's start (its main front): no second front yet.
        reset(core);
        return;
    }
    controllers(core, &b);
}

// --- Send ----------------------------------------------------------------------------

const SELECTED: u32 = 0x0300_40D8;
/// Where the moving unit ends up (x, y u16).
const DESTINATION: u32 = 0x0300_3100;
/// The command menu's Wait (the move ends as usual).
const WAIT_CHOSEN: u32 = 0x0802_CFFC;

/// The units a front may take: 50 per army.
const ARMY_UNITS: u32 = 50;

fn send_kind_ok(core: &Core, b: &Battle, unit: u32, x: u32, y: u32) -> bool {
    let t = core.raw_read_8(unit, -1);
    if core.raw_read_8(unit + 7, -1) != 0 || core.raw_read_8(unit + 8, -1) != 0 {
        return false;
    }
    match b.fronts.send {
        SendRule::None => false,
        SendRule::Air => crate::ds_campaign_data::SKY_UNITS.contains(&t),
        SendRule::Ground => {
            let army = core.raw_read_16(CURRENT_ARMY, -1) as u32;
            let row = core.raw_read_16(MAP + 0x417A + 2 * y, -1) as u32;
            let c = core.raw_read_8(MAP + 0x1432 + row + x, -1);
            // HQ, base, airport, port of the army.
            matches!(c & 0x1F, 0x08 | 0x0E | 0x0A | 0x0B) && (c >> 5) as u32 == army
        }
    }
}

/// The second front's units of `army` now (its live state or its store).
fn second_units(core: &Core, b: &Battle, army: u32) -> u32 {
    if core.raw_read_8(STARTED, -1) == 1 && core.raw_read_8(STORE_KIND, -1) == 1 {
        (0..SLOTS).filter(|&j| core.raw_read_8(STORE + B_UNITS + 12 * ((army - 1) * SLOTS + j), -1) != 0).count() as u32
    } else {
        let queued = (0..core.raw_read_8(QUEUED, -1) as u32).filter(|&k| core.raw_read_8(QUEUE_ARMY + k, -1) as u32 == army).count();
        (deployed(core, b.fronts.second, army).len() + queued) as u32
    }
}

/// Send's test: shown for a unit the second front takes (by the rule),
/// greyed while the army's units there are as many as it may have; hidden
/// otherwise.
fn send_usable(core: &Core, b: &Battle) -> u32 {
    if core.raw_read_8(LIVE, -1) != 0 || core.raw_read_8(SECOND, -1) != SECOND_ON || core.raw_read_8(BUSY, -1) != 0 {
        return 1;
    }
    let u = core.raw_read_32(SELECTED, -1);
    if !(0x0200_0000..0x0204_0000).contains(&u) {
        return 1;
    }
    let (x, y) = (core.raw_read_16(DESTINATION, -1) as u32, core.raw_read_16(DESTINATION + 2, -1) as u32);
    if !send_kind_ok(core, b, u, x, y) {
        return 1;
    }
    let army = core.raw_read_16(CURRENT_ARMY, -1) as u32;
    let full = second_units(core, b, army) >= ARMY_UNITS
        || (core.raw_read_8(STARTED, -1) != 1 && core.raw_read_8(QUEUED, -1) as u32 >= QUEUE_LEN);
    if full {
        2
    } else {
        0
    }
}

/// Send: the unit ends its move (the game's Wait), then leaves for the
/// second front ([`after_action`]).
fn send_chosen(core: &mut Core, b: &Battle) {
    // (a stub's r2 is its own: the item's flags are worked out again)
    if send_usable(core, b) != 0 {
        return return_to(core, 0);
    }
    let u = core.raw_read_32(SELECTED, -1);
    core.raw_write_32(SENDING, -1, u);
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(2, 0);
    cpu.set_thumb_pc(WAIT_CHOSEN)
}

/// `RunMapEventsAfterUnitAction` (the move over): a unit sent leaves the
/// main front and joins the second front's state.
fn after_action(core: &mut Core) {
    let u = core.raw_read_32(SENDING, -1);
    if u == 0 {
        return;
    }
    core.raw_write_32(SENDING, -1, 0);
    if on(core).is_none() || core.raw_read_8(u, -1) == 0 {
        return;
    }
    let mut rec = [0u8; 12];
    core.raw_read_range(u, -1, &mut rec);
    let base = core.raw_read_32(UNITS_PTR, -1);
    let id = (u - base) / 12;
    let army = id / 64 + 1;
    rec[1] = 0;
    // Into the second front: its store (placed now), or the queue (placed
    // when it starts).
    let placed = if core.raw_read_8(STARTED, -1) == 1 && core.raw_read_8(STORE_KIND, -1) == 1 {
        place_stored(core, army, &rec)
    } else {
        let n = core.raw_read_8(QUEUED, -1) as u32;
        if n < QUEUE_LEN {
            core.raw_write_range(QUEUE + 12 * n, -1, &rec);
            core.raw_write_8(QUEUE_ARMY + n, -1, army as u8);
            core.raw_write_8(QUEUED, -1, (n + 1) as u8);
            true
        } else {
            false
        }
    };
    if !placed {
        return;
    }
    // Off the main front.
    let (x, y) = (rec[2] as u32, rec[3] as u32);
    core.raw_write_range(u, -1, &[0u8; 12]);
    let row = core.raw_read_16(MAP + 0x417A + 2 * y, -1) as u32;
    if core.raw_read_8(MAP + 0x12 + row + x, -1) as u32 == id {
        core.raw_write_8(MAP + 0x12 + row + x, -1, 0);
    }
    // The unit layers redrawn: the trapped function runs after the call
    // (`RebuildMapUnitLayers2`, which returns to it).
    let cpu = core.gba_mut().cpu_mut();
    let lr = cpu.gpr(14);
    let r0 = cpu.gpr(0);
    core.raw_write_32(AFTER_LR, -1, lr as u32);
    core.raw_write_32(AFTER_R0, -1, r0 as u32);
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(14, (AFTER_BACK | 1) as i32);
    cpu.set_thumb_pc(REBUILD_UNITS);
}

/// Where the unit layers' rebuild returns to after a Send, trapped: the
/// alignment padding in `UnitSelectedEvent_Init` (`0x08074384`, after its
/// `bx`, before its literal pool), which the game never runs. From there
/// the trapped `RunMapEventsAfterUnitAction` runs on with its arguments.
const AFTER_BACK: u32 = 0x0807_43AA;
fn after_back(core: &mut Core) {
    let (lr, r0) = (core.raw_read_32(AFTER_LR, -1), core.raw_read_32(AFTER_R0, -1));
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, r0 as i32);
    cpu.set_gpr(14, lr as i32);
    cpu.set_thumb_pc(AFTER_ACTION);
}

/// A map to place a unit on: size, tiles, terrain classes, occupied cells.
struct Ground {
    w: u32,
    h: u32,
    class: Vec<u8>,
    taken: Vec<bool>,
}

impl Ground {
    fn cost(&self, core: &Core, t: u8, x: u32, y: u32) -> u8 {
        let cos = core.raw_read_32(0x0801_F8E4, -1);
        let chart = core.raw_read_32(cos + 0x50 + 0x104, -1);
        let movement = core.raw_read_8(crate::roster::table(core) + 0x5C * t as u32 + 0x19, -1) as u32;
        core.raw_read_8(chart + 32 * movement + (self.class[(y * self.w + x) as usize] & 0x1F) as u32, -1)
    }

    /// The free cell nearest `from` a unit of type `t` can stand on
    /// (rings outward; ties: the first in reading order).
    fn near(&self, core: &Core, t: u8, from: (u32, u32)) -> Option<(u32, u32)> {
        let (fx, fy) = (from.0 as i32, from.1 as i32);
        for r in 0..(self.w + self.h) as i32 {
            for y in (fy - r).max(0)..=(fy + r).min(self.h as i32 - 1) {
                for x in (fx - r).max(0)..=(fx + r).min(self.w as i32 - 1) {
                    if (x - fx).abs() + (y - fy).abs() != r {
                        continue;
                    }
                    let (ux, uy) = (x as u32, y as u32);
                    if !self.taken[(uy * self.w + ux) as usize] && self.cost(core, t, ux, uy) != 0xFF && self.cost(core, t, ux, uy) != 0 {
                        return Some((ux, uy));
                    }
                }
            }
        }
        None
    }

    /// Where an army's units arrive: its HQ, else its first unit's cell.
    fn home(&self, army: u32, units: &[(u32, u32)]) -> (u32, u32) {
        for y in 0..self.h {
            for x in 0..self.w {
                let c = self.class[(y * self.w + x) as usize];
                if c & 0x1F == 0x08 && (c >> 5) as u32 == army {
                    return (x, y);
                }
            }
        }
        units.first().copied().unwrap_or((0, 0))
    }
}

fn class_of(core: &Core, tile: u16) -> u8 {
    core.raw_read_8(core.raw_read_32(CLASS_TABLE_PTR, -1) + tile as u32, -1)
}

/// The live front's map.
fn live_ground(core: &Core) -> Ground {
    let (w, h) = (core.raw_read_16(MAP, -1) as u32, core.raw_read_16(MAP + 2, -1) as u32);
    let mut class = Vec::new();
    let mut taken = Vec::new();
    for y in 0..h {
        let row = core.raw_read_16(MAP + 0x417A + 2 * y, -1) as u32;
        for x in 0..w {
            class.push(core.raw_read_8(MAP + 0x1432 + row + x, -1));
            taken.push(core.raw_read_8(MAP + 0x12 + row + x, -1) != 0);
        }
    }
    Ground { w, h, class, taken }
}

/// A unit of `army` (its 12-byte record) placed on the live front.
fn place_live(core: &mut Core, army: u32, rec: &[u8; 12]) {
    if !(1..=4).contains(&army) {
        return;
    }
    let g = live_ground(core);
    let mine: Vec<(u32, u32)> = live_units(core, army).iter().map(|&(_, u)| (core.raw_read_8(u + 2, -1) as u32, core.raw_read_8(u + 3, -1) as u32)).collect();
    let Some((x, y)) = g.near(core, rec[0], g.home(army, &mine)) else { return };
    let base = core.raw_read_32(UNITS_PTR, -1);
    let Some(j) = (1..SLOTS).find(|&j| core.raw_read_8(base + 12 * ((army - 1) * 64 + j), -1) == 0) else { return };
    let mut r = *rec;
    r[2] = x as u8;
    r[3] = y as u8;
    let at = base + 12 * ((army - 1) * 64 + j);
    core.raw_write_range(at, -1, &r);
    let row = core.raw_read_16(MAP + 0x417A + 2 * y, -1) as u32;
    core.raw_write_8(MAP + 0x12 + row + x, -1, ((army - 1) * 64 + j) as u8);
}

/// A unit placed in the stored second front (its block: the map from its
/// header with the block's changed tiles, the block's units).
fn place_stored(core: &mut Core, army: u32, rec: &[u8; 12]) -> bool {
    let Some(b) = battle(core) else { return false };
    let Some(h) = header(core, b.fronts.second) else { return false };
    let map = u32::from_le_bytes(h[0..4].try_into().unwrap());
    let size = core.raw_read_32(map, -1) >> 8;
    let mut lz = vec![0u8; (size as usize) * 2 + 64];
    core.raw_read_range(map, -1, &mut lz);
    let Some(raw) = crate::ds_art::lz10(&lz) else { return false };
    let (w, hh) = (raw[0] as u32, raw[1] as u32);
    let mut tiles: Vec<u16> = (0..(w * hh) as usize).map(|i| u16::from_le_bytes([raw[2 + 2 * i], raw[3 + 2 * i]])).collect();
    let mut p = STORE + B_TILES;
    while p + 4 <= STORE + B_TILES_END {
        let t = core.raw_read_16(p + 2, -1);
        if t == 0xFFFF {
            break;
        }
        let (x, y) = (core.raw_read_8(p, -1) as u32, core.raw_read_8(p + 1, -1) as u32);
        if x < w && y < hh {
            tiles[(y * w + x) as usize] = t;
        }
        p += 4;
    }
    let class: Vec<u8> = tiles.iter().map(|&t| class_of(core, t)).collect();
    let mut taken = vec![false; (w * hh) as usize];
    let mut mine = Vec::new();
    let mut free = None;
    for a in 1..=4u32 {
        for j in 0..SLOTS {
            let at = STORE + B_UNITS + 12 * ((a - 1) * SLOTS + j);
            if core.raw_read_8(at, -1) == 0 {
                if a == army && j > 0 && free.is_none() {
                    free = Some(at);
                }
                continue;
            }
            let (x, y) = (core.raw_read_8(at + 2, -1) as u32, core.raw_read_8(at + 3, -1) as u32);
            // (a loaded unit sits with its transport)
            if x < w && y < hh && core.raw_read_8(at + 1, -1) & 0x08 == 0 {
                taken[(y * w + x) as usize] = true;
            }
            if a == army {
                mine.push((x, y));
            }
        }
    }
    let Some(free) = free else { return false };
    let g = Ground { w, h: hh, class, taken };
    // (the army's home there: from its first deployed unit when it has no HQ)
    let home = g.home(army, &deployed(core, b.fronts.second, army).into_iter().chain(mine).collect::<Vec<_>>());
    let Some((x, y)) = g.near(core, rec[0], home) else { return false };
    let mut r = *rec;
    r[2] = x as u8;
    r[3] = y as u8;
    core.raw_write_range(free, -1, &r);
    let _ = (B_DAY, B_ARMY, B_PLAY_ST, B_PLAYERS, B_INVENTIONS, INVENTIONS);
    true
}

/// An army's deployed cells on a front (its header's deployment).
fn deployed(core: &Core, front: u8, army: u32) -> Vec<(u32, u32)> {
    let Some(h) = header(core, front) else { return Vec::new() };
    let mut p = u32::from_le_bytes(h[0x34..0x38].try_into().unwrap());
    let mut out = Vec::new();
    let mut cur = 0u32;
    for _ in 0..400 {
        let mut r = [0u8; 12];
        core.raw_read_range(p, -1, &mut r);
        p += 12;
        match r[0] {
            0xFF => break,
            0xFE => cur = r[1] as u32,
            _ if cur == army => out.push((r[0] as u32, r[1] as u32)),
            _ => {}
        }
    }
    out
}

// --- On the screen: the help line, the front, the result ------------------------------

struct Sprites {
    at: u32,
    end: u32,
}

impl Sprites {
    fn put_shaped(&mut self, core: &mut Core, x: i32, y: i32, tile: u16, shape: u16) {
        if self.at + 8 > self.end || !(-8..240).contains(&x) || !(-16..160).contains(&y) {
            return;
        }
        core.raw_write_16(self.at, -1, (y as u16 & 0xFF) | shape);
        core.raw_write_16(self.at + 2, -1, x as u16 & 0x1FF);
        core.raw_write_16(self.at + 4, -1, tile);
        self.at += 8;
    }
}

/// The help line under the map menu while Front is highlighted, and the
/// view's and the result's lines: in a window of AW2's own (its map menu's
/// window tiles, on BG2) in AW2's proportional font ([`panel_tick`],
/// [`flush_sprites`]).
pub const FRONT_HELP: &str = "View the other front.";
pub const VIEW_TITLE: &str = "Second front";
pub const VIEW_MAIN_TITLE: &str = "Main front";
pub const VIEW_BACK: &str = "Back";
pub const RESULT_WON: &str = "Second front won!";
pub const RESULT_LOST: &str = "Second front lost.";
/// Auto CO's help lines (Dual Strike's, bank 0xC0 texts 740, 741): the
/// setting as it is.
pub const AUTO_ON_HELP: &str = "Allow CPU to direct the secondary front.";
pub const AUTO_OFF_HELP: &str = "Direct the secondary front manually.";

// --- Panels: AW2's window on BG2, AW2's font in sprites --------------------------------

/// What a panel shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
enum Panel {
    Help = 1,
    View = 2,
    Result = 3,
    /// Deploy's help line in the Setup phase (crate::setup_phase).
    SetupHelp = 4,
    /// Auto CO's help line (the Intel menu).
    AutoHelp = 5,
}

impl Panel {
    /// The window, in BG2 cells: (x, y, width, height).
    fn rect(self) -> (u32, u32, u32, u32) {
        match self {
            Panel::Help | Panel::SetupHelp | Panel::AutoHelp => (0, 16, 30, 4),
            // (at the top: neither the CO panel nor the terrain and unit
            // panels are drawn while the other front is looked at,
            // [`co_panel`], [`info_panels`])
            Panel::View => (8, 0, 14, 6),
            Panel::Result => (5, 8, 20, 4),
        }
    }

    /// Its lines: (x, y in pixels, text, with the B button before it).
    fn lines(self, core: &Core) -> Vec<(i32, i32, &'static str, bool)> {
        let (x, y, w, _) = self.rect();
        let (px, py) = (8 * x as i32, 8 * y as i32);
        match self {
            Panel::Help => vec![(px + 12, py + 9, FRONT_HELP, false)],
            Panel::SetupHelp => vec![(px + 12, py + 9, crate::setup_phase::DEPLOY_HELP, false)],
            Panel::AutoHelp => {
                let army = core.raw_read_16(CURRENT_ARMY, -1) as u32;
                let t = if auto_co(core, army) { AUTO_ON_HELP } else { AUTO_OFF_HELP };
                vec![(px + 12, py + 9, t, false)]
            }
            Panel::View => {
                // (the front looked at: the main one from a second-front turn)
                let t = if core.raw_read_8(LIVE, -1) == 0 { VIEW_MAIN_TITLE } else { VIEW_TITLE };
                vec![(px + 10, py + 9, t, false), (px + 10, py + 25, VIEW_BACK, true)]
            }
            Panel::Result => {
                let t = if core.raw_read_8(SECOND, -1) == SECOND_WON { RESULT_WON } else { RESULT_LOST };
                let tw = font_width(core, t) as i32;
                vec![(px + (8 * w as i32 - tw) / 2, py + 9, t, false)]
            }
        }
    }
}

/// The panel wanted now.
fn want_panel(core: &Core) -> Option<Panel> {
    if core.raw_read_32(MAIN_CALLBACK, -1) != MAP_CALLBACK {
        return None;
    }
    if menu_cursor(core, MAP_MENU_COPY) == Some(OPTIONS_AT + 1) {
        return Some(Panel::Help);
    }
    if matches!(menu_cursor(core, INTEL_MENU_COPY), Some(AUTO_ON_AT | AUTO_OFF_AT)) {
        return Some(Panel::AutoHelp);
    }
    match menu_cursor(core, crate::setup_phase::MENU) {
        Some(crate::setup_phase::FRONT_AT) => return Some(Panel::Help),
        Some(crate::setup_phase::DEPLOY_AT) => return Some(Panel::SetupHelp),
        _ => {}
    }
    let busy = core.raw_read_8(BUSY, -1);
    if core.raw_read_8(VIEW, -1) == 1 && busy == Stub::BeginViewIn as u8 && core.raw_read_8(MAP_LOCK, -1) == 0 {
        return Some(Panel::View);
    }
    if core.raw_read_8(BANNER, -1) > 0 && busy == 0 && core.raw_read_16(MAP_STATE, -1) == STATE_CURSOR {
        return Some(Panel::Result);
    }
    None
}

/// The panel drawn (its [`Panel`] value, 0 none), and where on BG2's
/// screen (its first column and row: BG2 scrolls with the map).
const PANEL: u32 = STATE + 0x18;
const PANEL_AT: u32 = STATE + 0x19;
/// BG2's cells under it, as they were (6 rows of 32 at most), in the staging
/// buffer's tail.
const PANEL_SAVED: u32 = STAGING + 0x1D80;
const PANEL_ROWS: u32 = 6;
const BG2_BUFFER_PTR: u32 = 0x0849_9580;
const BG2_SCREEN: u32 = 0x0600_7800;
const BG2HOFS: u32 = 0x0400_0018;
const BG2VOFS: u32 = 0x0400_001A;
const MAP_LOCK: u32 = 0x0300_40E8;
/// AW2's window (its map menu's, BG palette 8): corners, edges, the fill
/// (its two shades by row), the blank cell.
const WIN_TL: u16 = 0x8361;
const WIN_T: u16 = 0x8362;
const WIN_TR: u16 = 0x8363;
const WIN_L: u16 = 0x8364;
const WIN_FILL: u16 = 0x8365;
const WIN_R: u16 = 0x8366;
const WIN_BL: u16 = 0x8367;
const WIN_B: u16 = 0x8368;
const WIN_BR: u16 = 0x8369;
const WIN_FILL2: u16 = 0x836A;

fn window_cell(p: Panel, cx: u32, cy: u32) -> u16 {
    let (_, _, w, h) = p.rect();
    let (l, r, t, b) = (cx == 0, cx + 1 == w, cy == 0, cy + 1 == h);
    match (t, b, l, r) {
        (true, _, true, _) => WIN_TL,
        (true, _, _, true) => WIN_TR,
        (true, _, _, _) => WIN_T,
        (_, true, true, _) => WIN_BL,
        (_, true, _, true) => WIN_BR,
        (_, true, _, _) => WIN_B,
        (_, _, true, _) => WIN_L,
        (_, _, _, true) => WIN_R,
        _ if cy % 2 == 1 => WIN_FILL,
        _ => WIN_FILL2,
    }
}

fn bg2_cells(core: &mut Core, at: u32, v: u16) {
    let buffer = core.raw_read_32(BG2_BUFFER_PTR, -1);
    for base in [buffer, BG2_SCREEN] {
        if core.raw_read_16(base + 2 * at, -1) != v {
            core.raw_write_16(base + 2 * at, -1, v);
        }
    }
}

/// Every frame of a two-front battle: the panel wanted drawn (BG2's cells
/// under it kept, put back when it goes). BG2 scrolls with the map, so the
/// window goes where the screen shows it: at the scroll's cell (none while
/// the map moves between cells), moved when the scroll moves.
fn panel_tick(core: &mut Core) {
    let want = want_panel(core);
    let now = core.raw_read_8(PANEL, -1);
    let buffer = core.raw_read_32(BG2_BUFFER_PTR, -1);
    if !(0x0200_0000..0x0204_0000).contains(&buffer) {
        return;
    }
    let (hofs, vofs) = (core.raw_read_16(BG2HOFS, -1) as u32 & 0x1FF, core.raw_read_16(BG2VOFS, -1) as u32 & 0x1FF);
    let aligned = hofs % 8 == 0 && vofs % 8 == 0;
    let at = ((hofs / 8) % 32, (vofs / 8) % 32);
    let drawn = [Panel::Help, Panel::View, Panel::Result, Panel::SetupHelp, Panel::AutoHelp].into_iter().find(|p| *p as u8 == now);
    let drawn_at = (core.raw_read_8(PANEL_AT, -1) as u32, core.raw_read_8(PANEL_AT + 1, -1) as u32);
    let keep = drawn.is_some() && drawn == want && aligned && drawn_at == at;
    if let Some(d) = drawn.filter(|_| !keep) {
        // Put back the rows it covered.
        let (_, y, _, h) = d.rect();
        let mut rows = vec![0u8; (64 * h) as usize];
        core.raw_read_range(PANEL_SAVED, -1, &mut rows);
        for r in 0..h {
            let row = (drawn_at.1 + y + r) % 32;
            for k in 0..32 {
                let i = (64 * r + 2 * k) as usize;
                bg2_cells(core, 32 * row + k, u16::from_le_bytes([rows[i], rows[i + 1]]));
            }
        }
        core.raw_write_8(PANEL, -1, 0);
    }
    let Some(p) = want.filter(|_| aligned) else { return };
    let (x, y, w, h) = p.rect();
    debug_assert!(h <= PANEL_ROWS);
    if core.raw_read_8(PANEL, -1) != p as u8 {
        let mut rows = vec![0u8; (64 * h) as usize];
        for r in 0..h {
            let row = (at.1 + y + r) % 32;
            core.raw_read_range(buffer + 64 * row, -1, &mut rows[(64 * r) as usize..(64 * r + 64) as usize]);
        }
        core.raw_write_range(PANEL_SAVED, -1, &rows);
        core.raw_write_8(PANEL, -1, p as u8);
        core.raw_write_8(PANEL_AT, -1, at.0 as u8);
        core.raw_write_8(PANEL_AT + 1, -1, at.1 as u8);
    }
    for cy in 0..h {
        for cx in 0..w {
            let (col, row) = ((at.0 + x + cx) % 32, (at.1 + y + cy) % 32);
            bg2_cells(core, 32 * row + col, window_cell(p, cx, cy));
        }
    }
}

/// Every frame outside a two-front battle (crate::setup_phase): the Setup
/// phase's help line drawn, and taken away after it.
pub fn panels_outside(core: &mut Core) {
    if on(core).is_none() && (crate::setup_phase::active(core) || core.raw_read_8(PANEL, -1) != 0) {
        panel_tick(core);
    }
}

/// `DrawArmyCoPanel` (`0x080436DC`, crate::tag's trap calls this first):
/// while the other front is looked at, its army's panel is not drawn (its
/// face's tiles are the turn's army's): the panel goes below the screen.
pub fn co_panel(core: &mut Core) {
    if core.raw_read_8(VIEW, -1) == 1 && on(core).is_some() {
        core.gba_mut().cpu_mut().set_gpr(1, 200);
    }
}

/// The terrain and unit panels' frame function (`0x0802AA78`, the proc
/// `0x0802A7C4` starts), where both panels' height on the screen is settled
/// (`[sp + 0x10]`, after its slide): while the other front is looked at it
/// is below the screen, so neither is seen (the view's cursor is not the
/// turn's, and the panels would sit on the front's units at its edge).
const INFO_PANELS_Y: u32 = 0x0802_AB34;
fn info_panels(core: &mut Core) {
    if core.raw_read_8(VIEW, -1) == 1 && on(core).is_some() {
        let sp = core.gba().cpu().gpr(13) as u32;
        core.raw_write_32(sp + 0x10, -1, 200);
    }
}

/// AW2's proportional font (the CO screen's and the menus' text): per
/// character a pointer to its 16 rows of 4-bit pixels and its width.
const FONT_GLYPHS: u32 = 0x084C_32E4;
const FONT_WIDTHS: u32 = 0x084C_36E4;
/// The rows of a glyph a line shows (of 16), and its pixels: ink and shade.
const FONT_TOP: usize = 2;
const FONT_ROWS: usize = 14;
/// OBJ palette 0's colours (the battle map's): black ink, grey shade, a dark
/// button, white.
const INK: u8 = 14;
const SHADE: u8 = 11;
const BUTTON: u8 = 3;
const WHITE_PX: u8 = 6;

fn font_width(core: &Core, s: &str) -> u32 {
    s.bytes().map(|c| core.raw_read_8(FONT_WIDTHS + c as u32, -1) as u32 + 1).sum::<u32>().saturating_sub(1)
}

/// A line drawn into a 16-pixel-high strip: its columns of pixels.
fn render_line(core: &Core, s: &str, button: bool) -> Vec<[u8; 16]> {
    let mut cols: Vec<[u8; 16]> = Vec::new();
    if button {
        // The B button: a dark disc, its letter in white (AW2's font).
        let mut disc = vec![[0u8; 16]; 13];
        for (cx, col) in disc.iter_mut().enumerate().take(12) {
            for (cy, px) in col.iter_mut().enumerate().skip(2).take(12) {
                let (dx, dy) = (cx as i32 * 2 - 11, cy as i32 * 2 - 15);
                if dx * dx + dy * dy <= 121 {
                    *px = BUTTON;
                }
            }
        }
        let w = core.raw_read_8(FONT_WIDTHS + b'B' as u32, -1) as usize;
        let at = core.raw_read_32(FONT_GLYPHS + 4 * b'B' as u32, -1);
        if (0x0800_0000..0x0A00_0000).contains(&at) {
            let stride = w.div_ceil(2);
            let ox = (12 - w) / 2;
            for r in 0..16usize {
                for cx in 0..w {
                    let b = core.raw_read_8(at + (stride * r + cx / 2) as u32, -1);
                    if (b >> (4 * (cx & 1))) & 15 == 0xA {
                        disc[ox + cx][r] = WHITE_PX;
                    }
                }
            }
        }
        cols.extend(disc);
        cols.extend([[0u8; 16]; 3]);
    }
    for c in s.bytes() {
        let w = core.raw_read_8(FONT_WIDTHS + c as u32, -1) as usize;
        let at = core.raw_read_32(FONT_GLYPHS + 4 * c as u32, -1);
        let stride = w.div_ceil(2);
        for cx in 0..w {
            let mut col = [0u8; 16];
            if (0x0800_0000..0x0A00_0000).contains(&at) {
                for r in 0..FONT_ROWS {
                    let b = core.raw_read_8(at + (stride * (FONT_TOP + r) + cx / 2) as u32, -1);
                    col[r + 1] = match (b >> (4 * (cx & 1))) & 15 {
                        0 => 0,
                        0xA => INK,
                        _ => SHADE,
                    };
                }
            }
            cols.push(col);
        }
        cols.push([0u8; 16]);
    }
    cols
}

/// A line in white, outlined in black (on the map itself).
fn outlined(core: &Core, s: &str) -> Vec<[u8; 16]> {
    let mut on = vec![[false; 16]; 1];
    for col in render_line(core, s, false) {
        on.push(col.map(|v| v != 0));
    }
    on.push([false; 16]);
    let mut out = vec![[0u8; 16]; on.len()];
    for x in 0..on.len() {
        for y in 0..16 {
            if on[x][y] {
                out[x][y] = WHITE_PX;
            } else {
                let near = [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)].iter().any(|&(dx, dy)| {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    nx >= 0 && (nx as usize) < on.len() && (0..16).contains(&ny) && on[nx as usize][ny as usize]
                });
                if near {
                    out[x][y] = INK;
                }
            }
        }
    }
    out
}

/// OBJ tiles free on the battle map (crate::heal_effect's, which it uses
/// only while a structure's heal plays at a turn's start): pairs of tiles,
/// each an 8x16 sprite.
fn free_tile_pairs() -> Vec<u16> {
    const RUNS: [(u16, u16); 7] = [(0x1F9, 17), (0x2D2, 9), (0x2E4, 4), (0x2EC, 4), (0x2F4, 4), (0x2FC, 4), (0x309, 9)];
    RUNS.iter().flat_map(|&(t, n)| (0..n / 2).map(move |k| t + 2 * k)).collect()
}

const OBJ_TILES: u32 = 0x0601_0000;

/// The menu showing our table, and the entry its cursor is on.
fn menu_cursor(core: &Core, table: u32) -> Option<u32> {
    (0..30).find_map(|k| {
        let s = SLOT_ARRAY + SLOT_SIZE * k;
        if core.raw_read_32(s, -1) != MENU_SCRIPT || core.raw_read_32(s + 0x20, -1) != table {
            return None;
        }
        let cursor = core.raw_read_8(s + 0x42, -1) as u32;
        Some(core.raw_read_8(s + 0x31 + cursor, -1) as u32)
    })
}

const MAIN_CALLBACK: u32 = 0x0300_0000;
const MAP_CALLBACK: u32 = 0x0802_2049;

/// At the sprite flush (crate::branding::flush).
pub fn flush_sprites(core: &mut Core, at: u32, end: u32) -> u32 {
    let setup = crate::setup_phase::active(core);
    if (on(core).is_none() && !setup) || core.raw_read_32(MAIN_CALLBACK, -1) != MAP_CALLBACK {
        return at;
    }
    let mut sp = Sprites { at, end };
    let mut pairs = free_tile_pairs().into_iter();
    // A panel's lines (its window is on BG2, [`panel_tick`]), in tiles of
    // ours: 8x16 sprites, a column of 8 pixels each.
    if let Some(p) = want_panel(core).filter(|p| core.raw_read_8(PANEL, -1) == *p as u8) {
        for (x, y, s, button) in p.lines(core) {
            let cols = render_line(core, s, button);
            for (k, chunk) in cols.chunks(8).enumerate() {
                let Some(t) = pairs.next() else { break };
                let mut tiles = [0u8; 64];
                for (cx, col) in chunk.iter().enumerate() {
                    for (cy, &v) in col.iter().enumerate() {
                        tiles[32 * (cy / 8) + 4 * (cy % 8) + cx / 2] |= v << (4 * (cx & 1));
                    }
                }
                let at = OBJ_TILES + 32 * t as u32;
                let mut now = [0u8; 64];
                core.raw_read_range(at, -1, &mut now);
                if now != tiles {
                    core.raw_write_range(at, -1, &tiles);
                }
                // 8x16: shape tall (2 << 14), size 0; priority 0; palette 0.
                sp.put_shaped(core, x + 8 * k as i32, y, t, 2 << 14);
            }
        }
    }
    // At the top, AW2's font white outlined in black (no window: the game's
    // own windows come and go there): which front is on the screen during
    // the second front's rounds; "Setup" while the Setup phase lasts
    // (crate::setup_phase).
    let title = if setup {
        Some(crate::setup_phase::TITLE)
    } else if on(core).is_some() && core.raw_read_8(LIVE, -1) == 1 && core.raw_read_8(BUSY, -1) == 0 {
        Some(VIEW_TITLE)
    } else {
        None
    };
    if let Some(title) = title {
        let cols = outlined(core, title);
        let x = 120 - cols.len() as i32 / 2;
        for (k, chunk) in cols.chunks(8).enumerate() {
            let Some(t) = pairs.next() else { break };
            let mut tiles = [0u8; 64];
            for (cx, col) in chunk.iter().enumerate() {
                for (cy, &v) in col.iter().enumerate() {
                    tiles[32 * (cy / 8) + 4 * (cy % 8) + cx / 2] |= v << (4 * (cx & 1));
                }
            }
            let at = OBJ_TILES + 32 * t as u32;
            let mut now = [0u8; 64];
            core.raw_read_range(at, -1, &mut now);
            if now != tiles {
                core.raw_write_range(at, -1, &tiles);
            }
            sp.put_shaped(core, x + 8 * k as i32, 1, t, 2 << 14);
        }
    }
    sp.at
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![
        (HANDOVER, Box::new(handover)),
        (END_OF_GAME, Box::new(end_of_game)),
        (PICK_COUNT, Box::new(pick_count)),
        (SET_PICKS, Box::new(set_picks)),
        (AFTER_ACTION, Box::new(after_action)),
        (AFTER_BACK, Box::new(after_back)),
        (INFO_PANELS_Y, Box::new(info_panels)),
    ]
}

/// The state saved with a mission saved halfway (crate::suspend): the
/// module's state, then the store.
pub fn saved_state(core: &Core) -> Option<Vec<u8>> {
    battle(core)?;
    let mut b = vec![0u8; SAVED_LEN as usize];
    core.raw_read_range(STATE, -1, &mut b[..(STATE_LEN + PICKS_LEN) as usize]);
    core.raw_read_range(STORE, -1, &mut b[(STATE_LEN + PICKS_LEN) as usize..]);
    Some(b)
}

/// DS CAMPAIGN's Continue over a two-front battle saved on its front `live`
/// (crate::suspend, before AW2's resume runs): on the second front, its map
/// header in the battle's map table entry, as a swap puts it before the
/// game's `InitGameSettings` and the terrain read it.
pub fn continue_on(core: &mut Core, live: u8) {
    let Some(b) = battle(core) else { return };
    if live != 1 {
        return;
    }
    if let (Some(at), Some(h)) = (table_entry(core), header(core, b.fronts.second)) {
        core.raw_write_range(at, -1, &h);
    }
}

/// The length [`saved_state`] gives.
pub const SAVED_LEN: u32 = STATE_LEN + PICKS_LEN + STORE_LEN;

/// A mission saved halfway comes back (after AW2 restored its live front,
/// always the main one): the second front's state with it.
pub fn restore_saved(core: &mut Core, b: &[u8]) {
    if b.len() != SAVED_LEN as usize || battle(core).is_none() {
        return;
    }
    core.raw_write_range(STATE, -1, &b[..(STATE_LEN + PICKS_LEN) as usize]);
    core.raw_write_range(STORE, -1, &b[(STATE_LEN + PICKS_LEN) as usize..]);
    // (saved outside any swap: on the main front, or on the second during
    // a player's turn there, [`continue_on`]; the block AW2 restored is the
    // live front's, its look set before the map's graphics load)
    let live = if b[0] == 1 && core.raw_read_8(STORE_KIND, -1) == 1 { 1 } else { 0 };
    core.raw_write_8(LIVE, -1, live);
    core.raw_write_8(CONTINUED, -1, live);
    if let Some(m) = live_info(core) {
        crate::ds_campaign::set_look(core, m);
    }
    core.raw_write_8(BUSY, -1, 0);
    core.raw_write_8(VIEW, -1, 0);
    core.raw_write_8(PASS, -1, 0);
    core.raw_write_32(SENDING, -1, 0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ram_fits() {
        assert!(STATE >= 0x0203_E3E4, "after crate::grand_bolt's");
        assert!(STATE + STATE_LEN <= STORE);
        assert!(STORE + STORE_LEN <= RAM_END, "before the DS Campaign's records");
        assert!(PICKED_FOR + PICKS_LEN <= SKY_STATE && SKY_STATE + SKY_LEN <= STORE);
        assert!(SECOND_COS + PICKS_LEN <= STORE && QUEUE + 12 * QUEUE_LEN <= QUEUE_ARMY);
        assert!(EXTRA_PENDING + EXTRA_LEN <= STAGING + 0x2000);
        // (after a mission saved halfway's record: the block, the mark, both fronts)
        assert!(PANEL_SAVED >= STAGING + BLOCK_LEN + 4 + SAVED_LEN);
        assert!(PANEL_SAVED + 64 * PANEL_ROWS <= EXTRA_PENDING);
    }

    #[test]
    fn scripts_fit() {
        assert!(SCRIPT_ROUND + swap_script(Stub::BeginRound).len() as u32 <= SCRIPT_VIEW);
        assert!(SCRIPT_VIEW + swap_script(Stub::BeginViewIn).len() as u32 <= SCRIPT_OVER);
        assert!(SCRIPT_OVER + swap_script(Stub::BeginOver).len() as u32 <= MAP_MENU_COPY);
        assert!(stub_at(Stub::AutoChosen) + 16 <= SCRIPT_ROUND);
        assert!(MANUAL < QUEUE && MANUAL > PANEL_AT + 1);
        assert!(MAP_MENU_COPY + MENU_ENTRY * (MAP_MENU_LEN + 2) <= UNIT_MENU_COPY);
    }
}
