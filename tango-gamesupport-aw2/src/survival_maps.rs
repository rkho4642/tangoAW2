//! Dual Strike's Survival maps and runs, read from the Dual Strike pack and
//! converted to Advance Wars 2's map format at run time (nothing of either
//! game is in the repository).
//!
//! Where Dual Strike (USA) keeps them (overlay 0 is loaded at 0x022AD560):
//!
//! - **Map table**: overlay 0 `0x022DBDB0`, 0xA0 bytes per map, DS map id
//!   `n` at index `n - 1`. The record is AW2's map header grown: armies'
//!   colours at +0x01..+0x04, the name's text id at +0x2C, the map look at
//!   +0x32 (0 normal, 1 snow, 2 desert, 3 wasteland), the weather at +0x33
//!   (0 clear, 1 snow, 2 rain, 3 sandstorm: `sub_020D2104` stores it as the
//!   battle's weather), fog at +0x34, the army count at +0x3C, the speed
//!   rank's day limit at +0x44, the tiles (AW2's LZ77 blob: width, height,
//!   then a u16 per cell in AW2's own tile ids) at +0x5C, the pre-deployed
//!   units (13 bytes each: x, y, type, 0, HP, ammo, fuel, AI fields; `FE
//!   army` starts an army, `FF` ends) at +0x64, and the computer armies' COs
//!   (Dual Strike CO ids) at +0x70 (army 2) and +0x72 (army 3).
//! - **Map names**: Dual Strike's text table, overlay 0 `0x02306954` (a
//!   pointer per text id to an ASCII string).
//! - **The three runs**: eleven map ids each (u16, zero-ended), overlay 0
//!   `0x022F64FC` (Time), `0x022F652C` (Money), `0x022F6514` (Turn); the
//!   kind -> list switch is arm9 `0x020EAC50` (kinds 3..5, the Champion
//!   courses, use the same lists).
//! - **Budgets**: arm9 `0x02168D04`, a word per kind: Time 90000 frames
//!   (25 minutes), Money 500000 G, Turn 99 days; Champion 108000, 600000,
//!   120 (read by `sub_020EAA8C`).
//!
//! Terrain: Dual Strike uses AW2's tile ids (its maps came from AW2's
//! format), plus a few of its own: 0x1A1 the Black Crystal (tangoAW2's
//! 0x192, [`crate::obelisk`]) and 0x1B9..0x1BD the Com Tower, neutral and
//! armies 1..4 (tangoAW2's Com Tower is the Lab, 0x1D9..0x1DD,
//! [`crate::com_tower`]). Everything else is AW2's own tile, structures
//! included (the 4x4 blocks 0x1AA..0x1AD and 0x1AE..0x1B1 on Convoy Cape,
//! Lone Wolf and Silo Sweep are the same tiles AW2's T Minus 15 and Sea
//! Fortress use).

use std::sync::OnceLock;

const OV0: u32 = 0x022A_D560;
const DS_MAPS: u32 = 0x022D_BDB0;
const DS_MAP: u32 = 0xA0;
const DS_TEXT: u32 = 0x0230_6954;
const BUDGETS: u32 = 0x0216_8D04;
/// Each kind's map list (Time, Money, Turn).
const LISTS: [u32; 3] = [0x022F_64FC, 0x022F_652C, 0x022F_6514];
pub const MAPS_PER_RUN: usize = 11;

/// The three Survival kinds, in Dual Strike's order (its kind byte, survival
/// state +7: 0 Time, 1 Money, 2 Turn).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Time = 0,
    Money = 1,
    Turn = 2,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Money, Kind::Turn, Kind::Time];
    pub fn from_u8(v: u8) -> Option<Kind> {
        match v {
            0 => Some(Kind::Time),
            1 => Some(Kind::Money),
            2 => Some(Kind::Turn),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Kind::Time => "Time Survival",
            Kind::Money => "Money Survival",
            Kind::Turn => "Turn Survival",
        }
    }
}

/// A unit placed on a map: army 1.., position, tangoAW2 unit type, HP
/// (0..100), ammo, fuel.
#[derive(Clone, Debug, PartialEq)]
pub struct Unit {
    pub army: u8,
    pub x: u8,
    pub y: u8,
    pub kind: u8,
    pub hp: u8,
    pub ammo: u8,
    pub fuel: u8,
}

