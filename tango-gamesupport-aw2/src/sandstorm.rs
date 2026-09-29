//! Sandstorm, Dual Strike's fourth weather, with the Dual Strike pack on.
//!
//! In a sandstorm every unit that fires from a distance loses 1 of its
//! maximum range (never below its minimum), except under Max, Grit and
//! Jugger, as in Dual Strike.
//!
//! To AW2's own weather code a sandstorm is clear weather, so nothing that
//! indexes a table by the weather (palettes, battle backgrounds, sounds,
//! movement charts) ever sees a fourth value. The sandstorm is kept beside
//! it:
//! - Chosen on the Rules screen (a fifth Weather choice), it is stored in
//!   gPlaySt as fixed weather (mode 3) with clear as the default, a
//!   combination AW2 never makes. It is saved and sent wherever the rules
//!   are, and a CO power's snow or rain gives way to it again after a day.
//! - With Random weather, each turn end can also bring a one-day sandstorm
//!   (the same chance as snow), kept in [`STATE`], a byte of the weather
//!   block AW2 never uses and saves with a suspended game. Its day is
//!   counted by AW2's own counter (`sub_08035080`).

use mgba::core::Core;

use crate::ds_weather::{is_on, WEATHER};

/// gPlaySt's weather mode (0 fixed clear, 1 random, 2 permanent, 3 fixed)
/// and default weather.
const MODE: u32 = 0x0300_3FED;
const DEFAULT: u32 = 0x0300_3FEF;
const FIXED: u8 = 3;

/// AW2's weather block (`gUnknown_03004490`): [0] a change marker, [1] the
/// rain chance, [2] the snow chance, [4 + army] day counters. [3] is ours:
/// [`ONE_DAY`] while a one-day sandstorm lasts.
const WEATHER_BLOCK: u32 = 0x0300_4490;
pub const STATE: u32 = WEATHER_BLOCK + 3;
const ONE_DAY: u8 = 2;
const RNG: u32 = 0x0300_1FD4;

const UNITS: u32 = 0x085D_5ABC;
const UNIT_RECORD: u32 = 0x5C;
/// COs whose units keep their range: Max, Grit (and Jugger, when added).
const EXEMPT_COS: [u8; 2] = [2, 5];

/// Whether a sandstorm is blowing now.
pub fn active(core: &Core) -> bool {
    is_on(core)
        && core.raw_read_8(WEATHER, -1) == 0
        && (fixed(core) || core.raw_read_8(STATE, -1) == ONE_DAY)
}

fn fixed(core: &Core) -> bool {
    core.raw_read_8(MODE, -1) == FIXED && core.raw_read_8(DEFAULT, -1) == 0
}

// --- Range -------------------------------------------------------------

/// `GetUnitFiringRangeWithCoBonus` (army r5, unit type r6) has the maximum
/// range, CO bonus included, in r4 here.
const RANGE: u32 = 0x0804_2D76;
fn range(core: &mut Core) {
    if !active(core) {
        return;
    }
    let cpu = core.gba().cpu();
    let (max, army, unit) = (cpu.gpr(4), cpu.gpr(5) as u32, cpu.gpr(6) as u32);
    if max < 2 {
        return;
    }
    let co = core.raw_read_8(crate::five::players(core) + 0x3C * army + 0x1D, -1);
    if EXEMPT_COS.contains(&co) {
        return;
    }
    let min = core.raw_read_8(UNITS + UNIT_RECORD * (unit & 0xFF) + 0x0E, -1) as i32;
    core.gba_mut().cpu_mut().set_gpr(4, (max - 1).max(min));
}

// --- Turn end ----------------------------------------------------------

/// `sub_08035170` (the next turn's weather, at turn end), where it is about
/// to draw snow or rain: clear weather, Random mode.
const DRAW: u32 = 0x0803_5196;
/// Its "a day has passed?" call, and just after it returns.
const DAY_CHECK: u32 = 0x0803_51D8;
const DAY_CHECKED: u32 = 0x0803_51DC;
/// Its return, with the weather in r4.
const DONE: u32 = 0x0803_51E8;

fn next_random(core: &mut Core) -> u32 {
    let s = core.raw_read_32(RNG, -1).wrapping_mul(4);
    let r = s.wrapping_add(2).wrapping_mul(s.wrapping_add(3)) >> 2;
    core.raw_write_32(RNG, -1, r);
    r
}

