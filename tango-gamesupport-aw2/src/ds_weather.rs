//! Dual Strike's weather rules, with the Dual Strike pack on.
//!
//! Dual Strike keeps AW2's weathers but changes what they do: snow and rain
//! no longer slow units down; snow doubles the daily fuel burn (except for
//! Olaf's army); rain lowers vision by 1 (AW2 already does) and brings fog
//! of war with it. (Sandstorm, Dual Strike's fourth weather, is
//! [`crate::sandstorm`].) With the pack off everything is AW2's own.
//!
//! - Movement: snow's and rain's movement charts are clear's with the pack
//!   on ([`crate::roster`], which also gives the charts the new units' rows).
//! - Fuel: `sub_080253B0` works out a unit's daily burn in r5 (after its
//!   CO's modifier); a trap at [`FUEL`] doubles it in snow.
//! - Fog: while rain is coming the Rules' fog flag is set, and put back when
//!   it stops. The game rebuilds fog from that flag at every turn start
//!   (`sub_080213AC`), so the flag follows the *next* weather (decided at
//!   turn end, [`NEXT_WEATHER`]): fog comes and goes with the rain's first
//!   and last turns. Rain from a CO power brings fog from the next turn.

use mgba::core::Core;

/// Set every frame to 1 while the Dual Strike features are on, 0 otherwise
/// (for traps, which only see the core). In EWRAM, so saved with the state.
pub const DS_ON: u32 = 0x0203_FFA6;

const OLAF: u8 = 3;
/// The army whose turn it is (1..5).
const CURRENT_ARMY: u32 = 0x0300_33EC;

/// gPlaySt: the weather (0 clear, 1 snow, 2 rain) and the fog flag.
pub const WEATHER: u32 = 0x0300_3FEC;
/// The weather for the next turn, decided at turn end.
pub const NEXT_WEATHER: u32 = 0x0300_3FEE;
const FOG: u32 = 0x0300_3FCD;
const SNOW: u8 = 1;
const RAIN: u8 = 2;
/// The Rules' fog flag while rain has forced fog on (1 + flag; 0 = not
/// forced).
const RULE_FOG: u32 = 0x0203_FFA7;

/// The Rules' fog flag kept while rain forces fog on (1 + flag; 0 when
/// not forced), for a suspended game ([`crate::suspend`]): gPlaySt is
/// saved with the forced fog, so without it a game continued in rain
/// would keep fog on for good once the rain stops.
pub fn rule_fog(core: &Core) -> u8 {
    core.raw_read_8(RULE_FOG, -1) & !IN_BATTLE
}

/// [`RULE_FOG`]'s mark: set in the battle it was made in.
const IN_BATTLE: u8 = 0x80;

pub fn set_rule_fog(core: &mut Core, v: u8) {
    core.raw_write_8(RULE_FOG, -1, v);
}

pub fn is_on(core: &Core) -> bool {
    core.raw_read_8(DS_ON, -1) != 0
}

/// Every frame, before the game runs.
pub fn tick(core: &mut Core, on: bool) {
    core.raw_write_8(DS_ON, -1, on as u8);
    crate::sandstorm::tick(core, on);
    crate::wasteland::tick(core, on);
    crate::com_tower::tick(core, on);
    let raw = core.raw_read_8(RULE_FOG, -1);
    let forced = raw & !IN_BATTLE;
    // A battle left with the memory set drops it (it would else put that
    // battle's fog choice over the next one's); one restored by a Continue
    // is kept until its battle is on ([`IN_BATTLE`] not yet set).
    if !crate::ds_campaign::in_battle(core) {
        if raw & IN_BATTLE != 0 {
            core.raw_write_8(RULE_FOG, -1, 0);
        }
        return;
    }
    if forced != 0 && raw & IN_BATTLE == 0 {
        core.raw_write_8(RULE_FOG, -1, forced | IN_BATTLE);
    }
    if on && core.raw_read_8(NEXT_WEATHER, -1) == RAIN {
        if forced == 0 {
            let fog = core.raw_read_8(FOG, -1);
            core.raw_write_8(RULE_FOG, -1, (1 + fog) | IN_BATTLE);
        }
        core.raw_write_8(FOG, -1, 1);
    } else if forced != 0 {
        core.raw_write_8(FOG, -1, forced - 1);
        core.raw_write_8(RULE_FOG, -1, 0);
    }
}

/// The daily fuel burn is in r5 here (a signed byte, after the CO's
/// modifier; `sub_080253B0`).
pub const FUEL: u32 = 0x0802_5434;
pub fn fuel(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let army = core.raw_read_16(CURRENT_ARMY, -1) as u32;
    let unit = core.gba().cpu().gpr(4) as u32;
    let mut burn = core.gba().cpu().gpr(5) as u8 as i8 as i32;
    let start = burn;
    // A dived Sub's or hidden Stealth's burn less Sneaky / Stealthy
    // (crate::co_skills), before the snow doubles it.
    if (0x0200_0000..0x0204_0000).contains(&unit) && core.raw_read_8(unit + 1, -1) & 0x20 != 0 {
        burn = (burn - crate::co_skills::hidden_fuel_cut(core, army)).max(0);
    }
    if core.raw_read_8(WEATHER, -1) == SNOW {
        let co = core.raw_read_8(crate::five::players(core) + 0x3C * army + 0x1D, -1);
        if co != OLAF {
            burn *= 2;
        }
    }
    if burn != start {
        core.gba_mut().cpu_mut().set_gpr(5, burn.clamp(0, 127));
    }
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![(FUEL, Box::new(fuel))]
}
