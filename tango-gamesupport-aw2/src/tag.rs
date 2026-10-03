//! CO tag pairs: Dual Strike's tag battles in AW2's battle engine, with the
//! Dual Strike pack (docs/AW2.md "CO tag pairs").
//!
//! **Dual Strike, as found** (USA ROM; arm9 addresses). A player's two COs
//! are two words at player +0x6C (the active CO, "slot 0") and +0x70 (the
//! partner, "slot 1"): bits 0..6 the CO, 10..11 its power, 14..17 the
//! powers it has used, 21..31 its meter. Everything the game works out for
//! an army (day-to-day, powers, skills) reads slot 0: only the active CO's
//! abilities apply.
//! - **Change** (map menu, `0x020BD628` -> `0x020DD6BC`): the active CO's
//!   power ends (`0x020E2CE0`), the two words (and the COs' skill blocks)
//!   are swapped (`0x020E19B4`), the partner's tag-in quote and the CO★SWAP
//!   animation play, and the turn ends (`0x020DDB38`). It is offered while
//!   the army has a partner (player +0x70) and no Tag Power is under way.
//! - **Meters**: one per CO, each with its own power count (the star cost
//!   grows with its own uses). A battle's charge (`0x020DCA..`) goes to the
//!   active CO in full and to the partner at half (`asr #1` of the active
//!   CO's amount, then the partner's own Star Power skill); nothing charges
//!   while a power is on (`0x020E2E10`).
//! - **Tag Power** (`0x020BD740` -> `0x020DD404`): offered when both COs
//!   can use their Super CO Power (`0x020E2AC8`); the active CO's Super
//!   Power starts and player +0x25 becomes 1. End is then hidden (its test
//!   `0x020BED58`) and Change reads "Switch COs and move again." (the second
//!   Change entry, `0x020BFAB8`): the first CO's power ends, the COs swap,
//!   +0x25 becomes 2, every unit of the army may move again (`0x020C6F98`)
//!   and the partner's Super Power starts (`0x020DD3C0`). +0x25 goes back to
//!   0 at the army's next turn start (`0x020C1950`).
//! - **Compatibility**: while +0x25 is 1 or 2 the army's firepower gets
//!   (compatibility - 100)% (`0x020E5C40` -> `0x020E5508`); the
//!   compatibility of a pair is a byte at CO record +0x84 + partner
//!   (`0x020E1928`, 65..130). Defence gets nothing (its callers pass 0).
//!   Pairs with a name for their Tag Power ("Power Wrench") are listed at
//!   record +0x6C (`0x020E17F0`); the others' is "Dual Strike".
//! - **The computer** (`0x020995C4` at its turn start): Tag Power when both
//!   COs can; with only the active CO ready it holds its Super Power while
//!   the partner's meter is high, and it leaves the CO Power to a partner
//!   ready for its Super Power. At its turn's end (`0x02099F88`): during a
//!   Tag Power it always Changes (the second half); otherwise it Changes
//!   (`0x02099BA0`, with CO abilities on) when the active CO's meter is
//!   nearer full than the partner's (the emptier one charges at full rate),
//!   or, the two alike, to the CO it rates higher (`0x02099DAC`).
//! - **EXP**: both COs of a pair get the battle's EXP (`0x020E9C24`, the
//!   loop over both slots).
//!
//! **Here.** The partner of army a (1..5) is [`STATE`] + [`REC`] * (a - 1):
//! its CO, its meter, its power count and announcement byte (player +0x20,
//! +0x25, +0x24) and its skills; [`swap`] exchanges them with the player
//! block's. The map menu (`OpenMapMenu`'s table, pool word
//! [`MENU_POOL`]) is a copy with Tag and Change while a battle has pairs;
//! their tests and actions are stubs to [`LANDING`], dead code that is
//! trapped. Everything lives in emulated RAM (both netplay peers and every
//! replay see the same).

use mgba::core::Core;

use crate::ds_weather::is_on;

// --- RAM -----------------------------------------------------------------------

/// tangoAW2's tag state: 0x0203F400..0x0203F4FF (free EWRAM, see docs).
pub const STATE: u32 = 0x0203_F400;
pub const REC: u32 = 0x20;
const P_CO: u32 = 0x00;
const P_PHASE: u32 = 0x01;
const P_USES: u32 = 0x02;
const P_ANNOUNCE: u32 = 0x03;
const P_CHARGE: u32 = 0x04;
const P_SKILLS: u32 = 0x08; // 6 bytes (crate::co_skills::ACTIVE's layout)
/// The CPU's second half of a Tag Power is under way (skip the factory).
const P_CPU_SECOND: u32 = 0x0E;
/// 1 once this army's Change (or the second half) happened this turn: the
/// swap's banner ([`crate::tag_ui`]).
pub const P_SHOW: u32 = 0x0F;
const ARMIES: u32 = 5;

/// The partner each army picked on Versus' Teams screen (a CO id, 0xFF
/// none; an army with one plays as a pair, as Dual Strike's Versus: no
/// rule), armies 1..5 (five-army games too).
pub const TEAMS_PARTNER: u32 = STATE + 0xA0;
/// 1 while the battle has a pair (the map menu's copy is used).
const BATTLE_ON: u32 = STATE + 0xA9;
/// The CO panel as the game drew it this frame (`DrawArmyCoPanel`): x
/// (i16), y (i16), army, drawn (1).
pub const PANEL: u32 = STATE + 0xAC;
/// Pairs asked for before the next map start ([`set_pending`]): (CO,
/// partner) for armies 1..5, 0xFF none.
const PENDING: u32 = STATE + 0xB4;
/// Teams-screen bookkeeping ([`crate::tag_ui`]): STATE + 0xC0..0xCF.
pub const UI: u32 = STATE + 0xC0;
pub const STATE_END: u32 = STATE + 0x100;

const NONE: u8 = 0xFF;

const CURRENT_ARMY: u32 = 0x0300_33EC;
const PLAY_ST: u32 = 0x0300_3FC0;
const GAME_MODE: u32 = PLAY_ST + 0x01;
const POWERS_ON: u32 = PLAY_ST + 0x07;
const ABILITIES_ON: u32 = PLAY_ST + 0x08;
const VERSUS: u8 = 3;
/// The current army's first unit index (`StartArmyTurn`; 64 ids an army,
/// 51 in a five-army game).
const ARMY_FIRST_UNIT: u32 = 0x0300_3F2C;
const UNITS_POINTER: u32 = 0x0849_9594;
const UNIT: u32 = 12;
/// The AI's turn state (`AiDriverStep`: 0 begin, 1 next pass, ..., 4 end).
const AI_STATE: u32 = 0x0300_4780;

