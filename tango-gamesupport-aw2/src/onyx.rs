//! Crystal Calamity's Black Onyx (DS Campaign, with the Dual Strike pack):
//! Black Hole's satellite, as Dual Strike has it (docs/AW2.md "Crystal
//! Calamity: the Black Onyx").
//!
//! **Dual Strike** (read from its code and checked in melonDS):
//!
//! - The satellite's state is a block at `[0x021694C0]` (`0x020F3604(9,
//!   36000)` at the battle's setup): +0x14 the hits it still takes (9),
//!   +0x1C its state (1 charging, 2 the 90% warning, 3 firing, 4 hit, 5
//!   destroyed), +0x38 the full charge (36000) and +0x3C the charge.
//! - The mission's first script (day 1) starts a 50-minute countdown (op
//!   0x5A, 180000 frames), shown on the top screen as MM:SS. The charge
//!   rises by one each frame the countdown runs (`0x020F232C`, called from
//!   the battle's frame with `0x020C0688`): on every army's turn, with a
//!   menu open, but not while a script (dialogue) runs nor on a full
//!   screen (the CO screen, the mini map). 36000 frames are ten minutes.
//! - The header's seventh trigger list, tested every frame: at 90% the
//!   laser's warning (state 2; the first time, "Yo! Check out Black
//!   Onyx!"), at full charge the laser (`0x020F2134`: state 3, the charge
//!   emptied, then an 8 HP strike within 2 squares of the spot Black
//!   Hole's computer scores best, as Sturm's Meteor Strike: `0x020985D0(4,
//!   0, 2)`; the first time on Normal, "What happened?!"), and the
//!   countdown at 0: Black Hole wins.
//! - On map 0xF2 a missile silo launches at the satellite instead of the
//!   map (`0x020BD358`): the silo is spent, the missile rises on the top
//!   screen, the charge is emptied (state 4) and the satellite takes a hit.
//!   At 0 hits it is destroyed (state 5, "It can't be. I would never have
//!   believed Black Onyx could be destroyed."), the countdown stops (op
//!   0x5B); the mission is still won by breaking the Black Obelisk.
//!
//! **Here.** The state is in RAM ([`STATE`]), kept with a mission saved
//! halfway (crate::suspend). The countdown and the charge advance once a
//! frame while the battle map runs its own frame (main callback on the
//! map, no event script, not in the Setup phase): the same frames Dual
//! Strike counts. The seventh list is converted with the others
//! (crate::ds_campaign_data, [`crate::campaign_model::MissionInfo::realtime`])
//! and run by AW2's own list runner (`sub_08074484`) from the battle's
//! frame while the map waits for a unit's orders. The laser is AW2's
//! meteor strike (`0x084A0858`, Sturm's: 8 HP, radius 2, the computer's
//! target for Black Hole's army). A silo's Launch here runs our proc: the
//! camera to the silo, AW2's launch (the silo spent), the hit on the
//! satellite, then the unit's action ends as usual (its after-action events
//! see the hit). The top screen becomes a small panel at the map's top
//! (at its bottom while the cursor is up there): Dual Strike's satellite
//! (`bmap/085`, its palette `bmap/086`, laid out by its animation at ARM9
//! `0x0213B8AC`, made small at run time), the time left, a diamond per hit
//! still needed and the laser's charge; its warning, beam, the missile's
//! hit and the destruction drawn there.

use mgba::core::Core;
use std::sync::OnceLock;

// --- Dual Strike's numbers ------------------------------------------------------

/// The hits the satellite takes, its full charge (frames), the warning's
/// share (%).
pub const HITS: u8 = 9;
pub const FULL: u16 = 36000;
const WARN_PERCENT: u32 = 90;
/// The laser's damage (tenths of HP) and the strike's army (Black Hole).
const LASER_DAMAGE: u16 = 80;

// --- RAM (EWRAM the game never writes; netplay and rollback safe) -----------------

/// The satellite: +0 1 while Crystal Calamity's battle has it, +1 hits left,
/// +2 phase, +3 frames into the phase (u16 at +4), +6 the charge (u16),
/// +8 the countdown's state (bit 0 running, bit 1 run out), +9 Black
/// Hole's army, +0xA busy (the launch proc waits), +0xB 1 while the list
/// runs (the callback's next start is its own), +0xC the main
/// callback's return address while the list runs (u32), +0x10 the silo.
const STATE: u32 = 0x0203_FFC8;
const ON: u32 = STATE;
const HITS_LEFT: u32 = STATE + 1;
const PHASE: u32 = STATE + 2;
const ANIM: u32 = STATE + 4;
const CHARGE: u32 = STATE + 6;
const CLOCK: u32 = STATE + 8;
const ARMY: u32 = STATE + 9;
const BUSY: u32 = STATE + 0xA;
const LIST_RAN: u32 = STATE + 0xB;
const SAVED_LR: u32 = STATE + 0xC;
const SILO: u32 = STATE + 0x10;
/// The laser's beam: +0x14 1 while it plays, +0x15 the panel's side (1
/// left), +0x16 its frames (u16), +0x18 its square (x, y).
const BEAM: u32 = STATE + 0x14;
const SIDE: u32 = STATE + 0x15;
const BEAM_TIME: u32 = STATE + 0x16;
const BEAM_AT: u32 = STATE + 0x18;
const STATE_END: u32 = STATE + 0x1C;

/// The phases (Dual Strike's states).
pub const CHARGING: u8 = 1;
pub const WARNING: u8 = 2;
pub const FIRING: u8 = 3;
pub const HIT: u8 = 4;
pub const DESTROYED: u8 = 5;
/// Gone (destroyed and fallen away): nothing shows.
pub const GONE: u8 = 0;

const CLOCK_RUNNING: u8 = 1;
const CLOCK_OUT: u8 = 2;

// --- AW2 --------------------------------------------------------------------------

const MAIN_CALLBACK: u32 = 0x0300_0000;
const MAP_CALLBACK: u32 = 0x0802_2049;
/// The battle map's main callback (every frame on the map): trapped to run
/// the real-time list while the map waits.
const MAP_FRAME: u32 = 0x0802_2048;
const MAP_STATE: u32 = 0x0300_32D8;
/// The map waits for the player's cursor; the computer between two units
/// (with no proc busy).
const STATE_CURSOR: u16 = 0xD;
const STATE_CPU: u16 = 0xE;
/// The map's busy counter (menus, strikes, launches: `sub_08034F7C` /
/// `sub_08034F8C`).
const MAP_BUSY: u32 = 0x0300_30F0;
/// `RunEventList(list, unit, arg)`: tests each record, starts the scripts
/// of those that hold (`StartEventScript`, `sub_08019348`), sets their flags.
const RUN_LIST: u32 = 0x0807_4484;
const COUNTDOWN: u32 = crate::ds_campaign::COUNTDOWN;
const DESTINATION: u32 = 0x0300_3100;
const MAP_POINTER: u32 = 0x0849_9590;
const CURSOR_X: u32 = 0x0300_33E4;
const CURSOR_Y: u32 = 0x0300_33E6;
const MAP: u32 = 0x0201_E450;
const SILO_TILE: u16 = 0x180;
const PLAYER: u32 = 0x3C;
const COLOUR: u32 = 0x1A;
const BLACK_HOLE: u8 = 5;
const PROC_START: u32 = 0x0801_C8F5;
/// The meteor strike (Sturm's): 15 commands; the 4th picks the target for
/// the army moving now (`sub_080448E4`).
const METEOR_SCRIPT: u32 = 0x084A_0858;
const METEOR_COMMANDS: u32 = 15;
const METEOR_TARGET_AT: u32 = 3;

