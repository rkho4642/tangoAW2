//! Dual Strike's battle animations for the seven new units, with the Dual
//! Strike pack: the units themselves and their effects (muzzle flashes,
//! missiles and shells, hits and explosions).
//!
//! AW2's battle scene keeps running the fight: backgrounds per terrain and
//! weather, the slide-in, HP counters and timing come from a *donor* AW2
//! unit's scene (Neotank for the Megatank, Rockets or Missiles for the
//! Piperunner, ...; [`donor`]). On a side showing a new unit the donor gets
//! one figure (Dual Strike shows these units as one, whatever their HP),
//! drawn from Dual Strike's own frames, animated by Dual Strike's scripts,
//! in the army's Dual Strike colours; the donor's effects on that side are
//! replaced by Dual Strike's. An Oozium's attack has no scene: Dual Strike
//! resolves it on the map (it slides onto its target), and so does AW2 here.
//!
//! **Dual Strike's data** (the battle overlay, 2, loaded at `0x02350560`,
//! which draws everything with the 3D engine, a textured quad per piece;
//! the sheets and palettes in `battle/`):
//! - Unit records at `0x0236B1BC`, 0x50 bytes, by Dual Strike id - 1:
//!   words 1..5 are sprite sets (body, alternative body, primary weapon,
//!   secondary weapon, extra: ships' wakes), word 6 the palette table (five
//!   pointers to palette file names, one per army), word 7 the unit's scene
//!   numbers (from `+0xE`, five s16 (x, y) points: where its shots leave,
//!   from its origin), words 8 and 9 the muzzle flashes of the primary and
//!   secondary weapon (`{name, animation, flags}`), word 10 its projectile
//!   (five names, then the animation at `+0x14`), words 12..18 effects
//!   (`{name, animation}`): 12 its shots' hit, 14 an indirect shot's
//!   blast, 16 its own blast when destroyed (fire, splash or air burst).
//! - A sprite set: five sheet names (`char[4]`, one per army), five
//!   animation pointers, a word. An animation is `[frame table, script 0,
//!   script 1, ...]`.
//! - A frame: `u16 count`, then pieces of three u16 (as its drawing code,
//!   `0x023538C0`, reads them): x (9 bits, signed), flipped across (bit 10)
//!   and down (bit 11), width code (top 4: 0..3 for 8, 16, 32, 64 px); y (8
//!   bits, signed), height code (top 4); the texels' offset in the sheet in
//!   32-byte units (10 bits), depth (2 bits), palette half (top 4). A
//!   piece's texels are 4bpp rows, low nibble first (the sheets are 3D
//!   textures), drawn opaque (every piece's polygon has alpha 31, so the
//!   smoke is not blended). A frame pointer past the overlay's end is an
//!   empty frame (the blink of a destruction).
//! - A script: `(duration, frame)` byte pairs; frame `0xFC` is an event
//!   (`0x20`: a shot leaves; the Carrier's launches are `0x05`), `(0, 0xFD)`
//!   loops, `(0, 0xFE)` holds, `(0, 0xFF)` ends (the object disappears).
//! - Palettes: 64 bytes, two 16-colour halves; colour 0 transparent. The
//!   effects are neutral: every army's use `battle/119`.
//!
//! **Drawing.** Every frame of the scene, just before its VBlank copy of
//! the shadow OAM ([`OAM_COPY`], [`flush`]): the donor's figure sprites on
//! that side are dropped, and so are its effects (sprites from 512 on:
//! flipped ones are the left side's, the others the right side's; large
//! affine ones are explosions, the side's whose half they are in). The
//! unit's layers and its effects are composed at their current frames,
//! mirrored on the left (Dual Strike's art faces left), each half cropped
//! to its own (the scene's two windows both show sprites), cut into 8x8
//! tiles and covered with square sprites, one set per palette. The tiles go
//! in the side's 256 figure tiles (the donor's own copies there are dropped
//! from the copy queue, [`drain`]) and in 48 free effect tiles; the
//! palettes in the side's four OBJ palettes (the last two lightened, as AW2
//! does for a figure's flash) and in OBJ palettes 12 and 13 (effects).
//!
//! **Acting.** The donor figure's record (`0x02029A10`, per side 0xB4, five
//! figures of 0x24: `+0` state, `+4/+6` home, `+8/+A` position) is moved
//! to the unit's own place, so the enemy's shells land on it; the slide-in
//! and hit shakes still move it. When the side starts firing (`0x020296B0 +
//! 0x28 * side + 0x18` counts its shots), the weapon layer plays its whole
//! firing script, a volley: at each shot event a muzzle flash at the next
//! of the unit's points and a projectile (flying at the enemy's figures and
//! hitting one), or, for a direct shot, a hit on one (a Piperunner's shells
//! at planes and copters are its record-29 anti-air ones). A side fires one
//! volley a battle, timed from the donor's shot so that its last hit lands
//! as the donor's own shot would ([`UnitSpec::wait`]): then AW2 starts the
//! HP drain and the enemy's figures fall; should the drain start before all
//! hits have landed, it holds where the hits so far take it ([`hp_drain`]).
//! When the figure is destroyed its layers blink and its own blasts go off.
//!
//! **Struck.** An AW2 enemy's hits on the unit are Dual Strike's: at each of
//! its figure hits on the side (`0x020298E0 + 0x90 * side + 0x16`) the
//! enemy's own Dual Strike hit effect (its record's hit, by weapon, or its
//! indirect blast) where AW2's landed, and a few over the unit when an
//! indirect or area shot lands; AW2's own hit sprites on the unit are dropped.
//! A shot with no hit of its own (a missile, a shell) bursts on a plane or
//! copter with the target's own blast, as in Dual Strike ([`hit_on`]).
//! Dual Strike draws a hidden Stealth as usual in a battle, and so does this.
//!
//! Everything is worked out from emulated memory, and the animation state
//! lives in EWRAM ([`STATE`], [`FX`]), so rollback and both netplay peers
//! agree. With the pack off nothing here writes anything.

use mgba::core::Core;
use std::sync::OnceLock;

use crate::ds_weather::is_on;
use crate::roster::{BLACK_BOAT, BLACK_BOMB, CARRIER, MEGATANK, OOZIUM, PIPERUNNER, STEALTH};

// --- Dual Strike -------------------------------------------------------------

const OVERLAY: usize = 2;
const OVERLAY_BASE: u32 = 0x0235_0560;
const RECORDS: u32 = 0x0236_B1BC;
const RECORD: u32 = 0x50;
const PALETTE_TABLE: u32 = 6;
const NUMBERS: u32 = 7;
const POINTS: u32 = 0xE;
const MUZZLES: [u32; 2] = [8, 9];
const PROJECTILE: u32 = 10;
const HIT: u32 = 12;
const INDIRECT_HIT: u32 = 14;
const BLAST: u32 = 16;
/// The effects' palette.
const FX_PALETTE: &str = "battle/119";
const ARMIES: usize = 5;
const SIZES: [usize; 4] = [8, 16, 32, 64];

const FIRE_EVENT: u8 = 0x20;
const EVENT: u8 = 0xFC;
const LOOP: u8 = 0xFD;
const HOLD: u8 = 0xFE;
const END: u8 = 0xFF;

/// One layer of a unit: its sprite set (word of the record), the weapon it
/// shows with (0 always, else 1 primary / 2 secondary), and its firing and
/// destruction scripts (script 0 is idle).
struct LayerSpec {
    set: u32,
    weapon: u8,
    fire: Option<u8>,
    dead: Option<u8>,
}

const fn layer(set: u32, weapon: u8, fire: Option<u8>, dead: Option<u8>) -> LayerSpec {
    LayerSpec { set, weapon, fire, dead }
}

/// How a unit's shots leave: the event in its firing script that fires
/// (0 when the muzzle flash's own script fires, as the Stealth's missiles
/// drop from under its wings), which of its points each weapon uses, and
/// its projectile's flight: speed (px a frame), and a launch (dx, dy, frames)
/// before it turns to its target.
struct Shots {
    event: u8,
    points: [std::ops::Range<usize>; 2],
    speed: i32,
    launch: (i32, i32, u8),
    /// Without a firing script: shots in a burst, frames apart.
    burst: (u8, u8),
}

const fn shots(event: u8, w1: std::ops::Range<usize>, w2: std::ops::Range<usize>, speed: i32) -> Shots {
    Shots { event, points: [w1, w2], speed, launch: (0, 0, 0), burst: (0, 0) }
}

/// A new unit: its AW2 id, Dual Strike record (and the record it uses
/// against planes and copters, if another: its projectile), layers, shots,
/// whether it flies, and how far past the middle of its half its centre sits
/// (outwards, px).
struct UnitSpec {
    id: u8,
    record: u32,
    air_record: u32,
    layers: &'static [LayerSpec],
    shots: Shots,
    /// Frames from the donor's shot to the volley (per weapon), so that its
    /// last hit lands as the donor's would (when AW2's HP drain starts and
    /// the enemy's figures fall).
    wait: [u8; 2],
    air: bool,
    outwards: i32,
}

