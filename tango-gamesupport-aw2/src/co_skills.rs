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

pub const TOWER_POWER: u8 = 0x3A;
pub const PRAIRIE_DOG: u8 = 0x38;
pub const PATHFINDER: u8 = 0x39;
pub const SCOUT: u8 = 0x3B;
pub const EAGLE_EYE: u8 = 0x3C;
pub const INVADER: u8 = 0x3F;
pub const CONQUERER: u8 = 0x40;
pub const SNEAKY: u8 = 0x41;
pub const STEALTHY: u8 = 0x42;
pub const SALE_PRICE: u8 = 0x43;
pub const FIRE_SALE: u8 = 0x44;
pub const LUCK: u8 = 0x47;
pub const STAR_POWER: u8 = 0x48;
pub const MISTWALKER: u8 = 0x49;
pub const SOUL_OF_HACHI: u8 = 0x4A;

/// Firepower per Com Tower on top of the CO's (Tower Power: 10 -> 15).
pub fn tower_bonus(core: &Core, army: u32) -> i32 {
    if has(core, army, TOWER_POWER) { 5 } else { 0 }
}

/// A dived Sub's or hidden Stealth's daily fuel less (Sneaky 1, Stealthy 2).
pub fn hidden_fuel_cut(core: &Core, army: u32) -> i32 {
    (if has(core, army, SNEAKY) { 1 } else { 0 }) + (if has(core, army, STEALTHY) { 2 } else { 0 })
}

// --- Hooks in AW2's code ---------------------------------------------------------

const CURRENT_ARMY: u32 = 0x0300_33EC;
const CO_ABILITIES: u32 = 0x0300_3FC8;
const UNITS_POINTER: u32 = 0x0849_9594;
const UNIT: u32 = 12;
/// Kept between a function's entry and its return (army).
const LUCK_ARMY: u32 = 0x0203_F7FE;
const SPECIAL_ARMY: u32 = 0x0203_F7FF;

fn any_on(core: &Core) -> bool {
    core.raw_read_range_nonzero(ACTIVE, ACTIVE_LEN * 5)
}

trait NonZero {
    fn raw_read_range_nonzero(&self, at: u32, n: u32) -> bool;
}
impl NonZero for Core {
    fn raw_read_range_nonzero(&self, at: u32, n: u32) -> bool {
        (0..n).step_by(2).any(|k| self.raw_read_16(at + k, -1) != 0)
    }
}

/// `GetUnitMovementWithCoBonus(army r5, type r6)`, returning r4: APC Boost,
/// a transport's move +1 (every move range, the CPU's too).
const MOVE_DONE: u32 = 0x0804_2D42;
fn move_done(core: &mut Core) {
    if !any_on(core) {
        return;
    }
    let cpu = core.gba().cpu();
    let (army, t) = (cpu.gpr(5) as u32, cpu.gpr(6) as u8);
    if has(core, army, APC_BOOST) && is_transport(t) {
        let cpu = core.gba_mut().cpu_mut();
        let r4 = cpu.gpr(4);
        cpu.set_gpr(4, r4 + 1);
    }
}

/// `GetUnitVisionWithCoBonus(army r4)`, the CO's vision just added into r5
/// (before rain's -1): Scout +1, Eagle Eye +2.
const VISION_DONE: u32 = 0x0804_2DA6;
fn vision_done(core: &mut Core) {
    if !any_on(core) {
        return;
    }
    let army = core.gba().cpu().gpr(4) as u32;
    let add = (if has(core, army, SCOUT) { 1 } else { 0 }) + (if has(core, army, EAGLE_EYE) { 2 } else { 0 });
    if add != 0 {
        let cpu = core.gba_mut().cpu_mut();
        let r5 = cpu.gpr(5);
        cpu.set_gpr(5, r5 + add);
    }
}

