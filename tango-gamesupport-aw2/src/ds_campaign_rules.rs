//! Dual Strike's campaign conditions and actions, in Rust: what Dual
//! Strike's scripts call (its own code, in overlay 1 and the ARM9 image) is
//! reached through magic stubs ([`crate::ds_campaign_data::Magic`]) and runs
//! here, on AW2's state.

use mgba::core::Core;

use crate::ds_campaign_data::Magic;

/// AW2's player table pointer (tangoAW2 moves the table in five-army games).
const PLAYERS_PTR: u32 = 0x0849_9598;
const PLAYER: u32 = 0x3C;
const CO: u32 = 0x1D;

fn players(core: &Core) -> u32 {
    core.raw_read_32(PLAYERS_PTR, -1)
}

/// A Dual Strike CO's country, as the CO select screen's tabs number them
/// (0 Orange Star, 1 Blue Moon, 2 Green Earth, 3 Yellow Comet, 4 Black Hole).
pub fn country(ds_co: u8) -> u8 {
    match ds_co {
        1 | 2 | 3 | 5 | 20 | 21 => 0,
        4 | 6 | 17 | 22 => 1,
        9 | 10 | 18 | 23 => 2,
        7 | 8 | 19 | 24 => 3,
        _ => 4,
    }
}

/// Runs a magic function; returns r0.
pub fn run(core: &mut Core, m: &Magic) -> u32 {
    match *m {
        Magic::Predicate(f) => predicate(core, f) as u32,
        Magic::CoPair { army, a, b } => {
            let army = if army == 5 { core.raw_read_16(0x0300_33EC, -1) as u32 } else { army as u32 };
            let co = core.raw_read_8(players(core) + PLAYER * army + CO, -1);
            // AW2 armies have one CO: the tag partner (b) is not checked.
            let _ = b;
            (a == 0xFF || co == a) as u32
        }
        Magic::Call(f, arg) => {
            call(core, f, arg);
            0
        }
        Magic::Countdown(_) | Magic::ArmyFlag { .. } | Magic::Unhandled(_) | Magic::Flow(_) => 0,
    }
}

/// Dual Strike's predicates by address. Unknown ones are false.
fn predicate(core: &mut Core, f: u32) -> bool {
    let _ = core;
    match f {
        _ => false,
    }
}

fn call(core: &mut Core, f: u32, arg: u32) {
    let _ = (core, f, arg);
}