const UNITS: [UnitSpec; 7] = [
    UnitSpec {
        id: MEGATANK,
        record: 3,
        air_record: 0,
        layers: &[layer(1, 0, None, Some(1)), layer(3, 1, Some(1), Some(2)), layer(4, 2, Some(1), Some(2))],
        shots: shots(FIRE_EVENT, 0..5, 0..5, 0),
        wait: [14, 2],
        air: false,
        outwards: 0,
    },
    UnitSpec {
        id: PIPERUNNER,
        record: 8,
        air_record: 29,
        layers: &[layer(1, 0, None, Some(1)), layer(3, 0, Some(1), Some(2))],
        shots: shots(FIRE_EVENT, 0..1, 0..1, 9),
        wait: [76, 76],
        air: false,
        outwards: 0,
    },
    UnitSpec {
        id: STEALTH,
        record: 11,
        air_record: 0,
        layers: &[layer(1, 0, None, None)],
        shots: Shots { event: 0, points: [0..5, 0..5], speed: 7, launch: (3, 1, 6), burst: (5, 5) },
        wait: [63, 63],
        air: true,
        outwards: 0,
    },
    UnitSpec {
        id: BLACK_BOMB,
        record: 12,
        air_record: 0,
        layers: &[layer(1, 0, None, None)],
        shots: shots(0, 0..0, 0..0, 0),
        wait: [0, 0],
        air: true,
        outwards: 0,
    },
    UnitSpec {
        id: BLACK_BOAT,
        record: 17,
        air_record: 0,
        layers: &[layer(1, 0, None, None), layer(5, 0, None, None)],
        shots: shots(0, 0..0, 0..0, 0),
        wait: [0, 0],
        air: false,
        outwards: 8,
    },
    UnitSpec {
        id: CARRIER,
        record: 24,
        air_record: 0,
        layers: &[layer(1, 0, Some(1), None), layer(5, 0, None, None)],
        shots: Shots { event: 0x05, points: [0..5, 0..5], speed: 7, launch: (2, -5, 10), burst: (0, 0) },
        wait: [0, 0],
        air: false,
        outwards: -24,
    },
    UnitSpec {
        id: OOZIUM,
        record: 25,
        air_record: 0,
        layers: &[layer(1, 0, None, Some(1))],
        shots: shots(0, 0..0, 0..0, 0),
        wait: [0, 0],
        air: false,
        outwards: 12,
    },
];

fn spec_of(t: u8) -> Option<usize> {
    UNITS.iter().position(|u| u.id == t)
}

/// A piece of a frame: position from the figure's origin, size, palette
/// half, and its texels (one byte each, 0 transparent).
struct Piece {
    x: i32,
    y: i32,
    w: usize,
    half: u8,
    texels: Vec<u8>,
}

struct Layer {
    weapon: u8,
    frames: Vec<Vec<Piece>>,
    /// Idle, fire, destruction.
    scripts: [Vec<(u8, u8)>; 3],
}

/// An effect: its frames and its script (script 0 of its animation).
struct Fx {
    frames: Vec<Vec<Piece>>,
    script: Vec<(u8, u8)>,
    /// Whether its script fires a projectile (a fire event).
    fires: bool,
}

impl Fx {
    /// Where a new one starts: a projectile at its first drawn frame (its
    /// script opens with frames inside the barrel).
    fn first(&self, kind: u8) -> usize {
        if !is_projectile(kind) {
            return 0;
        }
        let drawn = |f: u8| self.frames.get(f as usize).is_some_and(|p| !p.is_empty());
        self.script.iter().position(|&(_, f)| f < EVENT && drawn(f)).unwrap_or(0)
    }
}

/// A unit converted for one army.
struct Figure {
    layers: Vec<Layer>,
    palettes: [[u16; 16]; 2],
    /// The idle picture's bounds, from the origin: x0, y0, x1, y1.
    bounds: (i32, i32, i32, i32),
    /// Muzzle flashes (per weapon), projectile, hit, own blast, projectile
    /// against planes and copters.
    fx: [Option<Fx>; 6],
    points: [(i32, i32); 5],
}

const FX_MUZZLE: u8 = 0;
const FX_PROJECTILE: u8 = 2;
const FX_HIT: u8 = 3;
const FX_BLAST: u8 = 4;
const FX_AIR: u8 = 5;
/// Not the figure's: the enemy's hit on it ([`struck`]).
const FX_STRUCK: u8 = 6;

fn is_projectile(kind: u8) -> bool {
    kind == FX_PROJECTILE || kind == FX_AIR
}

fn ov2() -> Option<&'static [u8]> {
    crate::ds_pack::pack()?.overlays.get(OVERLAY).map(|v| v.as_slice())
}

fn word(addr: u32) -> Option<u32> {
    let o = addr.checked_sub(OVERLAY_BASE)? as usize;
    ov2()?.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()))
}

fn bytes(addr: u32, len: usize) -> Option<&'static [u8]> {
    let o = addr.checked_sub(OVERLAY_BASE)? as usize;
    ov2()?.get(o..o + len)
}

fn in_overlay(addr: u32) -> bool {
    ov2().is_some_and(|o| (OVERLAY_BASE..OVERLAY_BASE + o.len() as u32).contains(&addr))
}

/// A three-letter file name stored at `addr`.
fn name(addr: u32) -> Option<String> {
    let b = bytes(addr, 4)?;
    let n: Vec<u8> = b.iter().copied().take_while(|&c| c != 0).collect();
    (n.len() == 3).then(|| String::from_utf8_lossy(&n).into_owned())
}

fn script(addr: u32) -> Option<Vec<(u8, u8)>> {
    let mut out = Vec::new();
    for k in 0..256 {
        let b = bytes(addr + 2 * k, 2)?;
        out.push((b[0], b[1]));
        if b[0] == 0 && b[1] >= LOOP {
            return Some(out);
        }
    }
    None
}

fn frame(addr: u32, sheet: &[u8]) -> Option<Vec<Piece>> {
    if !in_overlay(addr) {
        return Some(Vec::new());
    }
    let count = u16::from_le_bytes(bytes(addr, 2)?.try_into().ok()?) as u32;
    let mut out = Vec::new();
    for k in 0..count {
        let p = bytes(addr + 2 + 6 * k, 6)?;
        let h = |i: usize| u16::from_le_bytes([p[2 * i], p[2 * i + 1]]);
        let (h0, h1, h2) = (h(0), h(1), h(2));
        let (w, ht) = (SIZES[(h0 >> 12) as usize & 3], SIZES[(h1 >> 12) as usize & 3]);
        let at = 32 * (h2 & 0x3FF) as usize;
        let raw = sheet.get(at..at + w * ht / 2)?;
        let texel = |i: usize| if i & 1 == 0 { raw[i / 2] & 15 } else { raw[i / 2] >> 4 };
        let (hflip, vflip) = (h0 & 0x400 != 0, h0 & 0x800 != 0);
        let texels = (0..w * ht)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                texel(if vflip { ht - 1 - y } else { y } * w + if hflip { w - 1 - x } else { x })
            })
            .collect();
        // x: 9 bits signed; y: 8 bits signed.
        let x = ((h0 & 0x1FF) as i32 ^ 0x100) - 0x100;
        out.push(Piece { x, y: h1 as u8 as i8 as i32, w, half: (h2 >> 12) as u8 & 1, texels });
    }
    Some(out)
}

fn sheet(name: &str) -> Option<Vec<u8>> {
    let file = crate::ds_pack::pack()?.file(&format!("battle/{name}"))?;
    Some(crate::ds_art::lz10(file).unwrap_or_else(|| file.to_vec()))
}

fn palette_file(path: &str) -> Option<[[u16; 16]; 2]> {
    let f = crate::ds_pack::pack()?.file(path)?;
    let mut out = [[0u16; 16]; 2];
    for (h, half) in out.iter_mut().enumerate() {
        for (i, c) in half.iter_mut().enumerate() {
            let o = 32 * h + 2 * i;
            *c = u16::from_le_bytes([*f.get(o)?, *f.get(o + 1)?]);
        }
    }
    Some(out)
}

fn palette(addr: u32) -> Option<[[u16; 16]; 2]> {
    palette_file(&format!("battle/{}", name(addr)?))
}

/// The frames a set of scripts uses, from a frame table.
fn frames_for(table: u32, scripts: &[&[(u8, u8)]], sheet: &[u8]) -> Option<Vec<Vec<Piece>>> {
    let count = scripts.iter().flat_map(|s| s.iter()).filter(|&&(_, f)| f < EVENT).map(|&(_, f)| f as u32 + 1).max()?;
    (0..count).map(|f| frame(word(table + 4 * f)?, sheet)).collect()
}

/// An effect from its sheet's name and its animation.
fn effect(name: &str, anim: u32) -> Option<Fx> {
    let sheet = sheet(name)?;
    let script = script(word(anim + 4)?)?;
    let frames = frames_for(word(anim)?, &[&script], &sheet)?;
    let fires = script.iter().any(|&(d, f)| f == EVENT && d == FIRE_EVENT);
    Some(Fx { frames, script, fires })
}

/// An effect given as `{name, animation}` at `at` (a null `at`: none).
fn effect_at(at: u32) -> Option<Fx> {
    if at == 0 {
        return None;
    }
    effect(&name(at)?, word(at + 4)?)
}