fn tile_at(core: &Core, x: u32, y: u32) -> u16 {
    let row = core.raw_read_16(MAP + 0x417A + 2 * y, -1) as u32;
    core.raw_read_16(MAP + 0xA22 + 2 * (row + x), -1)
}

fn events_running(core: &Core) -> bool {
    (0..11).any(|k| core.raw_read_32(0x0200_C510 + 0x18 * k, -1) != 0)
}

// --- ROM (free: 0x08E75000.., past the Setup phase's) ------------------------------

const ROM: u32 = 0x08E7_5000;
/// Launch on Crystal Calamity's map (in place of `sub_0802D0A4`'s body):
/// `Proc_Start(LAUNCH_SCRIPT, 3)`, then the menu's close (`sub_0801A168`).
const LAUNCH_FN: u32 = ROM + 0x40;
/// The laser: `Proc_Start(LASER_SCRIPT, 3)->damage = 80`.
const LASER_FN: u32 = ROM + 0x80;
/// The laser's target: the computer's for Black Hole's army
/// (`sub_0805C290(army, 1)`), the camera to it (as `sub_080448E4`).
const TARGET_FN: u32 = ROM + 0xC0;
/// The launch proc's hit on the satellite: a magic stub (Rust, through
/// crate::ds_campaign's landing: ids [`MAGIC`] | n).
const SETUP_FN: u32 = ROM + 0x110;
/// `busy()`: the launch proc waits while the satellite's hit plays.
const BUSY_FN: u32 = ROM + 0x120;
/// The main callback's way back after the list ran: its return address
/// restored, then the callback itself.
const MAIN_STUB: u32 = ROM + 0x130;
/// The laser's beam (in place of the meteor's drawing, `sub_08044968`; a
/// magic stub), and `playing()` (in place of the meteor's wait,
/// `sub_0806AAC4`).
const BEAM_FN: u32 = ROM + 0x150;
const BEAM_WAIT_FN: u32 = ROM + 0x160;
const LAUNCH_SCRIPT: u32 = ROM + 0x170;
/// The magic ids of [`SETUP_FN`] and [`BEAM_FN`] (crate::ds_campaign's
/// landing hands them to [`magic`]).
pub const MAGIC: u32 = 0x2D00_0000;
const MAGIC_SETUP: u32 = MAGIC | 1;
const MAGIC_BEAM: u32 = MAGIC | 2;
const LASER_SCRIPT: u32 = ROM + 0x200;
const ROM_SENTINEL: u32 = ROM + 0x3FC;
const ROM_MAGIC: u32 = 0x4E59_4E4F; // "ONYN" (bump when the code changes)

