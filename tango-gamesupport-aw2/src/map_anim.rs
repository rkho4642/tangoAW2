//! Dual Strike's own map animations for the new units, with the Dual Strike
//! pack: a Black Bomb's explosion, a Stealth hiding and appearing, a Black
//! Boat's REPAIR label, Oozium's death, played for a human and for the CPU.
//!
//! Dual Strike's map effects run on the same animation engine as AW2's
//! (the same animation format: frame table, sequence table, OAM-shaped
//! pieces, 4bpp 1D tiles; Dual Strike's Dive animation is AW2's, byte for
//! byte), so each is played by AW2's own effect with Dual Strike's
//! pictures, palette and animation swapped in, by traps at the point AW2
//! loads its own. Everything is converted at run time from the pack and
//! written to free ROM ([`ROM`]) once; nothing changes without the pack.
//!
//! - **Black Bomb** (Dual Strike: anim `0x0213D11C`, `bmap/05f`, palette
//!   `bmap/060`, `0x020EBA28`): AW2 destroys the bomb with its unit
//!   explosion (`sub_0803FF48`, the proc `0x0849FB04`, which loads its
//!   pictures in `sub_0803FFA0` by explosion kind). When the unit going is
//!   a bomb that exploded ([`mark_bomb`], from the player's Explode and the
//!   CPU's), Dual Strike's explosion is loaded instead, moved 8 pixels up
//!   (Dual Strike centres it on the square, AW2 anchors explosions at the
//!   square's bottom).
//! - **Oozium's death** (anim `0x0213D6AC`, `bmap/045`, the army's unit
//!   palette `bmap/046..04a`, `0x020EE9B0`): the same place, when the unit
//!   going is an Oozium, in its army's colours.
//! - **Stealth Hide / Appear** (anim `0x02149EBC`, `bmap/076`, `077`,
//!   `0x020CA7A8`): AW2's Dive/Rise ripple (`sub_0802BFD0`) loads Dual
//!   Strike's Stealth cloud when the unit is a Stealth. (Dual Strike has one
//!   sequence for both; it serves hide and appear.)
//! - **Black Boat REPAIR** (`national/003`: Dual Strike's labels TRAP!,
//!   SUPPLY, REPAIR, each facing both ways, in AW2's label layout): AW2's
//!   SUPPLY label (`sub_08027560`, labels `0x081121D0`) shows REPAIR when a
//!   Black Boat supplies.
//!
//! **The CPU.** [`crate::cpu_tactics`] decides when a CPU Stealth hides or
//! appears and when a Black Boat repairs; with the pictures here those are
//! played out at the end of the CPU's turn, before the turn passes (a trap
//! in its end, `sub_08061AC4`, which the CPU calls every frame until it
//! returns): one at a time, the camera goes to the unit, the change is
//! made, AW2's ripple or labels play with the pictures above, and the turn
//! waits for them. Its Black Bombs are destroyed through AW2's unit
//! destruction as before, which now shows Dual Strike's explosion.
//!
//! All state is emulated RAM ([`STATE`]), so both netplay peers and every
//! replay play the same frames. None of these pictures is army-coloured
//! but Oozium's death, which takes its army's palette (all five, Black
//! Hole included).

use mgba::core::Core;
use std::sync::OnceLock;

use crate::ds_weather::is_on;
use crate::roster::{BLACK_BOAT, OOZIUM, STEALTH};

// --- Converting -----------------------------------------------------------------

/// A Dual Strike animation, read from the pack: frames of raw OAM triples
/// (attribute 0, attribute 1, tile), and its sequences as (duration,
/// frame) pairs.
struct Anim {
    frames: Vec<Vec<[u16; 3]>>,
    sequences: Vec<Vec<(u16, u16)>>,
}

/// Sequence opcodes with a zero duration that only mark a point (Dual
/// Strike's Oozium has one, `0x80`, between its two halves).
const MARK: u16 = 0x80;