fn convert(u: &UnitSpec, army: usize) -> Option<Figure> {
    let rec = RECORDS + RECORD * u.record;
    let palettes = palette(word(word(rec + 4 * PALETTE_TABLE)? + 4 * army as u32)?)?;
    let mut layers = Vec::new();
    for l in u.layers {
        let set = word(rec + 4 * l.set)?;
        let sheet = sheet(&name(set + 4 * army as u32)?)?;
        let anim = word(set + 20 + 4 * army as u32)?;
        let script_at = |k: u8| word(anim + 4 * (k as u32 + 1)).and_then(script);
        let scripts = [
            script_at(0)?,
            l.fire.map_or(Some(Vec::new()), script_at)?,
            l.dead.map_or(Some(Vec::new()), script_at)?,
        ];
        let frames = frames_for(word(anim)?, &[&scripts[0], &scripts[1], &scripts[2]], &sheet)?;
        layers.push(Layer { weapon: l.weapon, frames, scripts });
    }
    // The drawn pixels' bounds (pieces carry margins).
    let mut b = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for (l, spec) in layers.iter().zip(u.layers) {
        if spec.weapon > 1 {
            continue;
        }
        let idle = l.scripts[0].first().map_or(0, |&(_, f)| f as usize);
        for p in l.frames.get(idle).into_iter().flatten() {
            for (i, &t) in p.texels.iter().enumerate() {
                if t != 0 {
                    let (x, y) = (p.x + (i % p.w) as i32, p.y + (i / p.w) as i32);
                    b = (b.0.min(x), b.1.min(y), b.2.max(x + 1), b.3.max(y + 1));
                }
            }
        }
    }
    let numbers = word(rec + 4 * NUMBERS)?;
    let mut points = [(0, 0); 5];
    for (k, p) in points.iter_mut().enumerate() {
        let b = bytes(numbers + POINTS + 4 * k as u32, 4)?;
        *p = (i16::from_le_bytes([b[0], b[1]]) as i32, i16::from_le_bytes([b[2], b[3]]) as i32);
    }
    let muzzle = |w: usize| word(rec + 4 * MUZZLES[w]).and_then(effect_at);
    let projectile = |rec: u32| word(rec + 4 * PROJECTILE).filter(|&a| a != 0).and_then(|a| effect(&name(a + 4 * army as u32)?, word(a + 0x14)?));
    let hit = hit_of(u.record, 1);
    let blast = word(rec + 4 * BLAST).and_then(effect_at);
    let air = if u.air_record != 0 { projectile(RECORDS + RECORD * u.air_record) } else { None };
    let fx = [muzzle(0), muzzle(1), projectile(rec), hit, blast, air];
    Some(Figure { layers, palettes, bounds: b, fx, points })
}

/// The hit a unit's (Dual Strike record's) weapon makes: its own (the
/// secondary's from word 13), or an indirect shot's blast.
fn hit_of(record: u32, weapon: u8) -> Option<Fx> {
    let rec = RECORDS + RECORD * record;
    let own = if weapon == 2 { HIT + 1 } else { HIT };
    word(rec + 4 * own).and_then(effect_at).or_else(|| word(rec + 4 * INDIRECT_HIT).and_then(effect_at))
}

static FIGURES: OnceLock<Vec<Option<Figure>>> = OnceLock::new();
static FX_PALETTES: OnceLock<Option<[[u16; 16]; 2]>> = OnceLock::new();
static HITS: OnceLock<Vec<[Option<Fx>; 3]>> = OnceLock::new();

/// Dual Strike records with a battle scene (the units, then variants).
const SCENE_RECORDS: u32 = 32;

/// Whether record `record`'s weapon (1, 2) has a hit of its own (word 12,
/// 13), not only an indirect blast.
fn own_hit(record: u32, weapon: u8) -> bool {
    let own = if weapon == 2 { HIT + 1 } else { HIT };
    word(RECORDS + RECORD * record + 4 * own).is_some_and(|w| w != 0)
}

/// The hit of record `record`'s weapon (1, 2) on a target: its own hit, or
/// its indirect blast; on a plane or copter (`air_target`, its record) a
/// blast is the target's own (Dual Strike's missiles and shells burst in
/// the air: `0c7`).
fn hit_on(record: u32, weapon: u8, air_target: Option<u32>) -> Option<&'static Fx> {
    let all = HITS.get_or_init(|| {
        (0..SCENE_RECORDS).map(|r| [hit_of(r, 1), hit_of(r, 2), word(RECORDS + RECORD * r + 4 * BLAST).and_then(effect_at)]).collect()
    });
    match air_target {
        Some(t) if !own_hit(record, weapon) => all.get(t as usize)?[2].as_ref(),
        _ => all.get(record as usize)?[(weapon == 2) as usize].as_ref(),
    }
}

fn figure(unit: usize, army: usize) -> Option<&'static Figure> {
    let all = FIGURES.get_or_init(|| UNITS.iter().flat_map(|u| (0..ARMIES).map(move |a| convert(u, a))).collect());
    all.get(unit * ARMIES + army.min(ARMIES - 1))?.as_ref()
}

// --- AW2's scene ----------------------------------------------------------------

/// The battle's two sides (0 the left half, 1 the right): 16-byte rows of u16s,
/// `[0]` colour, `[1]` scene id, `[2]` weapon (1 primary, 2 secondary).
const ROWS: u32 = 0x0300_4580;
const ROW: u32 = 0x10;
/// The attacker's battle record (`+0` its unit); the two sides are the
/// screen's halves, not attacker and defender (a CPU attacker is on the
/// right).
const ATTACKER: u32 = 0x0300_13D0;
/// The two sides' units (pointers to 12-byte unit records).
const SIDE_UNITS: u32 = 0x0300_4528;
/// The two sides' army colours (0 Orange Star .. 4 Black Hole), kept by
/// the scene before it remaps `[0]`.
const SIDE_COLOURS: u32 = 0x0300_4500;
/// Figure records: per side 0xB4, five of 0x24.
const FIGURE_RECORDS: u32 = 0x0202_9A10;
const SIDE_FIGURES: u32 = 0xB4;
const FIGURE_RECORD: u32 = 0x24;
/// Per side (0x28): `+0x18` the shots fired so far.
const SHOTS: u32 = 0x0202_96B0 + 0x18;
const SIDE_SHOTS: u32 = 0x28;
/// Which figures stand, per side and display HP (`[2][11][5]`, 2 = whole).
const FIGURES_BY_HP: u32 = 0x0855_21DC;
/// The unit table's class byte: 0 foot, 1 vehicle, 2 plane, 3 copter, 4 ship.
const CLASS: u32 = 0x18;
const UNIT_RECORD: u32 = 0x5C;

/// The scene's proc script, and the proc pool.
const SCENE_PROC: u32 = 0x0849_FEF8;
const PROCS: u32 = 0x0200_D610;
const PROCS_END: u32 = 0x0200_E418;
const PROC_SIZE: u32 = 0x6C;
const MAIN_CALLBACK: u32 = 0x0300_0000;
const MAP_CALLBACK: u32 = 0x0802_2049;

/// The deferred copy queue (0x0C per entry: source, destination, size,
/// kind) and its length, drained at VBlank by `sub_08011FF0`.
pub const DRAIN: u32 = 0x0801_1FF0;
const QUEUE: u32 = 0x0200_B3B4;
const QUEUE_LEN: u32 = 0x0300_2F30;
const QUEUE_ENTRY: u32 = 0x0C;
const QUEUE_SKIP: u8 = 6;

/// `sub_08041978`, just before it chooses the scene or the map-only proc
/// (`r2`: the Visuals option, 0 map only): both rows are filled.
pub const ROWS_FILLED: u32 = 0x0804_1CF4;
/// `sub_08057164(before, after, side)` fills a side's figure records from
/// the display HP.
pub const FIGURE_COUNT: u32 = 0x0805_7164;
/// Display HP that shows exactly one whole figure.
const ONE_FIGURE: u32 = 2;

/// The shadow OAM (128 entries), copied to OAM at VBlank by `sub_0801F0AC`
/// (from the scene's VBlank callback `sub_080366F4`); a hidden entry.
pub const OAM_COPY: u32 = 0x0801_F0AC;
const SHADOW: u32 = 0x0300_2520;
const SHADOW_ENTRIES: u32 = 128;
const HIDDEN: u16 = 0x0200;
/// The figures' priority (the scene's shells and explosions have 1 and 2).
const FIGURE_PRIORITY: u16 = 3;
const FX_PRIORITY: u16 = 2;
/// The scene's effect sprites use tiles from here (below: the two sides'
/// figures); its explosions are affine sprites of 32 px and more.
const EFFECT_TILES: u16 = 512;
const HFLIP: u16 = 0x1000;

const OBJ_VRAM: u32 = 0x0601_0000;
const SIDE_TILES: u32 = 256;
/// Tiles no scene uses (its effects end below 880, the map's sheets start
/// at 976): 48 a side for Dual Strike's effects.
const FX_TILES: u32 = 880;
const SIDE_FX_TILES: u32 = 48;
const FX_PAL: u16 = 12;
const PAL_BUFFER: u32 = 0x0300_20C0 + 0x200;
const PAL_RAM: u32 = 0x0500_0200;
/// Each half of the screen, as the scene's windows show it.
const HALVES: [(i32, i32); 2] = [(0, 119), (121, 240)];
const SCREEN_H: i32 = 160;
/// Where a ground or sea unit stands, and where a plane flies (its centre).
const GROUND: i32 = 150;
const SKY: i32 = 88;

/// tangoAW2's state per side, in EWRAM (saved with the console).
const STATE: u32 = 0x0203_F800;
const SIDE_STATE: u32 = 0x40;
/// +0 unit (index + 1, 0 none), +1 army, +2 weapon, +3 figure record,
/// +4 shots seen (u16), +6 destroyed, +7 shots fired in the volley, +8 per
/// layer 4 bytes: script, step, time left, frame (script 3: gone). Then:
/// the enemy's Dual Strike record + 1 and weapon (for its hits on the unit),
/// whether it flies, its figure hits on the side so far (u8); the
/// volley's hits (0: none running), hits landed, frames, and whether the
/// donor has fired; the enemy's HP shown when the volley started (u16); the
/// frames left before the volley; whether the side's HP drain has started.
const S_UNIT: u32 = 0;
const S_ARMY: u32 = 1;
const S_WEAPON: u32 = 2;
const S_FIGURE: u32 = 3;
const S_SHOTS: u32 = 4;
const S_DEAD: u32 = 6;
const S_VOLLEY: u32 = 7;
const S_LAYERS: u32 = 8;
const S_ENEMY: u32 = 0x14;
const S_ENEMY_WEAPON: u32 = 0x15;
const S_ENEMY_AIR: u32 = 0x16;
const S_STRUCK: u32 = 0x17;
const S_HITS: u32 = 0x18;
const S_LANDED: u32 = 0x19;
const S_TIME: u32 = 0x1A;
const S_FIRED: u32 = 0x1B;
const S_WAIT: u32 = 0x1E;
const S_AREA: u32 = 0x1F;
const S_HP0: u32 = 0x1C;
const GONE: u8 = 3;