// Player block.
const PLAYER: u32 = 0x3C;
const PL_HUMAN: u32 = 0x1B;
const PL_CO: u32 = 0x1D;
const PL_MODE: u32 = 0x1E;
const PL_CHARGE: u32 = 0x20;
const PL_ANNOUNCE: u32 = 0x24;
const PL_USES: u32 = 0x25;
const PL_TEMP_FP: u32 = 0x26;
const PL_TEMP_DEF: u32 = 0x28;

fn rec(army: u32) -> u32 {
    STATE + REC * (army - 1)
}

fn valid_army(army: u32) -> bool {
    (1..=ARMIES).contains(&army)
}

pub fn player(core: &Core, army: u32) -> u32 {
    crate::five::players(core) + PLAYER * army
}

/// Army `army`'s active CO (AW2's player block).
pub fn army_co_of(core: &Core, army: u32) -> u8 {
    army_co(core, army)
}

fn army_co(core: &Core, army: u32) -> u8 {
    core.raw_read_8(player(core, army) + PL_CO, -1)
}

/// Army `army`'s partner CO, if it has one.
pub fn partner(core: &Core, army: u32) -> Option<u8> {
    if !valid_army(army) || !is_on(core) {
        return None;
    }
    let co = core.raw_read_8(rec(army) + P_CO, -1);
    (co != NONE && core.raw_read_8(STATE + 0xFC, -1) == MAGIC_RAM).then_some(co)
}

/// The partner's meter (AW2's units, as player +0x20).
pub fn partner_charge(core: &Core, army: u32) -> u32 {
    core.raw_read_32(rec(army) + P_CHARGE, -1)
}

pub fn partner_uses(core: &Core, army: u32) -> u8 {
    core.raw_read_8(rec(army) + P_USES, -1)
}

/// 0 none, 1 the first half of a Tag Power, 2 the second.
pub fn phase(core: &Core, army: u32) -> u8 {
    if partner(core, army).is_none() {
        return 0;
    }
    core.raw_read_8(rec(army) + P_PHASE, -1)
}

fn set_phase(core: &mut Core, army: u32, v: u8) {
    core.raw_write_8(rec(army) + P_PHASE, -1, v);
}

/// A word in our state says the records are set up (a state from a build
/// without tags reads 0 there: no pairs).
const MAGIC_RAM: u8 = 0x7A;

fn clear_pairs(core: &mut Core) {
    for a in 1..=ARMIES {
        let mut b = [0u8; REC as usize];
        b[P_CO as usize] = NONE;
        core.raw_write_range(rec(a), -1, &b);
    }
    core.raw_write_8(STATE + 0xFC, -1, MAGIC_RAM);
}

/// Whether any army has a partner.
pub fn any(core: &Core) -> bool {
    (1..=ARMIES).any(|a| partner(core, a).is_some())
}

// --- The CO table and the meters ----------------------------------------------------

/// The CO table the game reads now (tangoAW2's copy with the pack).
pub const CO_TABLE_POOL: u32 = 0x0804_2DDC;
const CO_ROW: u32 = 0x104;

fn stars(core: &Core, co: u8) -> (u32, u32) {
    let row = core.raw_read_32(CO_TABLE_POOL, -1) + CO_ROW * co as u32;
    (core.raw_read_32(row + 0x0C, -1), core.raw_read_32(row + 0x10, -1))
}

/// `GetCoPowerStarCost`: 9000 a star, +20% per power used (at most +100%).
pub fn star_cost(uses: u8) -> u32 {
    let pct = if uses > 9 { 200 } else { 100 + 20 * uses as u32 };
    9000 * pct / 100
}

/// The Super Power's cost for CO `co` after `uses` powers.
pub fn scop_cost(core: &Core, co: u8, uses: u8) -> u32 {
    star_cost(uses) * stars(core, co).1
}

pub fn cop_cost(core: &Core, co: u8, uses: u8) -> u32 {
    star_cost(uses) * stars(core, co).0
}

fn active_scop_ready(core: &Core, army: u32) -> bool {
    let p = player(core, army);
    core.raw_read_8(p + PL_MODE, -1) == 0
        && core.raw_read_32(p + PL_CHARGE, -1) >= scop_cost(core, army_co(core, army), core.raw_read_8(p + PL_USES, -1))
}

fn partner_scop_ready(core: &Core, army: u32) -> bool {
    match partner(core, army) {
        Some(co) => partner_charge(core, army) >= scop_cost(core, co, partner_uses(core, army)),
        None => false,
    }
}

/// Tag Power can be used now (Dual Strike's `0x020E2AC8`).
pub fn tag_ready(core: &Core, army: u32) -> bool {
    partner(core, army).is_some()
        && phase(core, army) == 0
        && core.raw_read_8(POWERS_ON, -1) != 0
        && active_scop_ready(core, army)
        && partner_scop_ready(core, army)
}

/// A battle's charge for army `army` (`AddCoPowerCharge(army, amount)`'s
/// entry, before Star Power): the partner gets half, with its own Star
/// Power, up to its Super Power's cost. Nothing while powers are off or a
/// power is on (Dual Strike's `0x020E2E10`).
pub fn charged(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let cpu = core.gba().cpu();
    let (army, amount) = (cpu.gpr(0) as u32, cpu.gpr(1) as i32);
    let Some(co) = partner(core, army) else { return };
    if amount <= 0 || core.raw_read_8(POWERS_ON, -1) == 0 || core.raw_read_8(player(core, army) + PL_MODE, -1) != 0 {
        return;
    }
    let mut add = amount as u32 / 2;
    if has_partner_skill(core, army, crate::co_skills::STAR_POWER) {
        add = add * 110 / 100;
    }
    let cap = scop_cost(core, co, partner_uses(core, army));
    let v = (partner_charge(core, army) + add).min(cap);
    core.raw_write_32(rec(army) + P_CHARGE, -1, v);
}

fn has_partner_skill(core: &Core, army: u32, id: u8) -> bool {
    if !(crate::co_skills::FIRST..=crate::co_skills::LAST).contains(&id) {
        return false;
    }
    let bit = (id - crate::co_skills::FIRST) as u32;
    core.raw_read_8(rec(army) + P_SKILLS + bit / 8, -1) & (1 << (bit % 8)) != 0
}

// --- Compatibility (Dual Strike's CO records, +0x84) --------------------------------

