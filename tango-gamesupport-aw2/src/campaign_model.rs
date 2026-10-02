//! The campaign model: a campaign as tangoAW2's campaign engine
//! ([`crate::ds_campaign`]) plays it, whatever it came from.
//!
//! A **source** loads a campaign into a [`Model`]: its missions compiled
//! for AW2 (map headers, maps, deployments, AW2 event scripts and trigger
//! lists, texts, magic stubs: [`Built`]), its play order, side missions and
//! last mission, its story (prologue, scenes after wins), its staff roll,
//! and its rules (the magic functions its scripts call, [`Source::rules`]).
//! The engine knows nothing of where a campaign came from: it reads the
//! model. [`SOURCES`] lists the sources; the Campaign sub-menu
//! ([`crate::campaign_menu`]) shows AW2's own campaign, then one entry per
//! source whose campaign is there (any number: two are shown at a time).
//!
//! Today's only source is Dual Strike's ([`crate::ds_campaign_data`], with
//! its world map [`crate::ds_worldmap`], story pictures
//! [`crate::ds_story_art`], staff roll [`crate::ds_credits`] and songs
//! [`crate::ds_music`]): it reads the player's own Dual Strike ROM (the
//! pack). Another campaign (a custom one) is a source that fills a
//! [`Model`] the same way: its missions as [`MissionInfo`] and map
//! headers, its scripts in AW2's event format (laid out with [`Built::add`]
//! and [`Built::add_magic`]), its order; its rules are its own (or none).

use std::collections::BTreeMap;

use mgba::core::Core;

/// The most missions a campaign has (the progress keeps one bit each).
pub const MAX_MISSIONS: usize = 32;

/// The AW2 map id a DS mission is played on: its header is written into
/// the map table's entry for it when the mission starts (tangoAW2's map
/// table with room for 0x100 ids, [`crate::survival::TABLE`]; Survival
/// uses 0xC9..0xEC).
pub const MAP_ID: u8 = 0xF0;

/// A campaign as the engine plays it.
pub struct Model {
    /// The Campaign sub-menu's label (the chooser's 5x10 letters).
    pub label: &'static str,
    /// Its missions compiled for AW2.
    pub built: Built,
    /// How many missions (indices 0..missions; at most [`MAX_MISSIONS`]).
    pub missions: usize,
    /// The story missions in play order, the side missions among them, each
    /// with the campaign flag that opens it (played when set, skipped
    /// otherwise), and the last mission (its win is the campaign's end).
    pub order: Vec<u8>,
    pub side_missions: Vec<(u8, u32)>,
    pub final_mission: u8,
    /// The staff roll after the last mission's scenes (crate::ds_credits).
    pub credits: Option<crate::ds_credits::Credits>,
    /// Pictures the story's narration shows (crate::ds_story_art), by the
    /// number its scripts' picture flow carries.
    pub pictures: Vec<Option<crate::ds_story_art::Picture>>,
    /// Its rules.
    pub source: &'static Source,
}

/// Where campaigns come from.
pub struct Source {
    /// The Campaign sub-menu's label.
    pub label: &'static str,
    /// Its campaign is there (Dual Strike's: the pack).
    pub available: fn(&Core) -> bool,
    /// Loads it (once: the engine keeps it).
    pub load: fn(&Core) -> Option<Model>,
    /// Runs a magic function of its own (a predicate's answer, a call's
    /// result); [`TAIL_CALLED`] when it has jumped to game code itself.
    pub rules: fn(&mut Core, &Magic) -> u32,
}

/// A rule that jumped to game code itself (the caller returns nothing).
pub const TAIL_CALLED: u32 = u32::MAX;

/// The sources, in the sub-menu's order (after AW2's own campaign).
pub static SOURCES: [Source; 1] = [Source {
    label: "DS CAMPAIGN",
    available: crate::ds_weather::is_on,
    load: crate::ds_campaign_data::load,
    rules: crate::ds_campaign_rules::run,
}];

/// What a magic function stands for: Rust runs it ([`crate::ds_campaign`]'s
/// own flow, else the campaign's source's rules, [`Source::rules`]). The
/// source's variants carry the source's own keys (Dual Strike's: the
/// addresses of its predicates and functions).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Magic {
    /// A Dual Strike predicate (`u8 f(void)`), by its address.
    Predicate(u32),
    /// Script op 0x4E: army's (CO, tag CO) is (a, b) (AW2 ids; 0xFF any).
    CoPair { army: u8, a: u8, b: u8 },
    /// A Dual Strike function called by a script (op 0x00/0x52/0x55/0x57).
    Call(u32, u32),
    /// Op 0x5A: a real-time countdown (frames); 0x5B clears it.
    Countdown(u32),
    /// Op 0x41/0x43/0x44: an army's units shown or hidden (Dual Strike's
    /// flag 0x80 on every unit of the army).
    ArmyFlag { op: u8, army: u8 },
    /// A Dual Strike op AW2 has nothing for (kept for the documentation).
    Unhandled(u8),
    /// The DS Campaign's own flow (crate::ds_campaign): its id.
    Flow(u8),
}

