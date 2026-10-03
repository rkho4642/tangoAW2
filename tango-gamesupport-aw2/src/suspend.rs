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
//!
//! **A DS Campaign mission saved halfway** (Dual Strike's campaign has the
//! map menu's Save: "Save over Mission / Day data"). AW2 saves a campaign
//! mission in slot 2 and marks it in the profile (`0x0200C429`), and AW2
//! CAMPAIGN's Continue resumes it; a DS mission goes to its own slot,
//! [`DS_SLOT`], instead, so AW2's saved mission and its mark are left alone:
//! `sub_08016D30` is trapped at its start (in a session, slot 2: the mark
//! as it is noted) and at its call of the slot writer (the slot made
//! [`DS_SLOT`] and the mark put back, before the writer serializes the
//! profile). The block's tail also gets the session's own state after
//! [`DS_MARK`]: the mission, its flags ([`crate::ds_campaign`]'s 0x20..0x9F),
//! the countdown of Dual Strike's op 0x5A, Means to an End's state (the
//! Hard choice is the flags' 0x60, the records are in the DS record). DS CAMPAIGN's Continue resumes
//! the mission when [`DS_SLOT`] is in AW2's sector directory (the session
//! set up as for the world map, then AW2's own resume, `sub_08017688`); the
//! slot is dropped from the directory (AW2's delete, `sub_0801ABF8`, without
//! its write: the next save writes it) when the mission ends and on a new
//! DS Campaign.

use mgba::core::Core;

use crate::ds_weather::is_on;

/// The block's staging buffer, its unused tail, and its length.
const BLOCK: u32 = 0x0200_0000;
const TAIL: u32 = BLOCK + 0xDAC;
#[cfg(test)]
const BLOCK_LEN: u32 = 0xE28;
/// The length `sub_08016D30` writes.
const BLOCK_WRITE: u32 = 0xE28;
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
const GAME_MODE: u32 = 0x0300_3FC1;
const VERSUS: u8 = 3;

/// The Versus rule Skills ([`crate::versus_rules`]) and the CO tag pairs
/// ([`crate::tag`]) ride past the 0xE28-byte block: while the rule is on or
/// a battle has pairs the block is written longer (the staging
/// buffer is 0x2000 bytes; AW2's reader takes the slot's whole length back).
/// A mark, a hash of AW2's part of the block (+0..+0xDAC, so a stale copy
/// from another save is never taken), the rule (and three spare bytes),
/// then the pairs.
const TAG_AT: u32 = BLOCK + 0xE28;
const TAG_MARK: u32 = 0x3247_4154; // "TAG2"
const RULES: [u32; 1] = [crate::co_skills::VERSUS_RULE];
pub const TAG_LEN: u32 = 8 + 4 + crate::tag::SAVED_LEN as u32;

fn block_hash(core: &Core) -> u32 {
    let mut b = vec![0u8; 0xDAC];
    core.raw_read_range(BLOCK, -1, &mut b);
    b.iter().fold(0x811C_9DC5u32, |h, &x| (h ^ x as u32).wrapping_mul(0x0100_0193))
}

fn tag_tail_ok(core: &Core) -> bool {
    core.raw_read_32(TAG_AT, -1) == TAG_MARK && core.raw_read_32(TAG_AT + 4, -1) == block_hash(core)
}