fn parse(pack: &crate::ds_pack::Pack, base: u32) -> Option<Anim> {
    let hw = |a: u32| pack.arm9_at(a, 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let (ft, st) = (base + hw(base)? as u32, base + hw(base + 2)? as u32);
    let nframes = st.checked_sub(ft)? / 2;
    if nframes == 0 || nframes > 64 {
        return None;
    }
    let mut frames = Vec::new();
    let mut first = u32::MAX;
    for i in 0..nframes {
        let a = ft + hw(ft + 2 * i)? as u32;
        first = first.min(a);
        let n = hw(a)? as u32;
        if n > 32 {
            return None;
        }
        let mut pieces = Vec::new();
        for k in 0..n {
            let p = a + 2 + 6 * k;
            pieces.push([hw(p)?, hw(p + 2)?, hw(p + 4)?]);
        }
        frames.push(pieces);
    }
    let nseq = first.checked_sub(st)? / 2;
    let mut sequences = Vec::new();
    for i in 0..nseq.clamp(1, 8) {
        let mut p = st + hw(st + 2 * i)? as u32;
        let mut seq = Vec::new();
        loop {
            let (d, f) = (hw(p)?, hw(p + 2)?);
            p += 4;
            if d == 0 && f == MARK {
                continue;
            }
            if d == 0 {
                break;
            }
            if f as u32 >= nframes || seq.len() > 64 {
                return None;
            }
            seq.push((d, f));
        }
        sequences.push(seq);
    }
    Some(Anim { frames, sequences })
}

impl Anim {
    /// Every piece moved by `dy` pixels.
    fn moved(mut self, dy: i16) -> Anim {
        for p in self.frames.iter_mut().flatten() {
            let y = (p[0] & 0xFF) as u8 as i8 as i16 + dy;
            p[0] = (p[0] & 0xFF00) | (y as u8 as u16);
        }
        self
    }

    /// In AW2's (the same) format, with at least `sequences` sequences (the
    /// last repeated): AW2 asks for its effect's sequence by number.
    fn bytes(&self, sequences: usize) -> Vec<u8> {
        let mut seqs = self.sequences.clone();
        while seqs.len() < sequences {
            seqs.push(seqs.last().cloned().unwrap_or_default());
        }
        let (nf, ns) = (self.frames.len(), seqs.len());
        let ft = 4usize;
        let st = ft + 2 * nf;
        let mut data = Vec::new();
        let mut frame_at = Vec::new();
        let mut at = st + 2 * ns;
        for f in &self.frames {
            frame_at.push(at - ft);
            let mut b = (f.len() as u16).to_le_bytes().to_vec();
            for p in f {
                for v in p {
                    b.extend_from_slice(&v.to_le_bytes());
                }
            }
            at += b.len();
            data.extend(b);
        }
        let mut seq_at = Vec::new();
        for s in &seqs {
            seq_at.push(at - st);
            let mut b = Vec::new();
            for &(d, f) in s {
                b.extend_from_slice(&d.to_le_bytes());
                b.extend_from_slice(&f.to_le_bytes());
            }
            b.extend_from_slice(&[0, 0, 1, 0]);
            at += b.len();
            data.extend(b);
        }
        let mut out = Vec::new();
        out.extend_from_slice(&(ft as u16).to_le_bytes());
        out.extend_from_slice(&(st as u16).to_le_bytes());
        for o in frame_at {
            out.extend_from_slice(&(o as u16).to_le_bytes());
        }
        for o in seq_at {
            out.extend_from_slice(&(o as u16).to_le_bytes());
        }
        out.extend(data);
        while out.len() % 4 != 0 {
            out.push(0);
        }
        out
    }

    fn length(&self, seq: usize) -> u32 {
        self.sequences.get(seq).map_or(0, |s| s.iter().map(|&(d, _)| d as u32).sum())
    }
}

/// One effect's data, ready for ROM: pictures (LZ77), palettes, animation.
struct Effect {
    gfx: Vec<u8>,
    palettes: Vec<[u8; 32]>,
    anim: Vec<u8>,
    length: u32,
}

struct Art {
    bomb: Effect,
    ooze: Effect,
    stealth: Effect,
    /// Dual Strike's REPAIR label, facing right then left (64 tiles).
    repair: Vec<u8>,
}

static ART: OnceLock<Option<Art>> = OnceLock::new();

fn art() -> Option<&'static Art> {
    ART.get_or_init(|| {
        let pack = crate::ds_pack::pack()?;
        let lz = |p: &str| crate::ds_art::lz10(pack.file(p)?);
        let pal = |p: &str| -> Option<[u8; 32]> { pack.file(p)?.get(..32)?.try_into().ok() };
        let effect = |anim: Anim, seqs: usize, gfx: &str, pals: &[&str]| -> Option<Effect> {
            let pixels = lz(gfx)?;
            if pixels.len() > 32 * EFFECT_TILES {
                return None;
            }
            Some(Effect {
                gfx: crate::lz77::compress(&pixels),
                palettes: pals.iter().map(|p| pal(p)).collect::<Option<Vec<_>>>()?,
                length: anim.length(0),
                anim: anim.bytes(seqs),
            })
        };
        let labels = lz("national/003")?;
        Some(Art {
            bomb: effect(parse(pack, BOMB_ANIM)?.moved(-8), 2, "bmap/05f", &["bmap/060"])?,
            ooze: effect(
                parse(pack, OOZE_ANIM)?,
                2,
                "bmap/045",
                &["bmap/046", "bmap/047", "bmap/048", "bmap/049", "bmap/04a"],
            )?,
            stealth: effect(parse(pack, STEALTH_ANIM)?, 2, "bmap/076", &["bmap/077"])?,
            repair: labels.get(LABEL_BYTES * 4..LABEL_BYTES * 6)?.to_vec(),
        })
    })
    .as_ref()
}

