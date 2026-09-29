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
const DATA_MAGIC: u32 = 0x3253_5344; // "DSS2"
/// The map's colours in a sandstorm: the clear set (BG palettes 0-7:
/// terrain, then the same darkened for fog) blown over with sand.
const SAND_PALETTE: u32 = DATA + 0x100;
const CLEAR_PALETTE: u32 = 0x080B_F8C4;
const MAP_COLOURS: u32 = 128;
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

// --- The look ----------------------------------------------------------

/// A colour (BGR555) with sand blown over it: 3/8 of the way to sand.
fn sand_colour(c: u16) -> u16 {
    const SAND: [u16; 3] = [27, 21, 12];
    let mut out = 0;
    for (i, sand) in SAND.iter().enumerate() {
        let v = (c >> (5 * i)) & 31;
        out |= ((v * 5 + sand * 3) / 8) << (5 * i);
    }
    out
}

/// `sub_08035020` (map colours for the weather), the palette in r0 just
/// before it is applied: the sand colours in a sandstorm. Whatever loads
/// the map's colours comes here (map start, a resumed game, the weather
/// change fade), so the colours follow the sandstorm.
const MAP_PALETTE: u32 = 0x0803_503C;
/// Whether the map's colours are the sand ones.
const SAND_SHOWN: u32 = 0x0203_FFA9;
fn map_palette(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let sand = active(core);
    core.raw_write_8(SAND_SHOWN, -1, sand as u8);
    if sand {
        core.gba_mut().cpu_mut().set_gpr(0, SAND_PALETTE as i32);
    }
}

/// `sub_080351F0`, at a turn start, comparing this turn's weather with the
/// next: when only the sandstorm comes or goes, the game's weather change
/// (fade, new colours, sound) is started all the same.
const TURN_WEATHER: u32 = 0x0803_5204;
const START_CHANGE: u32 = 0x0803_5208;
fn turn_weather(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let cpu = core.gba().cpu();
    if cpu.gpr(0) != cpu.gpr(1) {
        return;
    }
    if active(core) != (core.raw_read_8(SAND_SHOWN, -1) != 0) {
        core.gba_mut().cpu_mut().set_thumb_pc(START_CHANGE);
    }
}

/// Blowing sand, drawn like AW2's rain (`sub_080353E8`): 32 grains, as
/// 8x8 sprites on the rain's tiles and in the rain's particle slots, neither of which is used while it is not raining. Their
/// colours go in OBJ palette 7's colours 5-7, which nothing on the map uses.
const EFFECTS: u32 = 0x0803_5514;
const RAIN: u8 = 2;
const PARTICLES: u32 = 0x0202_7DE8;
const PARTICLE: u32 = 0x0C;
const PARTICLE_COUNT: u32 = 32;
/// Points at the map state (scroll x and y at +4 and +6).
const MAP_POINTER: u32 = 0x0849_9590;
const GAME_CLOCK: u32 = 0x0300_4008;
const OAM_NEXT: u32 = 0x0300_141C;
const RAIN_TILES_ROM: u32 = 0x0809_16FC;
const PARTICLE_VRAM: u32 = OBJ_VRAM + 32 * 0x173;
/// Palette 7, priority 0 (in front, as the rain).
const GRAIN_ATTR2: u16 = 0x7173;
const PAL_BUFFER: u32 = 0x0300_20C0;
const PAL_RAM: u32 = 0x0500_0000;
const GRAIN_COLOURS_AT: u32 = 0x200 + 7 * 32 + 2 * 5;
/// Light sand, sand, brown (BGR555).
const GRAIN_COLOURS: [u16; 3] = [0x473D, 0x3299, 0x1D91];

/// Three grains of sand (colour indices: 5 light sand, 6 sand, 7 brown).
const GRAINS: [[&str; 8]; 3] = [
    [
        "........", "....55..", "..556665", "5566677.", "..5566..", "........", "........", "........",
    ],
    [
        "....5...", "........", ".6......", "......5.", "...7....", "........", "..5.....", "........",
    ],
    [
        "........", "........", "...5556.", "55666777", "...6665.", "........", "........", "........",
    ],
];

fn grain_tiles() -> Vec<u8> {
    let mut out = vec![0u8; 32 * GRAINS.len()];
    for (t, tile) in GRAINS.iter().enumerate() {
        for (y, row) in tile.iter().enumerate() {
            for (x, c) in row.bytes().enumerate() {
                let p = if c == b'.' { 0 } else { c - b'0' };
                out[32 * t + 4 * y + x / 2] |= p << (4 * (x % 2));
            }
        }
    }
    out
}

fn effects(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let weather = core.gba().cpu().gpr(0) as u8;
    let grains = grain_tiles();
    let mut now = vec![0u8; grains.len()];
    core.raw_read_range(PARTICLE_VRAM, -1, &mut now);
    if weather == RAIN {
        let mut rain = vec![0u8; grains.len()];
        core.raw_read_range(RAIN_TILES_ROM, -1, &mut rain);
        if now != rain {
            core.raw_write_range(PARTICLE_VRAM, -1, &rain);
        }
        return;
    }
    if !active(core) {
        return;
    }
    if now != grains {
        core.raw_write_range(PARTICLE_VRAM, -1, &grains);
    }
    for base in [PAL_BUFFER, PAL_RAM] {
        for (i, c) in GRAIN_COLOURS.iter().enumerate() {
            let at = base + GRAIN_COLOURS_AT + 2 * i as u32;
            if core.raw_read_16(at, -1) != *c {
                core.raw_write_16(at, -1, *c);
            }
        }
    }
    // Sand blows from the left, fast, with a little rise and fall.
    for i in 0..PARTICLE_COUNT {
        let p = PARTICLES + PARTICLE * i;
        let dx = 0x300 + 0x80 * (i % 4) as u16;
        let dy = (0x20 * (i % 5) as u16).wrapping_sub(0x40);
        let x = core.raw_read_16(p, -1).wrapping_add(dx);
        let y = core.raw_read_16(p + 2, -1).wrapping_add(dy);
        core.raw_write_16(p, -1, x);
        core.raw_write_16(p + 2, -1, y);
    }
    let map = core.raw_read_32(MAP_POINTER, -1);
    let (sx, sy) = (core.raw_read_16(map + 4, -1), core.raw_read_16(map + 6, -1));
    // Half of them each frame, as the rain: the HUD's sprites need the rest
    // of the sprite list.
    let half = core.raw_read_32(GAME_CLOCK, -1) & 1;
    let mut oam = core.raw_read_32(OAM_NEXT, -1);
    for i in (half * 16)..(half * 16 + 16) {
        let p = PARTICLES + PARTICLE * i;
        let x = ((core.raw_read_16(p, -1) >> 8).wrapping_sub(sx)) & 0xFF;
        let y = ((core.raw_read_16(p + 2, -1) >> 8).wrapping_sub(sy)) & 0xFF;
        core.raw_write_16(oam, -1, y);
        core.raw_write_16(oam + 2, -1, x);
        core.raw_write_16(oam + 4, -1, GRAIN_ATTR2 + (i % 3) as u16);
        oam += 8;
    }
    core.raw_write_32(OAM_NEXT, -1, oam);
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
        for i in 0..MAP_COLOURS {
            let c = core.raw_read_16(CLEAR_PALETTE + 2 * i, -1);
            core.raw_write_16(SAND_PALETTE + 2 * i, -1, sand_colour(c));
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
        (MAP_PALETTE, Box::new(map_palette)),
        (TURN_WEATHER, Box::new(turn_weather)),
        (EFFECTS, Box::new(effects)),
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
