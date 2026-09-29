//! Dual Strike's battle animations for the seven new units, with the Dual
//! Strike pack (stage 4b of 0.3.0).
//!
//! AW2's battle scene keeps running the fight: backgrounds per terrain and
//! weather, the slide-in, shells, hits, explosions, HP counters and timing
//! all come from a *donor* AW2 unit's scene (Neotank for the Megatank,
//! Rockets or Missiles for the Piperunner, ...; [`donor`]). Only the donor's
//! figures are replaced: on a side showing a new unit, the donor gets one
//! figure (Dual Strike shows these units as one, whatever their HP), and
//! that figure is drawn from Dual Strike's own frames, animated by Dual
//! Strike's scripts, in the army's Dual Strike colours.
//!
//! **Dual Strike's data** (the battle overlay, 2, loaded at `0x02350560`,
//! which draws units with the 3D engine, a textured quad per piece; the
//! sheets and palettes in `battle/`):
//! - Unit records at `0x0236B1BC`, 0x50 bytes, by Dual Strike id - 1:
//!   words 1..5 are sprite sets (body, alternative body, primary weapon,
//!   secondary weapon, extra: ships' wakes), word 6 the palette table (five
//!   pointers to palette file names, one per army).
//! - A sprite set: five sheet names (`char[4]`, one per army), five
//!   animation pointers, a word. An animation is `[frame table, script 0,
//!   script 1, ...]`.
//! - A frame: `u16 count`, then 6-byte pieces: `s8 x`, width code << 4,
//!   `s8 y`, height code << 4 (codes 0..3: 8, 16, 32, 64 px), `u16`: the
//!   texels' offset in the sheet in 32-byte units (low 12 bits) and the
//!   palette half (top 4). A piece's texels are 4bpp rows, low nibble
//!   first (the sheets are 3D textures). A frame pointer past the
//!   overlay's end is an empty frame (the blink of a destruction).
//! - A script: `(duration, frame)` byte pairs; frame `0xFC` is an event
//!   (`0x20`: the weapon fires), `(0, 0xFD)` loops, `(0, 0xFE)` holds,
//!   `(0, 0xFF)` ends (the object disappears).
//! - Palettes: 64 bytes, two 16-colour halves; colour 0 transparent.
//!
//! **Drawing.** Every frame of the scene, just before its VBlank copy of
//! the shadow OAM ([`OAM_COPY`], [`flush`]): the donor's figure sprites on
//! that side are dropped; the unit's layers (body, weapon, wake) are
//! composed at their current frames, mirrored on the left (Dual Strike's
//! art faces left), cropped to the side's half of the screen (the scene's
//! two windows both show sprites), cut into 8x8 tiles and covered with
//! square sprites of 64, 32, 16 and 8 px, one set per palette half. The
//! tiles go in the side's 256 figure tiles (the donor's own copies there
//! are dropped from the copy queue, [`drain`]); the palettes in the side's
//! four OBJ palettes (the last two lightened, as AW2 does for a figure's
//! flash).
//!
//! **Moving and acting.** The donor figure's record (`0x02029A10`, per
//! side 0xB4, five figures of 0x24: `+0` state, `+4/+6` home, `+8/+A`
//! position) is moved to the unit's own place, so the enemy's shells and
//! AW2's explosions land on it; the slide-in and hit shakes still move it.
//! Each shot the side fires (`0x020296B0 + 0x28 * side + 0x18` counts them)
//! plays one recoil of the weapon layer's firing script (from one fire
//! event to the next); when the figure is destroyed its layers play their
//! destruction scripts (a blink) while AW2's explosion goes off.
//!
//! Everything is worked out from emulated memory, and the animation state
//! lives in EWRAM ([`STATE`]), so rollback and both netplay peers agree.
//! With the pack off nothing here writes anything.

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
    LayerSpec {
        set,
        weapon,
        fire,
        dead,
    }
}

/// A new unit: its AW2 id, Dual Strike record, layers, whether it flies,
/// and how far past the middle of its half its centre sits (outwards, px).
struct UnitSpec {
    id: u8,
    record: u32,
    layers: &'static [LayerSpec],
    air: bool,
    outwards: i32,
}

