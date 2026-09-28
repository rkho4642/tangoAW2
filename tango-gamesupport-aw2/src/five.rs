//! Five armies: Orange Star, Blue Moon, Green Earth, Yellow Comet and Black
//! Hole on one Versus map.
//!
//! Advance Wars 2 has room for four armies: unit ids are one byte with the
//! army in the top two bits, the player table has four slots, turns wrap
//! after army 4, and so on. tangoAW2 gives army a the unit ids
//! (a-1)*51 + 1..50, moves the player table to free RAM with a fifth slot and
//! rewrites every place the game counts to four (`five/patches.txt`, checked
//! against the ROM by `five/gen.py`). The patches go into the ROM image in
//! memory only while the 5-army map is being played, and come out again
//! otherwise, so every other game runs the game's own code. Everything runs
//! inside the emulated frame, the same on both netplay peers.

use mgba::core::Core;

use crate::five_patches::{HALVES, HOOKS, WORDS};

pub fn is_five_map(id: u8) -> bool {
    crate::five_map::is_five_map(id)
}
const GAME_MODE: u32 = 0x0300_3FC1;
const MAP_SELECTED: u32 = 0x0300_3FC2;
const VERSUS: u8 = 3;

/// The player table, moved: six 0x3C-byte slots (0 = neutral, 1..5).
pub const PLAYERS: u32 = 0x0203_0000;
const PLAYER_SIZE: u32 = 0x3C;
/// The AI's per-army threat maps (0xC00 bytes each, armies 1..5), moved.
const THREAT: u32 = 0x0203_1000;
const OLD_PLAYERS: u32 = 0x0202_3284;
const CURRENT_ARMY: u32 = 0x0300_33EC;
const CURRENT_BASE: u32 = 0x0300_3F2C;
const MAP: u32 = 0x0201_E450;
const ARMIES_ON_MAP: u32 = MAP + 0x4233;

/// tangoAW2's own tables, in the ROM image's free space (0xFF padding).
pub const ROM_DATA: u32 = 0x0861_8000;
const BASES: u32 = ROM_DATA;
const OWNERS: u32 = ROM_DATA + 0x10;
const ICON_PALETTES: u32 = ROM_DATA + 0x20;
const BASE_SPRITES: u32 = ROM_DATA + 0x40;
const HQ_SPRITES: u32 = ROM_DATA + 0x60;
const HQ5_SPRITE: u32 = ROM_DATA + 0x80;
/// The results screen's layouts for 1..4 winners / losers.
const RES_WSTEP: u32 = ROM_DATA + 0xA0;
const RES_LBASE: u32 = ROM_DATA + 0xA8;
const RES_LSTEP: u32 = ROM_DATA + 0xB8;
const DATA_SENTINEL: u32 = ROM_DATA + 0xFC;
const DATA_MAGIC: u32 = 0x3541_5754; // "TWA5"

/// The breakpoint the trapper writes over a trapped instruction.
const TRAP: u16 = 0xBEEF;

/// Army 5's own unit palette goes to BG palette 1.
const UNIT_BANK: u16 = 1;
const UNIT_PALETTES: u32 = 0x0810_E6E0;
const BLACK_HOLE: u8 = 5;
/// Army 5's HQ sprite: the lab's 8 tiles in the building sheet (the map has
/// no labs; every OBJ tile is used by some screen, and the sheet is
/// reloaded, with this hook, whenever the map comes back).
const HQ5_TILE: u32 = 0x90;
/// (building sheet, HQ art per country) for clear and snowy weather; army
/// 5's HQ art goes into the sheet's lab tiles while a 5-army game is on.
const SHEETS: [(u32, u32); 2] = [(0x080C_FFC4, 0x080D_16C4), (0x080D_0B44, 0x080D_1BC4)];
/// The lab tiles' originals, kept while the sheet holds army 5's HQ.
const SHEET_BACKUP: u32 = ROM_DATA + 0x2000;