/// Dual Strike's compatibility of a pair (65..130); with AW2's Sturm, who
/// is not in Dual Strike, tangoAW2's own (crate::sturm_pairs). 100
/// without the pack.
pub fn compatibility(a: u8, b: u8) -> u8 {
    if crate::ds_pack::pack().is_some() {
        if let Some(c) = crate::sturm_pairs::compatibility(a, b) {
            return c;
        }
    }
    let (Some(da), Some(db)) = (ds_id(a), ds_id(b)) else { return 100 };
    crate::ds_pack::pack()
        .and_then(|p| p.arm9_at(0x0215_360C + 0x220 * da as u32 + 0x84 + db as u32, 1).map(|b| b[0]))
        .unwrap_or(100)
}

/// Dual Strike's special pairs: a CO record's list at +0x6C (8 bytes an
/// entry: the partner's id, its star rating 1..3 at +2, a pointer to five
/// text ids (four victory lines and the pair's Tag Power name, "Power
/// Wrench"); a zero partner ends it). The stars are the CO page's TAG box
/// rating; the Tag Power's strength is the compatibility (+0x84), which
/// every pair has. Sturm's (crate::sturm_pairs, tangoAW2's own): his
/// stars and no pointer (0).
pub fn special_pair(a: u8, b: u8) -> Option<(u8, u32)> {
    let pack = crate::ds_pack::pack()?;
    if let Some(p) = crate::sturm_pairs::special(a, b) {
        return Some((p.stars, 0));
    }
    let (da, db) = (ds_id(a)?, ds_id(b)?);
    let rec = 0x0215_360C + 0x220 * da as u32;
    let list = pack.arm9_at(rec + 0x6C, 0x18)?;
    for e in list.chunks(8) {
        if e[0] == 0 {
            break;
        }
        if e[0] == db {
            return Some((e[2], u32::from_le_bytes([e[4], e[5], e[6], e[7]])));
        }
    }
    None
}

/// The AW2 CO of a Dual Strike id (AW2's 19 and the new nine).
pub fn aw2_co(d: u8) -> Option<u8> {
    (0..19u8).chain(72..81).find(|&c| ds_id(c) == Some(d))
}

/// A CO's Dual Strike id (AW2's COs and the new ones).
pub fn ds_id(co: u8) -> Option<u8> {
    crate::co_new::ds_id(co).or_else(|| (co < crate::co_roster::AW2_COS).then(|| crate::co_roster::ds_co(co)).flatten())
}

/// The firepower (percentage points) a Tag Power gives army `army` now:
/// compatibility - 100 while it is under way (both halves), with CO
/// abilities on (Dual Strike's `0x020E5C40`).
pub fn firepower(core: &Core, army: u32) -> i32 {
    if !valid_army(army) || phase(core, army) == 0 || core.raw_read_8(ABILITIES_ON, -1) == 0 {
        return 0;
    }
    let Some(b) = partner(core, army) else { return 0 };
    compatibility(army_co(core, army), b) as i32 - 100
}

// --- Pairs ----------------------------------------------------------------------------

/// Army `army` gets partner `co` with an empty meter (the army keeps its
/// active CO). For the two-front campaign: when the second front is won
/// its CO joins the main front's as a pair. `charge` is the partner's
/// meter (AW2's units); its skills are the mode's set for it.
pub fn form_pair(core: &mut Core, army: u32, co: u8, charge: u32) {
    if !valid_army(army) || !is_on(core) {
        return;
    }
    if core.raw_read_8(STATE + 0xFC, -1) != MAGIC_RAM {
        clear_pairs(core);
    }
    let r = rec(army);
    let mut b = [0u8; REC as usize];
    b[P_CO as usize] = co;
    let cap = scop_cost(core, co, 0);
    b[P_CHARGE as usize..P_CHARGE as usize + 4].copy_from_slice(&charge.min(cap).to_le_bytes());
    core.raw_write_range(r, -1, &b);
    let ids = crate::co_skills::ids_for(core, army, co);
    let mut bits = [0u8; 6];
    for id in ids {
        if (crate::co_skills::FIRST..=crate::co_skills::LAST).contains(&id) {
            let bit = (id - crate::co_skills::FIRST) as usize;
            bits[bit / 8] |= 1 << (bit % 8);
        }
    }
    core.raw_write_range(r + P_SKILLS, -1, &bits);
}

/// The army leaves its pair (its active CO stays).
pub fn break_pair(core: &mut Core, army: u32) {
    if valid_army(army) {
        core.raw_write_8(rec(army) + P_CO, -1, NONE);
        core.raw_write_8(rec(army) + P_PHASE, -1, 0);
    }
}

/// A pair for the next map start: army `army` starts with CO `co` and
/// partner `partner` (the DS Campaign's player picks; a mode's setup).
pub fn set_pending(core: &mut Core, army: u32, co: u8, partner: u8) {
    if valid_army(army) {
        core.raw_write_8(PENDING + 2 * (army - 1), -1, co);
        core.raw_write_8(PENDING + 2 * (army - 1) + 1, -1, partner);
    }
}

fn clear_pending(core: &mut Core) {
    core.raw_write_range(PENDING, -1, &[NONE; 2 * ARMIES as usize]);
}

/// Exchange army `army`'s active CO with its partner: the CO, the meter,
/// the power count and announcement byte, the skills.
pub fn swap(core: &mut Core, army: u32) {
    let Some(b) = partner(core, army) else { return };
    let p = player(core, army);
    let r = rec(army);
    let a = core.raw_read_8(p + PL_CO, -1);
    let (ac, au, aa) = (core.raw_read_32(p + PL_CHARGE, -1), core.raw_read_8(p + PL_USES, -1), core.raw_read_8(p + PL_ANNOUNCE, -1));
    core.raw_write_8(p + PL_CO, -1, b);
    core.raw_write_32(p + PL_CHARGE, -1, partner_charge(core, army));
    core.raw_write_8(p + PL_USES, -1, partner_uses(core, army));
    core.raw_write_8(p + PL_ANNOUNCE, -1, core.raw_read_8(r + P_ANNOUNCE, -1));
    core.raw_write_8(r + P_CO, -1, a);
    core.raw_write_32(r + P_CHARGE, -1, ac);
    core.raw_write_8(r + P_USES, -1, au);
    core.raw_write_8(r + P_ANNOUNCE, -1, aa);
    let skills = crate::co_skills::ACTIVE + 6 * (army - 1);
    let mut s1 = [0u8; 6];
    let mut s2 = [0u8; 6];
    core.raw_read_range(skills, -1, &mut s1);
    core.raw_read_range(r + P_SKILLS, -1, &mut s2);
    core.raw_write_range(skills, -1, &s2);
    core.raw_write_range(r + P_SKILLS, -1, &s1);
    core.raw_write_8(r + P_SHOW, -1, 1);
}

