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
//!   campaign only) or 0x16 (hard only: both kept, testing the campaign's
//!   difficulty), fired by 0x17..0x19, ended by
//!   0x1A.
//! - Event scripts: AW2's format (16 bytes a command), Dual Strike's
//!   handler table at arm9 0x021585C0 (0x5D opcodes); many are AW2's own
//!   ops at the same or a neighbouring number ([`convert_command`]).
//! - Texts: banks of string pointers by bank (ov0 0x022F6BF0: key << 24,
//!   pointer), a text reference being bank << 24 | index.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use mgba::core::Core;

pub use crate::campaign_model::{stub, Built, Magic, MissionInfo, Story, LANDING, MAP_ID, TEXT_FIRST, TEXT_LAST, TEXT_TABLE};

pub const OV0: u32 = 0x022A_D560;
pub const OV1: u32 = 0x0235_0560;
const RECORDS: u32 = 0x022D_BD28;
const RECORD: u32 = 0xA0;
pub const FIRST_RECORD: u32 = 0xE0;
pub const MISSIONS: usize = 28;
pub const SECOND_FRONTS: usize = 5;
const TEXT_GROUPS: u32 = OV0 + 0x49690;
const MAP_NAMES_BASE: u32 = 0x022F_6BF8; // the 0xC0 bank (general texts)


/// A view of Dual Strike's memory from the pack.
#[derive(Clone, Copy)]
pub struct Ds<'a> {
    pub arm9: &'a [u8],
    pub ov0: &'a [u8],
    /// Overlay 1 (the battle's events) at [`OV1`].
    pub ov1: &'a [u8],
    /// Overlay 5 (the story: prologue, interludes, ending, and the shop),
    /// loaded at the same address as overlay 1 when it runs.
    pub ov5: &'a [u8],
}

impl<'a> Ds<'a> {
    pub fn from_pack(p: &'a crate::ds_pack::Pack) -> Option<Self> {
        let ov5 = p.overlays.get(5).map(|v| &v[..]).unwrap_or(&[]);
        Some(Ds { arm9: &p.arm9, ov0: p.overlays.first()?, ov1: p.overlays.get(1)?, ov5 })
    }

    /// The same, with overlay 5 where overlay 1 was (its scripts' view).
    pub fn story(&self) -> Ds<'a> {
        Ds { ov1: self.ov5, ..*self }
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
    /// The look (0 normal, 1 snow, 2 desert, 3 wasteland), the weather (0
    /// clear, 1 snow, 2 rain, 3 sandstorm) and fog (+0x1A..+0x1C).
    pub look: u8,
    pub weather: u8,
    pub fog: bool,
    /// The `bmap` file of its 4x4 structure's picture (+0x0C: "0a5" the
    /// missile pad, "0a6" the fortress), if any.
    pub structure: Option<String>,
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
        look: r[0x1A],
        weather: r[0x1B],
        fog: r[0x1C] != 0,
        structure: match w(0x0C) {
            0 => None,
            p => ds.cstr(p).map(|s| String::from_utf8_lossy(&s).into_owned()),
        },
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

const PLAIN: u16 = 0x01;
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
    let (w, h, mut tiles) = raw_map(ds, at)?;
    volcanoes(&mut tiles, w as usize, h as usize);
    black_obelisks(&mut tiles, w as usize, h as usize);
    grand_bolt(ds, &mut tiles, w as usize, h as usize);
    for t in tiles.iter_mut() {
        *t = remap_tile(*t);
    }
    Some((w, h, tiles))
}

/// A Dual Strike map as its record holds it: width, height, terrain ids.
pub fn raw_map(ds: &Ds, at: u32) -> Option<(u8, u8, Vec<u16>)> {
    let head = ds.u32(at)?;
    let size = (head >> 8) as usize;
    let comp = ds.bytes(at, 4 + size * 2 + 64).or_else(|| ds.bytes(at, 4 + size + 64))?;
    let raw = crate::ds_art::lz10(comp)?;
    let (w, h) = (raw[0], raw[1]);
    let n = w as usize * h as usize;
    let tiles: Vec<u16> = (0..n).map(|i| Some(u16::from_le_bytes([*raw.get(2 + 2 * i)?, *raw.get(3 + 2 * i)?]))).collect::<Option<_>>()?;
    Some((w, h, tiles))
}

/// Dual Strike's Volcano (Ring of Fire; structure kind 2): a 4x4 of
/// underlay with its crater row 0x1A6, anchor 0x1A2 (or 0x1A3), 0x1A8,
/// 0x1A9 third. It becomes AW2's own Volcano (anchor 0x1A7, rim 0x1A5 on
/// the top two rows' edge: crate::design's shape), which erupts as in AW2
/// until Dual Strike's rule stills it (crate::ds_campaign_rules).
fn volcanoes(tiles: &mut [u16], w: usize, h: usize) {
    const RIM: u16 = 0x1A5;
    for y in 2..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(2) {
            let t = tiles[y * w + x];
            if t != 0x1A2 && t != 0x1A3 {
                continue;
            }
            let rows: [[u16; 4]; 4] = [
                [RIM; 4],
                [RIM, UNDERLAY, UNDERLAY, RIM],
                [0x1A6, 0x1A7, 0x1A8, 0x1A9],
                [UNDERLAY; 4],
            ];
            for (dy, row) in rows.iter().enumerate() {
                for (dx, &v) in row.iter().enumerate() {
                    tiles[(y + dy - 2) * w + x + dx - 1] = v;
                }
            }
        }
    }
}

/// Dual Strike's Black Obelisk (structure kind 0xA) is a 3x3 whose middle
/// row is tiles 0x18C..0x18E (terrain class 0x1D; AW2's Black Factory
/// tiles, without the factory's fourth row), 0x1A4 rows above and below. It
/// becomes tangoAW2's Obelisk (crate::obelisk: heals, never fires): AW2's
/// Black Cannon footprint with tile 0x193 in its middle.
///
/// A middle row 0x186..0x188 is no Obelisk: Dual Strike's terrain class
/// 0x1A, its Black Cannon facing down (kind 3, 99 HP, 5 HP a shot each day;
/// Crystal Calamity's at (9, 1), read from its battle's structure list in
/// melonDS), the same tiles and class as AW2's own Black Cannon, kept as
/// they are.
fn black_obelisks(tiles: &mut [u16], w: usize, h: usize) {
    for cy in 1..h.saturating_sub(1) {
        for cx in 1..w.saturating_sub(1) {
            let mid = tiles[cy * w + cx];
            if mid != 0x18D {
                continue;
            }
            let at = |dx: usize, dy: usize| tiles[(cy + dy - 1) * w + cx + dx - 1];
            let frame = (0..3).all(|dx| at(dx, 0) == UNDERLAY && at(dx, 2) == UNDERLAY);
            if !frame || at(0, 1) != mid - 1 || at(2, 1) != mid + 1 {
                continue;
            }
            tiles[cy * w + cx - 1] = UNDERLAY;
            tiles[cy * w + cx + 1] = UNDERLAY;
            tiles[cy * w + cx] = OBELISK;
        }
    }
}