/// Army 5's picks on the Teams screen, kept for the battle.
const ARMY5_CONTROL: u32 = 0x0203_0200; // 1 human, 2 computer
const ARMY5_CO: u32 = 0x0203_0201; // CO id + 1 (0: not picked yet)
const ARMY5_TEAM: u32 = 0x0203_0202; // team + 1
const LABEL_PENDING: u32 = 0x0203_0204;
/// Set while the vision detour for army 5 runs.
const VISION_DETOUR: u32 = 0x0203_0205;
const FLAK: u8 = 11;
const KANBEI: u8 = 6;

/// The Teams screen's record, moved here with room for five armies.
const TEAMS: u32 = 0x0203_0300;
const TEAMS_SIZE: usize = 0xE0;
const TEAMS_ARMIES: u32 = 0x08;
const TEAMS_CONTROLLER: u32 = 0x90;
const TEAMS_COLOUR: u32 = 0x98;
const TEAMS_TEAM: u32 = 0xA0;
const TEAMS_CO_CURSOR: u32 = 0xA8;
const TEAMS_CO: u32 = 0xB0;
const TEAMS_CO_LIST: u32 = 0x18;
/// Help texts per cursor stop (CO, controller) x 5 armies.
const TEAMS_HELP: u32 = 0x0862_1000;
const CHOOSE_CO: u16 = 0x9DC;
const CHOOSE_PLAYER: u16 = 0x9DD;
/// The Black Hole emblem (sprite 0x42's 4 tiles), put at OBJ tile 0x8C and
/// registered in the Teams screen's emblem group.
const EMBLEM_ART: u32 = 0x080F_8B84;
const EMBLEM_TILE: u32 = 0x8C;
const EMBLEM_GROUP: u32 = 0x0200_F9A8;
/// tangoAW2's "E Team" and "5P" pictures, at OBJ tiles 0x70 and 0x88.
const E_TEAM_TILE: u32 = 0x70;
const FIVE_P_TILE: u32 = 0x88;
/// Tiles of the sprites they are drawn in the shape of: "4P" and "D Team".
const FOUR_P_SPRITE_TILE: u32 = 0x2B4;
const D_TEAM_SPRITE_TILE: u32 = 0x35A;

#[derive(Clone, Copy, Debug)]
pub enum Op {
    A0,
    A0Id4,
    A0Id4Hi16,
    A0Hi16,
    A0Hi24,
    A0Shl6,
    B,
    S,
    Mul51,
    Mul102,
    Mul612,
    CurBase,
    Bitidx,
    Parity,
}

#[derive(Clone, Copy, Debug)]
pub enum Routine {
    SameTeamCurrent,
    SameTeamIds,
    SameTeamProperty,
    UnsupportedRedeal,
    NoSave,
    HideSave,
    CaptureTile,
    HqToCity,
    PropertyCensus,
    UnitPalette,
    SetupArmy5,
    TeamsInit,
    TeamsEmblem,
    Label5p,
    Label5pTiles,
    TeamE,
    TeamsCommit,
    VisionArmy5,
    UnitAnimCount,
}

#[derive(Clone, Copy, Debug)]
pub enum Hook {
    Set(Op, u8, u8),
    Replace(Routine),
    Before(Routine),
}

#[derive(Clone, Copy, Debug)]
pub enum Table {
    Players,
    IconPalettes,
    Bases,
    CountUnitsOffset,
    Owners,
    BaseSprites,
    HqSprites,
    Threat,
    Threat64,
    ResWstep,
    ResLbase,
    ResLstep,
    TeamsRecord,
    TeamsHelp,
}