/// Whether the pictures are there (the pack).
pub fn available() -> bool {
    art().is_some()
}

const BOMB_ANIM: u32 = 0x0213_D11C;
const OOZE_ANIM: u32 = 0x0213_D6AC;
const STEALTH_ANIM: u32 = 0x0214_9EBC;
/// A label: 64x32, 32 tiles.
const LABEL_BYTES: usize = 32 * 32;
/// AW2's map effects share OBJ tiles 0x1CA.. (224 of them, the Dive's).
const EFFECT_TILES: usize = 224;
const EFFECT_VRAM: u32 = 0x0601_3940;

// --- ROM -------------------------------------------------------------------------

pub const ROM: u32 = 0x087C_1000;
const ROM_END: u32 = 0x087C_4000;
const SENTINEL: u32 = ROM_END - 4;
const MAGIC: u32 = 0x3141_4D44; // "DMA1"

/// Where each effect's parts are in ROM, laid out by [`install`].
#[derive(Clone, Copy, Default)]
struct Placed {
    gfx: u32,
    palettes: u32,
    anim: u32,
}

static LAYOUT: OnceLock<Option<([Placed; 3], Vec<u8>)>> = OnceLock::new();

/// The ROM block: bomb, ooze, stealth (pictures, palettes, animation).
fn layout() -> Option<&'static ([Placed; 3], Vec<u8>)> {
    LAYOUT
        .get_or_init(|| {
            let art = art()?;
            let mut out = Vec::new();
            let mut placed = [Placed::default(); 3];
            for (k, e) in [&art.bomb, &art.ooze, &art.stealth].into_iter().enumerate() {
                let mut put = |b: &[u8]| {
                    let at = ROM + out.len() as u32;
                    out.extend_from_slice(b);
                    while out.len() % 4 != 0 {
                        out.push(0);
                    }
                    at
                };
                let gfx = put(&e.gfx);
                let pals: Vec<u8> = e.palettes.iter().flatten().copied().collect();
                let palettes = put(&pals);
                let anim = put(&e.anim);
                placed[k] = Placed { gfx, palettes, anim };
            }
            (ROM + out.len() as u32 <= SENTINEL).then_some((placed, out))
        })
        .as_ref()
}

