//! Dual Strike's weather rules, with the Dual Strike pack on.
//!
//! Dual Strike keeps AW2's weathers but changes what they do: snow and rain
//! no longer slow units down; snow doubles the daily fuel burn (except for
//! Olaf's army); rain lowers vision by 1 (AW2 already does) and brings fog
//! of war with it. (Sandstorm, Dual Strike's fourth weather,
//! comes separately.) With the pack off everything is AW2's own.
//!
//! - Movement: each CO's power blocks (CO table [`CO_TABLE`], 0x104 per CO,
//!   blocks at +0x38 + 0x44 * mode) hold three movement-cost chart pointers
//!   indexed by the weather (+0x18: clear, snow, rain). With the pack on,
//!   snow's and rain's point at the block's clear chart.
//! - Fuel: `sub_080253B0` works out a unit's daily burn in r5 (after its
//!   CO's modifier); a trap at [`FUEL`] doubles it in snow.
//! - Fog: while rain is coming the Rules' fog flag is set, and put back when
//!   it stops. The game rebuilds fog from that flag at every turn start
//!   (`sub_080213AC`), so the flag follows the *next* weather (decided at
//!   turn end, [`NEXT_WEATHER`]): fog comes and goes with the rain's first
//!   and last turns. Rain from a CO power brings fog from the next turn.

use mgba::core::Core;
use std::sync::OnceLock;

/// Set every frame to 1 while the Dual Strike features are on, 0 otherwise
/// (for traps, which only see the core). In EWRAM, so saved with the state.
pub const DS_ON: u32 = 0x0203_FFA6;

const CO_TABLE: u32 = 0x085D_3DD0;
const CO_ROW: u32 = 0x104;
const CO_COUNT: u32 = 19;
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

/// The movement pointers, as (address, AW2's word, Dual Strike's word).
static CHARTS: OnceLock<Vec<(u32, u32, u32)>> = OnceLock::new();

fn charts(core: &Core) -> &'static [(u32, u32, u32)] {
    CHARTS.get_or_init(|| {
        let mut out = Vec::new();
        for co in 0..CO_COUNT {
            for mode in 0..3 {
                let block = CO_TABLE + CO_ROW * co + 0x38 + 0x44 * mode;
                let clear = core.raw_read_32(block + 0x18, -1);
                for w in 1..3 {
                    let at = block + 0x18 + 4 * w;
                    out.push((at, core.raw_read_32(at, -1), clear));
                }
            }
        }
        out
    })
}

pub fn is_on(core: &Core) -> bool {
    core.raw_read_8(DS_ON, -1) != 0
}

/// Every frame, before the game runs.
pub fn tick(core: &mut Core, on: bool) {
    core.raw_write_8(DS_ON, -1, on as u8);
    if on || CHARTS.get().is_some() {
        for &(at, aw2, ds) in charts(core) {
            let want = if on { ds } else { aw2 };
            if core.raw_read_32(at, -1) != want {
                core.raw_write_32(at, -1, want);
            }
        }
    }
    let forced = core.raw_read_8(RULE_FOG, -1);
    if on && core.raw_read_8(NEXT_WEATHER, -1) == RAIN {
        if forced == 0 {
            let fog = core.raw_read_8(FOG, -1);
            core.raw_write_8(RULE_FOG, -1, 1 + fog);
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
    if !is_on(core) || core.raw_read_8(WEATHER, -1) != SNOW {
        return;
    }
    let army = core.raw_read_16(CURRENT_ARMY, -1) as u32;
    let co = core.raw_read_8(crate::five::players(core) + 0x3C * army + 0x1D, -1);
    if co == OLAF {
        return;
    }
    let burn = core.gba().cpu().gpr(5) as u8 as i8 as i32;
    core.gba_mut().cpu_mut().set_gpr(5, (burn * 2).clamp(0, 127));
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![(FUEL, Box::new(fuel))]
}