/// `ClearPlayerCoPowerStatus`: the active CO's power ends.
fn power_off(core: &mut Core, army: u32) {
    let p = player(core, army);
    core.raw_write_8(p + PL_MODE, -1, 0);
    core.raw_write_16(p + PL_TEMP_FP, -1, 0);
    core.raw_write_16(p + PL_TEMP_DEF, -1, 0);
}

/// `ReadyCurrentArmyUnits`: the current army's units may move again.
fn ready_units(core: &mut Core) {
    let first = core.raw_read_32(ARMY_FIRST_UNIT, -1);
    let units = core.raw_read_32(UNITS_POINTER, -1);
    for i in first..first + 0x33 {
        let u = units + UNIT * i;
        let flags = core.raw_read_8(u + 1, -1);
        if core.raw_read_8(u, -1) != 0 && flags & 8 == 0 && flags & 1 != 0 {
            core.raw_write_8(u + 1, -1, flags & !1);
        }
    }
}

/// Every map start (`crate::sandstorm`'s trap): the pairs of the battle.
/// Versus: each army's Teams-screen partner (none: single); a
/// mode's pending pairs ([`set_pending`]); the DS Campaign's missions
/// ([`crate::ds_campaign::tag_pairs`]).
pub fn map_start(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    clear_pairs(core);
    let versus = core.raw_read_8(GAME_MODE, -1) == VERSUS
        && !crate::survival::on(core)
        && !crate::ds_campaign::active(core);
    let mut pairs: Vec<(u32, u8, u8)> = Vec::new();
    if versus {
        // Dual Strike's Versus: an army (a human's or the computer's) with
        // a partner picked on Teams is a pair; one without, single.
        let last = if crate::five::active(core) { 5 } else { 4 };
        for a in 1..=last {
            if core.raw_read_8(player(core, a) + PL_HUMAN, -1) == 0 {
                continue;
            }
            let b = core.raw_read_8(TEAMS_PARTNER + a - 1, -1);
            let main = army_co(core, a);
            if b != NONE && b != main {
                pairs.push((a, main, b));
            }
        }
    }
    for (a, co, b) in crate::ds_campaign::tag_pairs(core) {
        pairs.push((a, co, b));
    }
    for a in 1..=ARMIES {
        let co = core.raw_read_8(PENDING + 2 * (a - 1), -1);
        let b = core.raw_read_8(PENDING + 2 * (a - 1) + 1, -1);
        if co != NONE && b != NONE && co != b {
            pairs.retain(|p| p.0 != a);
            pairs.push((a, co, b));
        }
    }
    clear_pending(core);
    for (a, co, b) in pairs {
        if core.raw_read_8(player(core, a) + PL_HUMAN, -1) == 0 {
            continue;
        }
        if army_co(core, a) != co {
            core.raw_write_8(player(core, a) + PL_CO, -1, co);
            let ids = crate::co_skills::ids_for(core, a, co);
            crate::co_skills::set(core, a, &ids);
        }
        form_pair(core, a, b, 0);
    }
}

// --- The DS Campaign's CO screen: the player's partners ------------------------------

/// The player's pairs a DS mission asks for (Dual Strike's (0x1C, 0x1C): the
/// player picks both): (the armies picked for, the partners picked). AW2's
/// CO screen has room for four picks, so with three armies only the first
/// gets a partner. A mission with a second front is left to its own flow.
fn ds_partner_picks(core: &Core) -> (u32, u32) {
    let Some(m) = crate::ds_campaign::mission_info(core) else { return (0, 0) };
    if m.two_front.is_some() {
        return (0, 0);
    }
    let armies = (m.armies as usize).min(4);
    let n = (0..armies).take_while(|&k| m.cos[k].0 == 0x1C).count() as u32;
    let pairs = (0..n as usize).take_while(|&k| m.cos[k].1 == 0x1C).count() as u32;
    (n, pairs.min(4 - n))
}

/// `sub_0803BD14` (the CO screen's count of picks: the leading player armies
/// of the map's header) at its return (r3): the partners are picked after
/// the armies' COs, on the same screen. Not for `SetArmyCoIdsFromList`
/// (which gives the picks to the armies: the partners are kept apart).
const PICK_COUNT: u32 = 0x0803_BD42;
const SET_COS: u32 = 0x0803_BCDC;
fn pick_count(core: &mut Core) {
    if !is_on(core) || !crate::ds_campaign::active(core) {
        return;
    }
    let lr = core.gba().cpu().gpr(14) as u32 & !1;
    if (SET_COS..PICK_COUNT).contains(&lr) {
        return;
    }
    let (_, k) = ds_partner_picks(core);
    if k > 0 {
        let cpu = core.gba_mut().cpu_mut();
        let r3 = cpu.gpr(3);
        cpu.set_gpr(3, r3 + k as i32);
    }
}

/// `SetArmyCoIdsFromList(picks)`: the picks past the armies' are their
/// partners (pending until the map starts). Called from crate::two_front's
/// trap there (one trap an address).
pub fn set_cos(core: &mut Core) {
    if !is_on(core) || !crate::ds_campaign::active(core) {
        return;
    }
    let (n, k) = ds_partner_picks(core);
    let src = core.gba().cpu().gpr(0) as u32;
    if k == 0 || !(0x0200_0000..0x0400_0000).contains(&src) {
        return;
    }
    for i in 0..k {
        let main = core.raw_read_8(src + i, -1);
        let partner = core.raw_read_8(src + n + i, -1);
        set_pending(core, i + 1, main, partner);
    }
}