fn halfwords(h: &[u16]) -> Vec<u8> {
    h.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn words(w: &[u32]) -> Vec<u8> {
    w.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn proc_cmd(op: u16, arg: i16, ptr: u32) -> [u8; 8] {
    let mut c = [0u8; 8];
    c[0..2].copy_from_slice(&op.to_le_bytes());
    c[2..4].copy_from_slice(&arg.to_le_bytes());
    c[4..8].copy_from_slice(&ptr.to_le_bytes());
    c
}

fn launch_fn() -> Vec<u8> {
    let mut b = halfwords(&[
        0xB500, // push {lr}
        0x4807, // ldr r0, =LAUNCH_SCRIPT
        0x2103, // movs r1, #3
        0x4B07, // ldr r3, =Proc_Start
        0x467A, // mov r2, pc
        0x3205, // adds r2, #5
        0x4696, // mov lr, r2
        0x4718, // bx r3
        0x4B05, // ldr r3, =sub_0801A168
        0x467A, // mov r2, pc
        0x3205, // adds r2, #5
        0x4696, // mov lr, r2
        0x4718, // bx r3
        0xBC01, // pop {r0}
        0x4700, // bx r0
        0x46C0, // nop
    ]);
    b.extend(words(&[LAUNCH_SCRIPT, PROC_START, 0x0801_A169]));
    b
}

fn laser_fn() -> Vec<u8> {
    let mut b = halfwords(&[
        0xB500, // push {lr}
        0x4806, // ldr r0, =LASER_SCRIPT
        0x2103, // movs r1, #3
        0x4B06, // ldr r3, =Proc_Start
        0x467A, // mov r2, pc
        0x3205, // adds r2, #5
        0x4696, // mov lr, r2
        0x4718, // bx r3
        0x3064, // adds r0, #0x64
        0x2100 | LASER_DAMAGE, // movs r1, #damage
        0x8001, // strh r1, [r0]
        0x2000, // movs r0, #0
        0xBC02, // pop {r1}
        0x4708, // bx r1
    ]);
    b.extend(words(&[LASER_SCRIPT, PROC_START]));
    b
}

fn target_fn() -> Vec<u8> {
    let mut b = halfwords(&[
        0xB510, // 00 push {r4, lr}
        0x1C04, // 02 adds r4, r0, #0
        0x480E, // 04 ldr r0, =ARMY
        0x7800, // 06 ldrb r0, [r0]
        0x2101, // 08 movs r1, #1
        0x4B0E, // 0a ldr r3, =sub_0805C290
        0x467A, // 0c mov r2, pc
        0x3205, // 0e adds r2, #5
        0x4696, // 10 mov lr, r2
        0x4718, // 12 bx r3
        0x0600, // 14 lsls r0, r0, #24
        0x0E00, // 16 lsrs r0, r0, #24
        0x3466, // 18 adds r4, #0x66
        0x8020, // 1a strh r0, [r4]
        0x2800, // 1c cmp r0, #0
        0xD00C, // 1e beq 0x3a
        0x4A09, // 20 ldr r2, =units pointer
        0x6812, // 22 ldr r2, [r2]
        0x0041, // 24 lsls r1, r0, #1
        0x1809, // 26 adds r1, r1, r0
        0x0089, // 28 lsls r1, r1, #2
        0x1851, // 2a adds r1, r2, r1
        0x7888, // 2c ldrb r0, [r1, #2]
        0x78C9, // 2e ldrb r1, [r1, #3]
        0x4B06, // 30 ldr r3, =sub_08029088 (the camera)
        0x467A, // 32 mov r2, pc
        0x3205, // 34 adds r2, #5
        0x4696, // 36 mov lr, r2
        0x4718, // 38 bx r3
        0xBC10, // 3a pop {r4}
        0xBC01, // 3c pop {r0}
        0x4700, // 3e bx r0
    ]);
    b.extend(words(&[ARMY, 0x0805_C291, 0x0849_9594, 0x0802_9089]));
    b
}

/// `u8 f(void)`: the byte at `at`.
fn byte_fn(at: u32) -> Vec<u8> {
    let mut b = halfwords(&[
        0x4801, // ldr r0, =at
        0x7800, // ldrb r0, [r0]
        0x4770, // bx lr
        0x46C0, // nop
    ]);
    b.extend(words(&[at]));
    b
}

fn busy_fn() -> Vec<u8> {
    byte_fn(BUSY)
}

fn main_stub() -> Vec<u8> {
    let mut b = halfwords(&[
        0x4802, // ldr r0, =SAVED_LR
        0x6800, // ldr r0, [r0]
        0x4686, // mov lr, r0
        0x4802, // ldr r0, =the main callback
        0x4700, // bx r0
        0x46C0, // nop
    ]);
    b.extend(words(&[SAVED_LR, MAP_CALLBACK]));
    b
}

fn launch_script() -> Vec<u8> {
    [
        // As the silo's own strike: the map busy while it plays.
        proc_cmd(0x04, 0, 0x0803_4F8D),
        proc_cmd(0x02, 0, 0x0803_4F7D),
        proc_cmd(0x02, 0, SETUP_FN | 1),
        // The camera to the silo, then AW2's launch there (the silo spent).
        proc_cmd(0x02, 0, 0x0804_0985),
        proc_cmd(0x1A, 0, 0x0849_A00C),
        proc_cmd(0x02, 0, 0x0804_09B5),
        // The missile's hit on the satellite (the panel).
        proc_cmd(0x14, 0, BUSY_FN | 1),
        // The unit's action ends as at a silo's target (`sub_0804096C`: its
        // Wait and the after-action events).
        proc_cmd(0x02, 0, 0x0804_096D),
        proc_cmd(0x00, 0, 0),
    ]
    .concat()
}

/// The meteor script's commands: its draw (`sub_08044968`), its wait for
/// the meteor (`PROC_WHILE sub_0806AAC4`) and its fade from white.
const METEOR_DRAW: u32 = 0x0804_4969;
const METEOR_WAIT: u32 = 0x0806_AAC5;
const OP_WHILE: u16 = 0x14;
const OP_FADE_FROM_WHITE: u16 = 0x26;
const OP_SLEEP: u16 = 0x0E;

/// AW2's meteor strike with Black Hole's target and Dual Strike's laser
/// beam in place of the meteor (no fade from white: the meteor's).
fn laser_script(core: &Core) -> Vec<u8> {
    let mut s = Vec::new();
    for k in 0..METEOR_COMMANDS {
        let mut c = [0u8; 8];
        core.raw_read_range(METEOR_SCRIPT + 8 * k, -1, &mut c);
        let op = u16::from_le_bytes([c[0], c[1]]);
        let ptr = u32::from_le_bytes([c[4], c[5], c[6], c[7]]);
        if k == METEOR_TARGET_AT {
            c = proc_cmd(op, 0, TARGET_FN | 1);
        } else if ptr == METEOR_DRAW {
            c = proc_cmd(op, 0, BEAM_FN | 1);
        } else if op == OP_WHILE && ptr == METEOR_WAIT {
            c = proc_cmd(OP_WHILE, 0, BEAM_WAIT_FN | 1);
        } else if op == OP_FADE_FROM_WHITE {
            c = proc_cmd(OP_SLEEP, 1, 0);
        }
        s.extend_from_slice(&c);
    }
    s
}

/// Writes the code and scripts once (the same bytes on every console).
fn install(core: &mut Core) {
    if core.raw_read_32(ROM_SENTINEL, -1) == ROM_MAGIC {
        return;
    }
    core.raw_write_range(LAUNCH_FN, -1, &launch_fn());
    core.raw_write_range(LASER_FN, -1, &laser_fn());
    core.raw_write_range(TARGET_FN, -1, &target_fn());
    core.raw_write_range(SETUP_FN, -1, &crate::campaign_model::stub(MAGIC_SETUP));
    core.raw_write_range(BUSY_FN, -1, &busy_fn());
    core.raw_write_range(MAIN_STUB, -1, &main_stub());
    core.raw_write_range(BEAM_FN, -1, &crate::campaign_model::stub(MAGIC_BEAM));
    core.raw_write_range(BEAM_WAIT_FN, -1, &byte_fn(BEAM));
    core.raw_write_range(LAUNCH_SCRIPT, -1, &launch_script());
    let laser = laser_script(core);
    core.raw_write_range(LASER_SCRIPT, -1, &laser);
    core.raw_write_32(ROM_SENTINEL, -1, ROM_MAGIC);
}

// --- State ------------------------------------------------------------------------

/// Crystal Calamity (its main front) in a DS Campaign session.
fn mission_is(core: &Core) -> bool {
    crate::ds_campaign::active(core)
        && crate::ds_campaign::mission(core) as usize == crate::ds_campaign_data::CRYSTAL_CALAMITY
        && !crate::two_front::second_live(core)
}

/// Crystal Calamity's battle.
fn mission_on(core: &Core) -> bool {
    mission_is(core) && crate::ds_campaign::in_battle(core)
}

/// The satellite is in this battle.
pub fn on(core: &Core) -> bool {
    core.raw_read_8(ON, -1) == 1 && mission_on(core)
}

pub fn hits_left(core: &Core) -> u8 {
    core.raw_read_8(HITS_LEFT, -1)
}

pub fn phase(core: &Core) -> u8 {
    core.raw_read_8(PHASE, -1)
}

pub fn charge(core: &Core) -> u16 {
    core.raw_read_16(CHARGE, -1)
}

/// Dual Strike's `0x020F22C8`: 100 x charge / full.
pub fn charge_percent(core: &Core) -> u32 {
    if !on(core) {
        return 0;
    }
    100 * charge(core) as u32 / FULL as u32
}

/// Dual Strike's `0x020F2308`: the charge is full.
pub fn charged(core: &Core) -> bool {
    on(core) && charge(core) >= FULL
}

/// Dual Strike's `0x02351708`: no hit left to take.
pub fn destroyed(core: &Core) -> bool {
    on(core) && hits_left(core) == 0
}

fn set_phase(core: &mut Core, p: u8) {
    core.raw_write_8(PHASE, -1, p);
    core.raw_write_16(ANIM, -1, 0);
}

/// Every map start: Crystal Calamity's satellite is set up (9 hits, no
/// charge, Black Hole's army noted); any other map has none.
pub fn map_start(core: &mut Core) {
    if !mission_is(core) {
        if core.raw_read_8(ON, -1) != 0 {
            core.raw_write_8(ON, -1, 0);
        }
        return;
    }
    install(core);
    for a in STATE..STATE_END {
        core.raw_write_8(a, -1, 0);
    }
    core.raw_write_8(ON, -1, 1);
    core.raw_write_8(HITS_LEFT, -1, HITS);
    core.raw_write_8(PHASE, -1, CHARGING);
    note_army(core);
}

fn note_army(core: &mut Core) {
    let p = crate::five::players(core);
    let army = (1..=4u32).find(|&a| core.raw_read_8(p + PLAYER * a + COLOUR, -1) == BLACK_HOLE).unwrap_or(4);
    core.raw_write_8(ARMY, -1, army as u8);
}

// --- The countdown (Dual Strike's op 0x5A) ------------------------------------------

/// Op 0x5A (`n` frames) / 0x5B (0: stopped).
pub fn set_countdown(core: &mut Core, n: u32) {
    core.raw_write_32(COUNTDOWN, -1, n);
    let c = core.raw_read_8(CLOCK, -1);
    core.raw_write_8(CLOCK, -1, if n > 0 { CLOCK_RUNNING } else { c & !CLOCK_RUNNING });
}

/// Dual Strike's `0x023516C8`: the countdown has run out.
pub fn countdown_expired(core: &Core) -> bool {
    on(core) && core.raw_read_8(CLOCK, -1) & CLOCK_OUT != 0
}

/// The battle's clock runs this frame: the battle map runs its own frame
/// (not a full screen such as the CO screen), no event script (dialogue),
/// not the Setup phase. Dual Strike's countdown and charge count these
/// frames.
fn clock_runs(core: &Core) -> bool {
    core.raw_read_32(MAIN_CALLBACK, -1) == MAP_CALLBACK
        && !events_running(core)
        && !crate::setup_phase::active(core)
        && !crate::two_front::swapping(core)
}

/// The map waits for orders (the player's cursor, or the computer between
/// two units), no event running: the real-time list may start a script.
fn map_waits(core: &Core) -> bool {
    let s = core.raw_read_16(MAP_STATE, -1);
    (s == STATE_CURSOR || s == STATE_CPU) && core.raw_read_8(MAP_BUSY, -1) == 0 && clock_runs(core)
}

/// Every frame of a DS Campaign session (crate::ds_campaign::tick): the
/// countdown and the charge.
pub fn tick(core: &mut Core) {
    if !on(core) {
        return;
    }
    if clock_runs(core) {
        let clock = core.raw_read_8(CLOCK, -1);
        if clock & CLOCK_RUNNING != 0 {
            let n = core.raw_read_32(COUNTDOWN, -1);
            if n > 1 {
                core.raw_write_32(COUNTDOWN, -1, n - 1);
            } else {
                core.raw_write_32(COUNTDOWN, -1, 0);
                core.raw_write_8(CLOCK, -1, (clock & !CLOCK_RUNNING) | CLOCK_OUT);
            }
            // (`0x020F232C`: while it has hits left, below full)
            let c = charge(core);
            if hits_left(core) > 0 && c < FULL {
                core.raw_write_16(CHARGE, -1, c + 1);
            }
        }
    }
    animate(core);
}

/// The phases' animations, a frame each while the battle shows.
fn animate(core: &mut Core) {
    let p = phase(core);
    let t = core.raw_read_16(ANIM, -1).saturating_add(1);
    core.raw_write_16(ANIM, -1, t);
    if core.raw_read_8(BEAM, -1) != 0 {
        let b = core.raw_read_16(BEAM_TIME, -1) + 1;
        core.raw_write_16(BEAM_TIME, -1, b);
        if b >= BEAM_FRAMES {
            core.raw_write_8(BEAM, -1, 0);
        }
    }
    match p {
        // The laser: the beam, then charging again (Dual Strike's fire task,
        // 0x48 + 0x1E frames, then its hit).
        FIRING if t >= FIRE_FRAMES => set_phase(core, CHARGING),
        // A missile's hit: it rises, explodes; the hit lands (`0x020F20EC`,
        // after 80 frames as Dual Strike's task waits).
        HIT if t == RISE_FRAMES => {
            let h = hits_left(core).saturating_sub(1);
            core.raw_write_8(HITS_LEFT, -1, h);
            if h == 0 {
                set_phase(core, DESTROYED);
            }
        }
        HIT if t >= RISE_FRAMES + BLAST_FRAMES => {
            set_phase(core, CHARGING);
            core.raw_write_8(BUSY, -1, 0);
        }
        DESTROYED if t == FALL_FRAMES / 2 => core.raw_write_8(BUSY, -1, 0),
        DESTROYED if t >= FALL_FRAMES => set_phase(core, GONE),
        _ => {}
    }
}

const FIRE_FRAMES: u16 = 102;
/// The beam on the map: it comes down, burns, narrows away.
const BEAM_FRAMES: u16 = 56;
const RISE_FRAMES: u16 = 80;
const BLAST_FRAMES: u16 = 40;
const FALL_FRAMES: u16 = 300;

/// `0x020F216C`: the warning (the laser charging).
pub fn warn(core: &mut Core) {
    if on(core) {
        set_phase(core, WARNING);
    }
}

/// `0x020F2134`: the laser fires (the charge emptied) and strikes; true
/// when the strike's function was tail-called.
pub fn fire(core: &mut Core) -> bool {
    if !on(core) {
        return false;
    }
    set_phase(core, FIRING);
    core.raw_write_16(CHARGE, -1, 0);
    note_army(core);
    core.gba_mut().cpu_mut().set_thumb_pc(LASER_FN);
    true
}

// --- The real-time list ----------------------------------------------------------------

fn realtime_list(core: &Core) -> u32 {
    crate::ds_campaign::campaign(core)
        .and_then(|c| c.model.built.missions.get(crate::ds_campaign_data::CRYSTAL_CALAMITY))
        .map_or(0, |m| m.realtime)
}

/// The battle map's frame: while the map waits, AW2's list runner tests the
/// real-time list (its scripts start as events), then the frame goes on.
fn map_frame(core: &mut Core) {
    // (back from the list: the callback itself, this time)
    if core.raw_read_8(LIST_RAN, -1) != 0 {
        core.raw_write_8(LIST_RAN, -1, 0);
        return;
    }
    if !on(core) || !map_waits(core) {
        return;
    }
    let list = realtime_list(core);
    if list == 0 {
        return;
    }
    let lr = core.gba().cpu().gpr(14) as u32;
    core.raw_write_32(SAVED_LR, -1, lr);
    core.raw_write_8(LIST_RAN, -1, 1);
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, list as i32);
    cpu.set_gpr(1, 0);
    cpu.set_gpr(2, 0);
    cpu.set_gpr(14, (MAIN_STUB | 1) as i32);
    cpu.set_thumb_pc(RUN_LIST);
}

/// The strike's draw (r0 the proc): the beam on its square (`proc+0x66`,
/// the target unit's index, as AW2's meteor takes it).
fn beam_start(core: &mut Core) {
    let proc = core.gba().cpu().gpr(0) as u32;
    let i = core.raw_read_16(proc + 0x66, -1) as u32;
    if i == 0 {
        return;
    }
    let u = core.raw_read_32(0x0849_9594, -1) + 12 * i;
    core.raw_write_8(BEAM_AT, -1, core.raw_read_8(u + 2, -1));
    core.raw_write_8(BEAM_AT + 1, -1, core.raw_read_8(u + 3, -1));
    core.raw_write_16(BEAM_TIME, -1, 0);
    core.raw_write_8(BEAM, -1, 1);
}

// --- Launch -----------------------------------------------------------------------------

/// Launch chosen (`sub_0802D0A4`, crate::unit_actions): on Crystal
/// Calamity's map a silo fires at the satellite ([`LAUNCH_FN`]). True if
/// it took the command.
pub fn launch(core: &mut Core) -> bool {
    if !on(core) || hits_left(core) == 0 {
        return false;
    }
    let (x, y) = (core.raw_read_16(DESTINATION, -1) as u32, core.raw_read_16(DESTINATION + 2, -1) as u32);
    if tile_at(core, x, y) != SILO_TILE {
        return false;
    }
    core.raw_write_16(SILO, -1, x as u16);
    core.raw_write_16(SILO + 2, -1, y as u16);
    core.gba_mut().cpu_mut().set_thumb_pc(LAUNCH_FN);
    true
}

/// The launch proc's start (r0 the proc): the silo's square for AW2's
/// launch, and the missile on its way (the charge emptied, `0x020F20FC`).
fn launch_setup(core: &mut Core) {
    let proc = core.gba().cpu().gpr(0) as u32;
    let (x, y) = (core.raw_read_16(SILO, -1), core.raw_read_16(SILO + 2, -1));
    core.raw_write_16(proc + 0x64, -1, x);
    core.raw_write_16(proc + 0x66, -1, y);
    if on(core) {
        set_phase(core, HIT);
        core.raw_write_16(CHARGE, -1, 0);
        core.raw_write_8(BUSY, -1, 1);
    }
}

// --- Saved halfway -----------------------------------------------------------------------

/// The state kept with a mission saved halfway (crate::suspend): a mark,
/// hits, phase, clock, the charge.
pub const SAVED_LEN: usize = 6;
const SAVED_MARK: u8 = b'O';

pub fn saved(core: &Core) -> [u8; SAVED_LEN] {
    let mut b = [0u8; SAVED_LEN];
    if core.raw_read_8(ON, -1) == 1 {
        let c = charge(core).to_le_bytes();
        // (a phase in progress is saved as charging: its animation is not)
        let p = match phase(core) {
            DESTROYED | GONE => GONE,
            WARNING => WARNING,
            _ => CHARGING,
        };
        b = [SAVED_MARK, hits_left(core), p, core.raw_read_8(CLOCK, -1), c[0], c[1]];
    }
    b
}

/// A mission saved halfway is continued: its satellite as it was saved.
pub fn restore(core: &mut Core, b: &[u8]) {
    if b.len() < SAVED_LEN || b[0] != SAVED_MARK || !mission_is(core) {
        return;
    }
    install(core);
    for a in STATE..STATE_END {
        core.raw_write_8(a, -1, 0);
    }
    core.raw_write_8(ON, -1, 1);
    core.raw_write_8(HITS_LEFT, -1, b[1].min(HITS));
    core.raw_write_8(PHASE, -1, b[2]);
    core.raw_write_8(CLOCK, -1, b[3]);
    core.raw_write_16(CHARGE, -1, u16::from_le_bytes([b[4], b[5]]).min(FULL));
    note_army(core);
}

// --- The panel ------------------------------------------------------------------------

/// The satellite's picture, small: 32x32, 4bpp in OBJ palette 15's entries
/// [`SAT_FIRST`].. (its top 24 rows the satellite, the bottom 8 left for
/// the beam and the missile).
struct Art {
    /// 32x32 pixels, palette indices (0 transparent).
    pixels: Vec<u8>,
    /// OBJ palette 15 entries 5..15 (BGR555).
    colours: [u16; 11],
}

const PALETTE: u16 = 15;
/// Palette 15's entries the panel uses (entries 1..4 are left as they are).
const FIRST_COLOUR: u8 = 5;
const BLACK: u8 = 5;
const SAT_FIRST: u8 = 6;
const SAT_COLOURS: usize = 6;
const DARK: u8 = 12;
const ORANGE: u8 = 13;
const PINK: u8 = 14;
const WHITE: u8 = 15;
const PAL_BUFFER: u32 = 0x0300_20C0;
const PAL_RAM: u32 = 0x0500_0000;

/// The satellite's sprite, read from the pack: `bmap/085` (256 4bpp tiles),
/// `bmap/086` (its colours), laid out by frame 0 of the animation at ARM9
/// `0x0213B8AC` (four 64x64 pieces round its anchor).
const SAT_TILES: &str = "bmap/085";
const SAT_PALETTE: &str = "bmap/086";
const SAT_ANIM: u32 = 0x0213_B8AC;

static ART: OnceLock<Option<Art>> = OnceLock::new();

fn art() -> Option<&'static Art> {
    ART.get_or_init(|| {
        let pack = crate::ds_pack::pack()?;
        let raw = pack.file(SAT_TILES)?;
        let tiles = crate::ds_art::lz10(raw).unwrap_or_else(|| raw.to_vec());
        let pal = pack.file(SAT_PALETTE)?;
        build_art(pack, &tiles, pal)
    })
    .as_ref()
}

