//! Survival, Dual Strike's mode of eleven maps fought in a row against the
//! computer on one budget, with the Dual Strike pack.
//!
//! Dual Strike (as read from its code, see [`crate::survival_maps`] for the
//! data): three kinds, each a fixed run of eleven of its own Survival maps.
//! **Money**: 500,000 G for the whole run and no income (properties, joins,
//! Colin's and Sasha's powers earn nothing); the run is lost the moment the
//! funds reach 0 (`sub_020EA944`). **Turn**: 99 days for the whole run; each
//! cleared map costs its days less one (`sub_020EA87C`), and a map is lost
//! when its day passes what is left. **Time**: 25 minutes, counted only on
//! the player's own turns (whole seconds are taken off per map). Points are
//! the battles' scores added up (at most 9999), plus a bonus for what is
//! left at the end (`sub_020EB024`); the final rank is S/A/B/C by what is
//! left (`sub_020EAD98`); the best leftover per kind is kept with its COs
//! (`sub_020EAE84`). The COs are picked once and kept for the whole run
//! (the survival state's CO pair is copied into the player at every map
//! start, `sub_020EACC4`). Each map brings its own fog, weather and look,
//! and the computer's CO.
//!
//! Here: a "Survival" entry on the Select Mode wheel ([`crate::mode_menu`])
//! opens the War Room's own screens with Survival's maps: SELECT MAP lists
//! Money, Turn and Time Survival (the record of each in a panel), then the
//! War Room's CO screen, the battle, the War Room's results and save; back
//! on SELECT MAP the next map of the run is the only one listed (its CO
//! screen offers only the run's CO), until the run is cleared or lost and
//! its results are shown there. In battle the budget left is shown at the
//! top of the screen. AW2 has no tag battles: the run keeps one CO.
//!
//! - Maps: ids 0xC9..0xCB are the three runs' entries (the first map of
//!   each, named after the kind), 0xCC.. the 33 maps; their headers are in
//!   a copy of the map table with room for 0x100 ids ([`TABLE`]; with the
//!   pack the game reads it instead of [`crate::five_map`]'s), listed on a
//!   tab of their own ([`TAB`]) that the War Room shows while Survival is
//!   on (the War Room's tab byte, `0x08090EF2`).
//! - The War Room keeps its records by `id - 0x6C`; while Survival is on its
//!   record readers and writer look at a zeroed block of ours ([`ROWS`]).
//! - State: [`STATE`] in EWRAM; records in three bytes each of the profile
//!   the game saves (`0x0200C435..`, bytes no code of the game touches).

use mgba::core::Core;
use std::sync::OnceLock;

use crate::survival_maps::{self as maps, Kind, MAPS_PER_RUN};

// --- Where things are ------------------------------------------------------

/// tangoAW2's copy of the map table (0x100 ids) and the Survival maps.
pub const TABLE: u32 = 0x08E0_0000;
#[cfg(test)]
const TABLE_IDS: u32 = 0x100;
const MAP_DATA: u32 = 0x08E0_8000;
const MAP_DATA_SIZE: u32 = 0x800;
const STRINGS: u32 = 0x08E3_0000;
const STRING_SIZE: u32 = 0x40;
const SENTINEL: u32 = 0x08E0_7FFC;
const MAGIC: u32 = 0x3256_5344; // "DSV2": headers with the structures' pictures

/// Map ids: the three entries (Money, Turn, Time), then the maps.
pub const FIRST_ID: u8 = 0xC9;
const ENTRY_IDS: [u8; 3] = [0xC9, 0xCA, 0xCB];
const MAPS_FROM: u8 = 0xCC;
/// The tab Survival's maps are listed on.
pub const TAB: u16 = 0x0A;
/// The War Room's tab in the mode -> tab table (`BuildMapListForMode`).
const WAR_ROOM_TAB: u32 = 0x0809_0EF2;
const WAR_ROOM_TAB_GAME: u8 = 7;

/// Text ids from 0x7172 read their pointer from 0x0862D000 (free ROM, past
/// [`crate::co_new`]'s); see [`crate::five_map`] for why that works.
const TEXT_TABLE: u32 = 0x0861_0A38;
pub const TEXT_BASE: u16 = 0x7172;
pub const T_HELP: u16 = 0;
const T_KINDS: u16 = 1; // three
// 4: spare
const T_MAPS: u16 = 5; // 33

/// The War Room record table (`id - 0x6C` rows of 0x14) and the
/// literal-pool words of its readers and writer.
const RECORDS: u32 = 0x0200_C078;
const RECORD_POOLS: [u32; 5] = [0x0808_759C, 0x0808_7664, 0x0808_7B18, 0x0808_7C6C, 0x0801_77E4];

// --- RAM ---------------------------------------------------------------------

pub const STATE: u32 = 0x0203_FA00;
const ON: u32 = STATE; // 1 while Survival's War Room is open
const KIND: u32 = STATE + 1;
const STAGE: u32 = STATE + 2; // maps cleared
const PHASE: u32 = STATE + 3;
const LEFT: u32 = STATE + 4;
const BUDGET: u32 = STATE + 8;
const POINTS: u32 = STATE + 0x0C;
const TIME: u32 = STATE + 0x10; // frames of the player's turns on this map
const FUNDS_CAP: u32 = STATE + 0x14;
const CO: u32 = STATE + 0x18;
pub(crate) const MENU_PICKED: u32 = STATE + 0x19;
const BONUS: u32 = STATE + 0x1C;
const RANK: u32 = STATE + 0x20;
const LAST_SCORE: u32 = STATE + 0x22;
const OUT_SET: u32 = STATE + 0x24;
/// A zeroed block the War Room's record code reads while Survival is on:
/// rows for ids FIRST_ID..=0xEC.
const ROWS: u32 = STATE + 0x40;
const ROW: u32 = 0x14;
const ROWS_LEN: u32 = ROW * 0x24;