/// The Grand Bolt's weak points (Means to an End): the cells Dual Strike's
/// code tests for its three parts (kinds 0xB, 0xC, 0xD at (3, 9), (9, 11),
/// (15, 9)); each part spawns an Oozium on the cell below it every sixth
/// day. Each is tangoAW2's Grand Bolt part ([`crate::grand_bolt`]).
pub const GRAND_BOLT_WEAK_POINTS: [(u32, u32); 3] = [(3, 9), (9, 11), (15, 9)];

/// Means to an End (record index). Its second front (record 0x100) holds
/// three Black Crystals, each guarding one of the Grand Bolt's weak points
/// on the main front (crate::ds_campaign_rules: a weak point can be hit once
/// its crystal is shattered; west to east, as the crystals stand); every
/// crystal shattered wins the second front.
pub const MEANS_TO_AN_END: usize = 24;
/// Means to an End's crystals on its second front, west to east (the
/// weak points' order, [`GRAND_BOLT_WEAK_POINTS`]).
pub const MTE_CRYSTALS: [(u32, u32); 3] = [(1, 1), (8, 1), (14, 1)];

/// Dual Strike's Grand Bolt is a picture drawn with tiles laid out as a
/// sheet (tile = base + 0x20 * y + x over its whole shape), which Dual
/// Strike's Means to an End draws through its own table of textures
/// ([`crate::grand_bolt`]): the cells it draws as the Grand Bolt (textures
/// 0x78..0xA7) become AW2's underlay (a structure's footprint: no unit
/// enters; [`crate::grand_bolt`] draws them), the rest of the sheet plains,
/// each weak point the Grand Bolt's part ([`crate::grand_bolt::PART_TILE`]).
fn grand_bolt(ds: &Ds, tiles: &mut [u16], w: usize, h: usize) {
    const REMAP: u32 = 0x0215_7F84;
    let key = |tiles: &[u16], x: usize, y: usize| tiles[y * w + x] as i32 - (0x20 * y + x) as i32;
    let mut counts: BTreeMap<i32, usize> = BTreeMap::new();
    for y in 0..h {
        for x in 0..w {
            *counts.entry(key(tiles, x, y)).or_default() += 1;
        }
    }
    let Some((&base, &n)) = counts.iter().max_by_key(|e| *e.1) else { return };
    if n < 40 || base < 0 {
        return;
    }
    let drawn = |id: u16| {
        ds.bytes(REMAP + 2 * id as u32, 2)
            .map(|b| (u16::from_le_bytes([b[0], b[1]]) & 0x3FF).wrapping_sub(0x78) < 48)
            .unwrap_or(false)
    };
    for y in 0..h {
        for x in 0..w {
            let id = tiles[y * w + x];
            if key(tiles, x, y) == base || drawn(id) {
                let part = GRAND_BOLT_WEAK_POINTS.contains(&(x as u32, y as u32));
                tiles[y * w + x] = if part {
                    crate::grand_bolt::PART_TILE
                } else if drawn(id) {
                    UNDERLAY
                } else {
                    PLAIN
                };
            }
        }
    }
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
/// A line of the world map's mission panel (narrower: its picture of the
/// CO screen on the left).
pub const PANEL_PIXELS: u32 = 168;
const BOX_LINES: usize = 2;

fn width(widths: &[u8], s: &[u8]) -> u32 {
    let w: u32 = s.iter().filter(|&&c| c >= 0x20).map(|&c| *widths.get(c as usize).unwrap_or(&6) as u32).sum();
    w + s.iter().filter(|&&c| c >= 0x20).count().saturating_sub(1) as u32
}

/// A Dual Strike dialogue text in AW2's box: each of Dual Strike's boxes
/// (0x0F) re-wrapped to AW2's line width, two lines a box, pauses (0x0E)
/// kept.
pub fn wrap_dialogue(t: &[u8], widths: &[u8]) -> Vec<u8> {
    let mut out = wrap_boxes(t, widths);
    // Dual Strike's two-option choice (0x16 in the text) is AW2's 0x17,
    // after a pause, in place of the last box's end.
    if t.contains(&0x16) {
        if out.last() == Some(&0x0F) {
            out.pop();
        }
        out.extend_from_slice(&[0x0E, 0x17]);
    }
    out
}

fn wrap_boxes(t: &[u8], widths: &[u8]) -> Vec<u8> {
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
        let wrap = |limit: u32| {
            let mut lines: Vec<Vec<u8>> = vec![Vec::new()];
            for w in &words {
                let cur = lines.last_mut().unwrap();
                let mut longer = cur.clone();
                if !longer.is_empty() {
                    longer.push(b' ');
                }
                longer.extend_from_slice(w);
                if !cur.is_empty() && width(widths, &longer) > limit {
                    lines.push(w.clone());
                } else {
                    *cur = longer;
                }
            }
            lines
        };
        let mut lines = wrap(LINE_PIXELS);
        // A box that needs more than one of AW2's boxes is spread evenly
        // over them (the narrowest width that still fits that many boxes),
        // so no box is left with a word or two.
        if lines.len() > BOX_LINES {
            let boxes_needed = lines.len().div_ceil(BOX_LINES);
            let (mut lo, mut hi) = (48, LINE_PIXELS);
            while lo < hi {
                let mid = (lo + hi) / 2;
                if wrap(mid).len() <= boxes_needed * BOX_LINES {
                    hi = mid;
                } else {
                    lo = mid + 1;
                }
            }
            lines = wrap(lo);
        }
        for chunk in lines.chunks(BOX_LINES) {
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

/// Dual Strike's answer to a two-option choice (true: the first option).
const CHOICE: u32 = 0x0201_99A4;
/// AW2's `IsTwoOptionChoiceFirst` (the answer to a text's 0x17 choice).
const AW2_CHOICE_FIRST: u32 = 0x0804_57BD;

/// The first text an objective script shows (op 0x19/0x1A's reference).
fn objective_text(ds: &Ds, script: u32) -> Option<Vec<u8>> {
    if script == 0 {
        return None;
    }
    for k in 0..64 {
        let c = ds.bytes(script + 16 * k, 16)?;
        match c[0] {
            0x19 | 0x1A => return ds.text(u32::from_le_bytes(c[12..16].try_into().ok()?)),
            0x02..=0x04 => return None,
            _ => {}
        }
    }
    None
}

/// A text as at most two lines of AW2's box (line breaks `\r`): the words
/// that fit, cut at the end of a sentence when there is one.
/// The box of an objective text that states how to win (its first box
/// saying "win" or "days"; else the first): the map panel's two lines. (Means to an
/// End's first box is Nell's farewell; its second, the objective.)
pub fn objective_box(t: &[u8]) -> &[u8] {
    t.split(|&c| c == 0x0F)
        .find(|b| b.windows(3).any(|w| w.eq_ignore_ascii_case(b"win")) || b.windows(4).any(|w| w == b"days"))
        .unwrap_or(t)
}

pub fn two_lines(t: &[u8], widths: &[u8]) -> Vec<u8> {
    let words: Vec<Vec<u8>> = t
        .split(|&c| c == b' ' || c == b'\r' || c == 0x0E || c == 0x0F)
        .filter(|w| !w.is_empty())
        .map(|w| w.iter().copied().filter(|&c| (0x20..0x7F).contains(&c)).collect::<Vec<u8>>())
        .filter(|w| !w.is_empty())
        .collect();
    let mut lines: Vec<Vec<u8>> = vec![Vec::new()];
    let mut taken = 0;
    for w in &words {
        let cur = lines.last().unwrap();
        let mut longer = cur.clone();
        if !longer.is_empty() {
            longer.push(b' ');
        }
        longer.extend_from_slice(w);
        if !cur.is_empty() && width(widths, &longer) > PANEL_PIXELS {
            if lines.len() == 2 {
                break;
            }
            lines.push(w.clone());
        } else {
            *lines.last_mut().unwrap() = longer;
        }
        taken += 1;
    }
    let mut out = lines.join(&b'\r');
    if taken < words.len() {
        // Cut back to the last full sentence, if any.
        if let Some(p) = out.iter().rposition(|&c| c == b'.' || c == b'!' || c == b'?') {
            out.truncate(p + 1);
        }
    }
    out
}

/// Dual Strike's campaign flags 0x60..0x6F as the DS Campaign keeps them,
/// 0x80..0x8F: AW2's own code reads some of 0x60..0x6B while a DS mission
/// runs (0x60 is Hard Campaign: its deployments, its world map).
pub fn ds_flag(f: u16) -> u16 {
    if (0x60..0x70).contains(&f) {
        f + 0x20
    } else {
        f
    }
}

/// A plain one-line text (names): printable ASCII only.
pub fn plain(t: &[u8]) -> Vec<u8> {
    t.iter().copied().filter(|&c| (0x20..0x7F).contains(&c)).collect()
}

// --- Events --------------------------------------------------------------------


/// The story's scenes as overlay 5 runs them: its proc scripts' steps
/// (0x023682E0 the party, 0x02368AF8 the ending) in order, each one
/// script.
const PARTY_SCRIPTS: [u32; 3] = [0x0236_83C8, 0x0236_86E8, 0x0236_84E8];
const ENDING_SCRIPTS: [u32; 5] = [0x0236_9180, 0x0236_8CA0, 0x0236_8E40, 0x0236_8FE0, 0x0236_8B60];
const PROLOGUE_TEXTS: [u32; 3] = [0x2100_0000, 0x2100_0001, 0x2100_0002];
const INTERLUDE_TEXTS: [u32; 1] = [0x2100_0003];
/// Missions those come after (indexes: Victory or Death!, Crystal
/// Calamity, Means to an End).
const AFTER_VICTORY_OR_DEATH: usize = 8;
const AFTER_CRYSTAL_CALAMITY: usize = 18;
const AFTER_MEANS_TO_AN_END: usize = 24;

/// Narration: each text over its picture (crate::ds_story_art, a magic
/// call puts it on the map's layer), in a box of its own with no speaker
/// (AW2's `ShowTextOnBg0`, op 0x1A); then the map back and the end.
fn narration(cx: &mut Ctx, texts: &[u32], song: Option<u16>) -> u32 {
    let mut s = Vec::new();
    if let Some(m) = song.and_then(crate::ds_music::story_song) {
        s.extend_from_slice(&cmd(0x41, 0, m, 0, 0));
    }
    for &r in texts {
        if let Some(n) = crate::ds_story_art::NARRATION.iter().position(|x| x.0 == r) {
            let stub = cx.magic(Magic::Flow(crate::ds_campaign::FLOW_PICTURE + n as u8));
            s.extend_from_slice(&cmd(0x00, stub, 0, 0, 0));
        }
        let id = cx.dialogue(r);
        s.extend_from_slice(&cmd(0x1A, 0, id, 0, 0));
    }
    let back = cx.magic(Magic::Flow(crate::ds_campaign::FLOW_MAP_BACK));
    s.extend_from_slice(&cmd(0x00, back, 0, 0, 0));
    if song.and_then(crate::ds_music::story_song).is_some() {
        // The map's song again.
        let map = crate::ds_music::story_song(crate::ds_music::WORLD_MAP).unwrap_or(0x1A8);
        s.extend_from_slice(&cmd(0x41, 0, map, 0, 0));
    }
    s.extend_from_slice(&cmd(0x04, 0, 0, 0, 0));
    cx.blob.push(&s)
}

/// Overlay 5's scripts in a row: converted as the battles' are, each one's
/// end made a jump to the next. The first one's AW2 address.
fn story_scripts<'a>(cx: &mut Ctx<'a>, story: Ds<'a>, scripts: &[u32]) -> u32 {
    if story.ov1.is_empty() {
        return 0;
    }
    let (saved_ds, saved_map) = (cx.ds, std::mem::take(&mut cx.cmd_map));
    cx.ds = story;
    cx.story = true;
    convert_scripts(cx, scripts);
    let starts: Vec<u32> = scripts.iter().map(|s| cx.cmd_map.get(s).copied().unwrap_or(0)).collect();
    for (k, &s) in scripts.iter().enumerate().take(scripts.len().saturating_sub(1)) {
        let mut a = s;
        for _ in 0..400 {
            if is_end(cx.ds.u32(a).unwrap_or(4) & 0xFF) {
                break;
            }
            a += 16;
        }
        if let (Some(&at), next) = (cx.cmd_map.get(&a), starts[k + 1]) {
            let o = (at - cx.blob.base) as usize;
            cx.blob.bytes[o..o + 16].copy_from_slice(&cmd(0x1D, next, 0, 0, 0));
        }
    }
    cx.ds = saved_ds;
    cx.cmd_map = saved_map;
    cx.story = false;
    starts.first().copied().unwrap_or(0)
}

fn convert_story_scenes<'a>(cx: &mut Ctx<'a>, ds: &Ds<'a>) -> Story {
    let story = ds.story();
    let prologue = narration(cx, &PROLOGUE_TEXTS, Some(crate::ds_music::OPENING));
    let interlude = narration(cx, &INTERLUDE_TEXTS, None);
    let party = story_scripts(cx, story, &PARTY_SCRIPTS);
    let mut ending = story_scripts(cx, story, &ENDING_SCRIPTS);
    // The ending plays Dual Strike's ending song, with a pack that has it.
    if let (Some(m), true) = (crate::ds_music::story_song(crate::ds_music::ENDING), ending != 0) {
        let s = [cmd(0x41, 0, m, 0, 0), cmd(0x1D, ending, 0, 0, 0)].concat();
        ending = cx.blob.push(&s);
    }
    let after_win = [(AFTER_VICTORY_OR_DEATH, interlude), (AFTER_CRYSTAL_CALAMITY, party), (AFTER_MEANS_TO_AN_END, ending)]
        .into_iter()
        .filter(|&(_, s)| s != 0)
        .collect();
    Story { prologue, after_win }
}

/// The cells of a Dual Strike map whose tile is a Lab (0x1D9..0x1DD).
fn lab_cells(ds: &Ds, at: u32) -> Vec<(u8, u8)> {
    let Some(head) = ds.u32(at) else { return Vec::new() };
    let size = (head >> 8) as usize;
    let Some(comp) = ds.bytes(at, 4 + size * 2 + 64).or_else(|| ds.bytes(at, 4 + size + 64)) else { return Vec::new() };
    let Some(raw) = crate::ds_art::lz10(comp) else { return Vec::new() };
    let (w, h) = (raw[0] as usize, raw[1] as usize);
    (0..w * h)
        .filter(|&i| (0x1D9..=0x1DD).contains(&u16::from_le_bytes([raw[2 + 2 * i], raw[3 + 2 * i]])))
        .map(|i| ((i % w) as u8, (i / w) as u8))
        .collect()
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

struct Ctx<'a> {
    ds: Ds<'a>,
    /// Converting the story's scripts (overlay 5): its function calls are
    /// its own screen's (pictures, fades), not the battle's.
    story: bool,
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
    /// The record index whose triggers are being converted.
    mission: usize,
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

/// AW2 music for a Dual Strike song id (op 0x47): Dual Strike's own,
/// converted (crate::ds_music::STORY_SONGS), with a pack that has it; else
/// where AW2 has a like one.
///
/// Dual Strike's mission scripts play six event songs (its sound archive's
/// names): its allies' scenes (`BGM_ALLY_EVENT1`/`2`) and a crisis
/// (`BGM_EVENT_PINCH1`) get the song AW2's campaign plays for its own
/// allies' alarms (413); Black Hole's scenes (`BGM_ENEMY_EVENT1`/`2`) the one
/// AW2 plays for Black Hole's officers (411); Von Bolt's
/// (`BGM_HAGEVOLT_EVENT1`) the one AW2 plays at Sturm's citadel (220).
fn aw2_song(ds_song: u32) -> Option<u16> {
    if let Some(s) = crate::ds_music::story_song(ds_song as u16) {
        return Some(s);
    }
    match ds_song {
        0x19 | 0x1A | 0x2D => Some(413),
        0x16 | 0x17 => Some(411),
        0x23 => Some(220),
        _ => None,
    }
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
        0x00 | 0x52 | 0x55 | 0x57 if cx.story => nop,
        // Waits on a scene's proc script (0x0201D298 / 0x0201D158 with the
        // script in the operand): Dual Strike's camera pans and
        // animations, nothing on the map's state.
        0x55 | 0x57 => nop,
        0x00 | 0x52 => match w1 {
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
        // Means to an End's choice (Dual Strike's answer, 0x020199A4, true
        // for the first option): AW2's own two-option answer, asked by the
        // text before it (its 0x16, AW2's 0x17: [`wrap_dialogue`]).
        0x1E if wc == CHOICE => {
            fix = Some((4, w1));
            cmd(0x1E, 0, 0, 0, AW2_CHOICE_FIRST)
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
        // The event's song ends (AW2 fades it out the same way, before the
        // window closes; the map's music comes back after the script).
        0x49 => cmd(0x42, 0, 0, 0, 0),
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
        0x4F => cmd(0x44, 0, ds_flag(h(8)), 0, 0),
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
    let all = reachable(&cx.ds, entries);
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

/// A pseudo predicate (no Dual Strike address): the campaign is Hard
/// (crate::ds_campaign_rules::predicate).
pub const HARD_CAMPAIGN: u32 = 0x60;

/// Pseudo predicates (no Dual Strike address): army `a` owns `n` properties
/// or more (`PROPERTY_COUNT | a << 8 | n`; crate::ds_campaign_rules).
pub const PROPERTY_COUNT: u32 = 0x0100_0000;

/// Dual Strike's property-count win (the record's +0x34 Normal, +0x36
/// Hard: Spiral Garden's "Whoever captures 15 properties wins"), which its
/// engine tests, not its scripts: trigger records for the after-action
/// list, one per army, each firing a script that ends the match with that
/// army's win (AW2's op 0x40). Empty when the mission has none.
fn property_win(cx: &mut Ctx, rec: &Record) -> Vec<u8> {
    let normal = u16::from_le_bytes([rec.raw[0x34], rec.raw[0x35]]);
    let hard = u16::from_le_bytes([rec.raw[0x36], rec.raw[0x37]]);
    let mut out = Vec::new();
    if normal == 0 && hard == 0 {
        return out;
    }
    for army in 1..=rec.armies.min(4) as u32 {
        let mut script = Vec::new();
        script.extend_from_slice(&cmd(0x40, 0, army as u16, 0, 0));
        script.extend_from_slice(&cmd(0x04, 0, 0, 0, 0));
        cx.blob.align(4);
        let fire = cx.blob.push(&script);
        for (n, only) in [(normal, Some(false)), (hard, Some(true))] {
            let only = if normal == hard { if only == Some(true) { continue } else { None } } else { only };
            if n == 0 {
                continue;
            }
            if let Some(h) = only {
                let s = cx.magic(Magic::Predicate(HARD_CAMPAIGN));
                let mut x = [0u8; 8];
                x[0] = if h { 5 } else { 6 };
                x[4..8].copy_from_slice(&s.to_le_bytes());
                out.extend_from_slice(&x);
            }
            let s = cx.magic(Magic::Predicate(PROPERTY_COUNT | army << 8 | n as u32 & 0xFF));
            let mut x = [0u8; 8];
            x[0] = 5;
            x[4..8].copy_from_slice(&s.to_le_bytes());
            out.extend_from_slice(&x);
            let mut x = [0u8; 8];
            x[0] = 7;
            x[4..8].copy_from_slice(&fire.to_le_bytes());
            out.extend_from_slice(&x);
        }
    }
    out
}

/// Converts a Dual Strike trigger list to AW2's (8-byte records): the
/// records of one front (`front`: 0 main, 1 second). Dual Strike keeps both
/// fronts' records in one list: a condition's op is 3 * AW2's op + its front
/// (0 main, 1 second, 2 either), a fire's 0x17 + its front. A group of
/// records (up to its fire) belongs to the second front when any of them
/// names it, else to the main front. Records for one difficulty test it.
fn convert_triggers(cx: &mut Ctx, at: u32, front: u8) -> Vec<u8> {
    let mut out = Vec::new();
    let mut p = at;
    // The group names the second front.
    let mut second = false;
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
            // Normal only (0x15) or Hard only (0x16): the record tests the
            // campaign's difficulty first (AW2's kinds 5/6: a predicate
            // true / false; [`HARD_CAMPAIGN`]).
            record.clear();
            second = false;
            let s = cx.magic(Magic::Predicate(HARD_CAMPAIGN));
            let mut x = [0u8; 8];
            x[0] = if op == 0x16 { 5 } else { 6 };
            x[4..8].copy_from_slice(&s.to_le_bytes());
            record.push(x);
            continue;
        }
        let (kind, of) = if (0x17..=0x19).contains(&op) { (7, op - 0x17) } else { (op / 3, op % 3) };
        second |= of == 1;
        let rec = |o: u8, a: u8, b: u16, p: u32| {
            let mut x = [0u8; 8];
            x[0] = o;
            x[1] = a;
            x[2..4].copy_from_slice(&b.to_le_bytes());
            x[4..8].copy_from_slice(&p.to_le_bytes());
            x
        };
        match kind {
            // Day 0xFFFF: any day (AW2's 0).
            0 => record.push(rec(0, r[1], if half == 0xFFFF { 0 } else { half }, 0)),
            1 => record.push(rec(1, aw2_unit(r[1]).unwrap_or(r[1]), 0, 0)),
            2 => record.push(rec(2, r[1], 0, 0)),
            3 => record.push(rec(3, r[1], 0, 0)),
            4 => record.push(rec(4, r[1], 0, 0)),
            5 | 6 => {
                let s = cx.magic(Magic::Predicate(ptr));
                record.push(rec(kind, 0, 0, s));
            }
            7 => {
                // Fire.
                let script = cx.cmd_map.get(&ptr).copied().unwrap_or(0);
                record.push(rec(7, r[1], 0, script));
                if second == (front == 1) {
                    for x in &record {
                        out.extend_from_slice(x);
                    }
                }
                record.clear();
                second = false;
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

/// The campaign's order: Dual Strike's 25 story missions, with its three
/// research-lab side missions (records 25..27). Dual Strike opens a side
/// mission on its world map when the player captures the city hiding the
/// lab's map in the mission before it (campaign flags 0x60..0x62, set by
/// the mission's own script): The Long March after Black Boats Ahoy! (flag
/// 0x60), Lash's Test after Frozen Fortress (0x61), Spiral Garden after
/// Snow Hunters (0x62). Here a side mission is played next when its flag
/// is set, and skipped otherwise ([`SIDE_MISSIONS`]).
pub const ORDER: [u8; MISSIONS] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 25, 10, 11, 26, 12, 13, 27, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
];
/// The side missions and the campaign flag that opens each.
pub const SIDE_MISSIONS: [(u8, u32); 3] = [(25, 0x90), (26, 0x91), (27, 0x92)];

/// Means to an End: the campaign's last mission.
pub const FINAL_MISSION: u8 = 24;

/// AW2's font widths (for re-wrapping Dual Strike's dialogue).
const FONT_WIDTHS: u32 = 0x084C_36E4;

/// Dual Strike's campaign as a [`crate::campaign_model::Model`] (the
/// source's `load`): its missions converted from the pack into the blob
/// at [`crate::ds_campaign::DATA`], its order, its story pictures and staff
/// roll.
pub fn load(core: &Core) -> Option<crate::campaign_model::Model> {
    let pack = crate::ds_pack::pack()?;
    let ds = Ds::from_pack(pack)?;
    let mut widths = vec![0u8; 256];
    core.raw_read_range(FONT_WIDTHS, -1, &mut widths);
    let mut built = build(&ds, crate::ds_campaign::DATA + 0x100, &widths)?;
    let credits = crate::ds_credits::build(core, &ds, &mut built);
    Some(crate::campaign_model::Model {
        label: crate::campaign_model::SOURCES[0].label,
        built,
        missions: MISSIONS,
        order: ORDER.to_vec(),
        side_missions: SIDE_MISSIONS.to_vec(),
        final_mission: FINAL_MISSION,
        credits,
        pictures: crate::ds_story_art::narration_pictures(),
        source: &crate::campaign_model::SOURCES[0],
    })
}

/// Dual Strike's air units a sky front takes (Fighter, Bomber, Stealth,
/// Black Bomb; not its copters, B Copter 19 and T Copter 20).
pub const SKY_UNITS: [u8; 4] = [16, 17, crate::roster::STEALTH, crate::roster::BLACK_BOMB];

/// A mission's second front as tangoAW2 plays it (crate::two_front), as
/// Dual Strike's record has it: its second front's record (+0x10), each
/// army's tag CO there (the player's tag CO: picked), the computer
/// directing it ("In Campaign mode, the second front is controlled
/// automatically"), no CO powers there ("CO Powers can't be used there,
/// either. The action is too fast."), and its Send rule: a front whose
/// deployment is air units only is in the sky (Victory or Death!'s Black
/// Arc, Omens and Signs), the others are on the ground.
/// A front whose deployment is all aircraft is in the sky (Victory or
/// Death!'s and Omens and Signs' second fronts: the Black Arc's).
fn in_the_sky(ds: &Ds, front: &Record) -> bool {
    let units = convert_units(ds, front.units.0);
    units.chunks(12).filter(|u| u[0] < 0xFE).all(|u| SKY_UNITS.contains(&u[2]) || matches!(u[2], 19 | 20))
}

/// Dual Strike's Intel > Auto CO item: its test (arm9 `0x020BE908`, "Auto CO
/// On"; its twin `0x020BE7A8`, "Auto CO Off") shows it only on the front
/// whose map record is one of two (`ldrh r0, [rec, #0x34]; cmp r0, #0xEA;
/// beq; cmp r0, #0xF5; movne r0, #1`: Lightning Strikes and Ring of Fire),
/// while the other front is not over, with the army's controller (its player
/// record's +0x1A) 2 (the computer: On) or not (Off); choosing it flips that
/// byte. The record ids, read from the two `cmp` instructions (none when the
/// code is not there).
const AUTO_CO_CMP: [u32; 2] = [0x020B_E940, 0x020B_E948];
pub fn auto_co_records(ds: &Ds) -> Vec<u32> {
    let ids: Vec<u32> = AUTO_CO_CMP
        .iter()
        .filter_map(|&a| ds.u32(a))
        .filter(|w| w & 0xFFFF_FF00 == 0xE350_0000)
        .map(|w| w & 0xFF)
        .collect();
    if ids.len() == AUTO_CO_CMP.len() {
        ids
    } else {
        Vec::new()
    }
}

fn two_front(ds: &Ds, rec: &Record) -> Option<crate::campaign_model::TwoFront> {
    use crate::campaign_model::{FrontControl, SendRule, TwoFront, PICK};
    if rec.index >= MISSIONS || rec.second_front < (FIRST_RECORD + MISSIONS as u32) as u16 {
        return None;
    }
    let second = (rec.second_front as u32 - FIRST_RECORD) as usize;
    let front = record(ds, second)?;
    let mut cos = [0xFFu8; 4];
    for (k, c) in cos.iter_mut().enumerate().take(front.armies.min(4) as usize) {
        *c = match front.cos[k].1 {
            0x1C => PICK,
            c => aw2_co(c).unwrap_or(0xFF),
        };
    }
    let sky = in_the_sky(ds, &front);
    // Intel > Auto CO where Dual Strike offers it (on at the start), the
    // computer's elsewhere ("In Campaign mode, the second front is
    // controlled automatically").
    let control = if auto_co_records(ds).contains(&(FIRST_RECORD + rec.index as u32)) {
        FrontControl::AutoCo { on: true }
    } else {
        FrontControl::Cpu
    };
    Some(TwoFront {
        second: second as u8,
        cos,
        control: [control; 4],
        send: if sky { SendRule::Air } else { SendRule::Ground },
        powers: false,
        sky,
    })
}

/// Builds everything for the ROM blob at `base`. `widths` is AW2's font
/// width table (0x084C36E4, 256 bytes).
pub fn build(ds: &Ds, base: u32, widths: &[u8]) -> Option<Built> {
    let mut cx = Ctx {
        ds: *ds,
        story: false,
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
        mission: 0,
    };
    cx.blob.push(b"DSCAMPGN");
    let recs: Vec<Record> = (0..MISSIONS + SECOND_FRONTS).map(|i| record(ds, i)).collect::<Option<_>>()?;
    let mut headers = Vec::new();
    let mut missions = Vec::new();
    for rec in &recs {
        cx.colours = rec.colours;
        cx.teams = rec.teams;
        cx.mission = rec.index;
        // Scripts: every fire record's, and the objective.
        let lists: Vec<u32> = (0..6).map(|k| if rec.header != 0 { ds.u32(rec.header + 4 * k).unwrap_or(0) } else { 0 }).collect();
        let mut entries: Vec<u32> = lists.iter().filter(|&&l| l != 0).flat_map(|&l| trigger_scripts(ds, l)).collect();
        if rec.objective != 0 {
            entries.push(rec.objective);
        }
        convert_scripts(&mut cx, &entries);
        let mut table = [0u32; 6];
        // A second front's record names its main mission's event header:
        // its own events are that header's second-front records
        // ([`convert_triggers`]), the main front's the rest.
        let front = if rec.index < MISSIONS { 0 } else { 1 };
        let wins = if front == 0 { property_win(&mut cx, rec) } else { Vec::new() };
        for (k, &l) in lists.iter().enumerate() {
            // (the after-action list, 3, also tests the property-count win)
            if l != 0 || (k == 3 && !wins.is_empty()) {
                let mut t = if l != 0 { convert_triggers(&mut cx, l, front) } else { vec![8, 0, 0, 0, 0, 0, 0, 0] };
                if k == 3 {
                    let end = t.len() - 8;
                    t.splice(end..end, wins.iter().copied());
                }
                table[k] = cx.blob.push(&t);
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
        let info = objective_text(ds, rec.objective).map(|t| two_lines(objective_box(&t), cx.widths)).unwrap_or_else(|| name.clone());
        let info_text = cx.text_id(info);

        let mut hd = [0u8; 0x5C];
        let w32 = |hd: &mut [u8; 0x5C], o: usize, v: u32| hd[o..o + 4].copy_from_slice(&v.to_le_bytes());
        let w16 = |hd: &mut [u8; 0x5C], o: usize, v: u16| hd[o..o + 2].copy_from_slice(&v.to_le_bytes());
        w32(&mut hd, 0x00, map);
        w32(&mut hd, 0x04, header);
        w32(&mut hd, 0x08, objective);
        w16(&mut hd, 0x14, name_id);
        hd[0x16] = 2;
        // A 4x4 structure's picture (AW2's own: Dual Strike's are the same
        // bytes); without it the game draws whatever OBJ VRAM holds there.
        let structure = match rec.structure.as_deref() {
            Some("0a5") => Some(crate::survival_maps::Structure::MissilePad),
            Some("0a6") => Some(crate::survival_maps::Structure::Fortress),
            _ => crate::survival_maps::Structure::of_tiles(&tiles),
        };
        w32(&mut hd, 0x10, structure.map_or(0, |s| s.aw2_picture()));
        // Fog (`ResetRulesAfterCampaignMap` reads it; the mission start
        // sets it too, crate::ds_campaign::map_start).
        hd[0x17] = rec.fog as u8;
        hd[0x18] = rec.armies.clamp(2, 4);
        w16(&mut hd, 0x1A, 1); // the campaign's category
        w16(&mut hd, 0x1C, 1);
        w16(&mut hd, 0x1E, 1);
        w16(&mut hd, 0x20, rec.rank_days.0);
        w16(&mut hd, 0x22, rec.rank_days.1);
        w16(&mut hd, 0x24, if rec.index < MISSIONS { rec.day_limit.0 } else { 0 });
        hd[0x26] = 0xFF;
        hd[0x27] = 0;
        hd[0x28] = 1;
        w32(&mut hd, 0x2C, map);
        w32(&mut hd, 0x30, if map_hard != 0 { map_hard } else { map });
        w32(&mut hd, 0x34, units);
        w32(&mut hd, 0x38, if units_hard != 0 { units_hard } else { units });
        for k in 0..4 {
            // A CO pair's first CO leads the main front, its tag CO the
            // second front.
            let co = if front == 0 { rec.cos[k].0 } else { rec.cos[k].1 };
            hd[0x3C + k] = if (k as u8) < rec.armies {
                match co {
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
        // The mission title's look and music: the player's country.
        hd[0x58] = rec.colours[0].clamp(1, 4);
        headers.push((rec.index as u8, hd));
        missions.push(MissionInfo {
            index: rec.index,
            name: String::from_utf8_lossy(&name).into_owned(),
            info_text,
            two_front: two_front(ds, rec),
            number: rec.number,
            cos: rec.cos,
            colours: rec.colours,
            teams: rec.teams,
            armies: rec.armies,
            pool: pool(ds, rec),
            day_limit: rec.day_limit.0,
            width: w,
            height: h,
            // A second front is drawn in the Normal look: Dual Strike's top
            // screen does not read the look byte (the second front's record
            // has its main mission's, 3 for Means to an End's, drawn green;
            // poked in melonDS, neither record's byte changes the top screen
            // while the main record's changes the bottom one).
            look: if front == 0 { rec.look } else { 0 },
            // A front in the sky has clear weather (Dual Strike's top screen
            // draws no sandstorm there; crate::sky_front).
            weather: if front == 1 && in_the_sky(ds, rec) { 0 } else { rec.weather },
            fog: rec.fog,
            labs: lab_cells(ds, rec.maps.0),
        });
    }
    let story = convert_story_scenes(&mut cx, ds);
    Some(Built {
        story,
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

    /// The event records of one header's lists: (list, op, flag, day,
    /// pointer) for every record.
    fn header_records(b: &Built, events: u32) -> Vec<(usize, u8, u8, u16, u32)> {
        let at = |a: u32, n: usize| &b.blob[(a - b.base) as usize..(a - b.base) as usize + n];
        let u32_at = |a: u32| u32::from_le_bytes(at(a, 4).try_into().unwrap());
        let mut out = Vec::new();
        for k in 0..6 {
            let mut p = u32_at(events + 4 * k);
            if p == 0 {
                continue;
            }
            loop {
                let r = at(p, 8);
                if r[0] == 8 {
                    break;
                }
                out.push((k as usize, r[0], r[1], u16::from_le_bytes([r[2], r[3]]), u32::from_le_bytes(r[4..8].try_into().unwrap())));
                p += 8;
            }
        }
        out
    }

    /// With `TANGOAW2_DS_ROM` (else nothing to check): every mission's day
    /// limit (the header's "Days Left" counter), the days its triggers fire
    /// on (its two fronts' together) and the days its texts state are Dual
    /// Strike's own, Means to an End's 24 included.
    #[test]
    fn day_limits_are_dual_strikes() {
        let Some(path) = std::env::var_os("TANGOAW2_DS_ROM") else { return };
        let Ok(rom) = std::fs::read(path) else { return };
        crate::ds_art::offer(&rom);
        let Some(pack) = crate::ds_pack::pack() else { return };
        let ds = Ds::from_pack(pack).unwrap();
        let b = build(&ds, 0x08E0_0000, &[6u8; 256]).unwrap();
        let header = |i: usize| b.headers.iter().find(|h| h.0 as usize == i).unwrap().1;
        for i in 0..MISSIONS {
            let rec = record(&ds, i).unwrap();
            let hd = header(i);
            assert_eq!(u16::from_le_bytes([hd[0x24], hd[0x25]]), rec.day_limit.0, "mission {i}: the day limit");
            // The days the triggers fire on (kind 0, a day): Dual Strike's.
            let ds_days: Vec<u16> = (0..6)
                .filter_map(|k| ds.u32(rec.header + 4 * k).filter(|&l| l != 0))
                .flat_map(|l| {
                    let mut out = Vec::new();
                    let mut p = l;
                    while let Some(r) = ds.bytes(p, 8) {
                        if r[0] == 0x1A {
                            break;
                        }
                        let day = u16::from_le_bytes([r[2], r[3]]);
                        if r[0] / 3 == 0 && day != 0xFFFF && day != 0 {
                            out.push(day);
                        }
                        p += 8;
                    }
                    out
                })
                .collect();
            let mut heads = vec![hd];
            if let Some(t) = &b.missions[i].two_front {
                heads.push(header(t.second as usize));
            }
            let ours: Vec<u16> = heads
                .iter()
                .flat_map(|hd| header_records(&b, u32::from_le_bytes(hd[4..8].try_into().unwrap())))
                .filter(|r| r.1 == 0 && r.3 != 0)
                .map(|r| r.3)
                .collect();
            let (mut a, mut c) = (ds_days.clone(), ours.clone());
            a.sort();
            a.dedup();
            c.sort();
            c.dedup();
            assert!(a.iter().all(|d| c.contains(d)) && c.iter().all(|d| a.contains(d)), "mission {i}: trigger days {c:?}, Dual Strike's {a:?}");
        }
        // Means to an End's texts are Dual Strike's own: its briefing, Von
        // Bolt's 24 days, Lash's crystals on the second front.
        for (r, raw) in [
            (0xC000_029Eu32, "15 days"),
            (0xC000_02A7, "18 days"),
            (0xC000_02A8, "24 days"),
            (0xC000_02AA, "within 24 days"),
            (0x2600_0046, "24 days"),
            (0x2600_0045, "on the second front"),
        ] {
            let t = String::from_utf8_lossy(&ds.text(r).unwrap()).replace('\r', " ").to_string();
            assert!(t.contains(raw), "text {r:#x} keeps Dual Strike's {raw}: {t}");
        }
    }

    /// With `TANGOAW2_DS_ROM`: Dual Strike's five two-front missions are
    /// described as two fronts (crate::two_front), every other mission as
    /// one; each front's header holds its own events (Dual Strike's records
    /// for that front), its own map and its own COs; Means to an End's
    /// Black Crystals stand on its second front only, and its main map has
    /// none.
    #[test]
    fn two_fronts_are_dual_strikes() {
        use crate::campaign_model::{FrontControl, SendRule, PICK};
        let Some(path) = std::env::var_os("TANGOAW2_DS_ROM") else { return };
        let Ok(rom) = std::fs::read(path) else { return };
        crate::ds_art::offer(&rom);
        let Some(pack) = crate::ds_pack::pack() else { return };
        let ds = Ds::from_pack(pack).unwrap();
        let b = build(&ds, 0x08E0_0000, &[6u8; 256]).unwrap();
        // (below the fixed ids of crate::two_front and crate::setup_phase)
        assert!(b.texts.iter().all(|t| t.0 < crate::two_front::TEXT_IDS_FROM), "the campaign's texts below the menus' own ids");
        assert_eq!(auto_co_records(&ds), vec![0xEA, 0xF5], "Dual Strike's Auto CO missions");
        let fronts: Vec<(usize, u8, SendRule)> = b
            .missions
            .iter()
            .filter_map(|m| m.two_front.as_ref().map(|t| (m.index, t.second, t.send)))
            .collect();
        assert_eq!(
            fronts,
            vec![
                (8, 28, SendRule::Air),
                (10, 29, SendRule::Ground),
                (14, 30, SendRule::Air),
                (21, 31, SendRule::Ground),
                (24, 32, SendRule::Ground)
            ],
            "Victory or Death!, Lightning Strikes, Omens and Signs, Ring of Fire, Means to an End"
        );
        let header = |i: usize| b.headers.iter().find(|h| h.0 as usize == i).unwrap().1;
        for &(i, second, _) in &fronts {
            let t = b.missions[i].two_front.clone().unwrap();
            // Intel > Auto CO (on at the start) in Lightning Strikes and Ring
            // of Fire, as Dual Strike's item; the computer's elsewhere.
            let want = if matches!(i, 10 | 21) { FrontControl::AutoCo { on: true } } else { FrontControl::Cpu };
            assert_eq!(t.control, [want; 4], "mission {i}: who directs the second front");
            assert_eq!(b.missions[second as usize].look, 0, "mission {i}: the second front in the Normal look (Dual Strike's top screen)");
            // The Black Arc's fronts are in the sky, with clear weather.
            assert_eq!(t.sky, matches!(i, 8 | 14), "mission {i}: in the sky");
            if t.sky {
                assert_eq!(b.missions[second as usize].weather, 0, "mission {i}: clear weather in the sky");
            }
            assert!(!t.powers);
            let rec = record(&ds, i).unwrap();
            assert_eq!(t.cos[0] == PICK, rec.cos[0].1 == 0x1C, "mission {i}: the player's tag CO is picked");
            // Each front's events: Dual Strike's records for it.
            let main = header_records(&b, u32::from_le_bytes(header(i)[4..8].try_into().unwrap()));
            let sec = header_records(&b, u32::from_le_bytes(header(second as usize)[4..8].try_into().unwrap()));
            let fires = |rs: &[(usize, u8, u8, u16, u32)]| rs.iter().filter(|r| r.1 == 7).count();
            let ds_fires = |front: u8| {
                (0..6)
                    .filter_map(|k| ds.u32(rec.header + 4 * k).filter(|&l| l != 0))
                    .map(|l| {
                        let mut n = 0;
                        let mut p = l;
                        while let Some(r) = ds.bytes(p, 8) {
                            if r[0] == 0x1A {
                                break;
                            }
                            if r[0] == 0x17 + front {
                                n += 1;
                            }
                            p += 8;
                        }
                        n
                    })
                    .sum::<usize>()
            };
            assert_eq!(fires(&sec), ds_fires(1), "mission {i}: the second front's events");
            assert!(fires(&main) >= ds_fires(0), "mission {i}: the main front's events");
            assert_ne!(header(i)[0..4], header(second as usize)[0..4], "mission {i}: two maps");
            assert_eq!(u16::from_le_bytes([header(second as usize)[0x24], header(second as usize)[0x25]]), 0, "no day limit on the second front");
        }
        for m in &b.missions {
            if !fronts.iter().any(|f| f.0 == m.index) {
                assert!(m.two_front.is_none(), "mission {}: one front", m.index);
            }
        }
        // Means to an End: the crystals on its second front, none on its main map.
        let tiles = |i: usize| {
            let rec = record(&ds, i).unwrap();
            convert_map(&ds, rec.maps.0).unwrap()
        };
        let (w, _, main) = tiles(MEANS_TO_AN_END);
        assert!(!main.contains(&CRYSTAL), "no Black Crystal on the main map");
        let (w2, _, sec) = tiles(32);
        for &(x, y) in &MTE_CRYSTALS {
            assert_eq!(sec[(y * w2 as u32 + x) as usize], CRYSTAL, "a Black Crystal at ({x}, {y}) on the second front");
        }
        assert_eq!(sec.iter().filter(|&&t| t == CRYSTAL).count(), 3);
        let _ = w;
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
        let mut unknown: Vec<String> = b
            .magic
            .iter()
            .filter_map(|m| match *m {
                Magic::Predicate(f) if f & 0xFF00_0000 == PROPERTY_COUNT => None,
                Magic::Predicate(f) | Magic::Call(f, _) if !crate::ds_campaign_rules::KNOWN.contains(&f) => Some(format!("{m:x?}")),
                _ => None,
            })
            .collect();
        unknown.sort();
        unknown.dedup();
        eprintln!("not implemented: {unknown:?}");
        for m in &b.missions {
            eprintln!("{:2} {:20} front {:?} {}x{} cos {:?} pool {:?}", m.index, m.name, m.two_front, m.width, m.height, m.cos, m.pool);
        }
        assert_eq!(b.missions.len(), MISSIONS + SECOND_FRONTS);
        assert_eq!(b.missions[0].name, "Jake's Trial");
    }
}