/// Per side (u16, HP x 10): the HP shown, and the HP after the battle.
const HP_SHOWN: u32 = 0x0202_9B78;
const HP_AFTER: u32 = 0x0202_9B7C;
/// `sub_08057BDC`, each frame of the scene, drains the HP counters: per
/// side, while [`DRAINING`] (u16) is set and the HP shown is not the HP
/// after, a 16.16 value ([`DRAIN_VALUE`], u32) less a rate ([`DRAIN_RATE`],
/// set up so that its last step lands on the HP after) is the HP shown; the
/// figures fall and the counter redraws from it. A side's shot landing
/// (`sub_08057BCC`) sets [`DRAINING`].
pub const HP_DRAIN: u32 = 0x0805_7BDC;
const DRAINING: u32 = 0x0300_05E8;
const DRAIN_VALUE: u32 = 0x0300_05D8;
const DRAIN_RATE: u32 = 0x0300_05E0;
/// A volley's hold on the enemy's HP ends after this many frames.
const VOLLEY_FRAMES: u8 = 240;

/// Effects in flight, per side: 12 of 16 bytes. +0 kind + 1 (0 free: the
/// [`Figure::fx`] index), +1 step, +2 time left, +3 frame, +4/+6 x, y (s16,
/// the effect's origin on screen), +8/+9 dx, dy (s8, a frame), +A launch
/// frames left, +B frames before it shows, +C/+E the target (s16).
pub const FX: u32 = 0x0203_F880;
const SIDE_FX: u32 = 0xC0;
const FX_SIZE: u32 = 0x10;
const FX_SLOTS: u32 = 12;
/// A direct shot's hit comes this many frames after the flash.
const HIT_DELAY: u8 = 5;
/// An AW2 enemy's effect sprite this close to a unit (px) is its hit.
const IMPACT_MARGIN: i32 = 12;
/// Per side (0x90, u16 at `+0x16`): the figure hits it has taken (a direct
/// shot landing on a figure, `sub_08054500`); an indirect or area shot
/// lands with no figure hits and starts the side's HP drain.
const FIGURE_HITS: u32 = 0x0202_98E0 + 0x16;
const SIDE_HITS: u32 = 0x90;
/// An area hit's Dual Strike hits on the unit (from its middle, in quarters
/// of its size), frames apart.
const AREA_HITS: [(i32, i32); 3] = [(0, 0), (-1, -1), (1, 1)];
const AREA_GAP: u8 = 6;

/// Whether the scene is on screen: its proc runs and the battle map's main
/// callback is not back yet (the proc outlives the scene by a few frames).
fn scene_running(core: &Core) -> bool {
    core.raw_read_32(MAIN_CALLBACK, -1) != MAP_CALLBACK
        && (PROCS..PROCS_END).step_by(PROC_SIZE as usize).any(|p| core.raw_read_32(p, -1) == SCENE_PROC)
}

fn side_unit_type(core: &Core, side: u32) -> u8 {
    let u = core.raw_read_32(SIDE_UNITS + 4 * side, -1);
    if (0x0200_0000..0x0204_0000).contains(&u) {
        core.raw_read_8(u, -1)
    } else {
        0
    }
}

fn class_of(core: &Core, t: u8) -> u8 {
    core.raw_read_8(crate::roster::table(core) + UNIT_RECORD * t as u32 + CLASS, -1)
}

/// The AW2 unit whose scene a new unit plays, and the weapon it shows,
/// against a target of `class`.
fn donor(t: u8, class: u8, weapon: u16) -> (u16, u16) {
    let air = matches!(class, 2 | 3);
    match t {
        MEGATANK => (7, weapon),
        PIPERUNNER if air => (14, 1),
        PIPERUNNER => (10, 1),
        STEALTH if air => (15, 1),
        STEALTH => (16, 1),
        BLACK_BOMB => (16, weapon),
        BLACK_BOAT => (22, weapon),
        CARRIER => (21, 2),
        OOZIUM if class == 4 => (4, 1),
        OOZIUM => (4, 2),
        _ => (0, weapon),
    }
}

/// Trap at [`ROWS_FILLED`]: a new unit's side plays its donor's scene; an
/// Oozium's attack plays on the map, as in Dual Strike.
fn rows_filled(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let attacker = core.raw_read_32(ATTACKER, -1);
    if (0x0200_0000..0x0204_0000).contains(&attacker) && core.raw_read_8(attacker, -1) == OOZIUM {
        core.gba_mut().cpu_mut().set_gpr(2, 0);
    }
    for side in 0..2 {
        let t = side_unit_type(core, side);
        if spec_of(t).is_none() {
            continue;
        }
        let other = class_of(core, side_unit_type(core, 1 - side));
        let row = ROWS + ROW * side;
        let weapon = core.raw_read_16(row + 4, -1);
        let (scene, w) = donor(t, other, weapon);
        core.raw_write_16(row + 2, -1, scene);
        if weapon != 0 {
            core.raw_write_16(row + 4, -1, w);
        }
    }
}

/// Trap at [`FIGURE_COUNT`]: a new unit's side gets one figure (whole, or
/// destroyed in this battle), and its animation state is set up.
fn figure_count(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let cpu = core.gba().cpu();
    let (before, after, side) = (cpu.gpr(0) as u32, cpu.gpr(1) as u32, cpu.gpr(2) as u32 & 0xFFFF);
    if side > 1 {
        return;
    }
    let state = STATE + SIDE_STATE * side;
    core.raw_write_range(state, -1, &[0u8; SIDE_STATE as usize]);
    core.raw_write_range(FX + SIDE_FX * side, -1, &[0u8; SIDE_FX as usize]);
    let t = side_unit_type(core, side);
    let Some(unit) = spec_of(t) else { return };
    let army = core.raw_read_8(SIDE_COLOURS + side, -1).min(ARMIES as u8 - 1) as usize;
    let Some(fig) = figure(unit, army) else { return };
    let one = |hp: u32| if hp > 0 { ONE_FIGURE } else { 0 };
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, one(before) as i32);
    cpu.set_gpr(1, one(after) as i32);
    // The figure that stands at that HP.
    let mut standing = 0;
    for f in 0..5 {
        if core.raw_read_8(FIGURES_BY_HP + 55 * side + 5 * ONE_FIGURE + f, -1) != 0 {
            standing = f;
        }
    }
    let weapon = core.raw_read_16(ROWS + ROW * side + 4, -1) as u8;
    let enemy = side_unit_type(core, 1 - side);
    if let Some(ds) = crate::roster::ds_id(enemy) {
        core.raw_write_8(state + S_ENEMY, -1, ds);
        core.raw_write_8(state + S_ENEMY_WEAPON, -1, core.raw_read_16(ROWS + ROW * (1 - side) + 4, -1) as u8);
    }
    core.raw_write_8(state + S_ENEMY_AIR, -1, matches!(class_of(core, enemy), 2 | 3) as u8);
    core.raw_write_8(state + S_STRUCK, -1, core.raw_read_16(FIGURE_HITS + SIDE_HITS * side, -1) as u8);
    core.raw_write_8(state + S_AREA, -1, (core.raw_read_16(DRAINING + 2 * side, -1) != 0) as u8);
    core.raw_write_8(state + S_UNIT, -1, unit as u8 + 1);
    core.raw_write_8(state + S_ARMY, -1, army as u8);
    core.raw_write_8(state + S_WEAPON, -1, weapon);
    core.raw_write_8(state + S_FIGURE, -1, standing as u8);
    for (l, layer) in fig.layers.iter().enumerate() {
        start(core, state + S_LAYERS + 4 * l as u32, &layer.scripts, 0, 0, 0);
    }
}

/// Start script `which` (0 idle, 1 fire, 2 destruction) at `step` for the
/// script state at `at`; returns the fire events passed (`event`).
fn start(core: &mut Core, at: u32, scripts: &[Vec<(u8, u8)>], which: u8, step: usize, event: u8) -> u8 {
    core.raw_write_8(at, -1, which);
    settle(core, at, scripts, step, event)
}

/// Run a script from `step` to its next frame entry; a held firing script
/// goes back to idle. Returns the fire events (`event`) passed.
fn settle(core: &mut Core, at: u32, scripts: &[Vec<(u8, u8)>], mut step: usize, event: u8) -> u8 {
    let which = core.raw_read_8(at, -1);
    let script = &scripts[which as usize % scripts.len()];
    let mut fired = 0;
    for _ in 0..script.len() + 2 {
        let Some(&(d, f)) = script.get(step) else { break };
        match f {
            EVENT => {
                if d == event && event != 0 {
                    fired += 1;
                }
                step += 1;
            }
            LOOP if d == 0 => step = 0,
            HOLD if d == 0 => {
                if which == 1 && scripts.len() > 1 {
                    return fired + start(core, at, scripts, 0, 0, 0);
                }
                core.raw_write_8(at + 2, -1, 0);
                return fired;
            }
            END if d == 0 => {
                core.raw_write_8(at, -1, GONE);
                return fired;
            }
            _ => {
                core.raw_write_8(at + 1, -1, step as u8);
                core.raw_write_8(at + 2, -1, d.max(1));
                core.raw_write_8(at + 3, -1, f);
                return fired;
            }
        }
    }
    core.raw_write_8(at + 2, -1, 0);
    fired
}

/// One frame of a script; returns the fire events (`event`) passed.
fn step_script(core: &mut Core, at: u32, scripts: &[Vec<(u8, u8)>], event: u8) -> u8 {
    let which = core.raw_read_8(at, -1);
    if which == GONE {
        return 0;
    }
    let left = core.raw_read_8(at + 2, -1);
    if left == 0 {
        return 0; // held
    }
    if left > 1 {
        core.raw_write_8(at + 2, -1, left - 1);
        return 0;
    }
    let step = core.raw_read_8(at + 1, -1) as usize + 1;
    settle(core, at, scripts, step, event)
}