/// The CO screen's per-pick "locks its country" words (`gUnknown_030059C0`,
/// crate::ds_campaign's CO_GROUP_SWITCH) and the countries locked so far
/// (`gUnknown_03005910`): AW2 gives every army of a campaign map its own
/// country, so a pick locks its country for the next. A partner is not an
/// army: its picks lock nothing, and while they are made every country is
/// open (Dual Strike pairs COs of one country).
const PICK_LOCKS: u32 = 0x0300_59C0;
const COUNTRY_LOCKED: u32 = 0x0300_5910;
const CO_SELECT_SCRIPT: u32 = 0x0861_6638;
const PROC_POOL: (u32, u32) = (0x0200_D610, 0x0200_E418);
fn co_screen_partners(core: &mut Core) {
    if !crate::ds_campaign::active(core) {
        return;
    }
    let Some(proc) = (PROC_POOL.0..PROC_POOL.1).step_by(0x6C).find(|&p| core.raw_read_32(p, -1) == CO_SELECT_SCRIPT) else {
        return;
    };
    let (n, k) = ds_partner_picks(core);
    if k == 0 {
        return;
    }
    for i in n..(n + k).min(5) {
        if core.raw_read_32(PICK_LOCKS + 4 * i, -1) != 0 {
            core.raw_write_32(PICK_LOCKS + 4 * i, -1, 0);
        }
    }
    if core.raw_read_16(proc + 0x64, -1) as u32 >= n {
        for c in 0..5 {
            if core.raw_read_8(COUNTRY_LOCKED + c, -1) != 0 {
                core.raw_write_8(COUNTRY_LOCKED + c, -1, 0);
            }
        }
    }
}

/// The CO screen's sprites, at the frame's flush (crate::tag_ui::flush): a
/// partner's row of the picks box shows its army's country emblem (AW2
/// gives every pick its own army's: a partner would show the next army's),
/// and while a partner is picked the badge on the face is its army's ("1P"
/// for army 1's partner, not "2P"). The box's emblems are 16x16 sprites of
/// OBJ palette 12 at x 20, a row every 16 pixels from y 104; the badges
/// 16x16 sprites of palette 13 at tile 836 + 4 a pick.
const BOX_EMBLEM_X: u16 = 20;
const BOX_EMBLEM_Y: u16 = 104;
const BOX_EMBLEM_PAL: u16 = 12;
const BADGE_PAL: u16 = 13;
const BADGE_TILE: u16 = 836;
pub fn co_screen_flush(core: &mut Core, start: u32, at: u32) {
    if !is_on(core) || !crate::ds_campaign::active(core) {
        return;
    }
    let Some(proc) = (PROC_POOL.0..PROC_POOL.1).step_by(0x6C).find(|&p| core.raw_read_32(p, -1) == CO_SELECT_SCRIPT) else {
        return;
    };
    let (n, k) = ds_partner_picks(core);
    if k == 0 {
        return;
    }
    let square16 = |a0: u16, a1: u16| (a0 >> 14) == 0 && (a1 >> 14) == 1 && (a0 >> 8) & 3 != 2;
    let mut emblems: [Option<(u32, u16)>; 5] = [None; 5];
    let mut e = start;
    while e + 8 <= at {
        let (a0, a1, a2) = (core.raw_read_16(e, -1), core.raw_read_16(e + 2, -1), core.raw_read_16(e + 4, -1));
        let y = a0 & 0xFF;
        if square16(a0, a1) && a1 & 0x1FF == BOX_EMBLEM_X && a2 >> 12 == BOX_EMBLEM_PAL && y >= BOX_EMBLEM_Y && (y - BOX_EMBLEM_Y) % 16 == 0 {
            let row = ((y - BOX_EMBLEM_Y) / 16) as usize;
            if row < 5 {
                emblems[row] = Some((e, a2));
            }
        }
        e += 8;
    }
    for r in n..(n + k).min(5) {
        if let (Some((entry, _)), Some((_, army_a2))) = (emblems[r as usize], emblems[(r - n) as usize]) {
            core.raw_write_16(entry + 4, -1, army_a2);
        }
    }
    let pick = core.raw_read_16(proc + 0x64, -1) as u32;
    if (n..n + k).contains(&pick) {
        let (from, to) = (BADGE_TILE + 4 * pick as u16, BADGE_TILE + 4 * (pick - n) as u16);
        let mut e = start;
        while e + 8 <= at {
            let (a0, a1, a2) = (core.raw_read_16(e, -1), core.raw_read_16(e + 2, -1), core.raw_read_16(e + 4, -1));
            if square16(a0, a1) && a2 >> 12 == BADGE_PAL && a2 & 0x3FF == from {
                core.raw_write_16(e + 4, -1, (a2 & !0x3FF) | to);
            }
            e += 8;
        }
    }
}

// --- The map menu: Tag and Change ---------------------------------------------------

/// `OpenMapMenu`'s literal-pool word for its table.
pub const MENU_POOL: u32 = 0x0802_D49C;
const AW2_MENU: u32 = 0x0849_AAC0;
const ITEM: u32 = 0x20;
/// AW2's entries: CO, Intel, Power, Super, Options, Save, End.
const AW2_ITEMS: u32 = 7;
const AW2_END: u32 = 6;
/// ROM tangoAW2 writes (free space, see docs/AW2.md): the menu, the stubs,
/// the labels.
pub const ROM: u32 = 0x0878_0000;
const MENU: u32 = ROM;
const STUBS: u32 = ROM + 0x200;
const STRINGS: u32 = ROM + 0x300;
const SENTINEL: u32 = ROM + 0xFFFC;
const MAGIC: u32 = 0x3247_4154; // "TAG2"
/// The labels' text ids (pointers at `0x08610A38 + 4 * id`, free ROM).
const TEXT_TABLE: u32 = 0x0861_0A38;
pub const TEXT_TAG: u16 = 0x7300;
pub const TEXT_CHANGE: u16 = 0x7301;
/// The Teams screen's help line on an army's CO stop, with the pack
/// (crate::tag_ui puts it in place of AW2's "Choose a CO.", text 0x9DC).
pub const CHOOSE_CO_AT: u32 = STRINGS + 0x20;
/// (AW2's help line takes 31 characters.)
const CHOOSE_CO: &[u8] = b"Choose a CO. START: partner.\0";
/// The help line while an army's partner is picked (crate::versus_rules's
/// help trap shows it).
pub const TEXT_PARTNER: u16 = 0x7304;
const CHOOSE_PARTNER_AT: u32 = STRINGS + 0x48;
const CHOOSE_PARTNER: &[u8] = b"Choose a partner CO.\0";
/// The game's icon glyphs (AW2 has no tag icons): Super's and the CO's.
const LABEL_TAG: &[u8] = b"\x09\x90Tag\0";
const LABEL_CHANGE: &[u8] = b"\x09\x94Change\0";
/// The landing every stub jumps to: dead code in `sub_0803CC3C`, after
/// crate::five_map's helper and before crate::campaign_model's landing.
pub const LANDING: u32 = 0x0803_CC5A;
const LANDING_OTHER: u32 = crate::campaign_model::LANDING;