const UNITS: [UnitSpec; 7] = [
    UnitSpec {
        id: MEGATANK,
        record: 3,
        layers: &[
            layer(1, 0, None, Some(1)),
            layer(3, 1, Some(1), Some(2)),
            layer(4, 2, Some(1), Some(2)),
        ],
        air: false,
        outwards: 0,
    },
    UnitSpec {
        id: PIPERUNNER,
        record: 8,
        layers: &[layer(1, 0, None, Some(1)), layer(3, 0, Some(1), Some(2))],
        air: false,
        outwards: 0,
    },
    UnitSpec {
        id: STEALTH,
        record: 11,
        layers: &[layer(1, 0, None, None)],
        air: true,
        outwards: 0,
    },
    UnitSpec {
        id: BLACK_BOMB,
        record: 12,
        layers: &[layer(1, 0, None, None)],
        air: true,
        outwards: 0,
    },
    UnitSpec {
        id: BLACK_BOAT,
        record: 17,
        layers: &[layer(1, 0, None, None), layer(5, 0, None, None)],
        air: false,
        outwards: 8,
    },
    UnitSpec {
        id: CARRIER,
        record: 24,
        layers: &[layer(1, 0, None, None), layer(5, 0, None, None)],
        air: false,
        outwards: -24,
    },
    UnitSpec {
        id: OOZIUM,
        record: 25,
        layers: &[layer(1, 0, None, Some(1))],
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
    h: usize,
    half: u8,
    texels: Vec<u8>,
}

struct Layer {
    weapon: u8,
    frames: Vec<Vec<Piece>>,
    /// Idle, fire, destruction.
    scripts: [Vec<(u8, u8)>; 3],
}

/// A unit converted for one army.
struct Figure {
    layers: Vec<Layer>,
    palettes: [[u16; 16]; 2],
    /// The idle picture's bounds, from the origin: x0, y0, x1, y1.
    bounds: (i32, i32, i32, i32),
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
        let (w, h) = (SIZES[(p[1] >> 4) as usize & 3], SIZES[(p[3] >> 4) as usize & 3]);
        let off = u16::from_le_bytes([p[4], p[5]]);
        let at = 32 * (off & 0xFFF) as usize;
        let raw = sheet.get(at..at + w * h / 2)?;
        let texels = (0..w * h)
            .map(|i| if i & 1 == 0 { raw[i / 2] & 15 } else { raw[i / 2] >> 4 })
            .collect();
        out.push(Piece {
            x: p[0] as i8 as i32,
            y: p[2] as i8 as i32,
            w,
            h,
            half: (off >> 12) as u8 & 1,
            texels,
        });
    }
    Some(out)
}

fn palette(addr: u32) -> Option<[[u16; 16]; 2]> {
    let f = crate::ds_pack::pack()?.file(&format!("battle/{}", name(addr)?))?;
    let mut out = [[0u16; 16]; 2];
    for (h, half) in out.iter_mut().enumerate() {
        for (i, c) in half.iter_mut().enumerate() {
            let o = 32 * h + 2 * i;
            *c = u16::from_le_bytes([*f.get(o)?, *f.get(o + 1)?]);
        }
    }
    Some(out)
}

fn convert(u: &UnitSpec, army: usize) -> Option<Figure> {
    let pack = crate::ds_pack::pack()?;
    let rec = RECORDS + RECORD * u.record;
    let palettes = palette(word(word(rec + 4 * PALETTE_TABLE)? + 4 * army as u32)?)?;
    let mut layers = Vec::new();
    let mut bounds = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for l in u.layers {
        let set = word(rec + 4 * l.set)?;
        let file = pack.file(&format!("battle/{}", name(set + 4 * army as u32)?))?;
        let sheet = crate::ds_art::lz10(file).unwrap_or_else(|| file.to_vec());
        let anim = word(set + 20 + 4 * army as u32)?;
        let table = word(anim)?;
        let script_at = |k: u8| word(anim + 4 * (k as u32 + 1)).and_then(script);
        let scripts = [
            script_at(0)?,
            l.fire.map_or(Some(Vec::new()), script_at)?,
            l.dead.map_or(Some(Vec::new()), script_at)?,
        ];
        let count = scripts
            .iter()
            .flatten()
            .filter(|&&(_, f)| f < EVENT)
            .map(|&(_, f)| f as u32 + 1)
            .max()?;
        let frames = (0..count)
            .map(|f| frame(word(table + 4 * f)?, &sheet))
            .collect::<Option<Vec<_>>>()?;
        let idle = scripts[0].first().map_or(0, |&(_, f)| f as usize);
        for p in frames.get(idle).into_iter().flatten() {
            bounds.0 = bounds.0.min(p.x);
            bounds.1 = bounds.1.min(p.y);
            bounds.2 = bounds.2.max(p.x + p.w as i32);
            bounds.3 = bounds.3.max(p.y + p.h as i32);
        }
        layers.push(Layer {
            weapon: l.weapon,
            frames,
            scripts,
        });
    }
    // The drawn pixels' bounds, not the pieces' (pieces carry margins).
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
    if b.0 < b.2 {
        bounds = b;
    }
    Some(Figure {
        layers,
        palettes,
        bounds,
    })
}