#[derive(Clone, Debug)]
pub struct Map {
    /// Dual Strike's map id.
    pub ds_id: u16,
    pub name: String,
    pub width: u8,
    pub height: u8,
    /// AW2 tile ids, row by row (converted).
    pub tiles: Vec<u16>,
    pub units: Vec<Unit>,
    pub armies: u8,
    /// Colour of armies 1..4 (1 Orange Star .. 5 Black Hole).
    pub colours: [u8; 4],
    /// The computer armies' COs as tangoAW2 CO ids (armies 2..4; None: none).
    pub cos: [Option<u8>; 3],
    pub fog: bool,
    /// 0 clear, 1 snow, 2 rain, 3 sandstorm.
    pub weather: u8,
    /// Dual Strike's look: 0 normal, 1 snow, 2 desert, 3 wasteland.
    pub look: u8,
    /// The speed rank's day limit.
    pub speed_days: u16,
    /// The picture of the map's 4x4 structure, for maps with one: the
    /// header's +0x24 points at a `bmap` file name (Convoy Cape and Lone
    /// Wolf "0a5", the missile pad; Silo Sweep "0a6", the fortress).
    pub structure: Option<Structure>,
}

/// The 4x4 structures some Survival maps stand on their map (tiles
/// 0x1AA..0x1AD and 0x1AE..0x1B1). Dual Strike's pictures of them
/// (`bmap/0a5`, `bmap/0a6`) are byte for byte AW2's (`0x080D2DA8`,
/// `0x080D38AC`, which T Minus 15 / Final Front and Show Stopper / Sea
/// Fortress name in their headers' +0x10).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Structure {
    MissilePad,
    Fortress,
}