/// A campaign's story outside its battles, as AW2 event scripts (their
/// addresses): the prologue before the first mission, and scenes played on
/// the world map after a mission's win.
#[derive(Clone, Debug, Default)]
pub struct Story {
    pub prologue: u32,
    /// (mission index, script) to play on the world map after its win.
    pub after_win: Vec<(usize, u32)>,
}

#[derive(Clone, Debug)]
pub struct Built {
    pub story: Story,
    /// The ROM blob, to be written at [`Built::base`].
    pub blob: Vec<u8>,
    pub base: u32,
    /// AW2 text ids and their strings' addresses (in the blob).
    pub texts: Vec<(u16, u32)>,
    /// Map headers (0x5C bytes) by record index (missions, then fronts).
    pub headers: Vec<(u8, [u8; 0x5C])>,
    /// Magic functions by id (the stub's r3).
    pub magic: Vec<Magic>,
    /// Stubs: id -> address of its Thumb stub.
    pub stubs: Vec<u32>,
    pub missions: Vec<MissionInfo>,
    /// Dual Strike script ops seen and not converted, with counts.
    pub unhandled: BTreeMap<u8, u32>,
}

impl Built {
    /// Appends a magic function's stub to the blob; its Thumb address.
    pub fn add_magic(&mut self, m: Magic) -> u32 {
        while self.blob.len() % 4 != 0 {
            self.blob.push(0);
        }
        let id = self.magic.len() as u32;
        self.magic.push(m);
        let at = self.base + self.blob.len() as u32;
        self.blob.extend_from_slice(&stub(id));
        self.stubs.push(at);
        at | 1
    }

    /// Appends bytes (word-aligned); their address.
    pub fn add(&mut self, b: &[u8]) -> u32 {
        while self.blob.len() % 4 != 0 {
            self.blob.push(0);
        }
        let at = self.base + self.blob.len() as u32;
        self.blob.extend_from_slice(b);
        at
    }
}

#[derive(Clone, Debug)]
pub struct MissionInfo {
    pub index: usize,
    pub name: String,
    /// The world map's mission panel text: the mission's objective (the
    /// first text of its objective script), two lines.
    pub info_text: u16,
    /// The record index of its second front (not played: see docs/AW2.md).
    pub second_front: Option<u8>,
    pub number: u8,
    pub cos: [(u8, u8); 4],
    pub colours: [u8; 4],
    pub teams: [u8; 4],
    pub armies: u8,
    pub pool: Vec<u8>,
    pub day_limit: u16,
    pub width: u8,
    pub height: u8,
    pub look: u8,
    pub weather: u8,
    pub fog: bool,
    /// Dual Strike's research labs on the map (its Lab tiles, 0x1D9..0x1DD;
    /// its Com Towers 0x1B9..0x1BD become the same AW2 tiles).
    pub labs: Vec<(u8, u8)>,
}

/// The landing every magic stub jumps to: dead code in `sub_0803CC3C`
/// (no callers; [`crate::five_map`] made its start a helper), trapped by
/// [`crate::ds_campaign`]. r3 = the magic id, lr = the caller's return.
pub const LANDING: u32 = 0x0803_CC5E;

pub fn stub(id: u32) -> [u8; 16] {
    let mut s = [0u8; 16];
    let h: [u16; 4] = [
        0x4B01, // ldr r3, [pc, #4] (id)
        0x4A02, // ldr r2, [pc, #8] (landing)
        0x4710, // bx r2
        0x46C0, // nop
    ];
    for (k, v) in h.iter().enumerate() {
        s[2 * k..2 * k + 2].copy_from_slice(&v.to_le_bytes());
    }
    s[8..12].copy_from_slice(&id.to_le_bytes());
    s[12..16].copy_from_slice(&(LANDING | 1).to_le_bytes());
    s
}

/// AW2 text ids for the campaign's strings: read from the text table's
/// free tail (`0x08610A38 + 4 * id`, free ROM from 0x0862DA38; ids are
/// read signed, so at most 0x7FFF).
pub const TEXT_FIRST: u16 = 0x7400;
pub const TEXT_LAST: u16 = 0x7FFF;
pub const TEXT_TABLE: u32 = 0x0861_0A38;

