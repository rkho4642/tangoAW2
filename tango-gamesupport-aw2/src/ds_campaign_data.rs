//! Dual Strike's campaign, converted for AW2's campaign engine from the
//! player's Dual Strike ROM (the pack, [`crate::ds_pack`]) at run time.
//! Nothing of Dual Strike is in the repository: this module reads Dual
//! Strike's mission records, maps, deployments, event scripts and texts
//! from the pack and writes AW2's equivalents into a ROM blob
//! ([`build`]), which [`crate::ds_campaign`] installs and plays.
//!
//! What Dual Strike has (USA, overlay 0 at 0x022AD560, overlay 1 at
//! 0x02350560):
//!
//! - Map records: 0xA0 bytes each at 0x022DBD28 + 0xA0 * id, the campaign
//!   from id 0xE0: 28 missions, then 5 second fronts (0xFC..0x100, named
//!   by the main mission's +0x10). Layout (AW2's map header grown): +0x00
//!   the event header (6 trigger lists, overlay 1), +0x04 the objective
//!   script, +0x10 the second front's id, +0x14 the name (text id), +0x20
//!   the COs the player may pick from (0-ended list), +0x24 armies,
//!   +0x2C/+0x2E speed-rank days, +0x30/+0x32 day limit, +0x41 mission
//!   number, +0x44/+0x48 map (normal, hard), +0x4C/+0x50 deployment
//!   (normal, hard), +0x54 two bytes then a (CO, tag CO) pair per army
//!   (0x1C: the player picks), +0x88 colour per army slot, +0x8D team.
//! - Maps: LZ77, width, height, then a u16 tile per cell. Tile ids are
//!   AW2's (Dual Strike grew AW2's set), so most carry over; the new ones
//!   are mapped ([`remap_tile`]).
//! - Deployments: 13-byte records (x, y, type, flags, HP, ammo, fuel, -, -,
//!   AI, -, x, y); `FE army` starts an army, `FF` ends. AW2's are 12 bytes.
//! - Event headers: 6 trigger lists (AW2's 6: turn start, after supply,
//!   unit selected, after a unit's action, an action chosen, match end).
//!   Records are AW2's grouped by front (op = 3 * AW2 op + front variant:
//!   +0 main front, +1 second front, +2 either), opened by 0x15 (normal
//!   campaign only) or 0x16 (hard only), fired by 0x17..0x19, ended by
//!   0x1A.
//! - Event scripts: AW2's format (16 bytes a command), Dual Strike's
//!   handler table at arm9 0x021585C0 (0x5D opcodes); many are AW2's own
//!   ops at the same or a neighbouring number ([`convert_command`]).
//! - Texts: banks of string pointers by bank (ov0 0x022F6BF0: key << 24,
//!   pointer), a text reference being bank << 24 | index.

use std::collections::{BTreeMap, BTreeSet, HashMap};

pub const OV0: u32 = 0x022A_D560;
pub const OV1: u32 = 0x0235_0560;
const RECORDS: u32 = 0x022D_BD28;
const RECORD: u32 = 0xA0;
pub const FIRST_RECORD: u32 = 0xE0;
pub const MISSIONS: usize = 28;
pub const SECOND_FRONTS: usize = 5;
const TEXT_GROUPS: u32 = OV0 + 0x49690;
const MAP_NAMES_BASE: u32 = 0x022F_6BF8; // the 0xC0 bank (general texts)

/// AW2 map ids for the campaign: the 28 missions, then the 5 second fronts.
pub const MAP_ID_BASE: u8 = 0xD8;

/// A view of Dual Strike's memory from the pack.
pub struct Ds<'a> {
    pub arm9: &'a [u8],
    pub ov0: &'a [u8],
    pub ov1: &'a [u8],
}

impl<'a> Ds<'a> {
    pub fn from_pack(p: &'a crate::ds_pack::Pack) -> Option<Self> {
        Some(Ds { arm9: &p.arm9, ov0: p.overlays.first()?, ov1: p.overlays.get(1)? })
    }