fn bgr(c: u16) -> (i32, i32, i32) {
    ((c & 31) as i32, (c >> 5 & 31) as i32, (c >> 10 & 31) as i32)
}

fn dist(a: u16, b: u16) -> i32 {
    let (x, y) = (bgr(a), bgr(b));
    (x.0 - y.0).pow(2) + (x.1 - y.1).pow(2) + (x.2 - y.2).pow(2)
}

/// Pieces of an animation frame (Dual Strike's format: crate::ds_power_art):
/// (x, y, width, height, first tile).
fn frame_pieces(pack: &crate::ds_pack::Pack, anim: u32, frame: usize) -> Option<Vec<(i32, i32, usize, usize, usize)>> {
    let h = |a: u32| pack.arm9_at(a, 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let table = anim + h(anim)? as u32;
    let at = table + h(table + 2 * frame as u32)? as u32;
    let n = h(at)? as u32;
    let mut out = Vec::new();
    for k in 0..n {
        let (a0, a1, a2) = (h(at + 2 + 6 * k)?, h(at + 4 + 6 * k)?, h(at + 6 + 6 * k)?);
        let shape = (a0 >> 14) as usize;
        let size = (a1 >> 14) as usize;
        const DIMS: [[(usize, usize); 4]; 3] = [
            [(8, 8), (16, 16), (32, 32), (64, 64)],
            [(16, 8), (32, 8), (32, 16), (64, 32)],
            [(8, 16), (8, 32), (16, 32), (32, 64)],
        ];
        let (w, hgt) = *DIMS.get(shape)?.get(size)?;
        let y = (a0 & 0xFF) as i8 as i32;
        let x = ((a1 & 0x1FF) as i32 ^ 0x100) - 0x100;
        out.push((x, y, w, hgt, (a2 & 0x3FF) as usize));
    }
    Some(out)
}

fn build_art(pack: &crate::ds_pack::Pack, tiles: &[u8], pal: &[u8]) -> Option<Art> {
    let colours: Vec<u16> = pal.chunks(2).take(16).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    // The whole picture (palette indices), from its pieces.
    let pieces = frame_pieces(pack, SAT_ANIM, 0)?;
    let (x0, y0) = (pieces.iter().map(|p| p.0).min()?, pieces.iter().map(|p| p.1).min()?);
    let (x1, y1) = (pieces.iter().map(|p| p.0 + p.2 as i32).max()?, pieces.iter().map(|p| p.1 + p.3 as i32).max()?);
    let (w, h) = ((x1 - x0) as usize, (y1 - y0) as usize);
    let mut big = vec![0u8; w * h];
    for &(px, py, pw, ph, first) in &pieces {
        let cols = pw / 8;
        for ty in 0..ph / 8 {
            for tx in 0..cols {
                let t = first + ty * cols + tx;
                for r in 0..8 {
                    for c in 0..8 {
                        let b = *tiles.get(32 * t + 4 * r + c / 2)?;
                        let v = (b >> (4 * (c & 1))) & 15;
                        let (x, y) = ((px - x0) as usize + 8 * tx + c, (py - y0) as usize + 8 * ty + r);
                        if v != 0 {
                            big[y * w + x] = v;
                        }
                    }
                }
            }
        }
    }
    // Its drawn part, made to fit 32x32.
    let (mut bx0, mut by0, mut bx1, mut by1) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if big[y * w + x] != 0 {
                bx0 = bx0.min(x);
                by0 = by0.min(y);
                bx1 = bx1.max(x + 1);
                by1 = by1.max(y + 1);
            }
        }
    }
    if bx1 <= bx0 || by1 <= by0 {
        return None;
    }
    let (sw, sh) = (bx1 - bx0, by1 - by0);
    let scale = (sw as f32 / 32.0).max(sh as f32 / 32.0);
    let (ow, oh) = (((sw as f32 / scale).round() as usize).min(32), ((sh as f32 / scale).round() as usize).min(32));
    let (ox, oy) = ((32 - ow) / 2, (32 - oh) / 2);
    let mut small = vec![0u8; 32 * 32];
    for y in 0..oh {
        for x in 0..ow {
            let (sx0, sx1) = ((x as f32 * scale) as usize, (((x + 1) as f32 * scale) as usize).max((x as f32 * scale) as usize + 1));
            let (sy0, sy1) = ((y as f32 * scale) as usize, (((y + 1) as f32 * scale) as usize).max((y as f32 * scale) as usize + 1));
            let mut count = [0usize; 16];
            let mut n = 0;
            for yy in sy0..sy1.min(sh) {
                for xx in sx0..sx1.min(sw) {
                    count[big[(by0 + yy) * w + bx0 + xx] as usize] += 1;
                    n += 1;
                }
            }
            let opaque: usize = count[1..].iter().sum();
            if opaque * 5 >= n * 2 {
                let v = (1..16).max_by_key(|&i| count[i]).unwrap_or(0);
                small[(oy + y) * 32 + ox + x] = v as u8;
            }
        }
    }
    // Its colours: black, the seven most used, white; the rest to the nearest.
    let mut uses = [0usize; 16];
    for &v in &small {
        uses[v as usize] += 1;
    }
    let black: u16 = 0x0421;
    let white: u16 = 0x7FFF;
    let mut by_use: Vec<usize> = (1..16).filter(|&i| uses[i] > 0).collect();
    by_use.sort_by_key(|&i| std::cmp::Reverse(uses[i]));
    let mut kept: Vec<u16> = by_use.iter().map(|&i| colours.get(i).copied().unwrap_or(0) & 0x7FFF).filter(|&c| dist(c, black) > 12 && dist(c, white) > 12).collect();
    kept.truncate(SAT_COLOURS);
    while kept.len() < SAT_COLOURS {
        kept.push(black);
    }
    let mut out = [0u16; 11];
    out[(BLACK - FIRST_COLOUR) as usize] = black;
    for (k, &c) in kept.iter().enumerate() {
        out[(SAT_FIRST - FIRST_COLOUR) as usize + k] = c;
    }
    out[(DARK - FIRST_COLOUR) as usize] = 0x300C; // (12, 0, 12)
    out[(ORANGE - FIRST_COLOUR) as usize] = 0x029F; // (31, 20, 0)
    out[(PINK - FIRST_COLOUR) as usize] = 0x7E1F; // (31, 16, 31)
    out[(WHITE - FIRST_COLOUR) as usize] = white;
    let remap: Vec<u8> = (0..16)
        .map(|i| {
            if i == 0 {
                return 0;
            }
            let c = colours.get(i).copied().unwrap_or(0) & 0x7FFF;
            (FIRST_COLOUR..=WHITE)
                .filter(|&k| k != ORANGE && k != PINK && k != DARK)
                .min_by_key(|&k| dist(c, out[(k - FIRST_COLOUR) as usize]))
                .unwrap_or(WHITE)
        })
        .collect();
    let pixels = small.iter().map(|&v| remap[v as usize]).collect();
    Some(Art { pixels, colours: out })
}