/// PHASE: choosing a kind; between maps; in a battle; cleared; lost.
const CHOOSING: u8 = 0;
const BETWEEN: u8 = 1;
const PLAYING: u8 = 2;
const CLEARED: u8 = 3;
const LOST: u8 = 4;

/// Records: [`RECORD_MAGIC`], then three bytes per kind (Time, Money,
/// Turn): rank (3 bits), CO (7 bits), what was left (14 bits: frames / 60,
/// G / 100, days).
const PROFILE_RECORDS: u32 = 0x0200_C435;
const RECORD_MAGIC: u8 = 0xD5;

// --- The game ------------------------------------------------------------

const PLAYST: u32 = 0x0300_3FC0;
const MAP_ID: u32 = PLAYST + 0x02;
const PROPERTY_FUNDS: u32 = PLAYST + 0x28;
const WEATHER: u32 = PLAYST + 0x2C;
const WEATHER_MODE: u32 = PLAYST + 0x2D;
const NEXT_WEATHER: u32 = PLAYST + 0x2E;
const DEFAULT_WEATHER: u32 = PLAYST + 0x2F;
const PLAYST_CO: u32 = PLAYST + 0x3D;
const FOG: u32 = PLAYST + 0x0D;
const DAY: u32 = 0x0300_4080;
const CURRENT_ARMY: u32 = 0x0300_33EC;
const MAIN_CALLBACK: u32 = 0x0300_0000;
const MAP_CALLBACK: u32 = 0x0802_2049;
const BATTLE_SCENE: u32 = 0x0300_0004;
/// Player blocks: funds +0, spent +4, defeated +0x14, CO +0x1D, yielded
/// +0x31, total score +0x38.
const P_FUNDS: u32 = 0x00;
const P_DEFEATED: u32 = 0x14;
const P_CO: u32 = 0x1D;
const P_YIELD: u32 = 0x31;
const P_SCORE: u32 = 0x38;

const PROCS: u32 = 0x0200_D610;
const PROCS_END: u32 = 0x0200_E418;
const PROC_SIZE: u32 = 0x6C;
/// `ProcScr_MainMenu` (the Select Mode wheel) and the War Room's SELECT
/// MAP (`PreviewMap`) and CO screen (`WarRoomMapSelected`).
/// The wheel's proc (`ProcScr_MainMenuC1`), running while Select Mode is up.
const MAIN_MENU: u32 = 0x0861_6990;
const SELECT_MAP: u32 = 0x0861_6C54;

fn running(core: &Core, script: u32) -> bool {
    (PROCS..PROCS_END)
        .step_by(PROC_SIZE as usize)
        .any(|p| core.raw_read_32(p, -1) == script)
}

fn player(core: &Core, army: u32) -> u32 {
    crate::five::players(core) + 0x3C * army
}

// --- The maps in the ROM image --------------------------------------------

struct Built {
    writes: Vec<(u32, Vec<u8>)>,
    /// Header address per id, for the per-frame tab and name fields.
    names: Vec<(u8, u16)>,
}

static BUILT: OnceLock<Option<Built>> = OnceLock::new();

fn text_slot(k: u16) -> u32 {
    TEXT_TABLE + 4 * (TEXT_BASE + k) as u32
}

fn string_at(k: u16) -> u32 {
    STRINGS + STRING_SIZE * k as u32
}

fn put_text(w: &mut Vec<(u32, Vec<u8>)>, k: u16, s: &str) {
    let mut b = s.as_bytes().to_vec();
    b.truncate(STRING_SIZE as usize - 1);
    b.push(0);
    w.push((string_at(k), b));
    w.push((text_slot(k), string_at(k).to_le_bytes().to_vec()));
}

pub const HELP: &str = "Clear 11 maps in a row on one budget.";

fn header(map: &maps::Map, tiles: u32, units: u32, name: u16) -> [u8; 0x5C] {
    let mut h = [0u8; 0x5C];
    let w32 = |h: &mut [u8], at: usize, v: u32| h[at..at + 4].copy_from_slice(&v.to_le_bytes());
    let w16 = |h: &mut [u8], at: usize, v: u16| h[at..at + 2].copy_from_slice(&v.to_le_bytes());
    w32(&mut h, 0x00, tiles);
    // The 4x4 structure's picture (none: the game would load nothing and
    // draw whatever OBJ VRAM holds there).
    w32(&mut h, 0x10, map.structure.map_or(0, |s| s.aw2_picture()));
    w16(&mut h, 0x14, TEXT_BASE + name);
    h[0x16] = 1;
    h[0x17] = map.fog as u8;
    h[0x18] = map.armies;
    w16(&mut h, 0x1A, crate::five_map::HIDDEN_TAB);
    w16(&mut h, 0x1C, 1);
    w16(&mut h, 0x1E, 1);
    w16(&mut h, 0x20, map.speed_days);
    w16(&mut h, 0x22, map.speed_days);
    h[0x26] = 0xFF;
    w32(&mut h, 0x2C, tiles);
    w32(&mut h, 0x34, units);
    h[0x3C] = 0xFF;
    for k in 0..3 {
        h[0x3D + k] = if (k as u8) + 2 <= map.armies {
            map.cos[k].unwrap_or(1)
        } else {
            0
        };
    }
    h[0x40..0x44].copy_from_slice(&map.colours);
    h[0x44..0x48].copy_from_slice(&[1, 2, 3, 4]);
    for i in 0..4 {
        h[0x48 + 4 * i..0x4C + 4 * i].copy_from_slice(&[0xFF, 0xFF, 0, 0]);
    }
    h
}