/// Writes the pictures and animations once (the same bytes on every peer).
pub fn install(core: &mut Core) {
    let Some((_, bytes)) = layout() else { return };
    if core.raw_read_32(SENTINEL, -1) == MAGIC {
        return;
    }
    core.raw_write_range(ROM, -1, bytes);
    core.raw_write_32(SENTINEL, -1, MAGIC);
}

// --- RAM ---------------------------------------------------------------------------

/// tangoAW2's RAM for this (0x60 bytes):
/// +0x00 u32 the bomb whose destruction is its explosion ([`mark_bomb`]);
/// +0x04 the unit being destroyed now: kind (0, 1 bomb, 2 Oozium), x, y,
/// country; +0x08 the CPU's context: 1 a Stealth's ripple, 2 a REPAIR
/// label; +0x09 the army whose turn end was planned; +0x0A queue length,
/// +0x0B index, +0x0C step, +0x0D timer, +0x0E labels, +0x0F label index;
/// +0x10 u32 the map's scroll last frame; +0x14 up to 4 labels (x, y);
/// +0x20 the queue: 32 x (unit id, action).
pub const STATE: u32 = 0x0203_F740;
const BOMB: u32 = STATE;
const GOING: u32 = STATE + 4;
const CONTEXT: u32 = STATE + 8;
const PLANNED: u32 = STATE + 9;
const QUEUE_LEN: u32 = STATE + 0x0A;
const INDEX: u32 = STATE + 0x0B;
const STEP: u32 = STATE + 0x0C;
const TIMER: u32 = STATE + 0x0D;
const LABELS: u32 = STATE + 0x0E;
const LABEL_INDEX: u32 = STATE + 0x0F;
const LAST_SCROLL: u32 = STATE + 0x10;
const LABEL_AT: u32 = STATE + 0x14;
const QUEUE: u32 = STATE + 0x20;
const QUEUE_MAX: u32 = 32;
const STATE_LEN: u32 = 0x60;

const CTX_STEALTH: u8 = 1;
const CTX_REPAIR: u8 = 2;
const GOING_BOMB: u8 = 1;
const GOING_OOZE: u8 = 2;

const SELECTED: u32 = 0x0300_40D8;
const CURRENT_ARMY: u32 = 0x0300_33EC;
const UNITS_POINTER: u32 = 0x0849_9594;
const UNIT: u32 = 12;
const MAP_POINTER: u32 = 0x0849_9590;
const DESTINATION: u32 = 0x0300_3100;

fn selected_type(core: &Core) -> Option<u8> {
    let u = core.raw_read_32(SELECTED, -1);
    (u != 0).then(|| core.raw_read_8(u, -1))
}

/// The Black Bomb record `unit` is destroyed because it exploded: its
/// destruction shows Dual Strike's explosion.
pub fn mark_bomb(core: &mut Core, unit: u32) {
    if available() {
        core.raw_write_32(BOMB, -1, unit);
    }
}

fn country(core: &Core, army: u32) -> u8 {
    core.raw_read_8(crate::five::players(core) + 0x3C * army + 0x1A, -1)
}

// --- Traps: the pictures ------------------------------------------------------------

/// `sub_080401B4` (a unit's destruction; r4 its record, about to start the
/// explosion and remove it): a bomb that exploded, or an Oozium, is noted.
const DESTROYING: u32 = 0x0804_01BC;
fn destroying(core: &mut Core) {
    if !is_on(core) || !available() {
        return;
    }
    let u = core.gba().cpu().gpr(4) as u32;
    let t = core.raw_read_8(u, -1);
    let bomb = core.raw_read_32(BOMB, -1) == u;
    let kind = if bomb {
        GOING_BOMB
    } else if t == OOZIUM {
        GOING_OOZE
    } else {
        0
    };
    if bomb {
        core.raw_write_32(BOMB, -1, 0);
    }
    let army = crate::unit_actions::army_of(core, u);
    let bytes = [kind, core.raw_read_8(u + 2, -1), core.raw_read_8(u + 3, -1), country(core, army)];
    core.raw_write_range(GOING, -1, &bytes);
}