static FIGURES: OnceLock<Vec<Option<Figure>>> = OnceLock::new();

fn figure(unit: usize, army: usize) -> Option<&'static Figure> {
    let all = FIGURES.get_or_init(|| {
        UNITS
            .iter()
            .flat_map(|u| (0..ARMIES).map(move |a| convert(u, a)))
            .collect()
    });
    all.get(unit * ARMIES + army.min(ARMIES - 1))?.as_ref()
}

// --- AW2's scene ----------------------------------------------------------------

/// The battle's two sides (0 attacker, on the left): 16-byte rows of u16s,
/// `[0]` colour, `[1]` scene id, `[2]` weapon (1 primary, 2 secondary).
const ROWS: u32 = 0x0300_4580;
const ROW: u32 = 0x10;
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

/// `sub_08041978`, just before it chooses the scene or the map-only proc:
/// both rows are filled.
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

const OBJ_VRAM: u32 = 0x0601_0000;
const SIDE_TILES: u32 = 256;
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
/// +4 shots seen (u16), +6 destroyed, +8 per layer 4 bytes: script, step,
/// time left, frame (script 3: gone).
const S_UNIT: u32 = 0;
const S_ARMY: u32 = 1;
const S_WEAPON: u32 = 2;
const S_FIGURE: u32 = 3;
const S_SHOTS: u32 = 4;
const S_DEAD: u32 = 6;
const S_LAYERS: u32 = 8;
const GONE: u8 = 3;