/// `sub_08042650` (a capture action), its points for this action just in r5
/// (before the progress so far is added and capped at 20): Invader +1,
/// Conquerer +2.
const CAPTURE_POINTS: u32 = 0x0804_269A;
fn capture_points(core: &mut Core) {
    if !any_on(core) {
        return;
    }
    let army = core.raw_read_16(CURRENT_ARMY, -1) as u32;
    let add = (if has(core, army, INVADER) { 1 } else { 0 }) + (if has(core, army, CONQUERER) { 2 } else { 0 });
    if add != 0 {
        let cpu = core.gba_mut().cpu_mut();
        let r5 = cpu.gpr(5);
        cpu.set_gpr(5, r5 + add);
    }
}

/// `GetCoPriceMultiplier(army r4, type r5)`: the CO's cost % in r0 before
/// 100 is added: Sale Price -5, Fire Sale -8. Not for the power meter
/// (Dual Strike's meter reads no price modifier): its calls from
/// `sub_08041978` and the Oozium's eat keep the CO's own.
const PRICE: u32 = 0x0804_2CC0;
const PRICE_METER_CALLERS: [u32; 3] = [0x0804_1C3F, 0x0804_1C89, 0x0802_C8E3];
fn price(core: &mut Core) {
    if !any_on(core) {
        return;
    }
    let cpu = core.gba().cpu();
    let (army, sp) = (cpu.gpr(4) as u32, cpu.gpr(13) as u32);
    let caller = core.raw_read_32(sp + 12, -1);
    if PRICE_METER_CALLERS.contains(&caller) {
        return;
    }
    let cut = (if has(core, army, SALE_PRICE) { 5 } else { 0 }) + (if has(core, army, FIRE_SALE) { 8 } else { 0 });
    if cut != 0 {
        let cpu = core.gba_mut().cpu_mut();
        let r0 = cpu.gpr(0);
        cpu.set_gpr(0, r0 - cut);
    }
}

/// `GetPlayerCoLuckBonus(army)`: its entry keeps the army; at its return the
/// luck in r0 (10 with CO abilities off): Luck adds 10 (abilities on).
const LUCK_ENTRY: u32 = 0x0804_2E64;
const LUCK_DONE: u32 = 0x0804_2E7A;
fn luck_entry(core: &mut Core) {
    if any_on(core) {
        let army = core.gba().cpu().gpr(0) as u8;
        core.raw_write_8(LUCK_ARMY, -1, army);
    }
}
fn luck_done(core: &mut Core) {
    if !any_on(core) || core.raw_read_8(CO_ABILITIES, -1) == 0 {
        return;
    }
    let army = core.raw_read_8(LUCK_ARMY, -1) as u32;
    if has(core, army, LUCK) {
        let cpu = core.gba_mut().cpu_mut();
        let r0 = cpu.gpr(0);
        cpu.set_gpr(0, r0 + 10);
    }
}

/// The power meter's charge (`sub_080440E0(army, amount)`): Star Power x1.1.
const METER_CHARGE: u32 = 0x0804_40E0;
fn meter_charge(core: &mut Core) {
    if !any_on(core) {
        return;
    }
    let cpu = core.gba().cpu();
    let (army, amount) = (cpu.gpr(0) as u32, cpu.gpr(1));
    if has(core, army, STAR_POWER) && amount > 0 {
        core.gba_mut().cpu_mut().set_gpr(1, amount * 110 / 100);
    }
}

/// `GetPlayerSpecialAbilities(army)`: its entry keeps the army; at its
/// return the ability bits in r0. In the CO's Super Power, Mistwalker
/// strikes first when attacked (bit 0x04, Sonja's Counter Break) and Soul
/// of Hachi deploys from cities (bit 0x02, Hachi's Merchant Union; the
/// player's: the computer never builds at cities).
const SPECIAL_ENTRY: u32 = 0x0804_3050;
const SPECIAL_DONE: u32 = 0x0804_3066;
const SCOP: u8 = 2;
fn special_entry(core: &mut Core) {
    if any_on(core) {
        let army = core.gba().cpu().gpr(0) as u8;
        core.raw_write_8(SPECIAL_ARMY, -1, army);
    }
}
fn special_done(core: &mut Core) {
    if !any_on(core) {
        return;
    }
    let army = core.raw_read_8(SPECIAL_ARMY, -1) as u32;
    if !(1..=5).contains(&army) || crate::co_roster::army_co(core, army).1 != SCOP {
        return;
    }
    let bits = (if has(core, army, MISTWALKER) { 0x04 } else { 0 }) | (if has(core, army, SOUL_OF_HACHI) { 0x02 } else { 0 });
    if bits != 0 {
        let cpu = core.gba_mut().cpu_mut();
        let r0 = cpu.gpr(0);
        cpu.set_gpr(0, r0 | bits);
    }
}