/// A map's tiles as the game loads them (LZ77 of width, height, tiles) and
/// its units (12-byte records; FE army, FF end).
/// Each pre-deployed unit's AI byte (record +9): 4, the one tangoAW2's
/// Versus maps use, under which the computer moves every unit (with 0, as
/// many campaign units have, it keeps some where they stand).
const AI: u8 = 4;

fn map_blobs(map: &maps::Map) -> (Vec<u8>, Vec<u8>) {
    let mut raw = vec![map.width, map.height];
    for t in &map.tiles {
        raw.extend_from_slice(&t.to_le_bytes());
    }
    let mut lz = crate::lz77::compress(&raw);
    while lz.len() % 4 != 0 {
        lz.push(0);
    }
    let mut units = Vec::new();
    for army in 1..=4u8 {
        let mine: Vec<&maps::Unit> = map.units.iter().filter(|u| u.army == army).collect();
        if mine.is_empty() {
            continue;
        }
        units.extend_from_slice(&[0xFE, army, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        for u in mine {
            units.extend_from_slice(&[u.x, u.y, u.kind, 0, u.hp, u.ammo, u.fuel, 0, 0, AI, 0, 0]);
        }
    }
    units.extend_from_slice(&[0xFF; 1]);
    units.extend_from_slice(&[0; 11]);
    (lz, units)
}

fn build() -> Option<Built> {
    let s = maps::survival()?;
    let mut writes = Vec::new();
    let mut names = Vec::new();
    put_text(&mut writes, T_HELP, HELP);
    for k in Kind::ALL {
        put_text(&mut writes, T_KINDS + entry_index(k) as u16, k.name());
    }
    for (k, map) in s.maps.iter().enumerate() {
        put_text(&mut writes, T_MAPS + k as u16, &map.name);
        let (lz, units) = map_blobs(map);
        let at = MAP_DATA + MAP_DATA_SIZE * k as u32;
        assert!(lz.len() + units.len() <= MAP_DATA_SIZE as usize, "{}", map.name);
        let units_at = at + lz.len() as u32;
        let id = MAPS_FROM + k as u8;
        writes.push((TABLE + 0x5C * id as u32, header(map, at, units_at, T_MAPS + k as u16).to_vec()));
        writes.push((at, lz));
        writes.push((units_at, units));
        names.push((id, T_MAPS + k as u16));
    }
    for k in Kind::ALL {
        let first = s.run(k).maps[0];
        let id = ENTRY_IDS[entry_index(k)];
        let at = MAP_DATA + MAP_DATA_SIZE * first as u32;
        let (lz, _) = map_blobs(&s.maps[first]);
        let units_at = at + lz.len() as u32;
        let name = T_KINDS + entry_index(k) as u16;
        writes.push((TABLE + 0x5C * id as u32, header(&s.maps[first], at, units_at, name).to_vec()));
        names.push((id, name));
    }
    Some(Built { writes, names })
}

/// Money, Turn, Time: the order of the entries in the list.
fn entry_index(k: Kind) -> usize {
    match k {
        Kind::Money => 0,
        Kind::Turn => 1,
        Kind::Time => 2,
    }
}

fn entry_kind(id: u8) -> Option<Kind> {
    ENTRY_IDS.iter().position(|&e| e == id).map(|i| Kind::ALL[i])
}

/// The last id the map list should walk to while Survival is on.
pub fn last_listed_id(core: &Core) -> Option<u8> {
    let n = maps::survival()?.maps.len() as u8;
    (core.raw_read_8(ON, -1) == 1).then_some(MAPS_FROM + n - 1)
}

fn installed(core: &Core) -> bool {
    core.raw_read_32(SENTINEL, -1) == MAGIC
}

fn install(core: &mut Core) -> bool {
    let Some(built) = BUILT.get_or_init(build).as_ref() else { return false };
    if !installed(core) {
        // The map table as crate::five_map leaves it (ids 0..0xC8), with room.
        let mut table = vec![0u8; (crate::five_map::ENTRY * crate::five_map::MAP_IDS) as usize];
        core.raw_read_range(crate::five_map::MAP_TABLE, -1, &mut table);
        core.raw_write_range(TABLE, -1, &table);
        for (at, b) in &built.writes {
            core.raw_write_range(*at, -1, b);
        }
        core.raw_write_32(SENTINEL, -1, MAGIC);
    }
    true
}

/// Every frame, before [`crate::five_map::show_maps`]: with the pack the
/// game reads tangoAW2's larger map table, without it five_map's.
pub fn tick_tables(core: &mut Core, on: bool) {
    let on = on && install(core);
    if on {
        crate::five_map::use_table(core, TABLE);
    } else if installed(core) {
        crate::five_map::use_table(core, crate::five_map::MAP_TABLE);
    }
}

// --- Per frame ----------------------------------------------------------------

fn kind(core: &Core) -> Kind {
    Kind::from_u8(core.raw_read_8(KIND, -1)).unwrap_or(Kind::Money)
}

/// The map id of map `k` of the run.
fn run_map_id(kind: Kind, k: usize) -> Option<u8> {
    let s = maps::survival()?;
    if k == 0 {
        return Some(ENTRY_IDS[entry_index(kind)]);
    }
    Some(MAPS_FROM + *s.run(kind).maps.get(k)? as u8)
}

/// The run's map for map id `id`, if it is one of ours.
fn map_of(id: u8) -> Option<&'static maps::Map> {
    let s = maps::survival()?;
    if let Some(k) = entry_kind(id) {
        return s.maps.get(s.run(k).maps[0]);
    }
    s.maps.get(id.checked_sub(MAPS_FROM)? as usize)
}

pub fn is_survival_map(id: u8) -> bool {
    id >= FIRST_ID && map_of(id).is_some()
}

fn set16(core: &mut Core, at: u32, v: u16) {
    if core.raw_read_16(at, -1) != v {
        core.raw_write_16(at, -1, v);
    }
}

fn set8(core: &mut Core, at: u32, v: u8) {
    if core.raw_read_8(at, -1) != v {
        core.raw_write_8(at, -1, v);
    }
}

/// The tabs and names of our maps, as the run stands.
fn list(core: &mut Core) {
    let Some(built) = BUILT.get().and_then(|b| b.as_ref()) else { return };
    let on = core.raw_read_8(ON, -1) == 1;
    let phase = core.raw_read_8(PHASE, -1);
    let stage = core.raw_read_8(STAGE, -1) as usize;
    let next = (on && (phase == BETWEEN || phase == PLAYING)).then(|| run_map_id(kind(core), stage)).flatten();
    for &(id, name) in &built.names {
        let shown = if !on {
            false
        } else if phase == CHOOSING || phase == CLEARED || phase == LOST {
            entry_kind(id).is_some()
        } else {
            Some(id) == next
        };
        let h = TABLE + 0x5C * id as u32;
        set16(core, h + 0x1A, if shown { TAB } else { crate::five_map::HIDDEN_TAB });
        set16(core, h + 0x14, TEXT_BASE + name);
    }
}

/// Every frame with the pack on: Survival's state, the War Room switches,
/// the battle's budget. Returns the keys the game sees.
pub fn tick(core: &mut Core, ds: bool, keys: u32, prev: u32) -> u32 {
    let mut keys = keys;
    if !ds || !installed(core) {
        if core.raw_read_8(ON, -1) != 0 {
            core.raw_write_8(ON, -1, 0);
        }
        set8(core, WAR_ROOM_TAB, WAR_ROOM_TAB_GAME);
        return keys;
    }
    // Picked on Select Mode: Survival's War Room opens. Back on Select
    // Mode otherwise: it is closed (a run in progress is left).
    if running(core, MAIN_MENU) {
        if core.raw_read_8(MENU_PICKED, -1) != 0 {
            set8(core, ON, 1);
            set8(core, PHASE, CHOOSING);
            // SELECT MAP opens on its first row (the War Room's own row
            // may be past the end of Survival's three).
            core.raw_write_32(LIST_FIRST, -1, 0);
            core.raw_write_32(LIST_CURSOR, -1, 0);
        } else if core.raw_read_8(ON, -1) != 0 {
            core.raw_write_8(ON, -1, 0);
            core.raw_write_8(PHASE, -1, CHOOSING);
        }
    } else if core.raw_read_8(MENU_PICKED, -1) != 0 {
        core.raw_write_8(MENU_PICKED, -1, 0);
    }
    let on = core.raw_read_8(ON, -1) == 1;
    set8(core, WAR_ROOM_TAB, if on { TAB as u8 } else { WAR_ROOM_TAB_GAME });
    let rows = if on { ROWS.wrapping_sub(ROW * (FIRST_ID as u32 - 0x6C)) } else { RECORDS };
    for at in RECORD_POOLS {
        if core.raw_read_32(at, -1) != rows {
            core.raw_write_32(at, -1, rows);
        }
    }
    list(core);
    if !on {
        return keys;
    }
    if core.raw_read_32(ROWS, -1) != 0 || core.raw_read_32(ROWS + ROW * 3, -1) != 0 {
        core.raw_write_range(ROWS, -1, &vec![0u8; ROWS_LEN as usize]);
    }
    let phase = core.raw_read_8(PHASE, -1);
    match phase {
        PLAYING => battle_frame(core),
        BETWEEN => {
            // The run's CO is kept: whatever the CO screen picks.
            let co = core.raw_read_8(CO, -1);
            set8(core, PLAYST_CO + 1, co);
        }
        CLEARED | LOST if running(core, SELECT_MAP) => {
            // The results panel: A or B closes it (and is not seen by the list).
            let pressed = keys & !prev;
            if pressed & 3 != 0 {
                core.raw_write_8(PHASE, -1, CHOOSING);
            }
            keys &= !3;
        }
        _ => {}
    }
    keys
}

fn battle_frame(core: &mut Core) {
    if core.raw_read_32(BATTLE_SCENE, -1) == 0 {
        return;
    }
    let p1 = player(core, 1);
    let left = core.raw_read_32(LEFT, -1);
    let out = match kind(core) {
        Kind::Money => {
            // No income: anything that adds funds is taken back.
            let funds = core.raw_read_32(p1 + P_FUNDS, -1);
            let cap = core.raw_read_32(FUNDS_CAP, -1);
            if funds > cap {
                core.raw_write_32(p1 + P_FUNDS, -1, cap);
            } else if funds < cap {
                core.raw_write_32(FUNDS_CAP, -1, funds);
            }
            set16(core, PROPERTY_FUNDS, 0);
            core.raw_read_32(p1 + P_FUNDS, -1) == 0
        }
        Kind::Turn => core.raw_read_16(DAY, -1) as u32 > left,
        Kind::Time => {
            if core.raw_read_8(CURRENT_ARMY, -1) == 1 {
                let t = core.raw_read_32(TIME, -1) + 1;
                core.raw_write_32(TIME, -1, t);
            }
            core.raw_read_32(TIME, -1) / 60 >= left / 60
        }
    };
    if out && core.raw_read_8(OUT_SET, -1) == 0 {
        // Out of budget: the player's army yields (defeated at the game's
        // next rules check, as Yield does).
        core.raw_write_8(OUT_SET, -1, 1);
        core.raw_write_8(p1 + P_YIELD, -1, 1);
    }
}

// --- Map start and end --------------------------------------------------------

/// At every map start ([`crate::sandstorm`]'s trap): a Survival map starts
/// the run (an entry) or its next map, with its own rules.
pub fn map_start(core: &mut Core) {
    if core.raw_read_8(ON, -1) != 1 || !crate::ds_weather::is_on(core) {
        return;
    }
    let id = core.raw_read_8(MAP_ID, -1);
    let Some(map) = map_of(id) else { return };
    let phase = core.raw_read_8(PHASE, -1);
    if let Some(k) = entry_kind(id) {
        if phase != CHOOSING && phase != CLEARED && phase != LOST {
            return;
        }
        let Some(s) = maps::survival() else { return };
        core.raw_write_8(KIND, -1, k as u8);
        core.raw_write_8(STAGE, -1, 0);
        core.raw_write_32(BUDGET, -1, s.run(k).budget);
        core.raw_write_32(LEFT, -1, s.run(k).budget);
        core.raw_write_32(POINTS, -1, 0);
        core.raw_write_8(CO, -1, core.raw_read_8(PLAYST_CO + 1, -1));
    } else if phase != BETWEEN || run_map_id(kind(core), core.raw_read_8(STAGE, -1) as usize) != Some(id) {
        return;
    }
    core.raw_write_8(PHASE, -1, PLAYING);
    core.raw_write_32(TIME, -1, 0);
    core.raw_write_8(OUT_SET, -1, 0);
    // The run's CO.
    let co = core.raw_read_8(CO, -1);
    core.raw_write_8(PLAYST_CO + 1, -1, co);
    core.raw_write_8(player(core, 1) + P_CO, -1, co);
    // Weather: 0 clear, 1 snow, 2 rain, 3 sandstorm (fixed weather with
    // clear as the default, crate::sandstorm).
    let (mode, w) = match map.weather {
        1 => (3, 1),
        2 => (3, 2),
        3 => (3, 0),
        _ => (0, 0),
    };
    core.raw_write_8(WEATHER_MODE, -1, mode);
    core.raw_write_8(DEFAULT_WEATHER, -1, w);
    core.raw_write_8(WEATHER, -1, w);
    core.raw_write_8(NEXT_WEATHER, -1, w);
    // Fog: the War Room sets gPlaySt's fog from the header of the map it
    // started with (`sub_080346FC`, before a map is picked), so it is set here.
    core.raw_write_8(FOG, -1, map.fog as u8);
    // Dual Strike's look: Wasteland, or Normal (its snow and desert looks
    // are not in tangoAW2).
    crate::wasteland::set_biome(
        core,
        if map.look == 3 { crate::wasteland::WASTELAND } else { crate::wasteland::NORMAL },
    );
    if kind(core) == Kind::Money {
        let left = core.raw_read_32(LEFT, -1);
        core.raw_write_32(player(core, 1) + P_FUNDS, -1, left);
        core.raw_write_32(FUNDS_CAP, -1, left);
        core.raw_write_16(PROPERTY_FUNDS, -1, 0);
    }
}

/// `EndOfGame_Finish` (every battle's end, before the War Room's save
/// prompt): the run goes on, is cleared, or is lost.
pub const END_OF_GAME: u32 = 0x0803_832C;
fn end_of_game(core: &mut Core) {
    if core.raw_read_8(ON, -1) != 1 || core.raw_read_8(PHASE, -1) != PLAYING {
        return;
    }
    let id = core.raw_read_8(MAP_ID, -1);
    if !is_survival_map(id) {
        return;
    }
    let p1 = player(core, 1);
    let won = core.raw_read_16(p1 + P_DEFEATED, -1) == 0 && core.raw_read_8(p1 + P_YIELD, -1) == 0;
    let k = kind(core);
    let left = core.raw_read_32(LEFT, -1);
    if !won {
        core.raw_write_8(PHASE, -1, LOST);
        core.raw_write_32(LEFT, -1, 0);
        core.raw_write_32(BONUS, -1, 0);
        core.raw_write_8(RANK, -1, 0);
        return;
    }
    let cost = match k {
        Kind::Money => left.saturating_sub(core.raw_read_32(p1 + P_FUNDS, -1)),
        Kind::Turn => (core.raw_read_16(DAY, -1) as u32).saturating_sub(1),
        Kind::Time => core.raw_read_32(TIME, -1) / 60 * 60,
    };
    let left = left.saturating_sub(cost);
    core.raw_write_32(LEFT, -1, left);
    let score = core.raw_read_16(p1 + P_SCORE, -1) as u32;
    core.raw_write_16(LAST_SCORE, -1, score as u16);
    let points = (core.raw_read_32(POINTS, -1) + score).min(maps::MAX_POINTS);
    core.raw_write_32(POINTS, -1, points);
    let stage = core.raw_read_8(STAGE, -1) + 1;
    core.raw_write_8(STAGE, -1, stage);
    if stage as usize >= MAPS_PER_RUN {
        let bonus = maps::leftover_points(k, left);
        core.raw_write_32(BONUS, -1, bonus);
        core.raw_write_32(POINTS, -1, (points + bonus).min(maps::MAX_POINTS));
        let rank = maps::rank(k, left);
        core.raw_write_8(RANK, -1, rank);
        save_record(core, k, rank, left);
        core.raw_write_8(PHASE, -1, CLEARED);
    } else {
        core.raw_write_8(PHASE, -1, BETWEEN);
    }
    // SELECT MAP lists only the next map now: its cursor starts at the top
    // (it keeps the row last picked, which may be past the end).
    core.raw_write_32(LIST_FIRST, -1, 0);
    core.raw_write_32(LIST_CURSOR, -1, 0);
}

/// `SetMapPlayed` (id r0): the "played" bits stop at id 0xBF; Survival's
/// ids keep out of the bytes after them.
const SET_MAP_PLAYED: u32 = 0x0803_CA28;
fn set_map_played(core: &mut Core) {
    let id = core.gba().cpu().gpr(0) as u32;
    if id >= FIRST_ID as u32 && core.raw_read_8(ON, -1) == 1 {
        let lr = core.gba().cpu().gpr(14) as u32;
        core.gba_mut().cpu_mut().set_thumb_pc(lr & !1);
    }
}

/// `DrawMapList(first, count, top)`: the War Room draws `count` rows
/// whatever the list holds (it always has more than seven maps); Survival's
/// list has fewer, so the rows past its end are left out, as Versus does.
const DRAW_MAP_LIST: u32 = 0x0808_6A58;
const MAP_LIST_LAST: u32 = 0x0202_7FAB;
fn draw_map_list(core: &mut Core) {
    if core.raw_read_8(ON, -1) != 1 {
        return;
    }
    let cpu = core.gba().cpu();
    let (first, count) = (cpu.gpr(0) as u32, cpu.gpr(1) as u32);
    let n = core.raw_read_8(MAP_LIST_LAST, -1) as u32 + 1;
    if first + count > n {
        core.gba_mut().cpu_mut().set_gpr(1, n.saturating_sub(first) as i32);
    }
}

/// `WarRoomMapSelected`'s first call (the CO screen starting; its CO list
/// is built by then): from the second map on, the run's CO is kept (Dual
/// Strike's COs are picked once a run), so the screen offers only that CO,
/// in its own army's group, as the Campaign's restricted CO screens do
/// (`AddCoSelectGroup*`: groups, their sizes and colours, the CO ids).
const CO_SCREEN: u32 = 0x0807_C588;
const CO_GROUPS: u32 = 0x0300_5944;
const CO_GROUP_SIZES: u32 = 0x0300_5948;
const CO_GROUP_COLOURS: u32 = 0x0300_5958;
const CO_LIST: u32 = 0x0300_58E0;
fn co_screen(core: &mut Core) {
    if core.raw_read_8(ON, -1) != 1 || core.raw_read_8(PHASE, -1) != BETWEEN || core.raw_read_8(STAGE, -1) == 0 {
        return;
    }
    let co = core.raw_read_8(CO, -1);
    let groups = core.raw_read_32(CO_GROUPS, -1).min(8);
    let mut at = 0;
    let mut colour = None;
    for g in 0..groups {
        let n = core.raw_read_8(CO_GROUP_SIZES + g, -1) as u32;
        if (0..n).any(|k| core.raw_read_8(CO_LIST + at + k, -1) == co) {
            colour = Some(core.raw_read_8(CO_GROUP_COLOURS + g, -1));
        }
        at += n;
    }
    let Some(colour) = colour else { return };
    core.raw_write_32(CO_GROUPS, -1, 1);
    core.raw_write_8(CO_GROUP_SIZES, -1, 1);
    core.raw_write_8(CO_GROUP_COLOURS, -1, colour);
    core.raw_write_8(CO_LIST, -1, co);
}

/// The map menu's Save test (`savingEnabled`, after its first load): a
/// Survival map hides Save, as linked play does (a suspended map would
/// come back without its run).
const SAVE_ITEM: u32 = 0x0802_C646;
fn save_item(core: &mut Core) {
    if core.raw_read_8(ON, -1) == 1 && core.raw_read_8(PHASE, -1) == PLAYING {
        let cpu = core.gba_mut().cpu_mut();
        cpu.set_gpr(0, 1);
        let lr = cpu.gpr(14) as u32;
        cpu.set_thumb_pc(lr & !1);
    }
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![
        (CO_SCREEN, Box::new(co_screen)),
        (SAVE_ITEM, Box::new(save_item)),
        (DRAW_MAP_LIST, Box::new(draw_map_list)),
        (END_OF_GAME, Box::new(end_of_game)),
        (SET_MAP_PLAYED, Box::new(set_map_played)),
    ]
}

// --- Records -------------------------------------------------------------------

fn record_slot(k: Kind) -> u32 {
    PROFILE_RECORDS + 1 + 3 * k as u32
}

fn record_unit(k: Kind) -> u32 {
    match k {
        Kind::Time => 60,
        Kind::Money => 100,
        Kind::Turn => 1,
    }
}

/// A kind's best clear: (rank, CO, what was left).
pub fn record(core: &Core, k: Kind) -> Option<(u8, u8, u32)> {
    if core.raw_read_8(PROFILE_RECORDS, -1) != RECORD_MAGIC {
        return None;
    }
    let at = record_slot(k);
    let v = core.raw_read_8(at, -1) as u32 | (core.raw_read_8(at + 1, -1) as u32) << 8 | (core.raw_read_8(at + 2, -1) as u32) << 16;
    let rank = (v & 7) as u8;
    (rank != 0).then(|| (rank, ((v >> 3) & 0x7F) as u8, (v >> 10) * record_unit(k)))
}

fn save_record(core: &mut Core, k: Kind, rank: u8, left: u32) {
    if let Some((_, _, best)) = record(core, k) {
        if best / record_unit(k) >= left / record_unit(k) {
            return;
        }
    }
    if core.raw_read_8(PROFILE_RECORDS, -1) != RECORD_MAGIC {
        core.raw_write_range(PROFILE_RECORDS, -1, &[RECORD_MAGIC, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }
    let co = core.raw_read_8(CO, -1) as u32 & 0x7F;
    let v = rank as u32 & 7 | co << 3 | ((left / record_unit(k)).min(0x3FFF)) << 10;
    let at = record_slot(k);
    core.raw_write_range(at, -1, &[v as u8, (v >> 8) as u8, (v >> 16) as u8]);
}

// --- Sprites ---------------------------------------------------------------------

/// AW2's own glyph font (`PutAsciiGlyphSprite`): OBJ tiles 0x3C0.. on the
/// battle map and the War Room's screens, OBJ palette 0.
fn glyph(c: char) -> Option<u16> {
    Some(match c {
        '0'..='9' => 0x3D0 + (c as u16 - '0' as u16),
        'A'..='Z' => 0x3E0 + (c as u16 - 'A' as u16),
        'a'..='z' => 0x3E0 + (c as u16 - 'a' as u16),
        '$' => 0x3DA, // the funds G
        '!' => 0x3DB,
        '?' => 0x3DC,
        '(' => 0x3FA,
        ')' => 0x3FB,
        '.' | ',' => 0x3FC,
        '/' => 0x3FD,
        '%' => 0x3FE,
        '-' => 0x3FF,
        _ => return None,
    })
}

struct Sprites {
    at: u32,
    end: u32,
}

impl Sprites {
    fn put(&mut self, core: &mut Core, x: i32, y: i32, attr2: u16, size_bits: (u16, u16)) {
        if self.at + 8 > self.end || x <= -64 || x >= 240 || y <= -64 || y >= 160 {
            return;
        }
        core.raw_write_16(self.at, -1, (y as u16 & 0xFF) | size_bits.0);
        core.raw_write_16(self.at + 2, -1, (x as u16 & 0x1FF) | size_bits.1);
        core.raw_write_16(self.at + 4, -1, attr2);
        self.at += 8;
    }
    /// Text in the glyph font; ':' is two dots.
    fn text(&mut self, core: &mut Core, x: i32, y: i32, s: &str) {
        let mut cx = x;
        for c in s.chars() {
            if c == ':' {
                self.put(core, cx - 2, y - 1, 0x3FC, (0, 0));
                self.put(core, cx - 2, y - 5, 0x3FC, (0, 0));
                cx += 4;
                continue;
            }
            if let Some(t) = glyph(c) {
                self.put(core, cx, y, t, (0, 0));
            }
            cx += 8;
        }
    }
}

/// The glyph font in ROM (`LoadGlyphSpriteTiles` copies it to OBJ tile
/// 0x3C0), for text drawn into tiles of our own.
const FONT: u32 = 0x080A_1424;
/// OBJ tiles free on SELECT MAP (0x304..0x39F are unused there): a panel
/// of 8 lines of 16 glyphs, as four 64x32 sprites.
const PANEL_TILES: u32 = 0x310;
const OBJ_VRAM: u32 = 0x0601_0000;

fn glyph_tile(core: &Core, c: char) -> [u8; 32] {
    let mut out = [0u8; 32];
    if c == ':' {
        // Two of the full stop's dots, one above the other.
        let mut dot = [0u8; 32];
        core.raw_read_range(FONT + 32 * (0x3FC - 0x3C0), -1, &mut dot);
        for y in 0..8 {
            for b in 0..4 {
                out[4 * y + b] = dot[4 * y + b];
                if y + 4 < 8 {
                    let above = dot[4 * (y + 4) + b];
                    for half in 0..2 {
                        let nib = (above >> (4 * half)) & 0xF;
                        if nib != 0 {
                            out[4 * y + b] = (out[4 * y + b] & !(0xF << (4 * half))) | (nib << (4 * half));
                        }
                    }
                }
            }
        }
    } else if let Some(t) = glyph(c) {
        core.raw_read_range(FONT + 32 * (t as u32 - 0x3C0), -1, &mut out);
    }
    out
}

/// Up to 8 lines of 16 characters at (x, y), in the glyph font, from
/// tiles of our own (SELECT MAP has no room for a sprite per glyph).
/// The panel's plate: OBJ palette 0's colour 14, a dark slate both on
/// SELECT MAP and in battle (the glyphs are white, outlined in colour 15).
const PLATE: u8 = 14;

fn panel(core: &mut Core, sp: &mut Sprites, x: i32, y: i32, lines: &[String]) {
    let mut buf = vec![0u8; 128 * 32];
    let mut used = [false; 4];
    let rows = lines.len().min(8);
    for block in 0..4 {
        if block / 2 * 4 < rows {
            used[block] = true;
        }
    }
    for (row, line) in lines.iter().take(8).enumerate() {
        for (col, c) in line.chars().take(16).enumerate() {
            if c == ' ' {
                continue;
            }
            let block = (row / 4) * 2 + col / 8;
            let t = block * 32 + (row % 4) * 8 + col % 8;
            buf[t * 32..t * 32 + 32].copy_from_slice(&glyph_tile(core, c));
        }
    }
    // The plate under the glyphs: every transparent pixel of the panel.
    for (block, &u) in used.iter().enumerate() {
        if !u {
            continue;
        }
        for b in &mut buf[block * 32 * 32..(block + 1) * 32 * 32] {
            if *b & 0x0F == 0 {
                *b |= PLATE;
            }
            if *b & 0xF0 == 0 {
                *b |= PLATE << 4;
            }
        }
    }
    let at = OBJ_VRAM + 32 * PANEL_TILES;
    let mut now = vec![0u8; buf.len()];
    core.raw_read_range(at, -1, &mut now);
    if now != buf {
        core.raw_write_range(at, -1, &buf);
    }
    for (block, &u) in used.iter().enumerate() {
        if u {
            let (bx, by) = (x + 64 * (block as i32 % 2), y + 32 * (block as i32 / 2));
            sp.put(core, bx, by, (PANEL_TILES + 32 * block as u32) as u16, (0x4000, 0xC000));
        }
    }
}

fn clock(frames: u32) -> String {
    let s = frames / 60;
    format!("{}:{:02}", s / 60, s % 60)
}

fn left_text(k: Kind, left: u32) -> String {
    match k {
        Kind::Money => format!("${}", left),
        Kind::Turn => format!("{} DAYS", left),
        Kind::Time => clock(left),
    }
}

/// The budget on the battle map: what is left now.
fn hud(core: &mut Core, sp: &mut Sprites) {
    if core.raw_read_32(MAIN_CALLBACK, -1) != MAP_CALLBACK {
        return;
    }
    let k = kind(core);
    let left = core.raw_read_32(LEFT, -1);
    let now = match k {
        Kind::Money => core.raw_read_32(player(core, 1) + P_FUNDS, -1),
        // Today included: a map won on day d costs d - 1.
        Kind::Turn => (left + 1).saturating_sub(core.raw_read_16(DAY, -1) as u32),
        Kind::Time => left.saturating_sub(core.raw_read_32(TIME, -1)),
    };
    let stage = core.raw_read_8(STAGE, -1) as u32 + 1;
    let line1 = format!("{}/{}", stage, MAPS_PER_RUN);
    let line2 = left_text(k, now);
    let w = 8 * line2.chars().filter(|&c| c != ':').count() as i32 + 4 * line2.contains(':') as i32;
    let x = 120 - w / 2;
    sp.text(core, 120 - 8 * line1.len() as i32 / 2, 2, &line1);
    sp.text(core, x, 11, &line2);
}

/// On SELECT MAP: the run's results (over the map preview), or the
/// highlighted kind's budget and record (under it), or the run so far.
fn select_map_panel(core: &mut Core, sp: &mut Sprites) {
    let phase = core.raw_read_8(PHASE, -1);
    let k = kind(core);
    let mut lines: Vec<String> = Vec::new();
    let (x, y);
    match phase {
        CLEARED | LOST => {
            let stage = core.raw_read_8(STAGE, -1);
            lines.push(k.name().to_uppercase());
            lines.push(if phase == CLEARED { "CLEAR!".into() } else { "GAME OVER".into() });
            lines.push(format!("MAPS {}/{}", stage, MAPS_PER_RUN));
            if phase == CLEARED {
                let left = core.raw_read_32(LEFT, -1);
                lines.push(format!("LEFT {}", left_text(k, left)));
                lines.push(format!("BONUS {}", core.raw_read_32(BONUS, -1)));
            }
            lines.push(format!("POINTS {}", core.raw_read_32(POINTS, -1)));
            if phase == CLEARED {
                lines.push(format!("RANK {}", maps::rank_letter(core.raw_read_8(RANK, -1))));
            }
            lines.push("PRESS A".into());
            (x, y) = (112, 36);
        }
        BETWEEN => {
            let left = core.raw_read_32(LEFT, -1);
            let stage = core.raw_read_8(STAGE, -1) as usize;
            lines.push(format!("MAP {}/{}", stage + 1, MAPS_PER_RUN));
            lines.push(format!("LEFT {}", left_text(k, left)));
            lines.push(format!("POINTS {}", core.raw_read_32(POINTS, -1)));
            (x, y) = (120, 104);
        }
        _ => {
            let (first, cursor) = (core.raw_read_32(LIST_FIRST, -1), core.raw_read_32(LIST_CURSOR, -1));
            let id = core.raw_read_8(LIST_IDS + (first + cursor).min(0x31), -1);
            let Some(k) = entry_kind(id) else { return };
            let b = maps::survival().map_or(0, |s| s.run(k).budget);
            lines.push(format!("{} MAPS", MAPS_PER_RUN));
            lines.push(format!("BUDGET {}", left_text(k, b)));
            lines.push(match record(core, k) {
                Some((rank, _co, left)) => format!("BEST {} {}", maps::rank_letter(rank), left_text(k, left)),
                None => "BEST -".into(),
            });
            (x, y) = (120, 104);
        }
    }
    panel(core, sp, x, y, &lines);
}

/// SELECT MAP's list: the first entry shown, the cursor's row, the ids.
const LIST_FIRST: u32 = 0x0300_5900;
const LIST_CURSOR: u32 = 0x0300_5930;
const LIST_IDS: u32 = 0x0202_7F78;

/// At the sprite flush ([`crate::branding::flush`]).
pub fn flush_sprites(core: &mut Core, at: u32, end: u32) -> u32 {
    if core.raw_read_8(ON, -1) != 1 || !crate::ds_weather::is_on(core) {
        return at;
    }
    let mut sp = Sprites { at, end };
    let phase = core.raw_read_8(PHASE, -1);
    if phase == PLAYING && core.raw_read_32(BATTLE_SCENE, -1) != 0 {
        hud(core, &mut sp);
    } else if running(core, SELECT_MAP) {
        select_map_panel(core, &mut sp);
    }
    sp.at
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout() {
        assert!(ROWS + ROWS_LEN <= 0x0203_FD60, "rows end before cpu_tactics' RAM");
        assert_eq!(ENTRY_IDS[2] + 1, MAPS_FROM);
        assert!((MAPS_FROM as u32 + 33) <= TABLE_IDS);
        assert!(MAP_DATA + MAP_DATA_SIZE * 33 <= STRINGS);
        assert!(text_slot(T_MAPS + 33) < 0x0862_E000);
        assert_eq!(clock(90000), "25:00");
    }
}