/// `sub_0803FFA0` (the explosion proc, r4; its pictures r6, palette r5 and
/// animation r0 just chosen by kind): Dual Strike's for the unit noted.
const EXPLOSION: u32 = 0x0804_001E;
fn explosion(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let Some((placed, _)) = layout() else { return };
    let mut going = [0u8; 4];
    core.raw_read_range(GOING, -1, &mut going);
    if going[0] == 0 {
        return;
    }
    let p = core.gba().cpu().gpr(4) as u32;
    let (x, y) = (core.raw_read_32(p + 0x2C, -1), core.raw_read_32(p + 0x30, -1));
    if (x, y) != (going[1] as u32, going[2] as u32) {
        return;
    }
    core.raw_write_32(GOING, -1, 0);
    let (e, pal) = match going[0] {
        GOING_BOMB => (placed[0], 0),
        _ => (placed[1], (going[3].clamp(1, 5) - 1) as u32),
    };
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(6, e.gfx as i32);
    cpu.set_gpr(5, (e.palettes + 32 * pal) as i32);
    cpu.set_gpr(0, e.anim as i32);
}

fn stealth_ripple(core: &Core) -> bool {
    is_on(core)
        && available()
        && (core.raw_read_8(CONTEXT, -1) == CTX_STEALTH || selected_type(core) == Some(STEALTH))
}

/// `sub_0802BFD0` (the Dive/Rise ripple): its pictures (r0, with r1 the
/// VRAM they go to), then its palette (r0), then its animation (r0).
const RIPPLE_GFX: u32 = 0x0802_C052;
const RIPPLE_PAL: u32 = 0x0802_C05A;
const RIPPLE_ANIM: u32 = 0x0802_C066;
fn ripple_part(core: &mut Core, part: u8) {
    if !stealth_ripple(core) {
        return;
    }
    let Some((placed, _)) = layout() else { return };
    let s = placed[2];
    let cpu = core.gba_mut().cpu_mut();
    match part {
        0 => {
            cpu.set_gpr(0, s.gfx as i32);
            cpu.set_gpr(1, EFFECT_VRAM as i32);
        }
        1 => cpu.set_gpr(0, s.palettes as i32),
        _ => cpu.set_gpr(0, s.anim as i32),
    }
}

/// `sub_08027560` (a label: AW2's labels just decompressed to the effect
/// tiles, its palette in r0): a Black Boat's SUPPLY reads REPAIR.
const LABELS_LOADED: u32 = 0x0802_7582;
/// The SUPPLY labels' tiles (the third and fourth label).
const SUPPLY_TILES: u32 = EFFECT_VRAM + 2 * LABEL_BYTES as u32;
fn labels_loaded(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let Some(art) = art() else { return };
    let boat = core.raw_read_8(CONTEXT, -1) == CTX_REPAIR || selected_type(core) == Some(BLACK_BOAT);
    if boat {
        core.raw_write_range(SUPPLY_TILES, -1, &art.repair);
    }
}

// --- The CPU's turn end ---------------------------------------------------------

/// What the CPU does at its turn's end, one after another.
pub const HIDE: u8 = 1;
pub const APPEAR: u8 = 2;
pub const REPAIR: u8 = 3;

/// `sub_08061AC4`, the CPU's turn end (called every frame until the turn
/// passes), after its `push {lr}`: first the Stealths and Black Boats act,
/// each shown; until they are all done the function returns at once.
const TURN_END: u32 = 0x0806_1AC8;
const TURN_END_RETURN: u32 = 0x0806_1AEE;
const CAMERA_TO: u32 = 0x0802_9088;
const RIPPLE: u32 = 0x0802_BFD0;
const SUPPLY_LABEL: u32 = 0x0802_723C;
const END_LABELS: u32 = 0x0802_72B4;
/// Frames the camera may take, and each effect is given.
const CAMERA_FRAMES: u8 = 60;
const HOLD_FRAMES: u8 = 36;

fn call(core: &mut Core, f: u32, r0: u32, r1: u32) {
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, r0 as i32);
    cpu.set_gpr(1, r1 as i32);
    cpu.set_gpr(14, (TURN_END_RETURN | 1) as i32);
    cpu.set_thumb_pc(f);
}