/// OBJ tiles the panel draws into (free on the battle map: crate::heal_effect's
/// and the two fronts', neither in use on Crystal Calamity while it shows):
/// the satellite's 16 (a 32x32 sprite), then five columns of four (8x32
/// sprites) for the time, the hits and the charge.
const SAT_TILE: u16 = 0x1F9;
const TEXT_TILES: [u16; 5] = [0x2D2, 0x2D6, 0x2E4, 0x2EC, 0x2F4];
const OBJ_TILES: u32 = 0x0601_0000;
const PANEL_W: i32 = 32 + 8 * TEXT_TILES.len() as i32;
const FONT_GLYPHS: u32 = 0x084C_32E4;
const FONT_WIDTHS: u32 = 0x084C_36E4;
const FONT_TOP: usize = 2;
const FONT_ROWS: usize = 14;

fn write_if_changed(core: &mut Core, at: u32, bytes: &[u8]) {
    let mut now = vec![0u8; bytes.len()];
    core.raw_read_range(at, -1, &mut now);
    if now != bytes {
        core.raw_write_range(at, -1, bytes);
    }
}

/// 4bpp tiles of a `w`-pixel-wide bitmap, in 1D order (rows of tiles).
fn to_tiles(px: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut out = vec![0u8; w * h / 2];
    for ty in 0..h / 8 {
        for tx in 0..w / 8 {
            let t = ty * (w / 8) + tx;
            for r in 0..8 {
                for c in 0..8 {
                    let v = px[(8 * ty + r) * w + 8 * tx + c] & 15;
                    out[32 * t + 4 * r + c / 2] |= v << (4 * (c & 1));
                }
            }
        }
    }
    out
}