/// `CacheUnitMovementCosts(type r6)` done (the flood fill's 32 costs by
/// terrain class at `*0x084999C8`; the army moving is u16 `0x03004480`):
/// Prairie Dog, tires pay 1 on plains; Pathfinder, treads and tires pay 1
/// in woods (a cost is only ever lowered; impassable stays so).
const MOVE_COSTS_DONE: u32 = 0x0801_F91E;
const MOVE_COSTS: u32 = 0x0849_99C8;
const FLOOD_ARMY: u32 = 0x0300_4480;
const TREADS: u8 = 2;
const TIRES: u8 = 3;
const PLAIN: u32 = 1;
const WOOD_CLASS: u32 = 4;
fn move_costs_done(core: &mut Core) {
    if !any_on(core) {
        return;
    }
    let army = core.raw_read_16(FLOOD_ARMY, -1) as u32;
    let t = core.gba().cpu().gpr(6) as u32 & 0xFF;
    let mt = core.raw_read_8(crate::roster::table(core) + 0x5C * t + 0x19, -1);
    let cache = core.raw_read_32(MOVE_COSTS, -1);
    if !(0x0200_0000..0x0400_0000).contains(&cache) {
        return;
    }
    let lower = |core: &mut Core, class: u32| {
        let c = core.raw_read_8(cache + class, -1) as i8;
        if c > 1 {
            core.raw_write_8(cache + class, -1, 1);
        }
    };
    if has(core, army, PRAIRIE_DOG) && mt == TIRES {
        lower(core, PLAIN);
    }
    if has(core, army, PATHFINDER) && (mt == TREADS || mt == TIRES) {
        lower(core, WOOD_CLASS);
    }
}

/// A structure's shot hits a unit (`sub_0803ECE8`: r1 the damage, r5 the
/// unit, r2 its entry in the hit list `*0x03003338`, whose first entry of
/// a shot has unit 0 and the shooter's kind at +2): Cannon Guard takes 20
/// off a cannon's or laser's hit (kinds 1, 3, 4; not the Volcano's).
const SHOT_HIT: u32 = 0x0803_ED12;
const HIT_LIST: u32 = 0x0300_3338;
fn shot_hit(core: &mut Core) {
    if !any_on(core) {
        return;
    }
    let cpu = core.gba().cpu();
    let (dmg, unit, entry) = (cpu.gpr(1) as u32, cpu.gpr(5) as u32, cpu.gpr(2) as u32);
    let units = core.raw_read_32(UNITS_POINTER, -1);
    if unit < units {
        return;
    }
    let army = crate::five::army_of_index(core, (unit - units) / UNIT);
    if !has(core, army, CANNON_GUARD) {
        return;
    }
    let list = core.raw_read_32(HIT_LIST, -1);
    let mut e = entry;
    let mut kind = 0;
    for _ in 0..64 {
        if e < list {
            break;
        }
        if core.raw_read_16(e, -1) == 0 {
            kind = core.raw_read_16(e + 2, -1);
            break;
        }
        e -= 8;
    }
    if matches!(kind, 1 | 3 | 4) {
        core.gba_mut().cpu_mut().set_gpr(1, dmg.saturating_sub(20) as i32);
    }
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![
        (MOVE_DONE, Box::new(move_done)),
        (VISION_DONE, Box::new(vision_done)),
        (CAPTURE_POINTS, Box::new(capture_points)),
        (PRICE, Box::new(price)),
        (LUCK_ENTRY, Box::new(luck_entry)),
        (LUCK_DONE, Box::new(luck_done)),
        (METER_CHARGE, Box::new(meter_charge)),
        (SPECIAL_ENTRY, Box::new(special_entry)),
        (SPECIAL_DONE, Box::new(special_done)),
        (MOVE_COSTS_DONE, Box::new(move_costs_done)),
        (SHOT_HIT, Box::new(shot_hit)),
    ]
}