impl Table {
    fn value(self) -> u32 {
        match self {
            Table::Players => PLAYERS,
            Table::IconPalettes => ICON_PALETTES,
            Table::Bases => BASES,
            // CountArmyUnits: &gUnits[base(a) + 1] = gUnits + a*612 - 612 + 12
            Table::CountUnitsOffset => (12i32 - 612) as u32,
            Table::Owners => OWNERS,
            Table::BaseSprites => BASE_SPRITES,
            Table::HqSprites => HQ_SPRITES,
            Table::Threat => THREAT,
            Table::Threat64 => THREAT + 0x64,
            Table::ResWstep => RES_WSTEP,
            Table::ResLbase => RES_LBASE,
            Table::ResLstep => RES_LSTEP,
            Table::TeamsRecord => TEAMS,
            Table::TeamsHelp => TEAMS_HELP,
        }
    }
}

// ---------- the id layout ----------

/// 0-based army of a unit id or of an army's first id.
fn a0(x: u32) -> u32 {
    (x & 0xFFFF) / 51
}

fn army(x: u32) -> u32 {
    a0(x) + 1
}

// ---------- switching ----------

/// Where the game's player table is now (slot 0 = neutral).
pub fn players(core: &Core) -> u32 {
    if core.raw_read_32(0x0849_9598, -1) == PLAYERS {
        PLAYERS
    } else {
        OLD_PLAYERS
    }
}

/// Set when Five Seas is picked on the map list (and cleared when another
/// map is, or a suspended game is resumed). It lives in emulated RAM, so a
/// rollback restores it with everything else, and the patches in ROM (which
/// a rollback does not restore) are switched from it every frame.
const FIVE_ON: u32 = 0x0203_0206;

pub fn active(core: &Core) -> bool {
    core.raw_read_8(FIVE_ON, -1) == 1
        && core.raw_read_8(GAME_MODE, -1) == VERSUS
        && is_five_map(core.raw_read_8(MAP_SELECTED, -1))
}

/// The map list stores the picked map (sub_0803BCD0(mapID)): start or end a
/// 5-army game. Its RAM is set up here, at an emulated event, not when the
/// patches are switched.
pub const MAP_PICKED: u32 = 0x0803_BCD0;

pub fn map_picked(core: &mut Core) {
    let map = core.gba().cpu().gpr(0) as u8;
    let five = is_five_map(map) && core.raw_read_8(GAME_MODE, -1) == VERSUS;
    core.raw_write_8(FIVE_ON, -1, five as u8);
    if five {
        core.raw_write_range(TEAMS, -1, &[0u8; TEAMS_SIZE]);
        // Start from the players the game had, so screens before the battle
        // read what they expect.
        let mut buf = vec![0u8; 5 * PLAYER_SIZE as usize];
        core.raw_read_range(OLD_PLAYERS, -1, &mut buf);
        core.raw_write_range(PLAYERS, -1, &buf);
    }
}

/// Resuming a suspended game (sub_08017658): those are always ordinary
/// games (a 5-army game cannot be saved), and loading needs the game's own
/// code, so switch the patches off now, before it runs.
pub const RESUME: u32 = 0x0801_7658;

pub fn before_resume(core: &mut Core) {
    core.raw_write_8(FIVE_ON, -1, 0);
    if core.raw_read_16(HALVES[0].0, -1) == HALVES[0].2 || core.raw_read_16(HOOKS[0].0, -1) == TRAP {
        apply(core, false);
    }
}

/// Every hook address, for the trapper (installed once; `sync` switches them).
pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    HOOKS
        .iter()
        .enumerate()
        .map(|(i, &(addr, _, _))| (addr, Box::new(move |core: &mut Core| run(core, i)) as Box<dyn Fn(&mut Core)>))
        .collect()
}

/// Every frame, before it runs: put tangoAW2's tables in place and switch the
/// patches to match the game being played.
pub fn sync(core: &mut Core) {
    install_data(core);
    let on = active(core);
    let hooks_on = core.raw_read_16(HOOKS[0].0, -1) == TRAP;
    let halves_on = core.raw_read_16(HALVES[0].0, -1) == HALVES[0].2;
    if hooks_on != on || halves_on != on {
        apply(core, on);
    }
}

