//! Dual Strike's CO skills (docs/AW2.md "CO skills"): the skills each army
//! has on in a battle, and what they do.
//!
//! Dual Strike's 43 player skills are ids 0x20..0x4A in its skill table
//! (overlay 0 `0x022F5ECC`: rank, name, description); its three tag skills
//! (0x35..0x37) need tag pairs and are left out. Each army's skills in the
//! battle being played are a bitmap in [`ACTIVE`] (bit `id - 0x20`), set when
//! the battle starts from the mode's rules (the DS Campaign, Survival and the
//! War Room as Dual Strike has them, AW2's campaign, Versus with its Skills
//! rule); everything here reads only that bitmap, so a battle with no skills
//! on (the default everywhere AW2 has no skills) plays as it always did.
//!
//! The effects, as Dual Strike's code has them (its functions in brackets):
//! - attack (`0x020E646C`, `0x020E68D0`, `0x020E6AE4`): Bruiser/Brawler +5/+8
//!   for direct units, Sharpshooter/Sniper +5/+8 for indirect ones; Road
//!   Rage, Ranger, Urban Fighter, Mountaineer, Seamanship +10 on road, wood,
//!   city, mountain, sea; Backstab +15 for a dived Sub or hidden Stealth;
//!   High and Dry, Icebreaker, Sand Scorpion +20 in rain, snow, sandstorm.
//! - defence (`0x020E61D8`, `0x020E63B0`): Slam Guard/Shield +8/+12 against
//!   direct attacks, Snipe Guard/Shield +8/+12 against indirect ones, APC
//!   Guard +10 for transports.
//! - funds and repairs: Gold Rush +100 a day per property that earns
//!   (`0x020E0158`); Mechanic/Gear Head repair 1/2 HP more (`0x020E5EC4`);
//!   Combat Pay, 2% of the value of the HP its attacks take (`0x020C86CC`).
//! - Missile Guard: a silo's blast (and Victory or Death!'s Black Arc) takes
//!   10 HP points less (`0x020E6874`).
//!
//! Everything lives in emulated RAM, the same on both netplay peers.

use mgba::core::Core;

pub const FIRST: u8 = 0x20;
pub const LAST: u8 = 0x4A;

pub const BRUISER: u8 = 0x20;
pub const BRAWLER: u8 = 0x21;
pub const SHARPSHOOTER: u8 = 0x22;
pub const SNIPER: u8 = 0x23;
pub const APC_BOOST: u8 = 0x24;
pub const SLAM_GUARD: u8 = 0x25;
pub const SLAM_SHIELD: u8 = 0x26;
pub const SNIPE_GUARD: u8 = 0x27;
pub const SNIPE_SHIELD: u8 = 0x28;
pub const APC_GUARD: u8 = 0x29;
pub const MISSILE_GUARD: u8 = 0x2A;
pub const CANNON_GUARD: u8 = 0x2B;
pub const ROAD_RAGE: u8 = 0x2C;
pub const RANGER: u8 = 0x2D;
pub const URBAN_FIGHTER: u8 = 0x2E;
pub const MOUNTAINEER: u8 = 0x2F;
pub const SEAMANSHIP: u8 = 0x30;
pub const BACKSTAB: u8 = 0x31;
pub const HIGH_AND_DRY: u8 = 0x32;
pub const ICEBREAKER: u8 = 0x33;
pub const SAND_SCORPION: u8 = 0x34;
/// Tag skills (0x35..0x37): left out.
pub const TAG_SKILLS: [u8; 3] = [0x35, 0x36, 0x37];
pub const MECHANIC: u8 = 0x3D;
pub const GEAR_HEAD: u8 = 0x3E;
pub const COMBAT_PAY: u8 = 0x45;
pub const GOLD_RUSH: u8 = 0x46;

/// The skills on in this battle: per army 1..5, a 6-byte bitmap (bit `id -
/// FIRST`). EWRAM the game never touches (after crate::power_anim's state).
pub const ACTIVE: u32 = 0x0203_F7E0;
const ACTIVE_LEN: u32 = 6;

/// Army `army` has skill `id` on in this battle.
pub fn has(core: &Core, army: u32, id: u8) -> bool {
    if !(1..=5).contains(&army) || !(FIRST..=LAST).contains(&id) {
        return false;
    }
    let bit = (id - FIRST) as u32;
    core.raw_read_8(ACTIVE + ACTIVE_LEN * (army - 1) + bit / 8, -1) & (1 << (bit % 8)) != 0
}

/// Sets army `army`'s skills for the battle (ids outside 0x20..0x4A and the
/// tag skills are ignored).
pub fn set(core: &mut Core, army: u32, ids: &[u8]) {
    if !(1..=5).contains(&army) {
        return;
    }
    let mut b = [0u8; ACTIVE_LEN as usize];
    for &id in ids {
        if (FIRST..=LAST).contains(&id) && !TAG_SKILLS.contains(&id) {
            let bit = (id - FIRST) as usize;
            b[bit / 8] |= 1 << (bit % 8);
        }
    }
    core.raw_write_range(ACTIVE + ACTIVE_LEN * (army - 1), -1, &b);
}

/// No army has a skill on (every battle's start until a mode sets them).
pub fn clear(core: &mut Core) {
    core.raw_write_range(ACTIVE, -1, &[0u8; (ACTIVE_LEN * 5) as usize]);
}

// --- Unit classes (Dual Strike's, `0x020DF7AC`) ---------------------------------