fn leave(core: &mut Core) {
    core.gba_mut().cpu_mut().set_thumb_pc(TURN_END_RETURN);
}

fn turn_end(core: &mut Core) {
    if !is_on(core) || !available() {
        return;
    }
    let army = core.raw_read_16(CURRENT_ARMY, -1) as u32;
    if core.raw_read_8(PLANNED, -1) as u32 != army {
        let actions = crate::cpu_tactics::turn_end_actions(core, army);
        let n = actions.len().min(QUEUE_MAX as usize);
        for (k, &(id, a)) in actions[..n].iter().enumerate() {
            core.raw_write_8(QUEUE + 2 * k as u32, -1, id);
            core.raw_write_8(QUEUE + 2 * k as u32 + 1, -1, a);
        }
        for (at, v) in [(PLANNED, army as u8), (QUEUE_LEN, n as u8), (INDEX, 0), (STEP, 0), (TIMER, 0), (CONTEXT, 0)] {
            core.raw_write_8(at, -1, v);
        }
    }
    let index = core.raw_read_8(INDEX, -1) as u32;
    if index >= core.raw_read_8(QUEUE_LEN, -1) as u32 {
        return;
    }
    let id = core.raw_read_8(QUEUE + 2 * index, -1) as u32;
    let action = core.raw_read_8(QUEUE + 2 * index + 1, -1);
    let u = core.raw_read_32(UNITS_POINTER, -1) + UNIT * id;
    let (x, y) = (core.raw_read_8(u + 2, -1) as u32, core.raw_read_8(u + 3, -1) as u32);
    let timer = core.raw_read_8(TIMER, -1);
    let next = |core: &mut Core| {
        core.raw_write_8(CONTEXT, -1, 0);
        core.raw_write_8(INDEX, -1, index as u8 + 1);
        core.raw_write_8(STEP, -1, 0);
        core.raw_write_8(TIMER, -1, 0);
    };
    match core.raw_read_8(STEP, -1) {
        // The camera goes to the unit.
        0 => {
            core.raw_write_8(STEP, -1, 1);
            core.raw_write_8(TIMER, -1, 0);
            core.raw_write_32(LAST_SCROLL, -1, 0xFFFF_FFFF);
            call(core, CAMERA_TO, x, y);
        }
        // ...and stops.
        1 => {
            let map = core.raw_read_32(MAP_POINTER, -1);
            let scroll = core.raw_read_32(map + 4, -1);
            let still = scroll == core.raw_read_32(LAST_SCROLL, -1);
            core.raw_write_32(LAST_SCROLL, -1, scroll);
            core.raw_write_8(TIMER, -1, timer + 1);
            if (still && timer >= 4) || timer >= CAMERA_FRAMES {
                core.raw_write_8(STEP, -1, 2);
            }
            leave(core);
        }
        // The action, and its effect.
        2 => {
            core.raw_write_8(TIMER, -1, 0);
            if action == REPAIR {
                let repaired = crate::cpu_tactics::repair_now(core, u);
                let n = repaired.len().min(4);
                for (k, &(rx, ry)) in repaired[..n].iter().enumerate() {
                    core.raw_write_8(LABEL_AT + 2 * k as u32, -1, rx as u8);
                    core.raw_write_8(LABEL_AT + 2 * k as u32 + 1, -1, ry as u8);
                }
                core.raw_write_8(LABELS, -1, n as u8);
                core.raw_write_8(LABEL_INDEX, -1, 0);
                core.raw_write_8(CONTEXT, -1, CTX_REPAIR);
                core.raw_write_8(STEP, -1, 4);
                leave(core);
            } else {
                let f = core.raw_read_8(u + 1, -1);
                let hidden = action == HIDE;
                core.raw_write_8(u + 1, -1, if hidden { f | 0x20 } else { f & !0x20 });
                core.raw_write_16(DESTINATION, -1, x as u16);
                core.raw_write_16(DESTINATION + 2, -1, y as u16);
                core.raw_write_8(CONTEXT, -1, CTX_STEALTH);
                core.raw_write_8(STEP, -1, 3);
                call(core, RIPPLE, if hidden { 0 } else { 1 }, 0);
            }
        }
        // The REPAIR labels, one a frame.
        4 => {
            let k = core.raw_read_8(LABEL_INDEX, -1) as u32;
            if k < core.raw_read_8(LABELS, -1) as u32 {
                core.raw_write_8(LABEL_INDEX, -1, k as u8 + 1);
                let (lx, ly) = (core.raw_read_8(LABEL_AT + 2 * k, -1), core.raw_read_8(LABEL_AT + 2 * k + 1, -1));
                call(core, SUPPLY_LABEL, lx as u32, ly as u32);
            } else {
                core.raw_write_8(STEP, -1, 3);
                leave(core);
            }
        }
        // Wait for it to play (and take the labels away: they stay until
        // their Supply event would end them, `sub_080272B4`).
        _ => {
            core.raw_write_8(TIMER, -1, timer + 1);
            if timer + 1 >= HOLD_FRAMES {
                next(core);
                if action == REPAIR {
                    call(core, END_LABELS, 0, 0);
                    return;
                }
            }
            leave(core);
        }
    }
}