fn apply(core: &mut Core, on: bool) {
    for &(addr, old, new) in HALVES {
        core.raw_write_16(addr, -1, if on { new } else { old });
    }
    for &(addr, old, table) in WORDS {
        core.raw_write_32(addr, -1, if on { table.value() } else { old });
    }
    for &(addr, old, _) in HOOKS {
        core.raw_write_16(addr, -1, if on { TRAP } else { old });
    }
    for (i, (sheet, art)) in SHEETS.iter().enumerate() {
        let at = sheet + (HQ5_TILE - 0x48) * 32;
        let mut buf = [0u8; 0x100];
        let src = if on { art + (BLACK_HOLE as u32 - 1) * 0x100 } else { SHEET_BACKUP + 0x100 * i as u32 };
        core.raw_read_range(src, -1, &mut buf);
        core.raw_write_range(at, -1, &buf);
    }
}

fn install_data(core: &mut Core) {
    if core.raw_read_32(DATA_SENTINEL, -1) == DATA_MAGIC {
        return;
    }
    for (i, v) in [0u16, 0, 51, 102, 153, 204].iter().enumerate() {
        core.raw_write_16(BASES + 2 * i as u32, -1, *v);
    }
    for i in 0..6u32 {
        core.raw_write_16(OWNERS + 2 * i, -1, (i * 0x20) as u16);
    }
    for i in 0..5u32 {
        let v = core.raw_read_16(0x0809_097C + 2 * i, -1);
        core.raw_write_16(ICON_PALETTES + 2 * i, -1, v);
    }
    core.raw_write_16(ICON_PALETTES + 10, -1, (UNIT_BANK << 12) | 0x235);
    // Building sprite definitions per owner (neutral, armies 1..5).
    for i in 0..5u32 {
        let base = core.raw_read_32(0x0849_FAB0 + 4 * i, -1);
        core.raw_write_32(BASE_SPRITES + 4 * i, -1, base);
        let hq = core.raw_read_32(0x0849_FAC4 + 4 * i, -1);
        core.raw_write_32(HQ_SPRITES + 4 * i, -1, hq);
    }
    core.raw_write_32(BASE_SPRITES + 20, -1, 0x0849_F988);
    core.raw_write_32(HQ_SPRITES + 20, -1, HQ5_SPRITE);
    // One 16x32 sprite like the other HQs, on army 5's tiles, priority 3.
    for (i, h) in [0x0001u16, 0x80F0, 0x8000, 0x0C00 | (HQ5_TILE - 0x48) as u16]
        .iter()
        .enumerate()
    {
        core.raw_write_16(HQ5_SPRITE + 2 * i as u32, -1, *h);
    }
    for (i, (sheet, _)) in SHEETS.iter().enumerate() {
        let mut buf = [0u8; 0x100];
        core.raw_read_range(sheet + (HQ5_TILE - 0x48) * 32, -1, &mut buf);
        core.raw_write_range(SHEET_BACKUP + 0x100 * i as u32, -1, &buf);
    }
    for (i, v) in [0u16, 0x24, 0x24, 0x24].iter().enumerate() {
        core.raw_write_16(RES_WSTEP + 2 * i as u32, -1, *v);
    }
    for (i, v) in [0xD8u32, 0xFC, 0xD8, 0xB8].iter().enumerate() {
        core.raw_write_32(RES_LBASE + 4 * i as u32, -1, *v);
    }
    for (i, v) in [0u16, 0x24, 0x20, 0x20].iter().enumerate() {
        core.raw_write_16(RES_LSTEP + 2 * i as u32, -1, *v);
    }
    for i in 0..10u32 {
        let id = if i % 2 == 0 { CHOOSE_CO } else { CHOOSE_PLAYER };
        core.raw_write_16(TEAMS_HELP + 2 * i, -1, id);
    }
    crate::five_map::install(core);
    core.raw_write_32(DATA_SENTINEL, -1, DATA_MAGIC);
}