// --- EXP (Dual Strike's `0x020EA240`, `0x020E9C24`) ------------------------------

/// EXP a CO gets for a DS Campaign win: the mission's score (AW2's results,
/// as Dual Strike's total of speed, power and technique), x2 played solo as
/// AW2 always is (but the first eight missions, Dual Strike's maps
/// 0xE0..0xE7, x1), and x2 on Hard. (Dual Strike's few extra points for its
/// own battle counters are left out.)
pub fn campaign_exp(score: u32, mission: u8, hard: bool) -> u32 {
    let base = score.min(999) * if hard { 2 } else { 1 };
    base * if mission < 8 { 1 } else { 2 }
}

// --- The skill table (Dual Strike's overlay 0, `0x022F5ECC`) ---------------------

/// 12-byte records by id: rank, name text, description text.
const SKILL_TABLE: u32 = 0x022F_5ECC;

/// Skill `id`'s rank, name and description (Dual Strike's, from the pack).
pub fn info(id: u8) -> Option<(u8, Vec<u8>, Vec<u8>)> {
    if !(FIRST..=LAST).contains(&id) || TAG_SKILLS.contains(&id) {
        return None;
    }
    let pack = crate::ds_pack::pack()?;
    let ds = crate::ds_campaign_data::Ds::from_pack(pack)?;
    let at = SKILL_TABLE + 12 * id as u32;
    let rank = ds.u32(at)? as u8;
    let name = ds.text(ds.u32(at + 4)?)?;
    let desc = ds.text(ds.u32(at + 8)?)?;
    Some((rank, name, desc))
}

/// The ids a player can equip (Dual Strike's, less the tag skills).
pub fn ids() -> impl Iterator<Item = u8> {
    (FIRST..=LAST).filter(|id| !TAG_SKILLS.contains(id))
}

// --- Per CO: EXP and the sets (saved with the DS Campaign's record) ---------------

/// The COs' skill data in RAM: a magic word, then per CO [`CO_LEN`] bytes:
/// EXP (u32), the Campaign set, the Survival set, the War Room set, the four
/// Versus sets (4 ids each, 0 = none). Saved in Flash slot 15 after the DS
/// Campaign's progress and records (crate::ds_campaign).
pub const DATA: u32 = 0x0203_E000;
const DATA_MAGIC: u32 = 0x314C_4B53; // "SKL1"
const CO_LEN: u32 = 32;
/// AW2's 19 COs (0..18) and Dual Strike's new nine (72..80).
pub const COS: u32 = 28;
pub const DATA_LEN: u32 = 4 + CO_LEN * COS;
/// EXP stops at Dual Strike's cap.
pub const MAX_EXP: u32 = 100_000;

/// Which set: the DS Campaign (and AW2's), Survival, the War Room, Versus 0..3.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Set {
    Campaign,
    Survival,
    WarRoom,
    Versus(u8),
}

impl Set {
    fn offset(self) -> u32 {
        4 + 4 * match self {
            Set::Campaign => 0,
            Set::Survival => 1,
            Set::WarRoom => 2,
            Set::Versus(n) => 3 + (n as u32).min(3),
        }
    }
}

/// The data slot of a CO (AW2's ids; the new COs 72..80).
pub fn co_slot(co: u8) -> Option<u32> {
    match co {
        0..=18 => Some(co as u32),
        72..=80 => Some(19 + (co - 72) as u32),
        _ => None,
    }
}

fn co_at(co: u8) -> Option<u32> {
    co_slot(co).map(|k| DATA + 4 + CO_LEN * k)
}

pub fn data_valid(core: &Core) -> bool {
    core.raw_read_32(DATA, -1) == DATA_MAGIC
}