impl Structure {
    /// AW2's picture: the map header's `tileGraphic4x4` (+0x10), which
    /// `LoadInventionGraphics` (`0x0803FD80`) loads into OBJ VRAM.
    pub fn aw2_picture(self) -> u32 {
        match self {
            Structure::MissilePad => 0x080D_2DA8,
            Structure::Fortress => 0x080D_38AC,
        }
    }
    /// The structure the tiles show.
    pub fn of_tiles(tiles: &[u16]) -> Option<Structure> {
        if tiles.iter().any(|t| (0x1AA..=0x1AD).contains(t)) {
            Some(Structure::MissilePad)
        } else if tiles.iter().any(|t| (0x1AE..=0x1B1).contains(t)) {
            Some(Structure::Fortress)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
pub struct Run {
    pub kind: Kind,
    /// Funds, days, or frames (60 a second).
    pub budget: u32,
    /// Indexes into [`Survival::maps`].
    pub maps: [usize; MAPS_PER_RUN],
}

#[derive(Debug)]
pub struct Survival {
    /// Every map any run uses, in Dual Strike's id order.
    pub maps: Vec<Map>,
    /// Time, Money, Turn (Dual Strike's kind order).
    pub runs: [Run; 3],
}

impl Survival {
    pub fn run(&self, kind: Kind) -> &Run {
        &self.runs[kind as usize]
    }
}

/// Dual Strike's tile -> tangoAW2's.
pub fn convert_tile(t: u16) -> u16 {
    match t {
        0x1A1 => 0x192,                     // Black Crystal
        // Dual Strike's other two mountains (it draws every mountain cell as
        // one of 0x020, 0x146, 0x147 by position: crate::ds_look): AW2's
        // mountain, as the DS Campaign's maps (crate::ds_campaign_data).
        0x146 | 0x147 => 0x022,
        0x1B9..=0x1BD => t - 0x1B9 + 0x1D9, // Com Tower: the Lab tiles
        _ => t,
    }
}

/// Dual Strike's unit type -> tangoAW2's (Carrier and Oozium move up one).
pub fn convert_unit(t: u8) -> Option<u8> {
    match t {
        1..=24 => Some(t),
        25 => Some(crate::roster::CARRIER),
        26 => Some(crate::roster::OOZIUM),
        _ => None,
    }
}

/// Dual Strike's CO id -> tangoAW2's.
pub fn convert_co(ds: u8) -> Option<u8> {
    (0..crate::co_roster::AW2_COS)
        .chain(crate::co_new::FIRST..crate::co_new::FIRST + crate::co_new::NEW.len() as u8)
        .find(|&co| crate::co_roster::ds_co(co) == Some(ds))
}

struct Reader<'a> {
    pack: &'a crate::ds_pack::Pack,
}

impl Reader<'_> {
    fn ov(&self, at: u32, len: usize) -> Option<&[u8]> {
        self.pack.overlay_at(0, OV0, at, len)
    }
    fn u8(&self, at: u32) -> Option<u8> {
        self.ov(at, 1).map(|b| b[0])
    }
    fn u16(&self, at: u32) -> Option<u16> {
        self.ov(at, 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&self, at: u32) -> Option<u32> {
        self.ov(at, 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()))
    }
    fn text(&self, id: u16) -> Option<String> {
        let p = self.u32(DS_TEXT + 4 * id as u32)?;
        let mut s = String::new();
        for k in 0..64 {
            let c = self.u8(p + k)?;
            if c == 0 {
                break;
            }
            s.push(c as char);
        }
        Some(s)
    }
    fn map(&self, ds_id: u16) -> Option<Map> {
        let e = DS_MAPS + DS_MAP * (ds_id as u32 - 1);
        let name = self.text(self.u16(e + 0x2C)?)?;
        let tiles_at = self.u32(e + 0x5C)?;
        let head = self.ov(tiles_at, 4)?;
        let len = (u32::from_le_bytes(head.try_into().unwrap()) >> 8) as usize;
        let raw = crate::ds_art::lz10(self.ov(tiles_at, len * 9 / 8 + 16)?)?;
        let (w, h) = (raw[0], raw[1]);
        let tiles: Vec<u16> = (0..w as usize * h as usize)
            .map(|k| convert_tile(u16::from_le_bytes([raw[2 + 2 * k], raw[3 + 2 * k]])))
            .collect();
        let mut units = Vec::new();
        let mut p = self.u32(e + 0x64)?;
        let mut army = 0;
        for _ in 0..200 {
            let r = self.ov(p, 13)?;
            p += 13;
            match r[0] {
                0xFF => break,
                0xFE => army = r[1],
                _ => units.push(Unit {
                    army,
                    x: r[0],
                    y: r[1],
                    kind: convert_unit(r[2])?,
                    hp: r[4],
                    ammo: r[5],
                    fuel: r[6],
                }),
            }
        }
        let armies = self.u8(e + 0x3C)?;
        let co = |o: u32| -> Option<Option<u8>> {
            let v = self.u8(e + o)?;
            Some(if v == 0 { None } else { convert_co(v) })
        };
        Some(Map {
            ds_id,
            name,
            width: w,
            height: h,
            tiles: tiles.clone(),
            units,
            armies,
            colours: [self.u8(e + 1)?, self.u8(e + 2)?, self.u8(e + 3)?, self.u8(e + 4)?],
            cos: [co(0x70)?, if armies >= 3 { co(0x72)? } else { None }, None],
            fog: self.u8(e + 0x34)? != 0,
            weather: self.u8(e + 0x33)?,
            look: self.u8(e + 0x32)?,
            speed_days: self.u16(e + 0x44)?,
            structure: {
                let name = match self.u32(e + 0x24)? {
                    0 => String::new(),
                    p => (0..3).filter_map(|k| self.u8(p + k)).map(|c| c as char).collect(),
                };
                match name.as_str() {
                    "0a5" => Some(Structure::MissilePad),
                    "0a6" => Some(Structure::Fortress),
                    _ => Structure::of_tiles(&tiles),
                }
            },
        })
    }
}

fn read(pack: &crate::ds_pack::Pack) -> Option<Survival> {
    let r = Reader { pack };
    let mut ids: Vec<u16> = Vec::new();
    let mut lists = Vec::new();
    for list in LISTS {
        let mut l = [0u16; MAPS_PER_RUN];
        for (k, slot) in l.iter_mut().enumerate() {
            *slot = r.u16(list + 2 * k as u32)?;
            if *slot == 0 {
                return None;
            }
        }
        ids.extend(l);
        lists.push(l);
    }
    ids.sort();
    ids.dedup();
    let maps: Vec<Map> = ids.iter().map(|&id| r.map(id)).collect::<Option<_>>()?;
    let budget = |k: u32| -> Option<u32> {
        pack.arm9_at(BUDGETS + 4 * k, 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()))
    };
    let run = |kind: Kind| -> Option<Run> {
        let l = &lists[kind as usize];
        let mut m = [0usize; MAPS_PER_RUN];
        for (k, id) in l.iter().enumerate() {
            m[k] = ids.iter().position(|x| x == id)?;
        }
        Some(Run { kind, budget: budget(kind as u32)?, maps: m })
    };
    Some(Survival { runs: [run(Kind::Time)?, run(Kind::Money)?, run(Kind::Turn)?], maps })
}

static SURVIVAL: OnceLock<Option<Survival>> = OnceLock::new();