fn draw(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    if core.raw_read_8(STATE, -1) == ONE_DAY {
        // Count the day as AW2 does for its own weather; clear meanwhile.
        let cpu = core.gba_mut().cpu_mut();
        cpu.set_gpr(4, 0);
        cpu.set_thumb_pc(DAY_CHECK);
        return;
    }
    // Sandstorm is drawn first, with snow's chance (`sub_080129F8`).
    let chance = core.raw_read_8(WEATHER_BLOCK + 2, -1) as u32;
    if next_random(core) % 10000 < chance * 100 {
        core.raw_write_8(STATE, -1, ONE_DAY);
        // A new weather's day starts now (`sub_080350E4`).
        for army in 1..=4 {
            core.raw_write_8(WEATHER_BLOCK + 3 + army, -1, 0);
        }
        core.raw_write_8(WEATHER_BLOCK, -1, 0x1E);
        let cpu = core.gba_mut().cpu_mut();
        cpu.set_gpr(4, 0);
        cpu.set_thumb_pc(DONE);
    }
}

fn day_checked(core: &mut Core) {
    if is_on(core)
        && core.raw_read_8(STATE, -1) == ONE_DAY
        && core.raw_read_8(WEATHER, -1) == 0
        && core.gba().cpu().gpr(0) & 0xFF != 0
    {
        core.raw_write_8(STATE, -1, 0);
    }
}

/// `CalcRandomWeatherChances`, at every map start: no sandstorm carries
/// over from another map.
const MAP_START: u32 = 0x0803_5490;
fn map_start(core: &mut Core) {
    if core.raw_read_8(STATE, -1) != 0 {
        core.raw_write_8(STATE, -1, 0);
    }
}

// --- Rules screen --------------------------------------------------------

/// The Weather rule's number of choices (Random, Clear, Rain, Snow; with
/// the pack also Sandstorm).
const WEATHER_CHOICES: u32 = 0x0858_09A6;
const SANDSTORM_CHOICE: u32 = 4;

/// Rules → gPlaySt (`sub_0803C1D4`), the choice in r3; the fixed-weather
/// arm stores mode 3 and r3 as the weather.
const TO_PLAY: u32 = 0x0803_C26A;
const TO_PLAY_FIXED: u32 = 0x0803_C296;
fn to_play(core: &mut Core) {
    let cpu = core.gba_mut().cpu_mut();
    if cpu.gpr(3) as u32 == SANDSTORM_CHOICE {
        cpu.set_gpr(3, 0);
        cpu.set_thumb_pc(TO_PLAY_FIXED);
    }
}

/// gPlaySt → Rules (`sub_0803BFBC`), the choice in r0.
const TO_RULES: u32 = 0x0803_C034;
fn to_rules(core: &mut Core) {
    if is_on(core) && fixed(core) {
        core.gba_mut().cpu_mut().set_gpr(0, SANDSTORM_CHOICE as i32);
    }
}

/// The Weather choice's word (`sub_08064774`, the rule object in r4, the
/// word's sprite id in r0, x in r1), just before it is drawn. "Sandstorm"
/// is drawn with Random's id over Random's tiles and the next two (Snow's,
/// never on screen at the same time), with a 48-pixel sprite pair in place
/// of the 32-pixel one Random's id uses, 8 pixels further left.
const WORD: u32 = 0x0806_47A6;
const WORD_DRAWN: u32 = 0x0806_47AA;
const RANDOM_WORD: i32 = 0xC9;
/// The sprite pointer Random's id (and other words') uses.
const WORD_SPRITE: u32 = 0x0848_BB38;
/// Random's and Snow's tiles in ROM (as loaded into VRAM).
const WORD_TILES_ROM: u32 = 0x0810_0704;
/// The word sprite group (`0x0200F920` + 0x88 * 5): count at +5, then
/// (tile, id) pairs from +8.
const WORD_GROUP: u32 = 0x0200_FBC8;
const OBJ_VRAM: u32 = 0x0601_0000;
/// Set while the word's tiles are in VRAM (so only tiles this module
/// wrote are ever put back).
const WORD_SHOWN: u32 = 0x0203_FFA8;

/// tangoAW2's Sandstorm data in the ROM image's free space.
const DATA: u32 = 0x0867_0000;
const SPRITE: u32 = DATA;
const DATA_SENTINEL: u32 = DATA + 0xFC;
const DATA_MAGIC: u32 = 0x3153_5344; // "DSS1"
/// Two wide sprites: 32x8 at x 0 (tiles 0-3), 16x8 at x 32 (tiles 4-5).
const SPRITE_DEF: [u16; 7] = [2, 0x4000, 0x4000, 0x0000, 0x4000, 0x0020, 0x0004];