// ---------- hooks ----------

fn run(core: &mut Core, i: usize) {
    let (addr, _, hook) = HOOKS[i];
    if std::env::var_os("AW2_FIVE_LOG").is_some() {
        let cpu = core.gba().cpu();
        eprintln!("five {addr:08x} {hook:?} r0={:x} r1={:x} r2={:x} lr={:x}", cpu.gpr(0), cpu.gpr(1), cpu.gpr(2), cpu.gpr(14));
    }
    match hook {
        Hook::Set(op, dst, src) => set(core, addr, op, dst as usize, src as usize),
        Hook::Replace(r) => {
            let result = routine(core, r);
            let cpu = core.gba_mut().cpu_mut();
            if let Some(v) = result {
                cpu.set_gpr(0, v as i32);
            }
            let lr = cpu.gpr(14) as u32;
            cpu.set_thumb_pc(lr & !1);
        }
        Hook::Before(r) => {
            routine(core, r);
        }
    }
}

fn set(core: &mut Core, addr: u32, op: Op, dst: usize, src: usize) {
    let cpu = core.gba().cpu();
    let v = cpu.gpr(src) as u32;
    let d = cpu.gpr(dst) as u32;
    let result = match op {
        Op::A0 => a0(v),
        Op::A0Id4 => a0(v >> 2),
        Op::A0Id4Hi16 => a0(v >> 2) << 16,
        Op::A0Hi16 => a0(v >> 16),
        Op::A0Hi24 => a0(v >> 24),
        Op::A0Shl6 => a0(v) << 6,
        Op::B => a0(v) * 51,
        Op::S => (v & 0xFF) - a0(v & 0xFF) * 51,
        Op::Mul51 => v.wrapping_mul(51),
        Op::Mul102 => v.wrapping_mul(102),
        Op::Mul612 => v.wrapping_mul(612),
        Op::CurBase => core.raw_read_16(CURRENT_BASE, -1) as u32,
        // r0 points into a 4-entry table of army bits, at the army's index
        Op::Bitidx => 1 << d.wrapping_sub(v).wrapping_sub(0x24),
        Op::Parity => {
            // Armies 1, 3 and 5 face one way, 2 and 4 the other.
            let to = if a0(v) % 2 == 0 { 0x0802_21F6 } else { 0x0802_22CA };
            core.gba_mut().cpu_mut().set_thumb_pc(to);
            return;
        }
    };
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(dst, result as i32);
    cpu.set_thumb_pc(addr + 2);
}

fn player(army: u32) -> u32 {
    PLAYERS + PLAYER_SIZE * army
}

fn team(core: &Core, army: u32) -> u8 {
    core.raw_read_8(player(army) + 0x2A, -1)
}