const T_TAG: u32 = 1;
const T_CHANGE: u32 = 2;
const T_CHANGE2: u32 = 3;
const T_END: u32 = 4;
const A_TAG: u32 = 5;
const A_CHANGE: u32 = 6;
const A_CHANGE2: u32 = 7;
/// crate::tag_extras's Change script: the incoming CO's quote, a frame of
/// the CO SWAP band, the swap.
pub const S_QUOTE: u32 = 8;
pub const S_SWAP_FRAME: u32 = 9;
pub const S_SWAP: u32 = 10;
/// The computer's Change script's swap (crate::tag_extras::cpu_change).
pub const S_CPU_SWAP: u32 = 11;
const STUBS_N: u32 = 11;
const MAGIC_ID: u32 = 0x5441_4700;

/// `MapMenu_SuperPower`.
const SUPER_POWER: u32 = 0x0802_CEFC;

fn stub(id: u32) -> [u8; 16] {
    let mut s = [0u8; 16];
    for (k, v) in [0x4B01u16, 0x4A02, 0x4710, 0x46C0].iter().enumerate() {
        s[2 * k..2 * k + 2].copy_from_slice(&v.to_le_bytes());
    }
    s[8..12].copy_from_slice(&(MAGIC_ID | id).to_le_bytes());
    s[12..16].copy_from_slice(&(LANDING | 1).to_le_bytes());
    s
}

pub fn stub_addr(id: u32) -> u32 {
    stub_at(id)
}

fn stub_at(id: u32) -> u32 {
    STUBS + 16 * (id - 1)
}

/// Writes the menu, its stubs and labels (once; the same bytes on every
/// console).
fn install(core: &mut Core) {
    if core.raw_read_32(SENTINEL, -1) == MAGIC {
        return;
    }
    let mut aw2 = vec![0u8; (ITEM * (AW2_ITEMS + 1)) as usize];
    core.raw_read_range(AW2_MENU, -1, &mut aw2);
    let item = |k: u32| aw2[(ITEM * k) as usize..(ITEM * (k + 1)) as usize].to_vec();
    let with = |mut e: Vec<u8>, test: u32, action: Option<u32>, text: Option<u16>| {
        e[4..8].copy_from_slice(&(stub_at(test) | 1).to_le_bytes());
        if let Some(a) = action {
            e[0x14..0x18].copy_from_slice(&(stub_at(a) | 1).to_le_bytes());
        }
        if let Some(t) = text {
            e[0x1C..0x20].copy_from_slice(&(t as u32).to_le_bytes());
        }
        e
    };
    let super_item = item(3);
    let end_item = item(AW2_END);
    let mut menu: Vec<u8> = Vec::new();
    for k in 0..4 {
        menu.extend(item(k)); // CO, Intel, Power, Super
    }
    menu.extend(with(super_item, T_TAG, Some(A_TAG), Some(TEXT_TAG)));
    menu.extend(item(4)); // Options
    menu.extend(item(5)); // Save
    let mut change = end_item.clone();
    change[0] = 0x00; // the CO entry's byte
    menu.extend(with(change.clone(), T_CHANGE, Some(A_CHANGE), Some(TEXT_CHANGE)));
    menu.extend(with(change, T_CHANGE2, Some(A_CHANGE2), Some(TEXT_CHANGE)));
    menu.extend(with(end_item, T_END, None, None));
    menu.extend(item(AW2_ITEMS)); // the end mark
    core.raw_write_range(MENU, -1, &menu);
    for id in 1..=STUBS_N {
        core.raw_write_range(stub_at(id), -1, &stub(id));
    }
    let tag_at = STRINGS;
    let change_at = STRINGS + 0x10;
    core.raw_write_range(tag_at, -1, LABEL_TAG);
    core.raw_write_range(change_at, -1, LABEL_CHANGE);
    core.raw_write_32(TEXT_TABLE + 4 * TEXT_TAG as u32, -1, tag_at);
    core.raw_write_32(TEXT_TABLE + 4 * TEXT_CHANGE as u32, -1, change_at);
    core.raw_write_range(CHOOSE_CO_AT, -1, CHOOSE_CO);
    core.raw_write_range(CHOOSE_PARTNER_AT, -1, CHOOSE_PARTNER);
    core.raw_write_32(TEXT_TABLE + 4 * TEXT_PARTNER as u32, -1, CHOOSE_PARTNER_AT);
    core.raw_write_32(SENTINEL, -1, MAGIC);
}

/// Shown entries the menu has now (to place a tall menu at the top).
fn shown_items(core: &Core) -> u32 {
    let army = core.raw_read_16(CURRENT_ARMY, -1) as u32;
    let mut n = 4; // CO, Intel, Options, End or Change
    let powers = core.raw_read_8(POWERS_ON, -1) != 0 && core.raw_read_8(player(core, army) + PL_MODE, -1) == 0;
    let p = player(core, army);
    let (co, uses, charge) = (army_co(core, army), core.raw_read_8(p + PL_USES, -1), core.raw_read_32(p + PL_CHARGE, -1));
    if powers && charge >= cop_cost(core, co, uses) && cop_cost(core, co, uses) != 0 {
        n += 1;
    }
    if powers && charge >= scop_cost(core, co, uses) {
        n += 1;
    }
    if tag_ready(core, army) {
        n += 1;
    }
    if phase(core, army) == 0 && partner(core, army).is_some() {
        n += 1; // Change and End
    }
    n + 1 // Save (shown in Versus and the campaigns)
}

/// `OpenMapMenu`'s `CreateRootMenuWithSfx(table, x, y = 1, 1)`: a menu of
/// nine entries starts at the top row (it would run off the screen's
/// bottom from the second).
const MENU_Y: u32 = 0x0802_D484;
fn menu_y(core: &mut Core) {
    if is_on(core) && core.raw_read_8(BATTLE_ON, -1) == 1 && shown_items(core) >= 9 {
        core.gba_mut().cpu_mut().set_gpr(2, 0);
    }
}

fn return_to(core: &mut Core, r0: u32) {
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, r0 as i32);
    let lr = cpu.gpr(14) as u32;
    cpu.set_thumb_pc(lr & !1);
}

fn tail_call(core: &mut Core, f: u32) {
    core.gba_mut().cpu_mut().set_thumb_pc(f & !1);
}