/// The satellite this frame: its phase's effects drawn in.
fn satellite_now(core: &Core, art: &Art) -> Vec<u8> {
    let mut px = art.pixels.clone();
    let t = core.raw_read_16(ANIM, -1) as i32;
    let put = |px: &mut Vec<u8>, x: i32, y: i32, c: u8| {
        if (0..32).contains(&x) && (0..32).contains(&y) {
            px[(y * 32 + x) as usize] = c;
        }
    };
    match phase(core) {
        // Pink sparks gathering under it.
        WARNING => {
            for k in 0..4 {
                let a = (t / 3 + 8 * k) % 32;
                let x = 16 + [-6, 6, -3, 3][k as usize] * (32 - a) / 32;
                let y = 30 - a / 4;
                put(&mut px, x, y, PINK);
            }
        }
        // The beam, down from its lens.
        FIRING => {
            let width = if t < 10 || t > FIRE_FRAMES as i32 - 20 { 1 } else { 3 };
            for y in 18..32 {
                for dx in 0..width {
                    put(&mut px, 15 + dx - width / 2, y, if dx == width / 2 { WHITE } else { PINK });
                }
            }
        }
        // The missile rises, then explodes on it.
        HIT if t < RISE_FRAMES as i32 => {
            let y = 31 - (t * 20 / RISE_FRAMES as i32);
            put(&mut px, 16, y, WHITE);
            put(&mut px, 16, y + 1, ORANGE);
        }
        HIT => {
            let r = ((t - RISE_FRAMES as i32) / 3).min(7);
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx * dx + dy * dy <= r * r {
                        put(&mut px, 16 + dx, 12 + dy, if 3 * (dx * dx + dy * dy) <= r * r { WHITE } else { ORANGE });
                    }
                }
            }
        }
        // It breaks apart (blinking) and falls away.
        DESTROYED => {
            let fall = (t * t / 2000).min(40);
            let spread = t / 40;
            let shown = t < 60 || (t / 4) % 2 == 0;
            let mut out = vec![0u8; 32 * 32];
            if shown {
                for y in 0..32 {
                    for x in 0..32 {
                        let v = px[(y * 32 + x) as usize];
                        let (nx, ny) = (if x < 16 { x - spread } else { x + spread }, y + fall);
                        if v != 0 && (0..32).contains(&nx) && (0..32).contains(&ny) {
                            out[(ny * 32 + nx) as usize] = v;
                        }
                    }
                }
            }
            px = out;
            for k in 0..6 {
                let (x, y) = (6 + (k * 37 + t) % 21, 5 + (k * 23 + t / 2) % 15 + fall);
                put(&mut px, x, y, if k % 2 == 0 { ORANGE } else { WHITE });
            }
        }
        _ => {}
    }
    px
}

