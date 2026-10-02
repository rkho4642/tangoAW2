//! tangoAW2's own battle state in a suspended game (the map menu's Save),
//! with the Dual Strike pack.
//!
//! AW2 saves a game in progress (`sub_08016D30`) as the 0xE28-byte block
//! `CaptureBattleSaveState` (`sub_08016F38`) fills at 0x02000000: day,
//! army, gPlaySt, the weather block, players, units, changed tiles,
//! inventions, up to +0xDAC. The rest of the block is written to Flash but
//! never read back (`sub_08017208`). State tangoAW2 keeps in its own RAM
//! and that lasts past a turn rides there, after a mark:
//!
//! - the Rules' fog flag while rain forces fog on ([`crate::ds_weather`]):
//!   gPlaySt is saved with the forced fog, and a game continued in rain
//!   kept fog on for good once the rain stopped;
//! - Ex Machina's stun ([`crate::co_powers`]): the units it marked were all
//!   free after Continue.
//!
//! (The sandstorm and the map's look are in the weather block, which AW2
//! saves itself; Com Towers are counted on the map.) Without the pack
//! nothing is written: the block is AW2's own, byte for byte.

use mgba::core::Core;

use crate::ds_weather::is_on;

/// The block's staging buffer, its unused tail, and its length.
const BLOCK: u32 = 0x0200_0000;
const TAIL: u32 = BLOCK + 0xDAC;
const BLOCK_LEN: u32 = 0xE28;
/// The tail: +0 [`MARK`], +4 version, +5 the rain's fog flag, +8 the stun.
const MARK: u32 = 0x3257_4154; // "TAW2"
const VERSION: u8 = 1;
const AT_RULE_FOG: u32 = 5;
const AT_STUN: u32 = 8;
const TAIL_LEN: u32 = AT_STUN + crate::co_powers::STUN_STATE_LEN as u32;

/// `sub_08016D30` right after `CaptureBattleSaveState` returns (the block
/// filled, not yet written to Flash).
pub const CAPTURED: u32 = 0x0801_6D88;
/// `sub_08016DB8` (Continue) right after `sub_08017208` returns (the block
/// loaded and applied; a design map's own slot is loaded over the buffer
/// next).
pub const APPLIED: u32 = 0x0801_6DD0;
const CURRENT_ARMY: u32 = 0x0300_33EC;

fn captured(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let mut b = vec![0u8; TAIL_LEN as usize];
    b[0..4].copy_from_slice(&MARK.to_le_bytes());
    b[4] = VERSION;
    b[AT_RULE_FOG as usize] = crate::ds_weather::rule_fog(core);
    b[AT_STUN as usize..].copy_from_slice(&crate::co_powers::stun_state(core));
    core.raw_write_range(TAIL, -1, &b);
}

fn applied(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let mut b = vec![0u8; TAIL_LEN as usize];
    core.raw_read_range(TAIL, -1, &mut b);
    let ours = u32::from_le_bytes(b[0..4].try_into().unwrap()) == MARK && b[4] == VERSION;
    let army = core.raw_read_8(CURRENT_ARMY, -1);
    crate::ds_weather::set_rule_fog(core, if ours { b[AT_RULE_FOG as usize] } else { 0 });
    crate::co_powers::set_stun_state(core, ours.then(|| &b[AT_STUN as usize..]), army);
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![(CAPTURED, Box::new(captured)), (APPLIED, Box::new(applied))]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_fits_the_block() {
        assert!(TAIL + TAIL_LEN <= BLOCK + BLOCK_LEN);
    }
}