/// The stubs' landing: r3 = [`MAGIC_ID`] | the entry.
fn landing(core: &mut Core) {
    let id = core.gba().cpu().gpr(3) as u32;
    if id & 0xFFFF_FF00 != MAGIC_ID {
        // Never: only our stubs come here. Return as a "hidden" test would.
        return_to(core, 1);
        return;
    }
    let army = core.raw_read_16(CURRENT_ARMY, -1) as u32;
    let paired = partner(core, army).is_some();
    let ph = phase(core, army);
    match id & 0xFF {
        T_TAG => return_to(core, (!tag_ready(core, army)) as u32),
        T_CHANGE => return_to(core, (!(paired && ph == 0)) as u32),
        T_CHANGE2 => return_to(core, (!(paired && ph == 1)) as u32),
        T_END => return_to(core, (paired && ph == 1) as u32),
        A_TAG => {
            set_phase(core, army, 1);
            core.raw_write_8(rec(army) + P_SHOW, -1, 2);
            crate::tag_extras::tag_chosen(core, army);
            tail_call(core, SUPER_POWER);
        }
        A_CHANGE => {
            // The incoming CO's quote and the CO SWAP band, then the swap
            // and the turn's end (crate::tag_extras's script).
            crate::tag_extras::change_chosen(core, army);
        }
        S_QUOTE => crate::tag_extras::quote(core),
        S_SWAP_FRAME => crate::tag_extras::swap_frame(core),
        S_SWAP => {
            power_off(core, army);
            swap(core, army);
            return_to(core, 0);
        }
        S_CPU_SWAP => {
            power_off(core, army);
            swap(core, army);
            crate::tag_extras::cpu_change_done(core);
            return_to(core, 0);
        }
        A_CHANGE2 => {
            second_half(core, army);
            tail_call(core, SUPER_POWER);
        }
        _ => return_to(core, 1),
    }
}

/// The second half of a Tag Power: the first CO's power ends, the COs
/// swap, every unit may move again; the caller then starts the partner's
/// Super Power.
fn second_half(core: &mut Core, army: u32) {
    power_off(core, army);
    swap(core, army);
    ready_units(core);
    set_phase(core, army, 2);
    core.raw_write_8(rec(army) + P_SHOW, -1, 3);
}

// --- Turns ---------------------------------------------------------------------------

/// `StartArmyTurn`: the army's Tag Power (both halves) is over.
const TURN_START: u32 = 0x0802_67AC;
fn turn_start(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let army = core.raw_read_16(CURRENT_ARMY, -1) as u32;
    if valid_army(army) {
        set_phase(core, army, 0);
        core.raw_write_8(rec(army) + P_CPU_SECOND, -1, 0);
    }
    crate::tag_extras::cpu_change_reset(core);
}

// --- The computer ---------------------------------------------------------------------

/// `AiDeliberateCoPower`'s entry (the current army): with a partner ready
/// for its Super Power the active CO keeps its CO Power (it Changes at the
/// turn's end); with only the active CO ready, its Super Power waits while
/// the partner's meter is at least half full.
const AI_POWER: u32 = 0x0805_DB70;
fn ai_power(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let army = core.raw_read_16(CURRENT_ARMY, -1) as u32;
    let Some(b) = partner(core, army) else { return };
    if phase(core, army) != 0 {
        return;
    }
    let a_ready = active_scop_ready(core, army);
    let b_ready = partner_scop_ready(core, army);
    let b_half = partner_charge(core, army) * 2 >= scop_cost(core, b, partner_uses(core, army));
    if (!a_ready && b_ready) || (a_ready && !b_ready && b_half) {
        return_to(core, 0);
    }
}

/// `AiDeliberateCoPower` paying for the Super Power (r0 the army): with the
/// partner ready too it is the Tag Power.
const AI_SUPER: u32 = 0x0805_DBCC;
fn ai_super(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let army = core.gba().cpu().gpr(0) as u32;
    if tag_ready(core, army) {
        set_phase(core, army, 1);
        crate::tag_extras::tag_chosen(core, army);
        core.raw_write_8(rec(army) + P_SHOW, -1, 2);
    }
}

/// `AiEndTurnStep`'s `EndCurrentArmyTurn` call: in the first half of a Tag
/// Power the computer goes on with the partner (its turn starts over:
/// `AiBeginTurn` with the partner's personality); otherwise it may Change.
const AI_END: u32 = 0x0806_1ACE;
const AI_END_RETURN: u32 = 0x0806_1AEE;
const PAY_FOR_POWER: u32 = 0x0804_438C;
const PROC_START: u32 = 0x0801_C8F4;
fn ai_end(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let army = core.raw_read_16(CURRENT_ARMY, -1) as u32;
    if partner(core, army).is_none() {
        return;
    }
    match phase(core, army) {
        1 => {
            second_half(core, army);
            core.raw_write_8(rec(army) + P_CPU_SECOND, -1, 1);
            core.raw_write_16(AI_STATE, -1, 0);
            let cpu = core.gba_mut().cpu_mut();
            cpu.set_gpr(0, army as i32);
            cpu.set_gpr(1, 2);
            cpu.set_gpr(14, (AI_END_RETURN | 1) as i32);
            cpu.set_thumb_pc(PAY_FOR_POWER);
        }
        0 => {
            let under_way = crate::tag_extras::cpu_change_state(core) != 0;
            if !under_way && !cpu_wants_change(core, army) {
                return;
            }
            if !crate::ds_weather::is_on(core) {
                power_off(core, army);
                swap(core, army);
                return;
            }
            // With the pack: the incoming CO's line and CO SWAP first
            // (crate::tag_extras's script); the turn ends once it is done.
            if crate::tag_extras::cpu_change(core, army) {
                let cpu = core.gba_mut().cpu_mut();
                if under_way {
                    cpu.set_thumb_pc(AI_END_RETURN);
                } else {
                    cpu.set_gpr(0, crate::tag_extras::SCRIPT_CPU_CHANGE as i32);
                    cpu.set_gpr(1, 3);
                    cpu.set_gpr(14, (AI_END_RETURN | 1) as i32);
                    cpu.set_thumb_pc(PROC_START);
                }
            }
        }
        _ => {}
    }
}

/// Dual Strike's `0x02099BA0`: with CO abilities on and no power on, the
/// computer Changes when the active CO is nearer its Super Power than the
/// partner (so the emptier meter charges at full rate); the two alike (or
/// both empty), to the CO it rates higher (`0x02099DAC`).
fn cpu_wants_change(core: &Core, army: u32) -> bool {
    let Some(b) = partner(core, army) else { return false };
    let p = player(core, army);
    if core.raw_read_8(ABILITIES_ON, -1) == 0 || core.raw_read_8(p + PL_MODE, -1) != 0 {
        return false;
    }
    let a = army_co(core, army);
    let (ma, mb) = (core.raw_read_32(p + PL_CHARGE, -1), partner_charge(core, army));
    let da = scop_cost(core, a, core.raw_read_8(p + PL_USES, -1)).saturating_sub(ma);
    let db = scop_cost(core, b, partner_uses(core, army)).saturating_sub(mb);
    if da == db || (ma == 0 && mb == 0) {
        return cpu_rating(a) < cpu_rating(b);
    }
    da < db
}