/// Trap at [`DRAIN`]: copies into a side's figure tiles are dropped while
/// that side shows a Dual Strike unit.
pub fn drain(core: &mut Core) {
    if !is_on(core) || !scene_running(core) {
        return;
    }
    let n = core.raw_read_16(QUEUE_LEN, -1) as u32;
    for k in 0..n.min(48) {
        let e = QUEUE + QUEUE_ENTRY * k;
        let (dst, kind) = (core.raw_read_32(e + 4, -1), core.raw_read_8(e + 0xA, -1));
        if kind > 2 {
            continue;
        }
        for side in 0..2 {
            let base = OBJ_VRAM + 32 * SIDE_TILES * side;
            if (base..base + 32 * SIDE_TILES).contains(&dst) && core.raw_read_8(STATE + SIDE_STATE * side, -1) != 0 {
                core.raw_write_8(e + 0xA, -1, QUEUE_SKIP);
            }
        }
    }
}

fn lighten(c: u16) -> u16 {
    let ch = |s: u16| (((c >> s) & 31) + 31) / 2;
    ch(0) | ch(5) << 5 | ch(10) << 10
}

fn put_palette(core: &mut Core, pal: u32, colours: &[u16; 16]) {
    let mut b = Vec::with_capacity(30);
    for c in &colours[1..] {
        b.extend_from_slice(&c.to_le_bytes());
    }
    for base in [PAL_BUFFER, PAL_RAM] {
        let at = base + 32 * pal + 2;
        let mut now = vec![0u8; b.len()];
        core.raw_read_range(at, -1, &mut now);
        if now != b {
            core.raw_write_range(at, -1, &b);
        }
    }
}

// --- Composing -------------------------------------------------------------------

/// A half of the screen being composed: a grid of 8x8 cells aligned on
/// `grid` over the half; each pixel is 0 or colour | (plane + 1) << 4
/// (planes: 0, 1 the unit's palette halves, 2, 3 the effects').
struct Canvas {
    half: (i32, i32),
    gx: i32,
    gy: i32,
    cols: usize,
    rows: usize,
    buf: Vec<u8>,
}

impl Canvas {
    fn new(half: (i32, i32), grid: (i32, i32)) -> Canvas {
        let c0 = (half.0 - grid.0).div_euclid(8);
        let c1 = (half.1 - grid.0 + 7).div_euclid(8);
        let r0 = (0 - grid.1).div_euclid(8);
        let r1 = (SCREEN_H - grid.1 + 7).div_euclid(8);
        let (cols, rows) = ((c1 - c0).max(0) as usize, (r1 - r0).max(0) as usize);
        Canvas { half, gx: grid.0 + 8 * c0, gy: grid.1 + 8 * r0, cols, rows, buf: vec![0; cols * rows * 64] }
    }

    /// Draw pieces with their origin at `(ox, oy)`, mirrored or not.
    fn draw(&mut self, pieces: &[Piece], (ox, oy): (i32, i32), mirror: bool, plane: u8) {
        let w = self.cols * 8;
        for p in pieces {
            for (i, &t) in p.texels.iter().enumerate() {
                if t == 0 {
                    continue;
                }
                let (dx, dy) = (p.x + (i % p.w) as i32, p.y + (i / p.w) as i32);
                let sx = if mirror { ox - dx - 1 } else { ox + dx };
                let sy = oy + dy;
                if sx < self.half.0 || sx >= self.half.1 || !(0..SCREEN_H).contains(&sy) {
                    continue;
                }
                let (bx, by) = ((sx - self.gx) as usize, (sy - self.gy) as usize);
                if bx < w && by < self.rows * 8 {
                    self.buf[by * w + bx] = t | (plane + p.half + 1) << 4;
                }
            }
        }
    }

    fn tile(&self, cx: usize, cy: usize, plane: u8) -> Option<[u8; 32]> {
        if cx >= self.cols || cy >= self.rows {
            return None;
        }
        let w = self.cols * 8;
        let mut out = [0u8; 32];
        let mut any = false;
        for y in 0..8 {
            for x in 0..8 {
                let v = self.buf[(cy * 8 + y) * w + cx * 8 + x];
                if v != 0 && (v >> 4) == plane + 1 {
                    out[y * 4 + x / 2] |= (v & 15) << (4 * (x & 1));
                    any = true;
                }
            }
        }
        any.then_some(out)
    }

    /// Square sprites of 8, 4, 2 or 1 cells covering a plane: a block
    /// becomes one sprite when enough of its cells are used (`need` per
    /// size), else splits in four.
    fn cover(&self, plane: u8, need: &[usize; 4]) -> Vec<(usize, usize, usize)> {
        let used: Vec<bool> =
            (0..self.rows * self.cols).map(|k| self.tile(k % self.cols, k / self.cols, plane).is_some()).collect();
        let mut out = Vec::new();
        let mut stack = Vec::new();
        for by in (0..self.rows).step_by(8) {
            for bx in (0..self.cols).step_by(8) {
                stack.push((bx, by, 8));
            }
        }
        while let Some((x0, y0, n)) = stack.pop() {
            let count = (y0..(y0 + n).min(self.rows))
                .flat_map(|y| (x0..(x0 + n).min(self.cols)).map(move |x| (x, y)))
                .filter(|&(x, y)| used[y * self.cols + x])
                .count();
            if count == 0 {
                continue;
            }
            if n == 1 || count >= need[n.trailing_zeros() as usize] {
                out.push((x0, y0, n));
            } else {
                let m = n / 2;
                for (dx, dy) in [(0, 0), (m, 0), (0, m), (m, m)] {
                    stack.push((x0 + dx, y0 + dy, m));
                }
            }
        }
        out
    }

    fn sprite(&self, plane: u8, (x0, y0, n): (usize, usize, usize)) -> Sprite {
        let mut tiles = Vec::with_capacity(n * n);
        for y in 0..n {
            for x in 0..n {
                tiles.push(self.tile(x0 + x, y0 + y, plane).unwrap_or([0u8; 32]));
            }
        }
        Sprite { x: self.gx + 8 * x0 as i32, y: self.gy + 8 * y0 as i32, size: 8 * n, tiles }
    }
}

/// A sprite: position, size (8 .. 64), and its tiles.
struct Sprite {
    x: i32,
    y: i32,
    size: usize,
    tiles: Vec<[u8; 32]>,
}

/// Covers from loosest (fewest sprites) to tightest (fewest tiles).
const COVERS: [[usize; 4]; 4] = [[1, 2, 6, 24], [1, 2, 9, 40], [1, 3, 13, 56], [1, 4, 16, 65]];

/// A side's tiles: its figure tiles, then its effect tiles.
struct Tiles {
    ranges: [(u32, u32); 2],
}

impl Tiles {
    fn left(&self) -> u32 {
        self.ranges.iter().map(|r| r.1 - r.0).sum()
    }

    /// `n` contiguous tiles, if there are.
    fn take(&mut self, n: u32) -> Option<u32> {
        for r in self.ranges.iter_mut() {
            if r.1 - r.0 >= n {
                r.0 += n;
                return Some(r.0 - n);
            }
        }
        None
    }
}

// --- Effects ------------------------------------------------------------------------

fn fx_slot(side: u32, k: u32) -> u32 {
    FX + SIDE_FX * side + FX_SIZE * k
}

fn s16(core: &Core, at: u32) -> i32 {
    core.raw_read_16(at, -1) as i16 as i32
}