fn captured(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let rules = RULES.map(|r| core.raw_read_8(r, -1) & 1);
    let versus = core.raw_read_8(GAME_MODE, -1) == VERSUS;
    if crate::tag::any(core) || (versus && rules != [0]) {
        let mut t = Vec::with_capacity(TAG_LEN as usize);
        t.extend_from_slice(&TAG_MARK.to_le_bytes());
        t.extend_from_slice(&block_hash(core).to_le_bytes());
        t.extend_from_slice(&[rules[0], 0, 0, 0]);
        t.extend_from_slice(&crate::tag::saved(core));
        core.raw_write_range(TAG_AT, -1, &t);
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
    if tag_tail_ok(core) {
        // The rules as the game was saved with them; the skills the armies
        // had on (crate::co_skills: a map start sets them, and a Continue
        // has none), then the pairs (their partners' skills).
        for (k, r) in RULES.iter().enumerate() {
            let v = core.raw_read_8(TAG_AT + 8 + k as u32, -1);
            core.raw_write_8(*r, -1, v);
        }
        if core.raw_read_8(GAME_MODE, -1) == VERSUS {
            crate::co_skills::clear(core);
            crate::co_skills::battle_start(core);
        }
        let mut t = vec![0u8; crate::tag::SAVED_LEN];
        core.raw_read_range(TAG_AT + 12, -1, &mut t);
        crate::tag::restore(core, Some(&t));
        // Read once: a later Continue of a game saved without pairs does
        // not take these.
        core.raw_write_32(TAG_AT, -1, 0);
    } else {
        crate::tag::restore(core, None);
    }
}

// --- A DS Campaign mission saved halfway ---------------------------------------

/// The Flash slot (save tag) of a DS mission saved halfway (AW2: 0 profile,
/// 2..4 suspends, 5..8 design maps; tangoAW2: 15 the DS Campaign's record).
pub const DS_SLOT: u8 = 14;
const CAMPAIGN_SLOT: u32 = 2;
/// The session's state in the block's tail, after the "TAW2" part: +0
/// [`DS_MARK`], +2 the mission, +3 version, +4 the countdown (u32), +8 the
/// flags (16 bytes), +24 the mission's state (Means to an End's, Ring of Fire's: `MTE_LEN` bytes),
/// then Crystal Calamity's Black Onyx (crate::onyx, `SAVED_LEN` bytes).
const DS_AT: u32 = TAIL + TAIL_LEN;
const DS_MARK: u16 = 0x5344; // "DS"
const DS_MTE_LEN: u32 = crate::ds_campaign_rules::MTE_LEN;
const DS_LEN: u32 = 8 + 16 + DS_MTE_LEN + crate::onyx::SAVED_LEN as u32;
/// AW2's campaign-suspend mark in the profile (`sub_08016C9C(2)`).
const CAMPAIGN_MARK: u32 = 0x0200_C429;
/// `sub_08016D30(slot, ..)` past its prologue (`cmp r4, #0`: r4 the slot), and its `bl sub_0801A7D8`
/// (r0 the slot, the block not yet written).
const SAVE_START: u32 = 0x0801_6D3A;
const SAVE_WRITE: u32 = 0x0801_6D8E;
/// While a DS mission is being saved: 0x80 | AW2's campaign mark as it was.
const DS_SAVING: u32 = 0x0203_FFAD;
/// AW2's sector directory in RAM (`gUnknown_0200CC38`): +0x00 each
/// sector's slot, +0x10 the directory the next write saves.
const DIRECTORY: u32 = 0x0200_CC38;
/// AW2's resume of a saved game (`sub_08017688(slot)`).
const RESUME: u32 = 0x0801_7688;
const FLASH: u32 = 0x0E00_0000;

fn save_start(core: &mut Core) {
    // (past its prologue: r4 the slot; five's patches hook the first instruction)
    if !crate::ds_campaign::active(core) || core.gba().cpu().gpr(4) as u32 != CAMPAIGN_SLOT {
        return;
    }
    let mark = core.raw_read_8(CAMPAIGN_MARK, -1);
    core.raw_write_8(DS_SAVING, -1, 0x80 | mark);
}

fn save_write(core: &mut Core) {
    if is_on(core) && core.gba().cpu().gpr(2) as u32 == BLOCK_WRITE && tag_tail_ok(core) {
        core.gba_mut().cpu_mut().set_gpr(2, (BLOCK_WRITE + TAG_LEN) as i32);
    }
    let saving = core.raw_read_8(DS_SAVING, -1);
    if saving & 0x80 == 0 {
        return;
    }
    core.raw_write_8(DS_SAVING, -1, 0);
    core.raw_write_8(CAMPAIGN_MARK, -1, saving & 0x7F);
    let mut b = vec![0u8; DS_LEN as usize];
    b[0..2].copy_from_slice(&DS_MARK.to_le_bytes());
    b[2] = core.raw_read_8(crate::ds_campaign::MISSION, -1);
    b[3] = VERSION;
    b[4..8].copy_from_slice(&core.raw_read_32(crate::ds_campaign::COUNTDOWN, -1).to_le_bytes());
    core.raw_read_range(crate::ds_campaign::FLAGS, -1, &mut b[8..24]);
    core.raw_read_range(crate::ds_campaign_rules::MTE_TOLD, -1, &mut b[24..24 + DS_MTE_LEN as usize]);
    b[24 + DS_MTE_LEN as usize..].copy_from_slice(&crate::onyx::saved(core));
    core.raw_write_range(DS_AT, -1, &b);
    core.gba_mut().cpu_mut().set_gpr(0, DS_SLOT as i32);
    // A battle on two fronts (crate::two_front): the front not on the
    // screen and the battle's state follow the block (the slot takes more
    // than one sector: AW2's writer splits a record in 0xFAD-byte parts).
    if let Some(fronts) = crate::two_front::saved_state(core) {
        let mut t = FRONTS_MARK.to_le_bytes().to_vec();
        t.extend_from_slice(&fronts);
        core.raw_write_range(BLOCK + BLOCK_SIZE, -1, &t);
        core.gba_mut().cpu_mut().set_gpr(2, (BLOCK_SIZE + t.len() as u32) as i32);
    }
}

/// The block's length as AW2 saves it, and the mark of the two-front state
/// saved after it.
const BLOCK_SIZE: u32 = 0xE28;
const FRONTS_MARK: u32 = 0x5446_3254; // "T2FT"

/// The block just applied by a DS mission's Continue: the session's flags
/// and countdown.
fn applied_ds(core: &mut Core) {
    if !crate::ds_campaign::active(core) || core.raw_read_16(DS_AT, -1) != DS_MARK {
        return;
    }
    let mut b = vec![0u8; DS_LEN as usize];
    core.raw_read_range(DS_AT, -1, &mut b);
    core.raw_write_32(crate::ds_campaign::COUNTDOWN, -1, u32::from_le_bytes(b[4..8].try_into().unwrap()));
    core.raw_write_range(crate::ds_campaign::FLAGS, -1, &b[8..24]);
    core.raw_write_range(crate::ds_campaign_rules::MTE_TOLD, -1, &b[24..24 + DS_MTE_LEN as usize]);
    crate::onyx::restore(core, &b[24 + DS_MTE_LEN as usize..]);
    // A two-front battle's other front and state (saved after the block).
    if core.raw_read_32(BLOCK + BLOCK_SIZE, -1) == FRONTS_MARK {
        let mut f = vec![0u8; crate::two_front::SAVED_LEN as usize];
        core.raw_read_range(BLOCK + BLOCK_SIZE + 4, -1, &mut f);
        crate::two_front::restore_saved(core, &f);
        core.raw_write_32(BLOCK + BLOCK_SIZE, -1, 0);
    }
}

/// The sector holding the DS mission saved halfway, if AW2's directory
/// lists one.
fn ds_sector(core: &Core) -> Option<u32> {
    // (its first part: a two-front battle's takes two sectors; the sector's
    // +0x0C is part << 4 | parts - 1)
    (0..16).find(|&i| core.raw_read_8(DIRECTORY + i, -1) == DS_SLOT && core.raw_read_8(FLASH + 0x1000 * i + 0x0C, -1) >> 4 == 0)
}

/// The DS mission saved halfway (its index), read from Flash.
pub fn ds_saved_mission(core: &Core) -> Option<u8> {
    let at = FLASH + 0x1000 * ds_sector(core)? + 0x52 + (DS_AT - BLOCK);
    (core.raw_read_16(at, -1) == DS_MARK).then(|| core.raw_read_8(at + 2, -1))
}

/// DS CAMPAIGN's Continue over a mission saved halfway (the session set
/// up): AW2's own resume of [`DS_SLOT`], tail-called from the trapped
/// Continue handler's first instruction (it returns to its caller).
pub fn resume_ds(core: &mut Core) {
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, DS_SLOT as i32);
    cpu.set_thumb_pc(RESUME);
}