fn routine(core: &mut Core, r: Routine) -> Option<u32> {
    let cpu = core.gba().cpu();
    let (r0, r1, r2) = (cpu.gpr(0) as u32, cpu.gpr(1) as u32, cpu.gpr(2) as u32);
    match r {
        Routine::SameTeamCurrent => {
            let cur = core.raw_read_16(CURRENT_ARMY, -1) as u32;
            Some((team(core, cur) == team(core, army(r0))) as u32)
        }
        Routine::SameTeamIds => Some((team(core, army(r0)) == team(core, army(r1))) as u32),
        Routine::SameTeamProperty => {
            let t = (r1 & 0xFF) >> 5;
            Some((t != 0 && team(core, army(r0)) == team(core, t)) as u32)
        }
        Routine::UnsupportedRedeal | Routine::NoSave => None,
        // The map menu's Save item hides when its test returns nonzero.
        Routine::HideSave => Some(1),
        Routine::CaptureTile => Some(capture_tile(core, r0 as i16, r1 as i16, r2 as u8, false)),
        Routine::HqToCity => Some(capture_tile(core, r0 as i16, r1 as i16, r2 as u8, true)),
        Routine::PropertyCensus => {
            property_census(core);
            None
        }
        Routine::UnitPalette => {
            let mut pal = [0u8; 0x20];
            core.raw_read_range(UNIT_PALETTES + (BLACK_HOLE as u32 - 1) * 0x20, -1, &mut pal);
            // The game's palette buffer (copied to palette RAM each frame)
            // and palette RAM itself.
            for base in [0x0300_20C0, 0x0500_0000] {
                core.raw_write_range(base + UNIT_BANK as u32 * 0x20, -1, &pal);
            }
            None
        }
        Routine::SetupArmy5 => {
            setup_army5(core);
            None
        }
        Routine::TeamsInit => {
            teams_init(core);
            None
        }
        Routine::TeamsEmblem => {
            let mut art = [0u8; 0x80];
            core.raw_read_range(EMBLEM_ART, -1, &mut art);
            core.raw_write_range(0x0601_0000 + EMBLEM_TILE * 32, -1, &art);
            core.raw_write_range(0x0601_0000 + E_TEAM_TILE * 32, -1, &crate::five_art::E_TEAM);
            core.raw_write_range(0x0601_0000 + FIVE_P_TILE * 32, -1, &crate::five_art::FIVE_P);
            let count = core.raw_read_8(EMBLEM_GROUP + 5, -1) as u32;
            if count == 4 {
                core.raw_write_16(EMBLEM_GROUP + 8 + 4 * count, -1, EMBLEM_TILE as u16);
                core.raw_write_16(EMBLEM_GROUP + 8 + 4 * count + 2, -1, 0x42);
                core.raw_write_8(EMBLEM_GROUP + 5, -1, 5);
            }
            None
        }
        Routine::Label5p => {
            // sub_08064DDC(x, y, army): the label is 1P..4P by army, or CP.
            // Army 5 played by a person gets "5P", drawn in 4P's shape.
            let cpu = core.gba().cpu();
            let (controller, army) = (cpu.gpr(0), cpu.gpr(2));
            if controller != 2 && army == 4 {
                core.raw_write_8(LABEL_PENDING, -1, 1);
                let cpu = core.gba_mut().cpu_mut();
                cpu.set_gpr(2, 3);
                cpu.set_thumb_pc(0x0806_4DF4);
            } else {
                core.raw_write_8(LABEL_PENDING, -1, 0);
            }
            None
        }
        Routine::Label5pTiles => {
            if core.raw_read_8(LABEL_PENDING, -1) != 0 {
                core.raw_write_8(LABEL_PENDING, -1, 0);
                let offset = FIVE_P_TILE.wrapping_sub(FOUR_P_SPRITE_TILE);
                core.gba_mut().cpu_mut().set_gpr(3, offset as i32);
            }
            None
        }
        Routine::TeamE => {
            // Team letters are sprites 0xBD..0xC0 (A..D); 0xC1 is a Rules
            // sprite. Team E is drawn in D's shape from tangoAW2's tiles.
            if core.gba().cpu().gpr(0) == 0xC1 {
                let offset = E_TEAM_TILE.wrapping_sub(D_TEAM_SPRITE_TILE);
                let cpu = core.gba_mut().cpu_mut();
                cpu.set_gpr(0, 0xC0);
                cpu.set_gpr(3, offset as i32);
            }
            None
        }
        Routine::VisionArmy5 => {
            // First time here: call sub_080212AC(5), returning to this
            // instruction (r0..r3 are free after the calls before it).
            if core.raw_read_8(VISION_DETOUR, -1) == 0 {
                core.raw_write_8(VISION_DETOUR, -1, 1);
                let cpu = core.gba_mut().cpu_mut();
                cpu.set_gpr(0, 5);
                cpu.set_gpr(14, (0x0802_147C | 1) as i32);
                cpu.set_thumb_pc(0x0802_12AC);
            } else {
                core.raw_write_8(VISION_DETOUR, -1, 0);
            }
            None
        }
        Routine::UnitAnimCount => {
            if core.raw_read_8(player(5) + 0x1B, -1) != 0 {
                let cpu = core.gba_mut().cpu_mut();
                let n = cpu.gpr(0);
                cpu.set_gpr(0, n + 1);
            }
            None
        }
        Routine::TeamsCommit => {
            let rec = TEAMS;
            let control = core.raw_read_8(rec + TEAMS_CONTROLLER + 4, -1);
            let co = core.raw_read_8(rec + TEAMS_CO + 4, -1);
            let team = core.raw_read_8(rec + TEAMS_TEAM + 4, -1);
            core.raw_write_8(ARMY5_CONTROL, -1, control);
            core.raw_write_8(ARMY5_CO, -1, co.wrapping_add(1));
            core.raw_write_8(ARMY5_TEAM, -1, team.wrapping_add(1));
            None
        }
    }
}