/// The data from a saved record's bytes (a record saved before skills has
/// none: every CO at 0 EXP, no set).
pub fn load(core: &mut Core, saved: Option<&[u8]>) {
    let mut b = vec![0u8; DATA_LEN as usize];
    if let Some(s) = saved.filter(|s| s.len() >= 4 && u32::from_le_bytes(s[0..4].try_into().unwrap()) == DATA_MAGIC) {
        let n = s.len().min(b.len());
        b[..n].copy_from_slice(&s[..n]);
    }
    b[0..4].copy_from_slice(&DATA_MAGIC.to_le_bytes());
    core.raw_write_range(DATA, -1, &b);
}

/// The data's bytes, as saved.
pub fn bytes(core: &Core) -> Vec<u8> {
    let mut b = vec![0u8; DATA_LEN as usize];
    core.raw_read_range(DATA, -1, &mut b);
    b
}

pub fn exp(core: &Core, co: u8) -> u32 {
    co_at(co).map_or(0, |a| core.raw_read_32(a, -1).min(MAX_EXP))
}

pub fn add_exp(core: &mut Core, co: u8, n: u32) {
    if let Some(a) = co_at(co) {
        let v = core.raw_read_32(a, -1).min(MAX_EXP);
        core.raw_write_32(a, -1, (v + n).min(MAX_EXP));
    }
}

/// A CO's rank: EXP / 1000, up to 100 (Dual Strike's `0x020E5450`).
pub fn rank(core: &Core, co: u8) -> u32 {
    (exp(core, co) / 1000).min(100)
}

/// Skill slots: min(rank, 4).
pub fn slots(core: &Core, co: u8) -> usize {
    rank(core, co).min(4) as usize
}

/// Skill `id` is open to CO `co`: its rank reached; the rank-10 skills need
/// Means to an End won instead (Eagle Eye, Gear Head, Conquerer on Normal;
/// Mistwalker and Soul of Hachi on Hard: Dual Strike's flags 0x21 / 0x22).
pub fn unlocked(core: &mut Core, co: u8, id: u8) -> bool {
    let Some((r, _, _)) = info(id) else { return false };
    match id {
        0x3C | 0x3E | 0x40 => crate::ds_campaign::cleared(core, false),
        0x49 | 0x4A => crate::ds_campaign::cleared(core, true),
        _ => rank(core, co) >= r as u32,
    }
}

pub fn set_of(core: &Core, co: u8, set: Set) -> [u8; 4] {
    let mut b = [0u8; 4];
    if let Some(a) = co_at(co) {
        core.raw_read_range(a + set.offset(), -1, &mut b);
    }
    b
}

pub fn store_set(core: &mut Core, co: u8, set: Set, ids: [u8; 4]) {
    if let Some(a) = co_at(co) {
        core.raw_write_range(a + set.offset(), -1, &ids);
    }
}

/// The skills of a set an army gets: the set's ids that are open to its CO,
/// as many as its slots.
pub fn usable(core: &mut Core, co: u8, set: Set) -> Vec<u8> {
    let n = slots(core, co);
    let ids = set_of(core, co, set);
    let mut out = Vec::new();
    for id in ids {
        if id != 0 && out.len() < n && !out.contains(&id) && unlocked(core, co, id) {
            out.push(id);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ram_fits() {
        assert!(ACTIVE >= 0x0203_F7E0 && ACTIVE + ACTIVE_LEN * 5 <= LUCK_ARMY, "between crate::power_anim's and crate::ds_battle's state");
        assert!(SPECIAL_ARMY < 0x0203_F800);
        assert!((LAST - FIRST) as u32 / 8 < ACTIVE_LEN);
    }

    #[test]
    fn data_layout() {
        assert_eq!(co_slot(18), Some(18));
        assert_eq!(co_slot(72), Some(19));
        assert_eq!(co_slot(80), Some(27));
        assert_eq!(co_slot(19), None);
        assert_eq!(Set::Versus(3).offset() + 4, CO_LEN);
        assert!(DATA + DATA_LEN <= 0x0203_F600, "below the DS Campaign's records");
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