    pub fn bytes(&self, a: u32, n: usize) -> Option<&'a [u8]> {
        let (base, data) = if (0x0200_0000..0x0200_0000 + self.arm9.len() as u32).contains(&a) {
            (0x0200_0000, self.arm9)
        } else if (OV0..OV0 + self.ov0.len() as u32).contains(&a) {
            (OV0, self.ov0)
        } else if (OV1..OV1 + self.ov1.len() as u32).contains(&a) {
            (OV1, self.ov1)
        } else {
            return None;
        };
        let o = (a - base) as usize;
        data.get(o..o + n)
    }

    pub fn u8(&self, a: u32) -> Option<u8> {
        self.bytes(a, 1).map(|b| b[0])
    }
    pub fn u16(&self, a: u32) -> Option<u16> {
        self.bytes(a, 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }
    pub fn u32(&self, a: u32) -> Option<u32> {
        self.bytes(a, 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// A NUL-ended string at a DS address.
    pub fn cstr(&self, a: u32) -> Option<Vec<u8>> {
        let mut out = Vec::new();
        for k in 0..0x800 {
            let c = self.u8(a + k)?;
            if c == 0 {
                return Some(out);
            }
            out.push(c);
        }
        Some(out)
    }

    /// A text by reference (bank << 24 | index).
    pub fn text(&self, r: u32) -> Option<Vec<u8>> {
        let mut p = TEXT_GROUPS;
        let group = loop {
            let key = self.u32(p)?;
            if key == u32::MAX {
                return None;
            }
            if key >> 24 == r >> 24 {
                break self.u32(p + 4)?;
            }
            p += 8;
        };
        let at = self.u32(group + 4 * (r & 0xFF_FFFF))?;
        self.cstr(at)
    }

    /// A text of the general table (map names, ...), by its id.
    pub fn name(&self, id: u16) -> Option<Vec<u8>> {
        let _ = MAP_NAMES_BASE;
        self.text(0xC000_0000 | id as u32)
    }
}

// --- Mission records -----------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Record {
    pub index: usize,
    pub name: u16,
    pub header: u32,
    pub objective: u32,
    pub second_front: u16,
    pub pool: u32,
    pub armies: u8,
    pub rank_days: (u16, u16),
    pub day_limit: (u16, u16),
    pub number: u8,
    pub maps: (u32, u32),
    pub units: (u32, u32),
    /// (CO, tag CO) per army slot 1..4 (0x1C: the player picks; 0: none).
    pub cos: [(u8, u8); 4],
    pub colours: [u8; 4],
    pub teams: [u8; 4],
    pub raw: Vec<u8>,
}

pub fn record(ds: &Ds, index: usize) -> Option<Record> {
    let a = RECORDS + RECORD * (FIRST_RECORD + index as u32);
    let r = ds.bytes(a, RECORD as usize)?.to_vec();
    let w = |o: usize| u32::from_le_bytes(r[o..o + 4].try_into().unwrap());
    let h = |o: usize| u16::from_le_bytes([r[o], r[o + 1]]);
    let mut cos = [(0u8, 0u8); 4];
    for (k, c) in cos.iter_mut().enumerate() {
        *c = (r[0x56 + 2 * k], r[0x57 + 2 * k]);
    }
    Some(Record {
        index,
        name: h(0x14),
        header: w(0x00),
        objective: w(0x04),
        second_front: h(0x10),
        pool: w(0x20),
        armies: r[0x24],
        rank_days: (h(0x2C), h(0x2E)),
        day_limit: (h(0x30), h(0x32)),
        number: r[0x41],
        maps: (w(0x44), w(0x48)),
        units: (w(0x4C), w(0x50)),
        cos,
        colours: [r[0x89], r[0x8A], r[0x8B], r[0x8C]],
        teams: [r[0x8E], r[0x8F], r[0x90], r[0x91]],
        raw: r,
    })
}

/// The COs the player may pick from (Dual Strike ids), if the mission has a pool.
pub fn pool(ds: &Ds, rec: &Record) -> Vec<u8> {
    let mut out = Vec::new();
    if rec.pool == 0 {
        return out;
    }
    for k in 0..32 {
        match ds.u8(rec.pool + k) {
            Some(0) | None => break,
            Some(c) => out.push(c),
        }
    }
    out
}

// --- Ids -----------------------------------------------------------------------

/// AW2 CO id for a Dual Strike CO id (None: none, or the player's pick).
pub fn aw2_co(ds_co: u8) -> Option<u8> {
    let c = ds_co & 0x7F;
    if c == 0 || c >= 0x1C {
        return None;
    }
    (0..crate::co_roster::AW2_COS)
        .find(|&a| crate::co_roster::ds_co(a) == Some(c))
        .or_else(|| crate::co_new::NEW.iter().position(|&(d, _)| d == c).map(|k| crate::co_new::FIRST + k as u8))
}

/// AW2's troopers (faces 19..23), for Dual Strike's soldiers 0x1C..0x20
/// (Orange Star, Blue Moon, Green Earth, Yellow Comet, Black Hole).
const TROOPERS: u8 = 19;

/// AW2 portrait (`co + 24 * expression`) for a Dual Strike one (`co |
/// expression << 8`, bit 7 a flag).
pub fn aw2_face(face: u32) -> u32 {
    let c = (face & 0x7F) as u8;
    let expr = (face >> 8) & 3;
    let co = match c {
        0x1C..=0x20 => TROOPERS + (c - 0x1C),
        0 => TROOPERS,
        _ => aw2_co(c).unwrap_or(TROOPERS),
    };
    co as u32 + 24 * expr.min(2)
}

/// AW2 unit type for a Dual Strike one.
pub fn aw2_unit(t: u8) -> Option<u8> {
    match t {
        1..=24 => Some(t),
        25 => Some(crate::roster::CARRIER),
        26 => Some(crate::roster::OOZIUM),
        _ => None,
    }
}

// --- Maps ----------------------------------------------------------------------

const MOUNTAIN: u16 = 0x22;
const UNDERLAY: u16 = 0x1A4;
const CRYSTAL: u16 = 0x192;
const OBELISK: u16 = 0x193;

/// AW2's tile for a Dual Strike tile id outside AW2's set.
pub fn remap_tile(t: u16) -> u16 {
    match t {
        // Com Towers: neutral, then owners 1..4 -> AW2's Lab tiles, which
        // are Com Towers with the pack (crate::com_tower).
        0x1B9..=0x1BD => 0x1D9 + (t - 0x1B9),
        // Mountain variants.
        0x146 | 0x147 => MOUNTAIN,
        // Black Crystal -> tangoAW2's Crystal (crate::obelisk).
        0x1A1 => CRYSTAL,
        _ => t,
    }
}

/// A converted map: (width, height, tiles) and its LZ77 blob.
pub fn convert_map(ds: &Ds, at: u32) -> Option<(u8, u8, Vec<u16>)> {
    let head = ds.u32(at)?;
    let size = (head >> 8) as usize;
    let comp = ds.bytes(at, 4 + size * 2 + 64).or_else(|| ds.bytes(at, 4 + size + 64))?;
    let raw = crate::ds_art::lz10(comp)?;
    let (w, h) = (raw[0], raw[1]);
    let n = w as usize * h as usize;
    let mut tiles: Vec<u16> = (0..n).map(|i| u16::from_le_bytes([raw[2 + 2 * i], raw[3 + 2 * i]])).collect();
    // Mega missile silos (a 4x4 structure, anchor 0x1A2/0x1A3 on its third
    // row): a Black Obelisk on the anchor's 3x3 (destructible, no firing).
    for y in 0..h as usize {
        for x in 0..w as usize {
            let t = tiles[y * w as usize + x];
            if t == 0x1A2 || t == 0x1A3 {
                for dy in 0..3 {
                    for dx in 0..3 {
                        let (xx, yy) = (x + dx, y - 1 + dy);
                        if xx >= 1 && xx - 1 < w as usize && yy < h as usize {
                            tiles[yy * w as usize + xx - 1] = if dx == 1 && dy == 1 { OBELISK } else { UNDERLAY };
                        }
                    }
                }
            }
        }
    }
    for t in tiles.iter_mut() {
        *t = remap_tile(*t);
        if matches!(*t, 0x1A6 | 0x1A8 | 0x1A9) {
            *t = UNDERLAY;
        }
    }
    Some((w, h, tiles))
}

/// The map blob AW2 loads (LZ77 of width, height, tiles).
pub fn map_blob(w: u8, h: u8, tiles: &[u16]) -> Vec<u8> {
    let mut raw = vec![w, h];
    for t in tiles {
        raw.extend_from_slice(&t.to_le_bytes());
    }
    let mut lz = crate::lz77::compress(&raw);
    while lz.len() % 4 != 0 {
        lz.push(0);
    }
    lz
}

/// A deployment, converted (12-byte records, FE army, FF end).
pub fn convert_units(ds: &Ds, at: u32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut p = at;
    for _ in 0..400 {
        let Some(r) = ds.bytes(p, 13) else { break };
        p += 13;
        match r[0] {
            0xFF => break,
            0xFE => out.extend_from_slice(&[0xFE, r[1], 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            _ => {
                let Some(t) = aw2_unit(r[2]) else { continue };
                // AW2's AI behaviours are 0..6; Dual Strike's 0, 1 and 5 match
                // by number, its others hold (1).
                let ai = match r[9] {
                    0 | 1 | 5 => r[9],
                    _ => 1,
                };
                let flags = if r[3] & 0x10 != 0 { 0x10 } else { 0 };
                out.extend_from_slice(&[r[0], r[1], t, flags, r[4], r[5], r[6], 0, 0, ai, 0, 0]);
            }
        }
    }
    out.extend_from_slice(&[0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    out
}

// --- Texts ---------------------------------------------------------------------

/// AW2's dialogue box: lines of at most this many pixels in its font, two
/// lines a box (the widest line of AW2's own campaign is 176).
pub const LINE_PIXELS: u32 = 176;
const BOX_LINES: usize = 2;

fn width(widths: &[u8], s: &[u8]) -> u32 {
    let w: u32 = s.iter().filter(|&&c| c >= 0x20).map(|&c| *widths.get(c as usize).unwrap_or(&6) as u32).sum();
    w + s.iter().filter(|&&c| c >= 0x20).count().saturating_sub(1) as u32
}

/// A Dual Strike dialogue text in AW2's box: each of Dual Strike's boxes
/// (0x0F) re-wrapped to AW2's line width, two lines a box, pauses (0x0E)
/// kept.
pub fn wrap_dialogue(t: &[u8], widths: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let boxes: Vec<&[u8]> = t.split(|&c| c == 0x0F).collect();
    let n = boxes.len();
    for (bi, b) in boxes.iter().enumerate() {
        if bi == n - 1 && b.iter().all(|&c| c == b' ' || c == b'\r') {
            break;
        }
        let words: Vec<Vec<u8>> = b
            .split(|&c| c == b' ' || c == b'\r')
            .filter(|w| !w.is_empty())
            .map(|w| w.iter().copied().filter(|&c| c == 0x0E || (0x20..0x7F).contains(&c)).collect())
            .collect();
        let mut lines: Vec<Vec<u8>> = vec![Vec::new()];
        for w in words {
            let cur = lines.last_mut().unwrap();
            let mut longer = cur.clone();
            if !longer.is_empty() {
                longer.push(b' ');
            }
            longer.extend_from_slice(&w);
            if !cur.is_empty() && width(widths, &longer) > LINE_PIXELS {
                lines.push(w);
            } else {
                *cur = longer;
            }
        }
        for (k, chunk) in lines.chunks(BOX_LINES).enumerate() {
            let _ = k;
            for (li, l) in chunk.iter().enumerate() {
                if li > 0 {
                    out.push(b'\r');
                }
                out.extend_from_slice(l);
            }
            out.push(0x0F);
        }
    }
    if out.is_empty() {
        out.push(0x0F);
    }
    out
}

/// A plain one-line text (names): printable ASCII only.
pub fn plain(t: &[u8]) -> Vec<u8> {
    t.iter().copied().filter(|&c| (0x20..0x7F).contains(&c)).collect()
}

// --- Events --------------------------------------------------------------------

/// What a magic function stands for (Rust runs it: [`crate::ds_campaign`]).
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

pub struct Built {
    /// The ROM blob, to be written at [`Built::base`].
    pub blob: Vec<u8>,
    pub base: u32,
    /// AW2 text ids and their strings' addresses (in the blob).
    pub texts: Vec<(u16, u32)>,
    /// Map headers (0x5C bytes) by AW2 map id.
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
    pub map_id: u8,
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
}

/// Blob builder at a fixed ROM address.
struct Blob {
    base: u32,
    bytes: Vec<u8>,
}

impl Blob {
    fn here(&self) -> u32 {
        self.base + self.bytes.len() as u32
    }
    fn align(&mut self, n: usize) {
        while self.bytes.len() % n != 0 {
            self.bytes.push(0);
        }
    }
    fn push(&mut self, b: &[u8]) -> u32 {
        self.align(4);
        let at = self.here();
        self.bytes.extend_from_slice(b);
        at
    }
    fn patch32(&mut self, at: u32, v: u32) {
        let o = (at - self.base) as usize;
        self.bytes[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
}

/// The landing every magic stub jumps to: dead code in `sub_0803CC3C`
/// (no callers; [`crate::five_map`] made its start a helper), trapped by
/// [`crate::ds_campaign`]. r3 = the magic id, lr = the caller's return.
pub const LANDING: u32 = 0x0803_CC5E;

fn stub(id: u32) -> [u8; 16] {
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

struct Ctx<'a> {
    ds: &'a Ds<'a>,
    widths: &'a [u8],
    blob: Blob,
    texts: Vec<(u16, u32)>,
    text_ids: HashMap<Vec<u8>, u16>,
    magic: Vec<Magic>,
    magic_ids: HashMap<Magic, u32>,
    stubs: Vec<u32>,
    /// DS command address -> AW2 command address.
    cmd_map: HashMap<u32, u32>,
    unhandled: BTreeMap<u8, u32>,
    colours: [u8; 4],
    teams: [u8; 4],
}

impl<'a> Ctx<'a> {
    fn text_id(&mut self, s: Vec<u8>) -> u16 {
        if let Some(&id) = self.text_ids.get(&s) {
            return id;
        }
        let id = TEXT_FIRST + self.texts.len() as u16;
        assert!(id <= TEXT_LAST, "out of text ids");
        let mut z = s.clone();
        z.push(0);
        let at = self.blob.push(&z);
        self.texts.push((id, at));
        self.text_ids.insert(s, id);
        id
    }

    fn dialogue(&mut self, r: u32) -> u16 {
        let t = self.ds.text(r).unwrap_or_else(|| b"...".to_vec());
        let w = wrap_dialogue(&t, self.widths);
        self.text_id(w)
    }

    /// The Thumb stub for a magic function (one per distinct function).
    fn magic(&mut self, m: Magic) -> u32 {
        if let Some(&a) = self.magic_ids.get(&m) {
            return a;
        }
        let id = self.magic.len() as u32;
        self.magic.push(m.clone());
        let at = self.blob.push(&stub(id));
        self.stubs.push(at);
        let a = at | 1;
        self.magic_ids.insert(m, a);
        a
    }
}

/// End ops of a script.
fn is_end(op: u32) -> bool {
    matches!(op, 2 | 3 | 4)
}

/// Every command address reachable from `entries` (scripts run until an
/// end op; jumps and branches are followed).
fn reachable(ds: &Ds, entries: &[u32]) -> BTreeSet<u32> {
    let mut seen = BTreeSet::new();
    let mut todo: Vec<u32> = entries.to_vec();
    while let Some(start) = todo.pop() {
        let mut a = start;
        for _ in 0..2000 {
            if !seen.insert(a) {
                break;
            }
            let Some(op) = ds.u32(a) else { break };
            let w1 = ds.u32(a + 4).unwrap_or(0);
            if matches!(op & 0xFF, 0x1D | 0x1E | 0x3E | 0x3F | 0x4E) && (OV1..OV1 + 0x30000).contains(&w1) {
                todo.push(w1);
            }
            if is_end(op & 0xFF) || op & 0xFF == 0x1D {
                break;
            }
            a += 16;
        }
    }
    seen
}

fn cmd(op: u32, w1: u32, h8: u16, ha: u16, wc: u32) -> [u8; 16] {
    let mut c = [0u8; 16];
    c[0..4].copy_from_slice(&op.to_le_bytes());
    c[4..8].copy_from_slice(&w1.to_le_bytes());
    c[8..10].copy_from_slice(&h8.to_le_bytes());
    c[10..12].copy_from_slice(&ha.to_le_bytes());
    c[12..16].copy_from_slice(&wc.to_le_bytes());
    c
}

/// AW2 music for a Dual Strike song id (op 0x47), where AW2 has a like one.
fn aw2_song(ds_song: u32) -> Option<u16> {
    let _ = ds_song;
    None
}

/// One Dual Strike command as AW2's. `at` is the AW2 address it lands on
/// (a no-op is a jump to the next command). Jump targets are DS addresses,
/// fixed up later through the command map.
fn convert_command(cx: &mut Ctx, at: u32, c: &[u8]) -> ([u8; 16], Option<(usize, u32)>) {
    let w = |o: usize| u32::from_le_bytes(c[o..o + 4].try_into().unwrap());
    let h = |o: usize| u16::from_le_bytes([c[o], c[o + 1]]);
    let op = w(0) & 0xFF;
    let (w1, w2, wc) = (w(4), w(8), w(12));
    let nop = cmd(0x1D, at + 16, 0, 0, 0);
    let mut fix = None;
    let out = match op {
        // Call a function: Dual Strike's script-lock counter (0x020B9C9C /
        // 0x020B9C68) and the tutorial's flag reset (0x02019B94) mean
        // nothing in AW2.
        0x00 | 0x52 | 0x55 | 0x57 => match w1 {
            0x020B_9C9C | 0x020B_9C68 | 0x0201_9B94 => nop,
            f => {
                let s = cx.magic(Magic::Call(f, wc));
                cmd(if op == 0x52 { 0x47 } else { 0x00 }, s, 0, 0, 0)
            }
        },
        0x01 => cmd(0x01, 0, 0, 0, wc),
        0x53 => cmd(0x48, 0, 0, 0, wc),
        0x02 | 0x03 | 0x04 => cmd(0x04, 0, 0, 0, 0),
        0x11 => cmd(0x11, 0, h(8), h(10), wc),
        0x14 => cmd(0x14, 0, h(8), h(10), 0),
        0x17 => cmd(0x17, 0, aw2_face(w2) as u16, 0, wc),
        0x18 => cmd(0x18, 0, 0, 0, 0),
        0x19 | 0x1A => {
            let id = cx.dialogue(wc);
            cmd(0x19, 0, id, 0, 0)
        }
        0x1D => {
            fix = Some((4, w1));
            cmd(0x1D, 0, 0, 0, 0)
        }
        0x1E => {
            fix = Some((4, w1));
            let s = cx.magic(Magic::Predicate(wc));
            cmd(0x1E, 0, 0, 0, s)
        }
        // Cursor (pointing at a cell), and its end.
        0x29 => cmd(0x28, 0, h(8), h(10), wc),
        0x2A => cmd(0x29, 0, 0, 0, 0),
        // The window frame in an army's colour (5: the army moving now),
        // or in a colour.
        0x31 => {
            let army = h(8);
            if army == 5 || army == 0 {
                cmd(0x32, 0, 0, 0, 0)
            } else {
                let col = cx.colours.get(army as usize - 1).copied().unwrap_or(1);
                cmd(0x31, 0, col as u16, 0, 0)
            }
        }
        0x32 => cmd(0x31, 0, h(8), 0, 0),
        0x33 => cmd(0x32, 0, 0, 0, 0),
        0x39 => cmd(0x38, 0, aw2_face(w2) as u16, 0, 0),
        // Jump if a completion flag is set.
        0x3F => {
            fix = Some((4, w1));
            cmd(0x3E, 0, h(8), 0, 0)
        }
        0x4E => {
            fix = Some((4, w1));
            let army = c[12];
            let a = aw2_co(c[8]).unwrap_or(0xFF);
            let b = aw2_co(c[10]).unwrap_or(0xFF);
            let s = cx.magic(Magic::CoPair { army, a, b });
            cmd(0x1E, 0, 0, 0, s)
        }
        // Skip n commands unless the army's (main) CO is this one.
        0x4D => {
            let army = (h(8) >> 1) as u16;
            let co = aw2_co(c[10]).unwrap_or(0) as u16;
            cmd(0x43, 0, army, co, wc)
        }
        0x47 => match aw2_song(wc) {
            Some(s) => cmd(0x41, 0, s, 0, 0),
            None => nop,
        },
        0x49 => nop,
        0x51 => cmd(0x46, 0, h(8), h(10), 0),
        0x5A => {
            let s = cx.magic(Magic::Countdown(wc));
            cmd(0x00, s, 0, 0, 0)
        }
        0x5B => {
            let s = cx.magic(Magic::Countdown(0));
            cmd(0x00, s, 0, 0, 0)
        }
        // An army's team wins (0x41/0x42) or loses (0x43/0x44): the other
        // teams defeated (Dual Strike's 0x02019A6C / 0x02019B00, flag 0x80),
        // AW2's `DefeatOtherTeamsAndEndMatch(winner)`.
        0x41 | 0x42 | 0x43 | 0x44 => {
            let army = match h(8) {
                1..=4 => h(8) as usize,
                _ => 1,
            };
            let winner = if op <= 0x42 {
                army
            } else {
                let team = cx.teams[army - 1];
                (1..=4).find(|&a| cx.teams[a - 1] != team && cx.teams[a - 1] != 0).unwrap_or(1)
            };
            cmd(0x40, 0, winner as u16, 0, 0)
        }
        // A campaign flag set (the endings and unlocks, 0x63..0x65).
        0x4F => cmd(0x44, 0, h(8), 0, 0),
        _ => {
            *cx.unhandled.entry(op as u8).or_default() += 1;
            nop
        }
    };
    (out, fix)
}

/// Converts every reachable command, in runs of consecutive DS commands,
/// filling the command map, then fixes jump targets.
fn convert_scripts(cx: &mut Ctx, entries: &[u32]) {
    let all = reachable(cx.ds, entries);
    let all: Vec<u32> = all.into_iter().filter(|a| !cx.cmd_map.contains_key(a)).collect();
    // Runs of consecutive commands.
    let mut runs: Vec<Vec<u32>> = Vec::new();
    for a in all {
        match runs.last_mut() {
            Some(r) if *r.last().unwrap() + 16 == a => r.push(a),
            _ => runs.push(vec![a]),
        }
    }
    let mut fixes = Vec::new();
    for run in runs {
        cx.blob.align(4);
        let start = cx.blob.here();
        // Reserve, then fill (commands may add texts and stubs elsewhere).
        let n = run.len() + 1;
        cx.blob.bytes.extend(std::iter::repeat_n(0, 16 * n));
        for (k, &a) in run.iter().enumerate() {
            cx.cmd_map.insert(a, start + 16 * k as u32);
        }
        for (k, &a) in run.iter().enumerate() {
            let at = start + 16 * k as u32;
            let c = cx.ds.bytes(a, 16).unwrap_or(&[4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).to_vec();
            let (out, fix) = convert_command(cx, at, &c);
            let o = (at - cx.blob.base) as usize;
            cx.blob.bytes[o..o + 16].copy_from_slice(&out);
            if let Some((off, target)) = fix {
                fixes.push((at + off as u32, target));
            }
        }
        // A run that falls off its end (not ended by an end op) ends here.
        let at = start + 16 * run.len() as u32;
        let o = (at - cx.blob.base) as usize;
        cx.blob.bytes[o..o + 16].copy_from_slice(&cmd(0x04, 0, 0, 0, 0));
    }
    for (at, target) in fixes {
        let v = cx.cmd_map.get(&target).copied().unwrap_or(0);
        cx.blob.patch32(at, v);
    }
}

/// Converts a Dual Strike trigger list to AW2's (8-byte records). Records
/// for the hard campaign only (0x16) and the second front only (+1
/// variants) are left out.
fn convert_triggers(cx: &mut Ctx, at: u32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut p = at;
    let mut skip = false;
    let mut record: Vec<[u8; 8]> = Vec::new();
    for _ in 0..512 {
        let Some(r) = cx.ds.bytes(p, 8) else { break };
        let r: [u8; 8] = r.try_into().unwrap();
        p += 8;
        let op = r[0];
        let ptr = u32::from_le_bytes(r[4..8].try_into().unwrap());
        let half = u16::from_le_bytes([r[2], r[3]]);
        if op == 0x1A {
            break;
        }
        if op == 0x15 || op == 0x16 {
            record.clear();
            skip = op == 0x16;
            continue;
        }
        let (kind, front) = (op / 3, op % 3);
        if front == 1 {
            skip = true;
        }
        let rec = |o: u8, a: u8, b: u16, p: u32| {
            let mut x = [0u8; 8];
            x[0] = o;
            x[1] = a;
            x[2..4].copy_from_slice(&b.to_le_bytes());
            x[4..8].copy_from_slice(&p.to_le_bytes());
            x
        };
        match kind {
            0 => {
                // Day 0xFFFF: any day (AW2's 0).
                let day = if half == 0xFFFF { 0 } else { half };
                record.push(rec(0, r[1], day, 0));
            }
            1 => record.push(rec(1, aw2_unit(r[1]).unwrap_or(r[1]), 0, 0)),
            2 => record.push(rec(2, r[1], 0, 0)),
            3 => record.push(rec(3, r[1], 0, 0)),
            4 => record.push(rec(4, r[1], 0, 0)),
            5 | 6 => {
                let s = cx.magic(Magic::Predicate(ptr));
                record.push(rec(kind, 0, 0, s));
            }
            7 | 8 => {
                // Fire.
                let script = cx.cmd_map.get(&ptr).copied().unwrap_or(0);
                record.push(rec(7, r[1], 0, script));
                if !skip {
                    for x in &record {
                        out.extend_from_slice(x);
                    }
                }
                record.clear();
                skip = false;
            }
            _ => {}
        }
    }
    out.extend_from_slice(&[8, 0, 0, 0, 0, 0, 0, 0]);
    out
}

fn trigger_scripts(ds: &Ds, at: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let mut p = at;
    for _ in 0..512 {
        let Some(op) = ds.u8(p) else { break };
        if op == 0x1A {
            break;
        }
        if (0x17..=0x19).contains(&op) {
            if let Some(s) = ds.u32(p + 4).filter(|&s| s != 0) {
                out.push(s);
            }
        }
        p += 8;
    }
    out
}

/// Builds everything for the ROM blob at `base`. `widths` is AW2's font
/// width table (0x084C36E4, 256 bytes).
pub fn build(ds: &Ds, base: u32, widths: &[u8]) -> Option<Built> {
    let mut cx = Ctx {
        ds,
        widths,
        blob: Blob { base, bytes: Vec::new() },
        texts: Vec::new(),
        text_ids: HashMap::new(),
        magic: Vec::new(),
        magic_ids: HashMap::new(),
        stubs: Vec::new(),
        cmd_map: HashMap::new(),
        unhandled: BTreeMap::new(),
        colours: [1, 2, 3, 4],
        teams: [1, 2, 3, 4],
    };
    cx.blob.push(b"DSCAMPGN");
    let recs: Vec<Record> = (0..MISSIONS + SECOND_FRONTS).map(|i| record(ds, i)).collect::<Option<_>>()?;
    let mut headers = Vec::new();
    let mut missions = Vec::new();
    for rec in &recs {
        let map_id = MAP_ID_BASE + rec.index as u8;
        cx.colours = rec.colours;
        cx.teams = rec.teams;
        // Scripts: every fire record's, and the objective.
        let lists: Vec<u32> = (0..6).map(|k| if rec.header != 0 { ds.u32(rec.header + 4 * k).unwrap_or(0) } else { 0 }).collect();
        let mut entries: Vec<u32> = lists.iter().filter(|&&l| l != 0).flat_map(|&l| trigger_scripts(ds, l)).collect();
        if rec.objective != 0 {
            entries.push(rec.objective);
        }
        convert_scripts(&mut cx, &entries);
        let mut table = [0u32; 6];
        // A second front shares its main mission's events; it has none of
        // its own here (played as the main map only).
        if rec.index < MISSIONS {
            for (k, &l) in lists.iter().enumerate() {
                if l != 0 {
                    let t = convert_triggers(&mut cx, l);
                    table[k] = cx.blob.push(&t);
                }
            }
        }
        let mut hdr6 = Vec::new();
        for t in table {
            hdr6.extend_from_slice(&t.to_le_bytes());
        }
        let header = cx.blob.push(&hdr6);
        let objective = cx.cmd_map.get(&rec.objective).copied().unwrap_or(0);
        let (w, h, tiles) = convert_map(ds, rec.maps.0)?;
        let map = cx.blob.push(&map_blob(w, h, &tiles));
        let map_hard = match rec.maps.1 {
            0 => 0,
            a => match convert_map(ds, a) {
                Some((w2, h2, t2)) => cx.blob.push(&map_blob(w2, h2, &t2)),
                None => 0,
            },
        };
        let units = cx.blob.push(&convert_units(ds, rec.units.0));
        let units_hard = if rec.units.1 != 0 { cx.blob.push(&convert_units(ds, rec.units.1)) } else { 0 };
        let name = ds.name(rec.name).map(|n| plain(&n)).unwrap_or_else(|| format!("Mission {}", rec.index + 1).into_bytes());
        let name_id = cx.text_id(name.clone());

        let mut hd = [0u8; 0x5C];
        let w32 = |hd: &mut [u8; 0x5C], o: usize, v: u32| hd[o..o + 4].copy_from_slice(&v.to_le_bytes());
        let w16 = |hd: &mut [u8; 0x5C], o: usize, v: u16| hd[o..o + 2].copy_from_slice(&v.to_le_bytes());
        w32(&mut hd, 0x00, map);
        w32(&mut hd, 0x04, if rec.index < MISSIONS { header } else { 0 });
        w32(&mut hd, 0x08, objective);
        w16(&mut hd, 0x14, name_id);
        hd[0x16] = 2;
        hd[0x17] = 0; // fog: the mission's rules set it (crate::ds_campaign)
        hd[0x18] = rec.armies.clamp(2, 4);
        w16(&mut hd, 0x1A, 1); // the campaign's category
        w16(&mut hd, 0x1C, 1);
        w16(&mut hd, 0x1E, 1);
        w16(&mut hd, 0x20, rec.rank_days.0);
        w16(&mut hd, 0x22, rec.rank_days.1);
        w16(&mut hd, 0x24, rec.day_limit.0);
        hd[0x26] = 0xFF;
        hd[0x27] = 0;
        hd[0x28] = 1;
        w32(&mut hd, 0x2C, map);
        w32(&mut hd, 0x30, if map_hard != 0 { map_hard } else { map });
        w32(&mut hd, 0x34, units);
        w32(&mut hd, 0x38, if units_hard != 0 { units_hard } else { units });
        for k in 0..4 {
            hd[0x3C + k] = if (k as u8) < rec.armies {
                match rec.cos[k].0 {
                    0x1C => 0xFF,
                    c => aw2_co(c).unwrap_or(0xFF),
                }
            } else {
                0xFF
            };
            hd[0x40 + k] = rec.colours[k].clamp(1, 5);
            hd[0x44 + k] = rec.teams[k].max(1);
            hd[0x48 + 4 * k] = 0xFF;
            hd[0x49 + 4 * k] = 0xFF;
        }
        hd[0x58] = 1;
        headers.push((map_id, hd));
        missions.push(MissionInfo {
            index: rec.index,
            name: String::from_utf8_lossy(&name).into_owned(),
            map_id,
            second_front: (rec.second_front >= 0xFC).then(|| MAP_ID_BASE + (rec.second_front - FIRST_RECORD as u16) as u8),
            number: rec.number,
            cos: rec.cos,
            colours: rec.colours,
            teams: rec.teams,
            armies: rec.armies,
            pool: pool(ds, rec),
            day_limit: rec.day_limit.0,
            width: w,
            height: h,
        });
    }
    // The prologue (bank 0x21) as texts the flow can show.
    for i in 0..8 {
        if let Some(t) = ds.text(0x2100_0000 | i) {
            let w = wrap_dialogue(&t, widths);
            cx.text_id(w);
        } else {
            break;
        }
    }
    Some(Built {
        blob: cx.blob.bytes,
        base,
        texts: cx.texts,
        headers,
        magic: cx.magic,
        stubs: cx.stubs,
        missions,
        unhandled: cx.unhandled,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces() {
        // Jake (DS 0x14) happy: AW2's Jake (72 + 7) + 24.
        let jake = crate::co_new::FIRST + 7;
        assert_eq!(aw2_face(0x114), jake as u32 + 24);
        assert_eq!(aw2_face(0x1C), 19);
        assert_eq!(aw2_face(0x20), 23);
        // Nell (DS 1) is AW2's 0.
        assert_eq!(aw2_face(0x01), 0);
        assert_eq!(aw2_co(0x02), Some(1));
        assert_eq!(aw2_co(0x1C), None);
    }

    #[test]
    fn wrapping_keeps_boxes_short() {
        let widths = [6u8; 256];
        let t = b"The real enemy is Black Hole, so their units will be black. Don't forget!\x0fShort.\x0f";
        let w = wrap_dialogue(t, &widths);
        for b in w.split(|&c| c == 0x0F).filter(|b| !b.is_empty()) {
            let lines: Vec<&[u8]> = b.split(|&c| c == b'\r').collect();
            assert!(lines.len() <= BOX_LINES, "{:?}", String::from_utf8_lossy(b));
            for l in lines {
                assert!(width(&widths, l) <= LINE_PIXELS, "{:?}", String::from_utf8_lossy(l));
            }
        }
        assert_eq!(*w.last().unwrap(), 0x0F);
    }

    #[test]
    fn stubs_jump_to_the_landing() {
        let s = stub(7);
        assert_eq!(u32::from_le_bytes(s[8..12].try_into().unwrap()), 7);
        assert_eq!(u32::from_le_bytes(s[12..16].try_into().unwrap()), LANDING | 1);
    }

    /// With `TANGOAW2_DS_ROM`: the whole campaign converts.
    #[test]
    #[ignore]
    fn the_campaign_converts() {
        let rom = std::fs::read(std::env::var_os("TANGOAW2_DS_ROM").expect("TANGOAW2_DS_ROM")).unwrap();
        crate::ds_art::offer(&rom);
        let pack = crate::ds_pack::pack().unwrap();
        let ds = Ds::from_pack(pack).unwrap();
        let widths = [6u8; 256];
        let b = build(&ds, 0x08E0_0000, &widths).unwrap();
        eprintln!("blob {} KB, {} texts, {} magic, unhandled {:?}", b.blob.len() / 1024, b.texts.len(), b.magic.len(), b.unhandled);
        for m in &b.missions {
            eprintln!("{:2} {:20} map {:02x} {}x{} cos {:?} pool {:?}", m.index, m.name, m.map_id, m.width, m.height, m.cos, m.pool);
        }
        assert_eq!(b.missions.len(), MISSIONS + SECOND_FRONTS);
        assert_eq!(b.missions[0].name, "Jake's Trial");
    }
}