/// The property tiles per kind (HQ, base, city, airport, port, lab), each for
/// neutral and armies 1..5. Army 5's are tangoAW2's new tile ids.
const PROPERTY_TILES: [[u16; 6]; 6] = [
    [0x1C0, 0x1C5, 0x1CA, 0x1CF, 0x1D4, 0x1B4],
    [0x1C1, 0x1C6, 0x1CB, 0x1D0, 0x1D5, 0x1B5],
    [0x1C2, 0x1C7, 0x1CC, 0x1D1, 0x1D6, 0x1B6],
    [0x1C3, 0x1C8, 0x1CD, 0x1D2, 0x1D7, 0x1B7],
    [0x1C4, 0x1C9, 0x1CE, 0x1D3, 0x1D8, 0x1B8],
    [0x1D9, 0x1DA, 0x1DB, 0x1DC, 0x1DD, 0x1B9],
];
const PREVIOUS_TILE: u32 = 0x0300_33F8;

/// sub_080240B4 (a property changes owner) and sub_0802419C (an HQ becomes
/// its captor's city), with six owners per kind. Returns the kind's index
/// times five, as the game's own versions do.
fn capture_tile(core: &mut Core, x: i16, y: i16, owner: u8, hq_to_city: bool) -> u32 {
    let row = core.raw_read_16(MAP + 0x417A + 2 * y as u32, -1) as u32;
    let cell = MAP + 0xA22 + 2 * (row + x as u32);
    let tile = core.raw_read_16(cell, -1);
    if !hq_to_city {
        core.raw_write_16(PREVIOUS_TILE, -1, tile);
    }
    let Some(kind) = PROPERTY_TILES.iter().position(|k| k.contains(&tile)) else {
        return 30;
    };
    let o = (owner >> 5) as usize;
    if !hq_to_city {
        core.raw_write_16(cell, -1, PROPERTY_TILES[kind][o]);
    } else if kind == 0 {
        core.raw_write_16(cell, -1, PROPERTY_TILES[2][o]);
    }
    kind as u32 * 5
}

/// End of sub_08021810: the most properties any army has (the rules
/// screen's capture limit), counted for five owners.
fn property_census(core: &mut Core) {
    const COUNTS: u32 = 0x0300_32D0;
    let (w, h) = (core.raw_read_16(MAP, -1) as u32, core.raw_read_16(MAP + 2, -1) as u32);
    let mut per = [0u32; 6];
    let mut total = 0;
    for y in 0..h {
        let row = core.raw_read_16(MAP + 0x417A + 2 * y, -1) as u32;
        for x in 0..w {
            let t = core.raw_read_8(MAP + 0x1432 + row + x, -1);
            if matches!(t & 0x1F, 6 | 8 | 0xA | 0xB | 0xE | 0x14) {
                total += 1;
                per[(t >> 5) as usize % 6] += 1;
            }
        }
    }
    let mut best = per[1..].iter().copied().max().unwrap_or(0);
    if best < total {
        best += 1;
    }
    core.raw_write_8(COUNTS, -1, best.min(255) as u8);
    for (i, &n) in per.iter().enumerate().take(5).skip(1) {
        core.raw_write_8(COUNTS + i as u32, -1, n.min(255) as u8);
    }
    core.raw_write_8(COUNTS + 5, -1, total.min(255) as u8);
    // ...and the two results it stored already: *b = all properties (r8),
    // *a = the smallest capture limit worth offering (sb).
    let cpu = core.gba().cpu();
    let (a_ptr, b_ptr) = (cpu.gpr(9) as u32, cpu.gpr(8) as u32);
    core.raw_write_8(b_ptr, -1, total.min(255) as u8);
    core.raw_write_8(a_ptr, -1, best.min(255) as u8);
}