/// Effect `kind` of the side's figure: its own, or (struck) the enemy's hit
/// on it; a secondary weapon's hit is its record's own.
fn fx_of(core: &Core, side: u32, fig: &'static Figure, kind: u8) -> Option<&'static Fx> {
    let state = STATE + SIDE_STATE * side;
    let spec = UNITS.get((core.raw_read_8(state + S_UNIT, -1) as usize).checked_sub(1)?)?;
    let enemy = (core.raw_read_8(state + S_ENEMY, -1) as u32).checked_sub(1);
    match kind {
        FX_STRUCK => hit_on(enemy?, core.raw_read_8(state + S_ENEMY_WEAPON, -1), spec.air.then_some(spec.record)),
        FX_HIT => {
            let air = enemy.filter(|_| core.raw_read_8(state + S_ENEMY_AIR, -1) != 0);
            hit_on(spec.record, core.raw_read_8(state + S_WEAPON, -1).max(1), air).or(fig.fx[kind as usize].as_ref())
        }
        _ => fig.fx.get(kind as usize)?.as_ref(),
    }
}

/// One of the side's volley's shots has landed (a hit shows, or a
/// projectile left the screen).
fn landed(core: &mut Core, side: u32) {
    let at = STATE + SIDE_STATE * side + S_LANDED;
    core.raw_write_8(at, -1, core.raw_read_8(at, -1).saturating_add(1));
}

/// Start effect `kind` of the side's figure at `(x, y)`, after `delay`
/// frames, flying at `(dx, dy)` for `launch` frames and then at `target`.
#[allow(clippy::too_many_arguments)]
fn spawn(core: &mut Core, side: u32, fig: &'static Figure, kind: u8, (x, y): (i32, i32), d: (i32, i32, u8), delay: u8, target: (i32, i32)) {
    let Some(fx) = fx_of(core, side, fig, kind) else { return };
    // A hit or projectile takes a muzzle flash's slot if none is free.
    let free = |core: &Core, muzzle: bool| {
        (0..FX_SLOTS).find(|&k| match core.raw_read_8(fx_slot(side, k), -1) {
            0 => true,
            n => muzzle && n <= FX_MUZZLE + 2,
        })
    };
    let Some(k) = free(core, false).or_else(|| free(core, kind > FX_MUZZLE + 1 && kind != FX_BLAST)) else { return };
    let at = fx_slot(side, k);
    let mut b = [0u8; FX_SIZE as usize];
    b[0] = kind + 1;
    b[4..6].copy_from_slice(&(x as i16).to_le_bytes());
    b[6..8].copy_from_slice(&(y as i16).to_le_bytes());
    b[8] = d.0.clamp(-127, 127) as i8 as u8;
    b[9] = d.1.clamp(-127, 127) as i8 as u8;
    b[0xA] = d.2;
    b[0xB] = delay;
    b[0xC..0xE].copy_from_slice(&(target.0 as i16).to_le_bytes());
    b[0xE..0x10].copy_from_slice(&(target.1 as i16).to_le_bytes());
    core.raw_write_range(at, -1, &b);
    core.raw_write_8(at + 1, -1, 0);
    let scripts = [fx.script.clone()];
    if delay == 0 {
        settle(core, at, &scripts, fx.first(kind), 0);
        // Kind + 1 again: settle wrote the script index (0) over it.
        if kind == FX_HIT {
            landed(core, side);
        }
    }
    core.raw_write_8(at, -1, kind + 1);
    if delay != 0 {
        core.raw_write_8(at + 2, -1, 1);
        core.raw_write_8(at + 3, -1, 0);
    }
}

/// Where the side's shots go: the enemy's standing figures, in turn.
fn target(core: &Core, side: u32, n: usize) -> (i32, i32) {
    let enemy = 1 - side;
    let ours = core.raw_read_8(STATE + SIDE_STATE * enemy + S_UNIT, -1) != 0;
    let mut at = Vec::new();
    for f in 0..5 {
        let rec = FIGURE_RECORDS + SIDE_FIGURES * enemy + FIGURE_RECORD * f;
        if core.raw_read_8(rec, -1) != 0 {
            let (x, y) = (s16(core, rec + 8), s16(core, rec + 0xA));
            at.push(if ours { (x, y) } else { (x + if enemy == 0 { 8 } else { -8 }, y - 14) });
        }
    }
    if at.is_empty() {
        return (HALVES[enemy as usize].0 + 60, 100);
    }
    at[n % at.len()]
}

/// The side's unit fires one shot of its volley (the `n`-th) from its
/// origin `o`: a muzzle flash at the next point, and a projectile or, for a
/// direct shot, a hit on the target.
fn shoot(core: &mut Core, side: u32, fig: &'static Figure, spec: &UnitSpec, weapon: u8, n: usize, o: (i32, i32), delay: u8) {
    let w = (weapon.max(1) - 1).min(1) as usize;
    let pts = spec.shots.points[w].clone();
    let p = if pts.is_empty() { (0, 0) } else { fig.points[pts.start + n % pts.len()] };
    // Mirrored on the left: the effect's own pixels mirror about this point.
    let at = if side == 0 { (o.0 - p.0, o.1 + p.1) } else { (o.0 + p.0, o.1 + p.1) };
    let t = target(core, side, n);
    let muzzle = fig.fx[w].as_ref().map(|m| (FX_MUZZLE + w as u8, m.fires));
    if let Some((kind, _)) = muzzle {
        spawn(core, side, fig, kind, at, (0, 0, 0), delay, t);
    }
    if muzzle.is_some_and(|(_, fires)| fires) {
        return; // the flash's own script fires
    }
    fly(core, side, fig, spec, at, delay, t);
}

/// A projectile from `at` to `t`, or a direct hit on `t`.
fn fly(core: &mut Core, side: u32, fig: &'static Figure, spec: &UnitSpec, at: (i32, i32), delay: u8, t: (i32, i32)) {
    let dir = if side == 0 { 1 } else { -1 };
    let air = core.raw_read_8(STATE + SIDE_STATE * side + S_ENEMY_AIR, -1) != 0;
    let kind = if air && fig.fx[FX_AIR as usize].is_some() { FX_AIR } else { FX_PROJECTILE };
    if fig.fx[kind as usize].is_some() && spec.shots.speed > 0 {
        let (lx, ly, lf) = spec.shots.launch;
        let d = if lf > 0 { (dir * lx, ly, lf) } else { aim(at, t, dir * spec.shots.speed) };
        spawn(core, side, fig, kind, at, d, delay, t);
    } else {
        spawn(core, side, fig, FX_HIT, t, (0, 0, 0), delay + HIT_DELAY, t);
    }
}

/// Velocity from `at` to `t` at horizontal speed `vx`.
fn aim(at: (i32, i32), t: (i32, i32), vx: i32) -> (i32, i32, u8) {
    let frames = ((t.0 - at.0) / vx.max(1).max(-vx)).abs().max(1);
    (vx, ((t.1 - at.1) / frames).clamp(-12, 12), 0)
}

/// One frame of the side's effects.
fn step_fx(core: &mut Core, side: u32, fig: &'static Figure, spec: &UnitSpec) {
    let dir = if side == 0 { 1 } else { -1 };
    for k in 0..FX_SLOTS {
        let at = fx_slot(side, k);
        let kind = core.raw_read_8(at, -1);
        if kind == 0 {
            continue;
        }
        let kind = kind - 1;
        let Some(fx) = fx_of(core, side, fig, kind) else {
            core.raw_write_8(at, -1, 0);
            continue;
        };
        let scripts = [fx.script.clone()];
        let delay = core.raw_read_8(at + 0xB, -1);
        if delay > 0 {
            core.raw_write_8(at + 0xB, -1, delay - 1);
            if delay == 1 {
                core.raw_write_8(at, -1, 0);
                let fired = settle(core, at, &scripts, fx.first(kind), FIRE_EVENT);
                core.raw_write_8(at, -1, kind + 1);
                if kind == FX_HIT {
                    landed(core, side);
                }
                emit(core, side, fig, spec, at, kind, fired);
            }
            continue;
        }
        let (x, y) = (s16(core, at + 4), s16(core, at + 6));
        let t = (s16(core, at + 0xC), s16(core, at + 0xE));
        if is_projectile(kind) {
            let (mut dx, mut dy) = (core.raw_read_8(at + 8, -1) as i8 as i32, core.raw_read_8(at + 9, -1) as i8 as i32);
            let launch = core.raw_read_8(at + 0xA, -1);
            if launch > 0 {
                core.raw_write_8(at + 0xA, -1, launch - 1);
                if launch == 1 {
                    let d = aim((x, y), t, dir * spec.shots.speed);
                    (dx, dy) = (d.0, d.1);
                    core.raw_write_8(at + 8, -1, dx as i8 as u8);
                    core.raw_write_8(at + 9, -1, dy as i8 as u8);
                }
            }
            let (nx, ny) = (x + dx, y + dy);
            core.raw_write_16(at + 4, -1, nx as u16);
            core.raw_write_16(at + 6, -1, ny as u16);
            if launch == 0 && (nx - t.0) * dir >= 0 {
                core.raw_write_8(at, -1, 0);
                spawn(core, side, fig, FX_HIT, t, (0, 0, 0), 0, t);
                continue;
            }
            if !(-64..304).contains(&nx) || !(-64..224).contains(&ny) {
                core.raw_write_8(at, -1, 0);
                landed(core, side);
                continue;
            }
        }
        // The script (index 0) shares +0 with the kind: swap it in and out.
        core.raw_write_8(at, -1, 0);
        let fired = step_script(core, at, &scripts, FIRE_EVENT);
        let now = core.raw_read_8(at, -1);
        let held = core.raw_read_8(at + 2, -1) == 0;
        if (now == GONE || held) && !is_projectile(kind) {
            core.raw_write_8(at, -1, 0);
        } else {
            core.raw_write_8(at, -1, kind + 1);
        }
        emit(core, side, fig, spec, at, kind, fired);
    }
}

/// A muzzle flash whose script fires launches its projectile.
fn emit(core: &mut Core, side: u32, fig: &'static Figure, spec: &UnitSpec, at: u32, kind: u8, fired: u8) {
    if fired == 0 || kind > 1 {
        return;
    }
    let (x, y) = (s16(core, at + 4), s16(core, at + 6));
    let t = (s16(core, at + 0xC), s16(core, at + 0xE));
    fly(core, side, fig, spec, (x, y), 0, t);
}

// --- Each frame -----------------------------------------------------------------

/// Where the figure's origin is on screen (from its record's position).
fn origin_of(core: &Core, side: u32, fig: &Figure) -> (i32, i32) {
    let rec = FIGURE_RECORDS + SIDE_FIGURES * side + FIGURE_RECORD * core.raw_read_8(STATE + SIDE_STATE * side + S_FIGURE, -1) as u32;
    let (b0, b1, b2, b3) = fig.bounds;
    let (cx, cy) = ((b0 + b2) / 2, (b1 + b3) / 2);
    let (px, py) = (s16(core, rec + 8), s16(core, rec + 0xA));
    (if side == 0 { px + cx + 1 } else { px - cx }, py - cy)
}

/// The figure's idle picture on screen: x0, y0, x1, y1.
fn screen_box(side: u32, fig: &Figure, o: (i32, i32)) -> (i32, i32, i32, i32) {
    let (b0, b1, b2, b3) = fig.bounds;
    let (x0, x1) = if side == 0 { (o.0 - b2, o.0 - b0) } else { (o.0 + b0, o.0 + b2) };
    (x0, o.1 + b1, x1, o.1 + b3)
}

/// A sprite's width and height (shape, size).
fn sprite_size(a0: u16, a1: u16) -> (i32, i32) {
    const SIZES: [[(i32, i32); 4]; 3] = [
        [(8, 8), (16, 16), (32, 32), (64, 64)],
        [(16, 8), (32, 8), (32, 16), (64, 32)],
        [(8, 16), (8, 32), (16, 32), (32, 64)],
    ];
    SIZES[((a0 >> 14) as usize).min(2)][(a1 >> 14) as usize]
}

/// The hits a volley makes: the firing script's shot events, or the burst.
fn volley_hits(fig: &Figure, spec: &UnitSpec, weapon: u8) -> u8 {
    let events = fig
        .layers
        .iter()
        .filter(|l| l.weapon == 0 || l.weapon == weapon.max(1))
        .map(|l| l.scripts[1].iter().filter(|&&(d, f)| f == EVENT && d == spec.shots.event && d != 0).count())
        .max()
        .unwrap_or(0);
    if events > 0 {
        events as u8
    } else {
        spec.shots.burst.0
    }
}

/// Trap at [`HP_DRAIN`], before AW2 drains the HP counters: while a side's
/// volley runs, the enemy's counter does not run ahead of its hits. The
/// volley is timed to end as the donor's shot lands and starts the drain
/// ([`UnitSpec::wait`]); should the drain start first, it holds at the HP
/// the hits landed so far take it to (the whole damage once all have
/// landed), each hold landing exactly on its HP as AW2's own steps do. The
/// drain's pace, the figures' fall and the damage stay AW2's.
pub fn hp_drain(core: &mut Core) {
    if !is_on(core) || !scene_running(core) {
        return;
    }
    for side in 0..2u32 {
        let state = STATE + SIDE_STATE * side;
        let n = core.raw_read_8(state + S_HITS, -1) as u32;
        if n == 0 {
            continue;
        }
        let enemy = 1 - side;
        let landed = (core.raw_read_8(state + S_LANDED, -1) as u32).min(n);
        let done = landed >= n || core.raw_read_8(state + S_TIME, -1) >= VOLLEY_FRAMES;
        let (hp0, after) = (core.raw_read_16(state + S_HP0, -1) as u32, core.raw_read_16(HP_AFTER + 2 * enemy, -1) as u32);
        if core.raw_read_16(DRAINING + 2 * enemy, -1) == 0 || core.raw_read_16(HP_SHOWN + 2 * enemy, -1) as u32 == after {
            if done {
                core.raw_write_8(state + S_HITS, -1, 0);
            }
            continue;
        }
        let goal = if done || after >= hp0 { after } else { hp0 - (hp0 - after) * landed / n };
        let (value, rate) = (core.raw_read_32(DRAIN_VALUE + 4 * enemy, -1), core.raw_read_32(DRAIN_RATE + 4 * enemy, -1));
        if value.saturating_sub(rate) < goal << 16 {
            core.raw_write_32(DRAIN_VALUE + 4 * enemy, -1, (goal << 16) + rate);
        }
    }
}

/// Trap at [`OAM_COPY`], the scene's VBlank copy of the whole shadow OAM
/// ([`SHADOW`]): the donor's figure and effect sprites are dropped and the
/// unit's and its effects' go in.
fn flush(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    // (Each battle's state is set up afresh by [`figure_count`].)
    if !scene_running(core) {
        return;
    }
    let mut ours = [None, None];
    for side in 0..2u32 {
        let unit = core.raw_read_8(STATE + SIDE_STATE * side + S_UNIT, -1);
        if unit != 0 {
            ours[side as usize] = Some(unit as usize - 1);
        }
    }
    if ours.iter().all(|o| o.is_none()) {
        return;
    }
    // Where the units stand, for the enemy's hits on them.
    let mut boxes = [None, None];
    for side in 0..2u32 {
        let Some(unit) = ours[side as usize] else { continue };
        let army = core.raw_read_8(STATE + SIDE_STATE * side + S_ARMY, -1) as usize;
        if let Some(fig) = figure(unit, army) {
            boxes[side as usize] = Some(screen_box(side, fig, origin_of(core, side, fig)));
        }
    }
    // Drop the donors' figure sprites (noting their priority and flash),
    // their effects, and an AW2 enemy's hits on the units (noting where);
    // keep the rest in order.
    let mut impacts = [(0i32, 0i32, 0i32); 2];
    let mut prio = [FIGURE_PRIORITY; 2];
    let mut flash = [false; 2];
    let mut kept: Vec<[u16; 3]> = Vec::new();
    for k in 0..SHADOW_ENTRIES {
        let e = SHADOW + 8 * k;
        let attrs = [core.raw_read_16(e, -1), core.raw_read_16(e + 2, -1), core.raw_read_16(e + 4, -1)];
        let (a0, a1, a2) = (attrs[0], attrs[1], attrs[2]);
        let affine = a0 & 0x100 != 0;
        let shown = a0 & 0x300 != 0x200 && (affine || (a0 & 0xFF) < SCREEN_H as u16);
        if !shown {
            continue;
        }
        let tile = a2 & 0x3FF;
        let side = (tile as u32 / SIDE_TILES) as usize;
        if side < 2 && ours[side].is_some() {
            prio[side] = (a2 >> 10) & 3;
            flash[side] |= (a2 >> 12) as u32 >= 4 * side as u32 + 2;
            continue;
        }
        if tile >= EFFECT_TILES {
            let owner = if affine {
                let big = a1 >> 14 >= 2 || a0 >> 14 != 0 && a1 >> 14 >= 1;
                let x = (a1 & 0x1FF) as i32 - if a1 & 0x100 != 0 { 512 } else { 0 };
                big.then_some(if x + 16 < 120 { 0 } else { 1 })
            } else {
                Some(if a1 & HFLIP != 0 { 0 } else { 1 })
            };
            if owner.is_some_and(|o| ours[o].is_some()) {
                continue;
            }
            if let (false, Some(o)) = (affine, owner) {
                let (w, h) = sprite_size(a0, a1);
                let x = (a1 & 0x1FF) as i32 - if a1 & 0x100 != 0 { 512 } else { 0 };
                let y = (a0 & 0xFF) as i32 - if (a0 & 0xFF) as i32 >= SCREEN_H { 256 } else { 0 };
                let (mx, my) = (x + w / 2, y + h / 2);
                if let Some((x0, y0, x1, y1)) = boxes[1 - o] {
                    if (x0 - IMPACT_MARGIN..x1 + IMPACT_MARGIN).contains(&mx) && (y0 - IMPACT_MARGIN..y1 + IMPACT_MARGIN).contains(&my) {
                        let i = &mut impacts[1 - o];
                        *i = (i.0 + mx, i.1 + my, i.2 + 1);
                        continue;
                    }
                }
            }
        }
        kept.push(attrs);
    }
    let mut mine: Vec<[u16; 3]> = Vec::new();
    let mut fx_mine: Vec<[u16; 3]> = Vec::new();
    for side in 0..2u32 {
        let Some(unit) = ours[side as usize] else { continue };
        let state = STATE + SIDE_STATE * side;
        let army = core.raw_read_8(state + S_ARMY, -1) as usize;
        let Some(fig) = figure(unit, army) else { continue };
        let spec = &UNITS[unit];
        let weapon = core.raw_read_8(state + S_WEAPON, -1);
        let rec = FIGURE_RECORDS + SIDE_FIGURES * side + FIGURE_RECORD * core.raw_read_8(state + S_FIGURE, -1) as u32;
        // Where the unit stands: its centre, mirrored for the left half.
        let (b0, b1, b2, b3) = fig.bounds;
        let (cx, cy) = ((b0 + b2) / 2, (b1 + b3) / 2);
        let middle = if side == 0 { 60 - spec.outwards } else { 180 + spec.outwards };
        let centre_y = if spec.air { SKY } else { GROUND - (b3 - cy) };
        let (hx, hy) = (s16(core, rec + 4), s16(core, rec + 6));
        let placed = hx != 0 || hy != 0;
        if placed && (hx, hy) != (middle, centre_y) {
            let (dx, dy) = (middle - hx, centre_y - hy);
            for (o, d) in [(4, dx), (6, dy), (8, dx), (0xA, dy)] {
                let v = s16(core, rec + o) + d;
                core.raw_write_16(rec + o, -1, v as u16);
            }
        }
        let (px, py) = (s16(core, rec + 8), s16(core, rec + 0xA));
        let mirror = side == 0;
        let origin = (if mirror { px + cx + 1 } else { px - cx }, py - cy);
        // Shots fired, and the figure's end.
        let shots = core.raw_read_16(SHOTS + SIDE_SHOTS * side, -1);
        let seen = core.raw_read_16(state + S_SHOTS, -1);
        let dead = core.raw_read_8(state + S_DEAD, -1) != 0;
        let standing = core.raw_read_8(rec, -1) != 0;
        let mut volley = core.raw_read_8(state + S_VOLLEY, -1);
        let firing_layers = fig.layers.iter().any(|l| !l.scripts[1].is_empty());
        // One volley a battle, as in Dual Strike (a donor may count more
        // shots), after its wait from the donor's first shot.
        let w = (weapon.max(1) - 1).min(1) as usize;
        if !dead && shots > seen && core.raw_read_8(state + S_FIRED, -1) == 0 {
            core.raw_write_8(state + S_FIRED, -1, 1);
            core.raw_write_8(state + S_WAIT, -1, spec.wait[w] + 1);
        }
        let wait = core.raw_read_8(state + S_WAIT, -1);
        if wait > 0 {
            core.raw_write_8(state + S_WAIT, -1, wait - 1);
        }
        let new_shots = wait == 1 && !dead;
        let mut fired = 0;
        let mut burst = false;
        let mut starts = false;
        for (l, layer) in fig.layers.iter().enumerate() {
            let at = state + S_LAYERS + 4 * l as u32;
            if !dead && !standing && placed {
                if layer.scripts[2].is_empty() {
                    core.raw_write_8(at, -1, GONE);
                } else {
                    start(core, at, &layer.scripts, 2, 0, 0);
                }
                continue;
            }
            let shows = layer.weapon == 0 || layer.weapon == weapon.max(1);
            let event = if shows { spec.shots.event } else { 0 };
            if new_shots && shows && !layer.scripts[1].is_empty() && core.raw_read_8(at, -1) != 1 {
                volley = 0;
                starts = true;
                // From its first shot: AW2's shell is leaving now.
                let first = layer.scripts[1].iter().position(|&(d, f)| f == EVENT && d == event).unwrap_or(0);
                fired += start(core, at, &layer.scripts, 1, first, event);
                continue;
            }
            fired += step_script(core, at, &layer.scripts, event);
        }
        if new_shots && !firing_layers && spec.shots.burst.0 > 0 {
            burst = true;
            starts = true;
        }
        core.raw_write_16(state + S_SHOTS, -1, shots);
        if starts && placed {
            let hp = core.raw_read_16(HP_SHOWN + 2 * (1 - side), -1);
            core.raw_write_8(state + S_HITS, -1, volley_hits(fig, spec, weapon));
            core.raw_write_8(state + S_LANDED, -1, 0);
            core.raw_write_8(state + S_TIME, -1, 0);
            core.raw_write_16(state + S_HP0, -1, hp);
        }
        if placed {
            for _ in 0..fired {
                shoot(core, side, fig, spec, weapon, volley as usize, origin, 0);
                volley = volley.wrapping_add(1);
            }
            if burst {
                let (n, gap) = spec.shots.burst;
                for k in 0..n {
                    shoot(core, side, fig, spec, weapon, k as usize, origin, 1 + k * gap);
                }
            }
        }
        core.raw_write_8(state + S_VOLLEY, -1, volley);
        if !standing && placed && !dead {
            core.raw_write_8(state + S_DEAD, -1, 1);
            // Its own blasts, over its body.
            let (w, h) = (b2 - b0, b3 - b1);
            for (k, (fx_, fy)) in [(0, 1), (-1, 0), (1, -1)].iter().enumerate() {
                let at = (px + fx_ * w / 4, py + fy * h / 5 + 12);
                spawn(core, side, fig, FX_BLAST, at, (0, 0, 0), 1 + 6 * k as u8, at);
            }
        }
        step_fx(core, side, fig, spec);
        let time = core.raw_read_8(state + S_TIME, -1);
        core.raw_write_8(state + S_TIME, -1, time.saturating_add(1));
        // An AW2 enemy's hits: Dual Strike's, where AW2's landed (inside the
        // unit); an indirect or area shot's (no figure hits, the HP drain
        // starts) a few over its body.
        let (hits, area) = (core.raw_read_16(FIGURE_HITS + SIDE_HITS * side, -1) as u8, core.raw_read_16(DRAINING + 2 * side, -1) != 0);
        let new = hits.saturating_sub(core.raw_read_8(state + S_STRUCK, -1));
        let area_starts = area && core.raw_read_8(state + S_AREA, -1) == 0;
        core.raw_write_8(state + S_STRUCK, -1, hits);
        core.raw_write_8(state + S_AREA, -1, area as u8);
        if placed && ours[1 - side as usize].is_none() && (new > 0 || area_starts) {
            let (x0, y0, x1, y1) = screen_box(side, fig, origin);
            let (w, h) = (x1 - x0, y1 - y0);
            let (ix, iy, n) = impacts[side as usize];
            let (mx, my) = if n > 0 { (ix / n, iy / n) } else { ((x0 + x1) / 2, (y0 + y1) / 2) };
            let inside = |(x, y): (i32, i32)| (x.clamp(x0 + w / 4, x1 - w / 4), y.clamp(y0 + h / 4, y1 - h / 4));
            if new > 0 {
                spawn(core, side, fig, FX_STRUCK, inside((mx, my)), (0, 0, 0), 0, (0, 0));
            } else {
                for (k, (fx_, fy)) in AREA_HITS.iter().enumerate() {
                    let at = inside((mx + fx_ * w / 4, my + fy * h / 4));
                    spawn(core, side, fig, FX_STRUCK, at, (0, 0, 0), AREA_GAP * k as u8, at);
                }
            }
        }
        if !placed {
            continue;
        }
        // Compose: the unit and its effects in its half, its effects in the
        // other half.
        let frames: Vec<Option<usize>> = fig
            .layers
            .iter()
            .enumerate()
            .map(|(l, layer)| {
                let at = state + S_LAYERS + 4 * l as u32;
                let shown = layer.weapon == 0 || layer.weapon == weapon.max(1);
                (shown && core.raw_read_8(at, -1) != GONE).then(|| core.raw_read_8(at + 3, -1) as usize)
            })
            .collect();
        let mut own = Canvas::new(HALVES[side as usize], origin);
        let mut other = Canvas::new(HALVES[1 - side as usize], (0, 0));
        for (layer, f) in fig.layers.iter().zip(&frames) {
            if let Some(pieces) = f.and_then(|f| layer.frames.get(f)) {
                own.draw(pieces, origin, mirror, 0);
            }
        }
        for k in 0..FX_SLOTS {
            let at = fx_slot(side, k);
            let kind = core.raw_read_8(at, -1);
            if kind == 0 || core.raw_read_8(at + 0xB, -1) != 0 {
                continue;
            }
            let Some(fx) = fx_of(core, side, fig, kind - 1) else { continue };
            let Some(pieces) = fx.frames.get(core.raw_read_8(at + 3, -1) as usize) else { continue };
            let o = (s16(core, at + 4), s16(core, at + 6));
            own.draw(pieces, o, mirror, 2);
            other.draw(pieces, o, mirror, 2);
        }
        put_palette(core, 4 * side, &fig.palettes[0]);
        put_palette(core, 4 * side + 1, &fig.palettes[1]);
        put_palette(core, 4 * side + 2, &fig.palettes[0].map(lighten));
        put_palette(core, 4 * side + 3, &fig.palettes[1].map(lighten));
        if let Some(p) = FX_PALETTES.get_or_init(|| palette_file(FX_PALETTE)) {
            put_palette(core, FX_PAL as u32, &p[0]);
            put_palette(core, FX_PAL as u32 + 1, &p[1]);
        }
        let mut tiles = Tiles {
            ranges: [
                (SIDE_TILES * side, SIDE_TILES * (side + 1)),
                (FX_TILES + SIDE_FX_TILES * side, FX_TILES + SIDE_FX_TILES * (side + 1)),
            ],
        };
        // The unit, then its effects: each the loosest cover that fits the
        // tiles left, biggest sprites first (so they find room).
        let groups: [&[(&Canvas, u8)]; 2] = [&[(&own, 0), (&own, 1)], &[(&own, 2), (&own, 3), (&other, 2), (&other, 3)]];
        for (g, group) in groups.iter().enumerate() {
            // The unit's sprites in its own tiles (the effect tiles are too
            // few for its big ones).
            let room = if g == 0 { tiles.ranges[0].1 - tiles.ranges[0].0 } else { tiles.left() };
            let mut blocks: Vec<(&Canvas, u8, (usize, usize, usize))> = Vec::new();
            for need in &COVERS {
                blocks = group.iter().flat_map(|&(c, p)| c.cover(p, need).into_iter().map(move |b| (c, p, b))).collect();
                if blocks.iter().map(|b| (b.2 .2 * b.2 .2) as u32).sum::<u32>() <= room {
                    break;
                }
            }
            blocks.sort_by_key(|b| std::cmp::Reverse(b.2 .2));
            for (canvas, plane, block) in blocks {
                let s = canvas.sprite(plane, block);
                let n = s.tiles.len() as u32;
                if mine.len() + fx_mine.len() + kept.len() >= SHADOW_ENTRIES as usize {
                    break;
                }
                let Some(tile) = tiles.take(n) else { continue };
                let data: Vec<u8> = s.tiles.iter().flatten().copied().collect();
                core.raw_write_range(OBJ_VRAM + 32 * tile, -1, &data);
                let size = s.size.trailing_zeros() as u16 - 3;
                let (pal, pr) = if plane < 2 {
                    (4 * side as u16 + plane as u16 + if flash[side as usize] { 2 } else { 0 }, prio[side as usize])
                } else {
                    (FX_PAL + plane as u16 - 2, FX_PRIORITY)
                };
                let attrs = [(s.y as u16) & 0xFF, (s.x as u16 & 0x1FF) | size << 14, tile as u16 | pr << 10 | pal << 12];
                if plane < 2 {
                    mine.push(attrs);
                } else {
                    fx_mine.push(attrs);
                }
            }
        }
    }
    // The units first, as AW2's figures come first: a line's sprite time
    // goes to them before the effects, which still draw over them (they have
    // a higher priority). Each entry's fourth halfword (affine parameters)
    // stays where it is.
    for (k, attrs) in mine
        .iter()
        .chain(fx_mine.iter())
        .chain(kept.iter())
        .chain(std::iter::repeat(&[HIDDEN, 0, 0]))
        .take(SHADOW_ENTRIES as usize)
        .enumerate()
    {
        let e = SHADOW + 8 * k as u32;
        for (i, &a) in attrs.iter().enumerate() {
            core.raw_write_16(e + 2 * i as u32, -1, a);
        }
    }
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![
        (ROWS_FILLED, Box::new(rows_filled)),
        (FIGURE_COUNT, Box::new(figure_count)),
        (DRAIN, Box::new(drain)),
        (OAM_COPY, Box::new(flush)),
        (HP_DRAIN, Box::new(hp_drain)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(lighten(0x0421), 0x4210);
        assert_eq!(aim((0, 0), (40, 20), 8), (8, 4, 0));
        let mut t = Tiles { ranges: [(0, 20), (100, 116)] };
        assert_eq!(t.take(16), Some(0));
        assert_eq!(t.take(16), Some(100));
        assert_eq!(t.take(8), None);
        assert_eq!(t.take(4), Some(16));
    }
}