/// Dual Strike's rating of a CO for the computer's Change (`0x02099DAC`):
/// Max 1, Sami 4, Grit 5, the others 2 (its Kanbei, Sonja, Hachi and Colin
/// answer by the battle's state; here 2).
fn cpu_rating(co: u8) -> u8 {
    match ds_id(co) {
        Some(3) => 1,
        Some(5) => 4,
        Some(6) => 5,
        _ => 2,
    }
}

/// `AiBeginTurn`'s Black Factory call: not again in the second half.
const AI_FACTORY: u32 = 0x0806_1900;
const AI_FACTORY_SKIP: u32 = 0x0806_1904;
fn ai_factory(core: &mut Core) {
    let army = core.raw_read_16(CURRENT_ARMY, -1) as u32;
    if is_on(core) && valid_army(army) && core.raw_read_8(rec(army) + P_CPU_SECOND, -1) == 1 {
        core.gba_mut().cpu_mut().set_thumb_pc(AI_FACTORY_SKIP);
    }
}

// --- The CO panel ----------------------------------------------------------------------

/// `DrawArmyCoPanel(x, y, army)`: where the panel is this frame.
const DRAW_PANEL: u32 = 0x0804_36DC;
fn draw_panel(core: &mut Core) {
    // (a two-front battle's view of its other front draws no panel)
    crate::two_front::co_panel(core);
    if !is_on(core) || !any(core) {
        return;
    }
    let cpu = core.gba().cpu();
    let (x, y, a) = (cpu.gpr(0) as u16, cpu.gpr(1) as u16, cpu.gpr(2) as u8);
    core.raw_write_16(PANEL, -1, x);
    core.raw_write_16(PANEL + 2, -1, y);
    core.raw_write_8(PANEL + 4, -1, a);
    core.raw_write_8(PANEL + 5, -1, 1);
}

// --- Every frame -------------------------------------------------------------------------

pub fn tick(core: &mut Core, on: bool) {
    if !on {
        return;
    }
    install(core);
    crate::tag_extras::install(core);
    if core.raw_read_8(STATE + 0xFC, -1) != MAGIC_RAM {
        clear_pairs(core);
        clear_pending(core);
        core.raw_write_range(TEAMS_PARTNER, -1, &[NONE; 5]);
    }
    co_screen_partners(core);
    let battle = any(core);
    core.raw_write_8(BATTLE_ON, -1, battle as u8);
    // The map menu's table: ours while the battle has a pair; without one,
    // back to AW2's only if it is ours (crate::two_front puts its own there
    // in a two-front battle: its Front; a pair formed there takes over, the
    // second front being over by then).
    let now = core.raw_read_32(MENU_POOL, -1);
    let want = if battle {
        MENU
    } else if now == MENU {
        AW2_MENU
    } else {
        return;
    };
    if now != want {
        core.raw_write_32(MENU_POOL, -1, want);
    }
}

/// Without the pack the menu is AW2's (the pool word put back).
pub fn put_back(core: &mut Core) {
    if core.raw_read_32(MENU_POOL, -1) == MENU {
        core.raw_write_32(MENU_POOL, -1, AW2_MENU);
    }
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    let _ = LANDING_OTHER;
    vec![
        (LANDING, Box::new(landing)),
        (MENU_Y, Box::new(menu_y)),
        (TURN_START, Box::new(turn_start)),
        (AI_POWER, Box::new(ai_power)),
        (AI_SUPER, Box::new(ai_super)),
        (AI_END, Box::new(ai_end)),
        (AI_FACTORY, Box::new(ai_factory)),
        (DRAW_PANEL, Box::new(draw_panel)),
        (PICK_COUNT, Box::new(pick_count)),
    ]
}

// --- A suspended game ----------------------------------------------------------------------

/// The pairs, as a suspended game keeps them ([`crate::suspend`]): per army
/// 1..5 the partner, the phase, its power count, its announcement byte and
/// its meter (8 bytes); the skills are the mode's, worked out again.
const SAVED_REC: usize = 8;
pub const SAVED_LEN: usize = SAVED_REC * ARMIES as usize;

pub fn saved(core: &Core) -> Vec<u8> {
    let mut b = vec![0u8; SAVED_LEN];
    for a in 1..=ARMIES {
        let o = SAVED_REC * (a as usize - 1);
        core.raw_read_range(rec(a), -1, &mut b[o..o + SAVED_REC]);
        if core.raw_read_8(STATE + 0xFC, -1) != MAGIC_RAM {
            b[o] = NONE;
        }
    }
    b
}

/// A suspended game continued: its pairs (none from a game saved without).
pub fn restore(core: &mut Core, b: Option<&[u8]>) {
    clear_pairs(core);
    let Some(b) = b.filter(|b| b.len() == SAVED_LEN) else { return };
    for a in 1..=ARMIES {
        let o = SAVED_REC * (a as usize - 1);
        let r = &b[o..o + SAVED_REC];
        if r[0] == NONE {
            continue;
        }
        let charge = u32::from_le_bytes(r[4..8].try_into().unwrap());
        form_pair(core, a, r[0], 0);
        core.raw_write_32(rec(a) + P_CHARGE, -1, charge);
        core.raw_write_8(rec(a) + P_PHASE, -1, r[1]);
        core.raw_write_8(rec(a) + P_USES, -1, r[2]);
        core.raw_write_8(rec(a) + P_ANNOUNCE, -1, r[3]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ram_fits() {
        assert!(STATE + REC * ARMIES <= TEAMS_PARTNER);
        assert!(PENDING + 2 * ARMIES <= UI);
        assert!(UI + 0x10 <= STATE + 0xFC);
        assert!(STATE >= 0x0203_F400 && STATE_END <= 0x0203_F600, "below the DS Campaign's records");
    }

    #[test]
    fn stubs() {
        let s = stub(T_TAG);
        assert_eq!(u32::from_le_bytes(s[12..16].try_into().unwrap()), LANDING | 1);
        assert_ne!(LANDING, LANDING_OTHER);
    }

    #[test]
    fn star_costs() {
        assert_eq!(star_cost(0), 9000);
        assert_eq!(star_cost(1), 10800);
        assert_eq!(star_cost(10), 18000);
    }
}