/// Whether the scene is on screen: its proc runs and the battle map's main
/// callback is not back yet (the proc outlives the scene by a few frames).
fn scene_running(core: &Core) -> bool {
    core.raw_read_32(MAIN_CALLBACK, -1) != MAP_CALLBACK
        && (PROCS..PROCS_END)
            .step_by(PROC_SIZE as usize)
            .any(|p| core.raw_read_32(p, -1) == SCENE_PROC)
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

/// Trap at [`ROWS_FILLED`]: a new unit's side plays its donor's scene.
fn rows_filled(core: &mut Core) {
    if !is_on(core) {
        return;
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
    core.raw_write_8(state + S_UNIT, -1, unit as u8 + 1);
    core.raw_write_8(state + S_ARMY, -1, army as u8);
    core.raw_write_8(state + S_WEAPON, -1, weapon);
    core.raw_write_8(state + S_FIGURE, -1, standing as u8);
    for (l, layer) in fig.layers.iter().enumerate() {
        start(core, state + S_LAYERS + 4 * l as u32, layer, 0, 0);
    }
}

/// Start script `which` (0 idle, 1 fire, 2 destruction) at `step` for the
/// layer state at `at`.
fn start(core: &mut Core, at: u32, layer: &Layer, which: u8, step: usize) {
    core.raw_write_8(at, -1, which);
    settle(core, at, layer, step);
}

/// Run a layer's script from `step` to its next frame entry.
fn settle(core: &mut Core, at: u32, layer: &Layer, mut step: usize) {
    let which = core.raw_read_8(at, -1);
    let script = &layer.scripts[which as usize % 3];
    for _ in 0..script.len() + 2 {
        let Some(&(d, f)) = script.get(step) else { break };
        match f {
            EVENT if d == FIRE_EVENT && which == 1 && step > 0 => {
                // One recoil played: back to idle.
                start(core, at, layer, 0, 0);
                return;
            }
            EVENT => step += 1,
            LOOP if d == 0 => step = 0,
            HOLD if d == 0 => {
                core.raw_write_8(at + 2, -1, 0);
                return;
            }
            END if d == 0 => {
                core.raw_write_8(at, -1, GONE);
                return;
            }
            _ => {
                core.raw_write_8(at + 1, -1, step as u8);
                core.raw_write_8(at + 2, -1, d.max(1));
                core.raw_write_8(at + 3, -1, f);
                return;
            }
        }
    }
    core.raw_write_8(at + 2, -1, 0);
}

/// One frame of a layer's script.
fn step_layer(core: &mut Core, at: u32, layer: &Layer) {
    let which = core.raw_read_8(at, -1);
    if which == GONE {
        return;
    }
    let left = core.raw_read_8(at + 2, -1);
    if left == 0 {
        return; // held
    }
    if left > 1 {
        core.raw_write_8(at + 2, -1, left - 1);
        return;
    }
    let step = core.raw_read_8(at + 1, -1) as usize + 1;
    settle(core, at, layer, step);
}

/// The first step after the `n`-th fire event of a script (cycling), or
/// `None` when it has none.
fn recoil(script: &[(u8, u8)], n: usize) -> Option<usize> {
    let after: Vec<usize> = script
        .iter()
        .enumerate()
        .filter(|(_, &(d, f))| f == EVENT && d == FIRE_EVENT)
        .map(|(i, _)| i + 1)
        .collect();
    (!after.is_empty()).then(|| after[n % after.len()])
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

fn put_palettes(core: &mut Core, side: u32, fig: &Figure) {
    for (k, half) in [
        fig.palettes[0],
        fig.palettes[1],
        fig.palettes[0].map(lighten),
        fig.palettes[1].map(lighten),
    ]
    .iter()
    .enumerate()
    {
        let mut b = Vec::with_capacity(30);
        for c in &half[1..] {
            b.extend_from_slice(&c.to_le_bytes());
        }
        let pal = 4 * side + k as u32;
        for base in [PAL_BUFFER, PAL_RAM] {
            let at = base + 32 * pal + 2;
            let mut now = vec![0u8; b.len()];
            core.raw_read_range(at, -1, &mut now);
            if now != b {
                core.raw_write_range(at, -1, &b);
            }
        }
    }
}

/// A sprite: position, size (8, 16 or 32), palette half, and its tiles.
struct Sprite {
    x: i32,
    y: i32,
    size: usize,
    half: u8,
    tiles: Vec<[u8; 32]>,
}

/// Compose the unit's current frame at `origin`, mirrored or not, cropped to
/// the half; cover it with sprites.
fn sprites(fig: &Figure, frames: &[Option<usize>], origin: (i32, i32), mirror: bool, half: (i32, i32)) -> Vec<Sprite> {
    let (ox, oy) = origin;
    // Grid of 8x8 cells aligned on the origin, over the half.
    let c0 = (half.0 - ox).div_euclid(8);
    let c1 = (half.1 - ox + 7).div_euclid(8);
    let r0 = (0 - oy).div_euclid(8);
    let r1 = (SCREEN_H - oy + 7).div_euclid(8);
    let (cols, rows) = ((c1 - c0).max(0) as usize, (r1 - r0).max(0) as usize);
    let (w, h) = (cols * 8, rows * 8);
    let mut buf = vec![0u8; w * h];
    for (layer, f) in fig.layers.iter().zip(frames) {
        let Some(pieces) = f.and_then(|f| layer.frames.get(f)) else {
            continue;
        };
        for p in pieces {
            for (i, &t) in p.texels.iter().enumerate() {
                if t == 0 {
                    continue;
                }
                let (dx, dy) = (p.x + (i % p.w) as i32, p.y + (i / p.w) as i32);
                let sx = if mirror { ox - dx - 1 } else { ox + dx };
                let sy = oy + dy;
                if sx < half.0 || sx >= half.1 || !(0..SCREEN_H).contains(&sy) {
                    continue;
                }
                let (bx, by) = (sx - (ox + 8 * c0), sy - (oy + 8 * r0));
                buf[by as usize * w + bx as usize] = t | p.half << 4 | 0x80;
            }
        }
    }
    let tile = |cx: usize, cy: usize, half: u8| -> Option<[u8; 32]> {
        if cx >= cols || cy >= rows {
            return None;
        }
        let mut out = [0u8; 32];
        let mut any = false;
        for y in 0..8 {
            for x in 0..8 {
                let v = buf[(cy * 8 + y) * w + cx * 8 + x];
                if v != 0 && (v >> 4) & 1 == half {
                    out[y * 4 + x / 2] |= (v & 15) << (4 * (x & 1));
                    any = true;
                }
            }
        }
        any.then_some(out)
    };
    // Which cells each palette half has.
    let used: Vec<Vec<bool>> = (0..2u8)
        .map(|h| {
            (0..rows * cols)
                .map(|k| tile(k % cols, k / cols, h).is_some())
                .collect()
        })
        .collect();
    // Cover with square sprites of 8, 4, 2 or 1 cells: a block becomes one
    // sprite when enough of its cells are used, else splits in four. The
    // loosest cover that fits the side's tiles (fewest sprites) is taken.
    let cover = |need: &[usize; 4]| -> Vec<(u8, usize, usize, usize)> {
        let mut out = Vec::new();
        let mut stack: Vec<(u8, usize, usize, usize)> = Vec::new();
        for h in 0..2u8 {
            for by in (0..rows).step_by(8) {
                for bx in (0..cols).step_by(8) {
                    stack.push((h, bx, by, 8));
                }
            }
        }
        while let Some((h, x0, y0, n)) = stack.pop() {
            let count = (y0..(y0 + n).min(rows))
                .flat_map(|y| (x0..(x0 + n).min(cols)).map(move |x| (x, y)))
                .filter(|&(x, y)| used[h as usize][y * cols + x])
                .count();
            if count == 0 {
                continue;
            }
            let level = n.trailing_zeros() as usize;
            if n == 1 || count >= need[level] {
                out.push((h, x0, y0, n));
            } else {
                let m = n / 2;
                for (dx, dy) in [(0, 0), (m, 0), (0, m), (m, m)] {
                    stack.push((h, x0 + dx, y0 + dy, m));
                }
            }
        }
        out
    };
    let covers: [[usize; 4]; 4] = [[1, 2, 6, 24], [1, 2, 9, 40], [1, 3, 13, 56], [1, 4, 16, 65]];
    let mut blocks = Vec::new();
    for need in &covers {
        blocks = cover(need);
        if blocks.iter().map(|b| b.3 * b.3).sum::<usize>() <= SIDE_TILES as usize {
            break;
        }
    }
    blocks
        .into_iter()
        .map(|(h, x0, y0, n)| {
            let mut tiles = Vec::with_capacity(n * n);
            for y in 0..n {
                for x in 0..n {
                    tiles.push(tile(x0 + x, y0 + y, h).unwrap_or([0u8; 32]));
                }
            }
            let (x, y) = (ox + 8 * (c0 + x0 as i32), oy + 8 * (r0 + y0 as i32));
            Sprite {
                x,
                y,
                size: 8 * n,
                half: h,
                tiles,
            }
        })
        .collect()
}

/// Trap at [`OAM_COPY`], the scene's VBlank copy of the whole shadow OAM
/// ([`SHADOW`]): the donor's figure sprites are hidden and the unit's go
/// in free entries.
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
    // Drop the donors' figure sprites, noting their priority and flash; keep
    // the rest in order.
    let mut prio = [FIGURE_PRIORITY; 2];
    let mut flash = [false; 2];
    let mut kept: Vec<[u16; 3]> = Vec::new();
    for k in 0..SHADOW_ENTRIES {
        let e = SHADOW + 8 * k;
        let attrs = [
            core.raw_read_16(e, -1),
            core.raw_read_16(e + 2, -1),
            core.raw_read_16(e + 4, -1),
        ];
        let (a0, a2) = (attrs[0], attrs[2]);
        let side = ((a2 & 0x3FF) as u32 / SIDE_TILES) as usize;
        let shown = a0 & 0x300 != 0x200 && (a0 & 0x100 != 0 || (a0 & 0xFF) < SCREEN_H as u16);
        if !shown {
            continue;
        }
        if side < 2 && ours[side].is_some() {
            prio[side] = (a2 >> 10) & 3;
            flash[side] |= (a2 >> 12) as u32 >= 4 * side as u32 + 2;
            continue;
        }
        kept.push(attrs);
    }
    let mut mine: Vec<[u16; 3]> = Vec::new();
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
        let middle = if side == 0 {
            60 - spec.outwards
        } else {
            180 + spec.outwards
        };
        let centre_y = if spec.air { SKY } else { GROUND - (b3 - cy) };
        let (hx, hy) = (
            core.raw_read_16(rec + 4, -1) as i16 as i32,
            core.raw_read_16(rec + 6, -1) as i16 as i32,
        );
        let placed = hx != 0 || hy != 0;
        if placed && (hx, hy) != (middle, centre_y) {
            let (dx, dy) = (middle - hx, centre_y - hy);
            for (o, d) in [(4, dx), (6, dy), (8, dx), (0xA, dy)] {
                let v = core.raw_read_16(rec + o, -1) as i16 as i32 + d;
                core.raw_write_16(rec + o, -1, v as u16);
            }
        }
        let (px, py) = (
            core.raw_read_16(rec + 8, -1) as i16 as i32,
            core.raw_read_16(rec + 0xA, -1) as i16 as i32,
        );
        // Shots fired, and the figure's end.
        let shots = core.raw_read_16(SHOTS + SIDE_SHOTS * side, -1);
        let seen = core.raw_read_16(state + S_SHOTS, -1);
        let dead = core.raw_read_8(state + S_DEAD, -1) != 0;
        let standing = core.raw_read_8(rec, -1) != 0;
        for (l, layer) in fig.layers.iter().enumerate() {
            let at = state + S_LAYERS + 4 * l as u32;
            if !dead && !standing && placed {
                if layer.scripts[2].is_empty() {
                    core.raw_write_8(at, -1, GONE);
                } else {
                    start(core, at, layer, 2, 0);
                }
                continue;
            }
            if !dead && shots > seen && (layer.weapon == 0 || layer.weapon == weapon) {
                if let Some(s) = recoil(&layer.scripts[1], seen as usize) {
                    start(core, at, layer, 1, s);
                    continue;
                }
            }
            step_layer(core, at, layer);
        }
        core.raw_write_16(state + S_SHOTS, -1, shots);
        if !standing && placed {
            core.raw_write_8(state + S_DEAD, -1, 1);
        }
        if !placed {
            continue;
        }
        let frames: Vec<Option<usize>> = fig
            .layers
            .iter()
            .enumerate()
            .map(|(l, layer)| {
                let at = state + S_LAYERS + 4 * l as u32;
                let shown = layer.weapon == 0 || layer.weapon == weapon;
                (shown && core.raw_read_8(at, -1) != GONE).then(|| core.raw_read_8(at + 3, -1) as usize)
            })
            .collect();
        let mirror = side == 0;
        let origin = (if mirror { px + cx + 1 } else { px - cx }, py - cy);
        let list = sprites(fig, &frames, origin, mirror, HALVES[side as usize]);
        put_palettes(core, side, fig);
        let mut tile = SIDE_TILES * side;
        for s in list {
            let n = s.tiles.len() as u32;
            if tile + n > SIDE_TILES * (side + 1) || mine.len() + kept.len() >= SHADOW_ENTRIES as usize {
                break;
            }
            let data: Vec<u8> = s.tiles.iter().flatten().copied().collect();
            core.raw_write_range(OBJ_VRAM + 32 * tile, -1, &data);
            let size = match s.size {
                8 => 0,
                16 => 1,
                32 => 2,
                _ => 3,
            };
            let pal = 4 * side as u16 + s.half as u16 + if flash[side as usize] { 2 } else { 0 };
            mine.push([
                (s.y as u16) & 0xFF,
                (s.x as u16 & 0x1FF) | size << 14,
                tile as u16 | prio[side as usize] << 10 | pal << 12,
            ]);
            tile += n;
        }
    }
    // Ours first, as AW2's figures come first: a line's sprite time goes to
    // the unit before the effects, which still draw over it (they have a
    // higher priority). Each entry's fourth halfword (affine parameters)
    // stays where it is.
    for (k, attrs) in mine
        .iter()
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
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recoils() {
        let s = [
            (4, 0),
            (5, EVENT),
            (32, EVENT),
            (4, 1),
            (4, 0),
            (32, EVENT),
            (4, 2),
            (0, HOLD),
        ];
        assert_eq!(recoil(&s, 0), Some(3));
        assert_eq!(recoil(&s, 1), Some(6));
        assert_eq!(recoil(&s, 2), Some(3));
        assert_eq!(recoil(&[(4, 0), (0, LOOP)], 0), None);
        assert_eq!(lighten(0x0421), 0x4210);
    }
}