/// Entry of sub_08026B28 (team masks), after the players were reset and
/// armies 1..4 set from the Teams screen: set army 5, Black Hole.
fn setup_army5(core: &mut Core) {
    let p = player(5);
    let control = match core.raw_read_8(ARMY5_CONTROL, -1) {
        1 => 1,
        _ => 2,
    };
    let co = match core.raw_read_8(ARMY5_CO, -1) {
        0 => FLAK,
        c => c - 1,
    };
    let team = match core.raw_read_8(ARMY5_TEAM, -1) {
        0 => 4,
        t => t - 1,
    };
    core.raw_write_8(p + 0x1B, -1, control);
    core.raw_write_8(p + 0x1A, -1, BLACK_HOLE);
    core.raw_write_8(p + 0x1D, -1, co);
    core.raw_write_8(p + 0x2A, -1, team);
    core.raw_write_8(p + 0x2B, -1, 0x10);
    core.raw_write_8(p + 0x2C, -1, 0);
    // The armies on the map: the header says 4 for the Teams screen, and
    // sub_08026924 (just done) needed that; the CO and Intel screens count
    // armies with it.
    core.raw_write_8(ARMIES_ON_MAP, -1, 5);
}

/// sub_0806574C, before its per-army loop (the record is filled for the
/// four slots of the map header): add army 5, Black Hole, with the team and
/// CO it had last time (Flak the first time). The loop then sets its
/// controller (computer) and CO id from the cursor.
fn teams_init(core: &mut Core) {
    let rec = TEAMS;
    core.raw_write_8(rec + TEAMS_ARMIES, -1, 5);
    core.raw_write_8(rec + TEAMS_COLOUR + 4, -1, BLACK_HOLE);
    let team = match core.raw_read_8(ARMY5_TEAM, -1) {
        0 => 4,
        t => t - 1,
    };
    core.raw_write_8(rec + TEAMS_TEAM + 4, -1, team);
    let want = match core.raw_read_8(ARMY5_CO, -1) {
        0 => FLAK,
        c => c - 1,
    };
    let list = core.raw_read_32(rec + TEAMS_CO_LIST, -1);
    let mut cursor = 0u8;
    for i in 0..32u32 {
        let co = core.raw_read_8(list + i, -1);
        if co == 0xFF {
            break;
        }
        if co == want {
            cursor = i as u8;
            break;
        }
    }
    core.raw_write_8(rec + TEAMS_CO_CURSOR + 4, -1, cursor);
    // Black Hole's COs belong to army 5 here: an earlier army sitting on
    // one (the Versus default gives Yellow Comet Flak) gets its own army's
    // first CO instead (Kanbei for Yellow Comet).
    for (slot, own) in [(0u32, 1u8), (1, 2), (2, 3), (3, KANBEI)] {
        let at = rec + TEAMS_CO_CURSOR + slot;
        let co = core.raw_read_8(list + core.raw_read_8(at, -1) as u32, -1);
        if (10..=14).contains(&co) {
            if let Some(i) = (0..32u32).find(|&i| core.raw_read_8(list + i, -1) == own) {
                core.raw_write_8(at, -1, i as u8);
            }
        }
    }
}