/// Dual Strike's Survival, from the pack (None without it).
pub fn survival() -> Option<&'static Survival> {
    SURVIVAL
        .get_or_init(|| crate::ds_pack::pack().and_then(read))
        .as_ref()
}

// --- Dual Strike's rank and score rules -----------------------------------

/// The final rank for what is left of a basic run (`sub_020EAD98`): 5 S,
/// 4 A, 3 B, 2 C. Time in frames, money in G, turns in days.
pub fn rank(kind: Kind, left: u32) -> u8 {
    let t = match kind {
        Kind::Time => [32400, 21600, 10800], // 9, 6 and 3 minutes
        Kind::Money => [50000, 25000, 10000],
        Kind::Turn => [25, 15, 5],
    };
    if left >= t[0] {
        5
    } else if left >= t[1] {
        4
    } else if left >= t[2] {
        3
    } else {
        2
    }
}

pub fn rank_letter(r: u8) -> char {
    match r {
        5 => 'S',
        4 => 'A',
        3 => 'B',
        _ => 'C',
    }
}

/// The points a cleared run's leftover is worth (`sub_020EB024`): a point
/// per 2 seconds, 10 per day, 1 per 200 G.
pub fn leftover_points(kind: Kind, left: u32) -> u32 {
    match kind {
        Kind::Time => left / 60 / 2,
        Kind::Turn => left * 10,
        Kind::Money => left / 200,
    }
}

/// Dual Strike caps a run's points at 9999.
pub const MAX_POINTS: u32 = 9999;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_convert() {
        assert_eq!(convert_tile(0x1A1), 0x192);
        assert_eq!(convert_tile(0x1B9), 0x1D9);
        assert_eq!(convert_tile(0x1BD), 0x1DD);
        assert_eq!(convert_tile(0x02A), 0x02A);
        assert_eq!(convert_unit(25), Some(crate::roster::CARRIER));
        assert_eq!(convert_unit(26), Some(crate::roster::OOZIUM));
        assert_eq!(convert_unit(0), None);
    }

    #[test]
    fn ranks() {
        assert_eq!(rank(Kind::Money, 50000), 5);
        assert_eq!(rank(Kind::Money, 49999), 4);
        assert_eq!(rank(Kind::Turn, 4), 2);
        assert_eq!(rank(Kind::Time, 32400), 5);
        assert_eq!(leftover_points(Kind::Money, 500000), 2500);
    }

    #[test]
    fn cos_convert() {
        assert_eq!(convert_co(2), Some(1)); // Andy
        assert_eq!(convert_co(10), Some(9)); // Drake
        assert_eq!(convert_co(14), Some(73)); // Koal
    }

    /// With a Dual Strike ROM (TANGOAW2_DS_ROM), the three runs read as
    /// Dual Strike has them.
    #[test]
    fn runs_from_the_rom() {
        let Some(path) = std::env::var_os("TANGOAW2_DS_ROM") else { return };
        let Ok(buf) = std::fs::read(path) else { return };
        crate::ds_art::offer(&buf);
        let s = survival().expect("survival data");
        assert_eq!(s.maps.len(), 33);
        assert_eq!(s.run(Kind::Money).budget, 500000);
        assert_eq!(s.run(Kind::Turn).budget, 99);
        assert_eq!(s.run(Kind::Time).budget, 90000);
        let first = |k| &s.maps[s.run(k).maps[0]].name;
        assert_eq!(first(Kind::Money), "Silo Sweep");
        assert_eq!(first(Kind::Turn), "Convoy Cape");
        assert_eq!(first(Kind::Time), "Red Heart");
        let structure = |n: &str| s.maps.iter().find(|m| m.name == n).unwrap().structure;
        assert_eq!(structure("Convoy Cape"), Some(Structure::MissilePad));
        assert_eq!(structure("Lone Wolf"), Some(Structure::MissilePad));
        assert_eq!(structure("Silo Sweep"), Some(Structure::Fortress));
        assert_eq!(s.maps.iter().filter(|m| m.structure.is_some()).count(), 3);
        for m in &s.maps {
            // A map with a structure names its picture, the one its tiles show.
            assert_eq!(m.structure, Structure::of_tiles(&m.tiles), "{}", m.name);
            assert!(m.width as usize * m.height as usize <= 0x508 && m.height <= 40, "{}", m.name);
            assert!(m.tiles.iter().all(|&t| t < 0x200), "{}", m.name);
            assert!(m.cos[0].is_some(), "{} has a computer CO", m.name);
        }
    }
}