/// Indirect units: Artillery, Rockets, Missiles, Piperunner, Battleship,
/// Carrier.
const INDIRECT: [u8; 6] = [10, 11, 15, 9, 21, crate::roster::CARRIER];
/// Units with no weapon: APC, T Copter, Lander, Black Boat.
const TRANSPORT: [u8; 4] = [7, 20, 23, crate::roster::BLACK_BOAT];
/// Black Bomb, Oozium: neither.
const OTHER: [u8; 2] = [crate::roster::BLACK_BOMB, crate::roster::OOZIUM];

pub fn is_indirect(t: u8) -> bool {
    INDIRECT.contains(&t)
}
pub fn is_transport(t: u8) -> bool {
    TRANSPORT.contains(&t)
}
/// A direct unit: any other with a weapon (a Sub and a Stealth, dived or
/// hidden too).
pub fn is_direct(t: u8) -> bool {
    t != 0 && !is_indirect(t) && !is_transport(t) && !OTHER.contains(&t)
}
/// A unit's flags: dived (a Sub) or hidden (a Stealth).
const DIVED: u8 = 0x20;

// Terrain classes (the map's class & 0x1F).
const MOUNTAIN: u8 = 3;
const WOOD: u8 = 4;
const ROAD: u8 = 5;
const CITY: u8 = 6;
const SEA: u8 = 7;

const WEATHER: u32 = 0x0300_3FEC;
const SNOW: u8 = 1;
const RAIN: u8 = 2;

/// The firepower army `army`'s unit of type `t` (with unit flags `flags`)
/// on terrain class `terrain` gets from skills (percentage points, as
/// AW2's firepower bonus).
pub fn firepower(core: &Core, army: u32, t: u8, flags: u8, terrain: u8) -> i32 {
    let on = |id| has(core, army, id);
    let mut v = 0;
    if is_direct(t) {
        v += if on(BRUISER) { 5 } else { 0 } + if on(BRAWLER) { 8 } else { 0 };
    }
    if is_indirect(t) {
        v += if on(SHARPSHOOTER) { 5 } else { 0 } + if on(SNIPER) { 8 } else { 0 };
    }
    for (id, class) in [(ROAD_RAGE, ROAD), (RANGER, WOOD), (URBAN_FIGHTER, CITY), (MOUNTAINEER, MOUNTAIN), (SEAMANSHIP, SEA)] {
        if on(id) && terrain & 0x1F == class {
            v += 10;
        }
    }
    if on(BACKSTAB) && flags & DIVED != 0 {
        v += 15;
    }
    let weather = core.raw_read_8(WEATHER, -1);
    let sandstorm = crate::sandstorm::active(core);
    if (on(HIGH_AND_DRY) && weather == RAIN) || (on(ICEBREAKER) && weather == SNOW) || (on(SAND_SCORPION) && sandstorm) {
        v += 20;
    }
    v
}

/// The defence army `army`'s unit of type `t` gets from skills against an
/// attack from `distance` squares (1: a direct attack).
pub fn defence(core: &Core, army: u32, t: u8, distance: u8) -> i32 {
    let on = |id| has(core, army, id);
    let mut v = 0;
    if distance <= 1 {
        v += if on(SLAM_GUARD) { 8 } else { 0 } + if on(SLAM_SHIELD) { 12 } else { 0 };
    } else {
        v += if on(SNIPE_GUARD) { 8 } else { 0 } + if on(SNIPE_SHIELD) { 12 } else { 0 };
    }
    if on(APC_GUARD) && is_transport(t) {
        v += 10;
    }
    v
}

/// HP (display) a property repairs on top of AW2's 2.
pub fn repair_bonus(core: &Core, army: u32) -> i32 {
    (if has(core, army, MECHANIC) { 1 } else { 0 }) + (if has(core, army, GEAR_HEAD) { 2 } else { 0 })
}

/// Funds a day on top of the income: Gold Rush's 100 per property that earns.
pub fn income_bonus(core: &Core, army: u32, earners: i32) -> i32 {
    if has(core, army, GOLD_RUSH) { 100 * earners } else { 0 }
}

/// Combat Pay: funds for taking `bars` HP (display) off a unit costing
/// `cost`: 2% of that HP's value.
pub fn combat_pay(core: &Core, army: u32, bars: u32, cost: u32) -> u32 {
    if has(core, army, COMBAT_PAY) { bars * cost / 10 * 2 / 100 } else { 0 }
}

/// HP points (of 100) a silo's blast takes off army `army`'s unit: Missile
/// Guard takes 10 off `hit`.
pub fn blast(core: &Core, army: u32, hit: i32) -> i32 {
    if has(core, army, MISSILE_GUARD) { (hit - 10).max(0) } else { hit }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ram_fits() {
        assert!(ACTIVE >= 0x0203_F7E0 && ACTIVE + ACTIVE_LEN * 5 <= 0x0203_F800, "between crate::power_anim's and crate::ds_battle's state");
        assert!((LAST - FIRST) as u32 / 8 < ACTIVE_LEN);
    }

    #[test]
    fn classes() {
        for t in 1..=27u8 {
            let n = [is_direct(t), is_indirect(t), is_transport(t)].iter().filter(|&&b| b).count();
            assert!(n <= 1, "type {t} in one class at most");
        }
        assert!(is_direct(1) && is_direct(24) && is_direct(12) && is_indirect(10) && is_transport(7));
        assert!(!is_direct(crate::roster::OOZIUM) && !is_direct(crate::roster::BLACK_BOMB));
    }
}