/// The DS mission saved halfway is over (won or lost) or a new DS Campaign
/// is started: its slot leaves AW2's directory as AW2's delete does it
/// (`sub_0801ABF8`), to be written with the next save.
pub fn drop_ds(core: &mut Core) {
    for i in 0..16 {
        if core.raw_read_8(DIRECTORY + i, -1) == DS_SLOT {
            core.raw_write_8(DIRECTORY + i, -1, 0xFF);
            core.raw_write_8(DIRECTORY + 0x10 + i, -1, 0xFF);
        }
    }
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![
        (CAPTURED, Box::new(captured)),
        (APPLIED, Box::new(|core: &mut Core| {
            applied(core);
            applied_ds(core);
        })),
        (SAVE_START, Box::new(save_start)),
        (SAVE_WRITE, Box::new(save_write)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_fits_the_block() {
        assert!(TAIL + TAIL_LEN <= BLOCK + BLOCK_LEN);
        assert!(DS_AT + DS_LEN <= BLOCK + BLOCK_LEN);
        // The longer write still fits one Flash sector (payload from +0x52,
        // the profile's directory from +0xFEF).
        assert!(0x52 + BLOCK_LEN + TAG_LEN <= 0xFEF);
        assert_eq!(TAG_AT, BLOCK + BLOCK_LEN);
    }
}