/// The time, the hits and the charge: a 40x32 bitmap in palette 15.
fn text_now(core: &Core) -> Vec<u8> {
    let (w, h) = (8 * TEXT_TILES.len(), 32);
    let mut px = vec![0u8; w * h];
    // The time left (MM:SS, as Dual Strike's top screen: frames / 60).
    let left = core.raw_read_32(COUNTDOWN, -1) / 60;
    let s = format!("{:02}:{:02}", (left / 60).min(99), left % 60);
    let mut on = vec![vec![false; 16]; w];
    let mut x = 1usize;
    for c in s.bytes() {
        let cw = core.raw_read_8(FONT_WIDTHS + c as u32, -1) as usize;
        let at = core.raw_read_32(FONT_GLYPHS + 4 * c as u32, -1);
        if (0x0800_0000..0x0A00_0000).contains(&at) {
            let stride = cw.div_ceil(2);
            for cx in 0..cw {
                for r in 0..FONT_ROWS {
                    let b = core.raw_read_8(at + (stride * (FONT_TOP + r) + cx / 2) as u32, -1);
                    if (b >> (4 * (cx & 1))) & 15 != 0 && x + cx < w {
                        on[x + cx][r + 1] = true;
                    }
                }
            }
        }
        x += cw + 1;
    }
    for xx in 0..w {
        for yy in 0..16 {
            if on[xx][yy] {
                px[yy * w + xx] = WHITE;
            } else if [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)].iter().any(|&(dx, dy)| {
                let (nx, ny) = (xx as i32 + dx, yy as i32 + dy);
                (0..w as i32).contains(&nx) && (0..16).contains(&ny) && on[nx as usize][ny as usize]
            }) {
                px[yy * w + xx] = BLACK;
            }
        }
    }
    // A diamond for each hit still needed (outlined; filled while it is).
    let left = hits_left(core) as usize;
    for k in 0..HITS as usize {
        let cx = 2 + 4 * k as i32;
        for dy in -2i32..=2 {
            for dx in -2i32..=2 {
                let d = dx.abs() + dy.abs();
                let (xx, yy) = (cx + dx, 20 + dy);
                if !(0..w as i32).contains(&xx) {
                    continue;
                }
                let v = if d == 2 { BLACK } else if d < 2 && k < left { WHITE } else if d < 2 { 0 } else { continue };
                if v != 0 {
                    px[yy as usize * w + xx as usize] = v;
                }
            }
        }
    }
    // The laser's charge: a bar, pink at 90% and up (blinking).
    let c = charge(core) as usize;
    let fill = c * (w - 4) / FULL as usize;
    let t = core.raw_read_16(ANIM, -1);
    let warn = 100 * c >= WARN_PERCENT as usize * FULL as usize;
    for yy in 25..30 {
        for xx in 1..w - 1 {
            let edge = yy == 25 || yy == 29 || xx == 1 || xx == w - 2;
            px[yy * w + xx] = if edge {
                BLACK
            } else if xx - 2 < fill {
                if warn && (t / 8) % 2 == 0 { WHITE } else if warn { PINK } else { SAT_FIRST }
            } else {
                BLACK
            };
        }
    }
    px
}

fn shows(core: &Core) -> bool {
    on(core) && core.raw_read_32(MAIN_CALLBACK, -1) == MAP_CALLBACK && !crate::heal_effect::playing(core) && art().is_some()
}

fn panel_shows(core: &Core) -> bool {
    shows(core)
        && phase(core) != GONE
        && core.raw_read_8(CLOCK, -1) & (CLOCK_RUNNING | CLOCK_OUT) != 0
        && !crate::setup_phase::active(core)
}

/// The CO window's side at the map's top (its face, a sprite 32 pixels
/// wide or more in the frame's list, in the top corner), if it shows: 1
/// left, 2 right.
fn co_window_side(core: &Core, start: u32, at: u32) -> Option<u8> {
    let mut p = start;
    while p + 8 <= at {
        let (a0, a1) = (core.raw_read_16(p, -1), core.raw_read_16(p + 2, -1));
        let hidden = a0 & 0x300 == 0x200;
        let (y, x) = ((a0 & 0xFF) as i32, (a1 & 0x1FF) as i32);
        if !hidden && a1 >> 14 >= 2 && a0 >> 14 != 3 && y < 48 && (x < 64 || (176..240).contains(&x)) {
            return Some(if x < 120 { 1 } else { 2 });
        }
        p += 8;
    }
    None
}

/// Where the panel goes: under AW2's CO window, on its side (the side it
/// was last seen on while it is away, a dialogue's for one); on the other
/// side while the cursor is under it.
fn panel_at(core: &mut Core, start: u32, at: u32) -> (i32, i32) {
    let side = match co_window_side(core, start, at) {
        Some(s) if !events_running(core) => {
            if core.raw_read_8(SIDE, -1) != s {
                core.raw_write_8(SIDE, -1, s);
            }
            s
        }
        _ => core.raw_read_8(SIDE, -1),
    };
    let left = (2, PANEL_Y);
    let right = (240 - PANEL_W - 2, PANEL_Y);
    let (mut x, y) = if side == 1 { left } else { right };
    let map = core.raw_read_32(MAP_POINTER, -1);
    let (sx, sy) = (core.raw_read_16(map + 4, -1) as i16 as i32, core.raw_read_16(map + 6, -1) as i16 as i32);
    let (cx, cy) = (core.raw_read_16(CURSOR_X, -1) as i32 * 16 - sx, core.raw_read_16(CURSOR_Y, -1) as i32 * 16 - sy);
    if cx + 16 > x && cx < x + PANEL_W && cy + 16 > y && cy < y + 32 {
        x = if side == 1 { right.0 } else { left.0 };
    }
    (x, y)
}

/// The panel's top: below the CO window (funds, face and power meter).
const PANEL_Y: i32 = 66;

/// The beam's tiles: the meteor's (`0x1CA..`, free while the strike plays,
/// crate::power_anim), a 32x32 piece of beam and the 32x32 glow where it
/// lands.
const BEAM_TILE: u16 = 0x1CA;
const GLOW_TILE: u16 = 0x1DA;

/// The beam's width this frame (its half-width in pixels; 0: none) and its
/// head's height (0 the top of the screen .. 1 down on its square).
fn beam_shape(t: i32) -> (i32, f32) {
    let head = (t as f32 / 8.0).min(1.0);
    let w = match t {
        0..=7 => 2,
        8..=11 => 6,
        12..=39 => 9 + (t / 3) % 2,
        40..=47 => 6,
        48..=55 => 2,
        _ => 0,
    };
    (w, head)
}

fn beam_pixels(half: i32) -> Vec<u8> {
    let mut px = vec![0u8; 32 * 32];
    for y in 0..32 {
        for x in 0..32 {
            let d = (2 * x as i32 - 31).abs();
            px[y * 32 + x as usize] = if d <= half * 2 / 3 {
                WHITE
            } else if d <= half * 2 {
                PINK
            } else if d <= half * 2 + 3 && half > 2 {
                DARK
            } else {
                0
            };
        }
    }
    px
}

fn glow_pixels(t: i32) -> Vec<u8> {
    let r = match t {
        0..=7 => 0,
        8..=39 => 10 + (t / 2) % 3,
        40..=55 => 12 - (t - 40) * 3 / 4,
        _ => 0,
    };
    let mut px = vec![0u8; 32 * 32];
    for y in 0..32i32 {
        for x in 0..32i32 {
            let d2 = (2 * x - 31).pow(2) + (2 * y - 31).pow(2);
            let rr = (2 * r).pow(2);
            px[(y * 32 + x) as usize] = if r > 0 && 4 * d2 <= rr { WHITE } else if r > 0 && d2 <= rr { PINK } else if r > 2 && d2 <= (2 * r + 4).pow(2) { DARK } else { 0 };
        }
    }
    px
}