/// A new army's turn: the turn-end plan is for the army ending it.
pub fn turn_changed(core: &mut Core) {
    if core.raw_read_8(PLANNED, -1) != 0 {
        for k in 0..STATE_LEN - 8 {
            if core.raw_read_8(STATE + 8 + k, -1) != 0 {
                core.raw_write_8(STATE + 8 + k, -1, 0);
            }
        }
    }
}

/// Frames the Stealth effect and the bomb's explosion take (for tests).
pub fn lengths() -> Option<(u32, u32)> {
    art().map(|a| (a.stealth.length, a.bomb.length))
}

pub fn tick(core: &mut Core, on: bool) {
    if on {
        install(core);
    }
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![
        (DESTROYING, Box::new(destroying)),
        (EXPLOSION, Box::new(explosion)),
        (RIPPLE_GFX, Box::new(|core: &mut Core| ripple_part(core, 0))),
        (RIPPLE_PAL, Box::new(|core: &mut Core| ripple_part(core, 1))),
        (RIPPLE_ANIM, Box::new(|core: &mut Core| ripple_part(core, 2))),
        (LABELS_LOADED, Box::new(labels_loaded)),
        (TURN_END, Box::new(turn_end)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room() {
        assert!(STATE + STATE_LEN <= 0x0203_F7A0);
        assert!(QUEUE + 2 * QUEUE_MAX <= STATE + STATE_LEN);
        assert!(ROM >= 0x087C_1000 && ROM_END <= 0x087C_4000);
    }

    #[test]
    fn anim_bytes_round_trip() {
        let a = Anim { frames: vec![vec![[0xF0, 0x81F0, 4]], vec![]], sequences: vec![vec![(3, 0), (2, 1)]] };
        let b = a.bytes(2);
        let hw = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]) as usize;
        let (ft, st) = (hw(0), hw(2));
        assert_eq!((ft, st), (4, 8));
        let f0 = ft + hw(ft);
        assert_eq!((hw(f0), hw(f0 + 2), hw(f0 + 4), hw(f0 + 6)), (1, 0xF0, 0x81F0, 4));
        let s1 = st + hw(st + 2);
        assert_eq!((hw(s1), hw(s1 + 2), hw(s1 + 8), hw(s1 + 10)), (3, 0, 0, 1));
        assert_eq!(a.moved(-8).frames[0][0][0], 0xE8);
    }

    /// With `TANGOAW2_DS_ROM`: everything converts and fits.
    #[test]
    #[ignore]
    fn converts() {
        crate::ds_pack::pack().expect("TANGOAW2_DS_ROM");
        let art = art().expect("art");
        assert_eq!(art.repair.len(), 2 * LABEL_BYTES);
        assert_eq!(art.ooze.palettes.len(), 5);
        assert!(layout().is_some());
        eprintln!("lengths {:?}", lengths());
    }
}