/// "Sandstorm" in the Rules screen's word font (' ' clear, '.' fill, 'c'
/// shade, 'e' outline), 48 x 8.
const SANDSTORM_WORD: [&str; 8] = [
    "    eeeeee          eee                         ",
    "   ec....e          e.e    eee                  ",
    "   e..eeeeeeeeeeee ee.eeeeee.eeeeeeeeeeeeeee    ",
    "   e.....ec..ee..cec..ec..e...ec.ce..ce..c.ce   ",
    "   eeee..e.e.ee.e.e.e.e.ceee.ee.e.e.e.e.e.e.e   ",
    "   eeee..e.e.ee.e.e.e.eec.ee.ee.e.e.eee.e.e.e   ",
    "   e....cec.c.e.e.ec..e..cee..ec.ce.eee.e.e.e   ",
    "   eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee   ",
];
const WORD_TILES: usize = 6;

/// The word as 4bpp tiles.
fn word_tiles() -> Vec<u8> {
    let mut out = vec![0u8; 32 * WORD_TILES];
    for (y, row) in SANDSTORM_WORD.iter().enumerate() {
        for (x, c) in row.bytes().enumerate() {
            let p = match c {
                b'.' => 0x1,
                b'c' => 0xD,
                b'e' => 0xF,
                _ => 0,
            };
            out[32 * (x / 8) + 4 * y + (x % 8) / 2] |= p << (4 * (x % 2));
        }
    }
    out
}

fn word_tile_base(core: &Core) -> Option<u32> {
    let count = core.raw_read_8(WORD_GROUP + 5, -1) as u32;
    (0..count.min(32)).find_map(|i| {
        let e = WORD_GROUP + 8 + 4 * i;
        (core.raw_read_16(e + 2, -1) as i32 == RANDOM_WORD).then(|| core.raw_read_16(e, -1) as u32)
    })
}

fn word(core: &mut Core) {
    let Some(base) = word_tile_base(core) else {
        return;
    };
    let vram = OBJ_VRAM + 32 * base;
    let value = core.raw_read_8(core.gba().cpu().gpr(4) as u32 + 0x48, -1) as u32;
    if is_on(core) && value == SANDSTORM_CHOICE {
        core.raw_write_range(vram, -1, &word_tiles());
        core.raw_write_8(WORD_SHOWN, -1, 1);
        let def = core.raw_read_32(WORD_SPRITE, -1);
        core.raw_write_32(SPRITE + 0x80, -1, def);
        core.raw_write_32(WORD_SPRITE, -1, SPRITE);
        let cpu = core.gba_mut().cpu_mut();
        let x = cpu.gpr(1);
        cpu.set_gpr(0, RANDOM_WORD);
        cpu.set_gpr(1, (x - 8) & 0x1FF);
        return;
    }
    // Any other choice: Random's and Snow's own tiles back.
    if core.raw_read_8(WORD_SHOWN, -1) != 0 {
        let mut rom = vec![0u8; 32 * WORD_TILES];
        core.raw_read_range(WORD_TILES_ROM, -1, &mut rom);
        core.raw_write_range(vram, -1, &rom);
        core.raw_write_8(WORD_SHOWN, -1, 0);
    }
}

fn word_drawn(core: &mut Core) {
    if core.raw_read_32(WORD_SPRITE, -1) == SPRITE {
        let def = core.raw_read_32(SPRITE + 0x80, -1);
        core.raw_write_32(WORD_SPRITE, -1, def);
    }
}

/// Every frame, before the game runs.
pub fn tick(core: &mut Core, on: bool) {
    let choices = if on { 5 } else { 4 };
    if core.raw_read_16(WEATHER_CHOICES, -1) != choices {
        core.raw_write_16(WEATHER_CHOICES, -1, choices);
    }
    if on && core.raw_read_32(DATA_SENTINEL, -1) != DATA_MAGIC {
        for (i, h) in SPRITE_DEF.iter().enumerate() {
            core.raw_write_16(SPRITE + 2 * i as u32, -1, *h);
        }
        core.raw_write_32(DATA_SENTINEL, -1, DATA_MAGIC);
    }
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![
        (RANGE, Box::new(range)),
        (DRAW, Box::new(draw)),
        (DAY_CHECKED, Box::new(day_checked)),
        (MAP_START, Box::new(map_start)),
        (TO_PLAY, Box::new(|core: &mut Core| {
            if is_on(core) {
                to_play(core)
            }
        })),
        (TO_RULES, Box::new(to_rules)),
        (WORD, Box::new(word)),
        (WORD_DRAWN, Box::new(word_drawn)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_word_fills_six_tiles() {
        assert!(SANDSTORM_WORD.iter().all(|r| r.len() == 8 * WORD_TILES));
        let t = word_tiles();
        // Row 0 of tile 0: pixels 4..7 are outline ("    eeee").
        assert_eq!(&t[0..4], &[0x00, 0x00, 0xFF, 0xFF]);
    }
}