/// At the sprite flush (crate::branding::flush): the panel, and the
/// laser's beam while it plays.
pub fn flush_sprites(core: &mut Core, start: u32, at: u32, end: u32) -> u32 {
    if !shows(core) {
        return at;
    }
    let Some(art) = art() else { return at };
    let beam = core.raw_read_8(BEAM, -1) != 0;
    let panel = panel_shows(core);
    if !beam && !panel {
        return at;
    }
    let mut at = at;
    // Palette 15's entries 5..15: the panel's colours.
    let colours: Vec<u8> = art.colours.iter().flat_map(|c| c.to_le_bytes()).collect();
    for base in [PAL_BUFFER, PAL_RAM] {
        write_if_changed(core, base + 0x200 + 32 * PALETTE as u32 + 2 * FIRST_COLOUR as u32, &colours);
    }
    let mut sprites: Vec<(i32, i32, u16, u16, u16)> = Vec::new();
    if beam {
        let t = core.raw_read_16(BEAM_TIME, -1) as i32;
        let (half, head) = beam_shape(t);
        write_if_changed(core, OBJ_TILES + 32 * BEAM_TILE as u32, &to_tiles(&beam_pixels(half), 32, 32));
        write_if_changed(core, OBJ_TILES + 32 * GLOW_TILE as u32, &to_tiles(&glow_pixels(t), 32, 32));
        let map = core.raw_read_32(MAP_POINTER, -1);
        let (sx, sy) = (core.raw_read_16(map + 4, -1) as i16 as i32, core.raw_read_16(map + 6, -1) as i16 as i32);
        let (bx, by) = (core.raw_read_8(BEAM_AT, -1) as i32, core.raw_read_8(BEAM_AT + 1, -1) as i32);
        let (cx, bottom) = (16 * bx + 8 - sx, 16 * by + 8 - sy);
        let reach = ((bottom + 32) as f32 * head) as i32 - 32;
        // The glow first (over the beam's foot).
        if head >= 1.0 {
            sprites.push((cx - 16, bottom - 16, GLOW_TILE, 0, 2 << 14));
        }
        let mut y = reach - 32;
        while y > -32 {
            if y < 160 {
                sprites.push((cx - 16, y, BEAM_TILE, 0, 2 << 14));
            }
            y -= 32;
        }
    }
    if panel {
        let sat = to_tiles(&satellite_now(core, art), 32, 32);
        write_if_changed(core, OBJ_TILES + 32 * SAT_TILE as u32, &sat);
        let text = text_now(core);
        let tw = 8 * TEXT_TILES.len();
        for (k, &t) in TEXT_TILES.iter().enumerate() {
            let mut col = vec![0u8; 8 * 32];
            for y in 0..32 {
                col[y * 8..y * 8 + 8].copy_from_slice(&text[y * tw + 8 * k..y * tw + 8 * k + 8]);
            }
            write_if_changed(core, OBJ_TILES + 32 * t as u32, &to_tiles(&col, 8, 32));
        }
        let (x, y) = panel_at(core, start, at);
        // 32x32: square, size 2; 8x32: tall, size 1.
        sprites.push((x, y, SAT_TILE, 0, 2 << 14));
        for (k, &t) in TEXT_TILES.iter().enumerate() {
            sprites.push((x + 32 + 8 * k as i32, y, t, 2 << 14, 1 << 14));
        }
    }
    for (x, y, tile, attr0, attr1) in sprites {
        if at + 8 > end || x <= -32 || x >= 240 || y <= -32 || y >= 160 {
            continue;
        }
        core.raw_write_16(at, -1, (y as u16 & 0xFF) | attr0);
        core.raw_write_16(at + 2, -1, (x as u16 & 0x1FF) | attr1);
        core.raw_write_16(at + 4, -1, tile | PALETTE << 12);
        at += 8;
    }
    at
}

// --- Traps --------------------------------------------------------------------------------

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![(MAP_FRAME, Box::new(map_frame))]
}

/// A magic stub of ours reached crate::ds_campaign's landing (r3 the id,
/// r0 the proc that called it): its work, then back to the proc.
pub fn magic(core: &mut Core, id: u32) {
    match id {
        MAGIC_SETUP => launch_setup(core),
        MAGIC_BEAM => beam_start(core),
        _ => {}
    }
    let cpu = core.gba_mut().cpu_mut();
    let lr = cpu.gpr(14) as u32;
    cpu.set_gpr(0, 0);
    cpu.set_thumb_pc(lr & !1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ram_and_rom_fit() {
        // Past crate::unit_actions' bomb (0x0203FFC0..C7), before
        // crate::pvp's keys (0x0203FFF0).
        assert!(STATE >= 0x0203_FFC8 && STATE_END <= 0x0203_FFF0);
        assert!(LAUNCH_FN >= crate::setup_phase::ROM + 0x400);
        assert!(LAUNCH_SCRIPT + launch_script().len() as u32 <= LASER_SCRIPT);
        assert!(LASER_SCRIPT + 8 * METEOR_COMMANDS <= ROM_SENTINEL);
        assert!(LAUNCH_FN + launch_fn().len() as u32 <= LASER_FN);
        assert!(LASER_FN + laser_fn().len() as u32 <= TARGET_FN);
        assert!(TARGET_FN + target_fn().len() as u32 <= SETUP_FN);
        assert!(SETUP_FN + 16 <= BUSY_FN && BEAM_FN + 16 <= BEAM_WAIT_FN);
        assert!(MAGIC != crate::two_front::MAGIC && MAGIC != crate::setup_phase::MAGIC);
        assert!(BUSY_FN + busy_fn().len() as u32 <= MAIN_STUB);
        assert!(MAIN_STUB + main_stub().len() as u32 <= BEAM_FN);
        assert!(BEAM_WAIT_FN + byte_fn(BEAM).len() as u32 <= LAUNCH_SCRIPT);
        // The countdown's state and ours.
        assert!(COUNTDOWN + 4 <= crate::ds_campaign::FLAGS);
    }

    #[test]
    fn stubs_load_their_literals() {
        // ldr rd, [pc, #imm]: ((at + 4) & !3) + 4 * imm is the literal.
        let lit = |code: &[u8], at: usize| {
            let h = u16::from_le_bytes([code[at], code[at + 1]]);
            assert_eq!(h & 0xF800, 0x4800, "ldr at {at:#x}");
            let a = ((at + 4) & !3) + 4 * (h & 0xFF) as usize;
            u32::from_le_bytes(code[a..a + 4].try_into().unwrap())
        };
        let l = launch_fn();
        assert_eq!(lit(&l, 2), LAUNCH_SCRIPT);
        assert_eq!(lit(&l, 6), PROC_START);
        assert_eq!(lit(&l, 0x10), 0x0801_A169);
        let f = laser_fn();
        assert_eq!(lit(&f, 2), LASER_SCRIPT);
        assert_eq!(lit(&f, 6), PROC_START);
        let t = target_fn();
        assert_eq!(lit(&t, 4), ARMY);
        assert_eq!(lit(&t, 0xA), 0x0805_C291);
        assert_eq!(lit(&t, 0x20), 0x0849_9594);
        assert_eq!(lit(&t, 0x30), 0x0802_9089);
        let b = busy_fn();
        assert_eq!(lit(&b, 0), BUSY);
        let m = main_stub();
        assert_eq!(lit(&m, 0), SAVED_LR);
        assert_eq!(lit(&m, 6), MAP_CALLBACK);
    }

    #[test]
    fn the_satellite_converts() {
        let Some(path) = std::env::var_os("TANGOAW2_DS_ROM") else { return };
        let Ok(rom) = std::fs::read(path) else { return };
        crate::ds_art::offer(&rom);
        let a = art().expect("the satellite's picture");
        assert_eq!(a.pixels.len(), 32 * 32);
        let drawn = a.pixels.iter().filter(|&&v| v != 0).count();
        assert!(drawn > 200 && drawn < 32 * 32, "{drawn} pixels drawn");
        assert!(a.pixels.iter().all(|&v| v == 0 || (FIRST_COLOUR..=WHITE).contains(&v)));
    }
}
