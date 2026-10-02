//! The Set Skills panel (crate::co_skills): on AW2's CO screens, a panel of
//! the highlighted CO's skills, with the Dual Strike pack.
//!
//! - Where: the War Room's CO screen (also the DS Campaign's, Survival's
//!   and AW2's campaign's: `ProcScr_CoSelect` 0x08616638, at its input
//!   stage, its idle callback `CoSelect_IDLE` 0x0807CE5D), SELECT opens it
//!   for the CO highlighted (the carousel's `+0x52` of its group `+0x58`,
//!   the group's list offset `+0x5C`, the CO list 0x030058E0, group sizes
//!   0x03005948), editing the set of the mode: the DS Campaign's and AW2's
//!   campaign's (Campaign), Survival's, the War Room's. On Versus' Teams
//!   screen (on an army's CO stop: the record `0x02017C50`, cursor `+0x32`
//!   even, CO list `+0x18`, index `+0x1C + army`), START opens it for that
//!   army's CO, editing its Versus set; there L turns the Versus rule
//!   Skills on or off (crate::co_skills::VERSUS_RULE).
//! - The panel: the CO's rank and slots (min(rank, 4)), one line per slot,
//!   the description of the skill on the slot picked. UP/DOWN pick a slot,
//!   LEFT/RIGHT go through the skills open to the CO (rank, or Means to an
//!   End won for the rank-10 ones) and none, A keeps the set (saved with
//!   the next save, crate::co_skills), B closes it as it was.
//! - While it is up the game gets no button ([`tick`]).
//! - Drawn as four 64x32 sprites of AW2's glyph font (`0x080A1424`) on a
//!   plate, in OBJ tiles each screen leaves unused while the panel is up
//!   (CO screen: its face slots 0x1EC.., which the game reloads before it
//!   shows them; Teams: 0x090..), OBJ palette 14 (unused on both), in
//!   front of the game's sprites (put first at the sprite flush, [`flush`]).
//!
//! Everything is in emulated RAM: in netplay both seats' buttons reach the
//! Teams screen and the console boots from seat 0's save, so both peers see
//! the same sets and rule.

use mgba::core::Core;

use crate::co_skills::{self, Set};

// --- State (EWRAM after the skill data) ------------------------------------------

const STATE: u32 = 0x0203_E3A0;
const OPEN: u32 = STATE;
const SLOT: u32 = STATE + 1;
const CO: u32 = STATE + 2;
/// The set being edited: 0 Campaign, 1 Survival, 2 War Room, 3 Versus 0.
const WHICH: u32 = STATE + 3;
const IDS: u32 = STATE + 4;
/// 1 on the Teams screen (the rule's line).
const ON_TEAMS: u32 = STATE + 8;
#[cfg(test)]
const STATE_LEN: u32 = 9;

fn set_code(s: Set) -> u8 {
    match s {
        Set::Campaign => 0,
        Set::Survival => 1,
        Set::WarRoom => 2,
        Set::Versus(_) => 3,
    }
}

fn code_set(c: u8) -> Set {
    match c {
        0 => Set::Campaign,
        1 => Set::Survival,
        2 => Set::WarRoom,
        _ => Set::Versus(0),
    }
}

pub fn is_open(core: &Core) -> bool {
    core.raw_read_8(OPEN, -1) == 1
}

// --- The screens -----------------------------------------------------------------

const PROC_POOL: (u32, u32) = (0x0200_D610, 0x0200_E418);
const PROC_SIZE: u32 = 0x6C;
const CO_SELECT: u32 = 0x0861_6638;
const CO_SELECT_IDLE: u32 = 0x0807_CE5D;
const CO_LIST: u32 = 0x0300_58E0;
const GROUP_SIZES: u32 = 0x0300_5948;
const GAME_MODE: u32 = 0x0300_3FC1;

/// The CO screen at its input stage: the CO highlighted, and the set the mode
/// uses.
fn co_select(core: &Core) -> Option<(u8, Set)> {
    let proc = (PROC_POOL.0..PROC_POOL.1)
        .step_by(PROC_SIZE as usize)
        .find(|&p| core.raw_read_32(p, -1) == CO_SELECT && core.raw_read_32(p + 0x10, -1) == CO_SELECT_IDLE)?;
    if core.raw_read_16(proc + 0x4E, -1) != 0 || core.raw_read_16(proc + 0x60, -1) != 0 {
        return None;
    }
    let group = core.raw_read_32(proc + 0x58, -1);
    let size = core.raw_read_8(GROUP_SIZES + group.min(15), -1).max(1) as u32;
    let k = core.raw_read_16(proc + 0x52, -1) as u32 % size + core.raw_read_32(proc + 0x5C, -1);
    let co = core.raw_read_8(CO_LIST + k.min(63), -1);
    co_skills::co_slot(co)?;
    let set = if crate::ds_campaign::active(core) {
        Set::Campaign
    } else if crate::survival::on(core) {
        Set::Survival
    } else if core.raw_read_8(GAME_MODE, -1) == 1 {
        Set::Campaign
    } else {
        Set::WarRoom
    };
    Some((co, set))
}

const TEAMS: u32 = 0x0201_7C50;

/// Versus' Teams screen on an army's CO stop: the army's CO.
fn teams(core: &Core) -> Option<u8> {
    if !crate::pvp::on_teams_screen(core) || crate::five::active(core) {
        return None;
    }
    if core.raw_read_8(TEAMS + 0x30, -1) != 1
        || core.raw_read_8(TEAMS + 0x26, -1) != 0
        || core.raw_read_8(TEAMS + 0x2D, -1) != 0
        || core.raw_read_8(TEAMS + 0x24, -1) != 0
    {
        return None;
    }
    let cursor = core.raw_read_8(TEAMS + 0x32, -1) as u32;
    if cursor % 2 != 0 {
        return None;
    }
    let army = cursor / 2;
    let list = core.raw_read_32(TEAMS + 0x18, -1);
    if !(0x0200_0000..0x0400_0000).contains(&list) {
        return None;
    }
    let co = core.raw_read_8(list + core.raw_read_8(TEAMS + 0x1C + army, -1) as u32, -1);
    co_skills::co_slot(co).map(|_| co)
}

// --- Input -----------------------------------------------------------------------

const KEY_A: u32 = 1 << 0;
const KEY_B: u32 = 1 << 1;
const KEY_SELECT: u32 = 1 << 2;
const KEY_START: u32 = 1 << 3;
const KEY_RIGHT: u32 = 1 << 4;
const KEY_LEFT: u32 = 1 << 5;
const KEY_UP: u32 = 1 << 6;
const KEY_DOWN: u32 = 1 << 7;
const KEY_L: u32 = 1 << 9;
const ALL_KEYS: u32 = 0x3FF;

fn ids(core: &Core) -> [u8; 4] {
    let mut b = [0u8; 4];
    core.raw_read_range(IDS, -1, &mut b);
    b
}

/// The choices for slot `k`: none, then every skill open to the CO not on
/// another slot.
fn choices(core: &mut Core, co: u8, k: usize) -> Vec<u8> {
    let cur = ids(core);
    let mut out = vec![0u8];
    for id in co_skills::ids() {
        if co_skills::unlocked(core, co, id) && !cur.iter().enumerate().any(|(j, &x)| j != k && x == id) {
            out.push(id);
        }
    }
    out
}

/// Every frame (before the game reads the pad): open, run or close the
/// panel; while it is up the game gets no button. `keys` the frame's,
/// `prev` the last frame's. Returns the keys the game gets.
pub fn tick(core: &mut Core, ds: bool, keys: u32, prev: u32) -> u32 {
    if !ds {
        if is_open(core) {
            core.raw_write_8(OPEN, -1, 0);
        }
        return keys;
    }
    let pressed = keys & !prev;
    if !is_open(core) {
        let target = co_select(core)
            .filter(|_| pressed & KEY_SELECT != 0)
            .map(|(co, s)| (co, s, false))
            .or_else(|| teams(core).filter(|_| pressed & KEY_START != 0).map(|co| (co, Set::Versus(0), true)));
        if let Some((co, set, on_teams)) = target {
            crate::ds_campaign::skills_loaded(core);
            core.raw_write_8(OPEN, -1, 1);
            core.raw_write_8(SLOT, -1, 0);
            core.raw_write_8(CO, -1, co);
            core.raw_write_8(WHICH, -1, set_code(set));
            core.raw_write_range(IDS, -1, &co_skills::set_of(core, co, set));
            core.raw_write_8(ON_TEAMS, -1, on_teams as u8);
            return keys & !ALL_KEYS;
        }
        return keys;
    }
    // The screen went away under the panel: it closes as it was.
    let on_teams = core.raw_read_8(ON_TEAMS, -1) == 1;
    if (on_teams && teams(core).is_none()) || (!on_teams && co_select(core).is_none()) {
        core.raw_write_8(OPEN, -1, 0);
        return keys & !ALL_KEYS;
    }
    let co = core.raw_read_8(CO, -1);
    let n = co_skills::slots(core, co);
    let mut slot = core.raw_read_8(SLOT, -1) as usize;
    if pressed & KEY_B != 0 {
        core.raw_write_8(OPEN, -1, 0);
    } else if pressed & KEY_A != 0 {
        let set = code_set(core.raw_read_8(WHICH, -1));
        let mut kept = [0u8; 4];
        for (k, &id) in ids(core).iter().take(n).filter(|&&id| id != 0).enumerate() {
            kept[k] = id;
        }
        co_skills::store_set(core, co, set, kept);
        core.raw_write_8(OPEN, -1, 0);
    } else if n > 0 {
        if pressed & KEY_UP != 0 {
            slot = (slot + n - 1) % n;
        }
        if pressed & KEY_DOWN != 0 {
            slot = (slot + 1) % n;
        }
        let step = if pressed & KEY_RIGHT != 0 { 1 } else if pressed & KEY_LEFT != 0 { -1 } else { 0 };
        if step != 0 {
            let options = choices(core, co, slot);
            let cur = ids(core)[slot];
            let at = options.iter().position(|&x| x == cur).unwrap_or(0) as i32;
            let next = options[(at + step).rem_euclid(options.len() as i32) as usize];
            core.raw_write_8(IDS + slot as u32, -1, next);
        }
        core.raw_write_8(SLOT, -1, slot as u8);
    }
    if on_teams && pressed & KEY_L != 0 {
        let v = core.raw_read_8(co_skills::VERSUS_RULE, -1);
        core.raw_write_8(co_skills::VERSUS_RULE, -1, (v == 0) as u8);
    }
    keys & !ALL_KEYS
}

// --- Drawing ---------------------------------------------------------------------

/// The lines of the panel (16 glyphs each, AW2's font: letters, digits and
/// `$!?().,/%-`).
fn lines(core: &mut Core) -> Vec<String> {
    let co = core.raw_read_8(CO, -1);
    let n = co_skills::slots(core, co);
    let slot = core.raw_read_8(SLOT, -1) as usize;
    let cur = ids(core);
    let mut out = vec![format!("RANK {} SLOTS {}", co_skills::rank(core, co), n)];
    if n == 0 {
        out.push("NO SKILLS YET".into());
        out.push("WIN BATTLES".into());
        out.push("FOR EXP".into());
    }
    for k in 0..n {
        let name = match co_skills::info(cur[k]) {
            Some((_, name, _)) => plain(&name),
            None => "-".into(),
        };
        out.push(if k == slot { format!("({}){}", k + 1, name) } else { format!(" {} {}", k + 1, name) });
    }
    let desc = if n > 0 { co_skills::info(cur[slot]).map(|(_, _, d)| plain(&d)).unwrap_or_default() } else { String::new() };
    for l in wrap(&desc, 16).into_iter().take(2) {
        out.push(l);
    }
    while out.len() < 7 {
        out.push(String::new());
    }
    if core.raw_read_8(ON_TEAMS, -1) == 1 {
        let on = core.raw_read_8(co_skills::VERSUS_RULE, -1) == 1;
        out.push(format!("L RULE {}", if on { "ON" } else { "OFF" }));
    } else {
        out.push("A SET B BACK".into());
    }
    out.truncate(8);
    out
}

/// A Dual Strike text as the font's characters (upper case; '+' as UP).
fn plain(t: &[u8]) -> String {
    let mut s = String::new();
    for &c in t {
        match c {
            b'+' => s.push_str("UP "),
            b'\r' | b'\n' | 0x0E | 0x0F => s.push(' '),
            b'a'..=b'z' => s.push((c - 32) as char),
            b'A'..=b'Z' | b'0'..=b'9' | b' ' | b'$' | b'!' | b'?' | b'(' | b')' | b'.' | b',' | b'/' | b'%' | b'-' => s.push(c as char),
            b':' => s.push(' '),
            _ => {}
        }
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn wrap(s: &str, w: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = String::new();
    for word in s.split(' ') {
        if !line.is_empty() && line.len() + 1 + word.len() > w {
            out.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(&word[..word.len().min(w)]);
    }
    if !line.is_empty() {
        out.push(line);
    }
    out
}

const FONT: u32 = 0x080A_1424;
const OBJ_VRAM: u32 = 0x0601_0000;
const PAL_BUFFER: u32 = 0x0300_20C0;
const PAL_RAM: u32 = 0x0500_0000;
const PALETTE: u32 = 14;
/// The plate, the glyphs' fill and outline (OBJ palette 14).
const PLATE: u8 = 14;
const COLOURS: [(usize, u16); 3] = [(1, 0x7FFF), (14, 0x1063), (15, 0x0000)];
/// The panel's OBJ tiles: four 64x32 blocks.
const CO_SELECT_TILES: u32 = 0x1EC;
const TEAMS_TILES: u32 = 0x090;

fn glyph(c: char) -> Option<u32> {
    Some(match c {
        '0'..='9' => 0x3D0 + (c as u32 - '0' as u32),
        'A'..='Z' => 0x3E0 + (c as u32 - 'A' as u32),
        '$' => 0x3DA,
        '!' => 0x3DB,
        '?' => 0x3DC,
        '(' => 0x3FA,
        ')' => 0x3FB,
        '.' | ',' => 0x3FC,
        '/' => 0x3FD,
        '%' => 0x3FE,
        '-' => 0x3FF,
        _ => return None,
    })
}

/// The panel's tiles (glyphs on the plate) and palette, written when they
/// change; returns the first tile.
fn render(core: &mut Core) -> u32 {
    let first = if core.raw_read_8(ON_TEAMS, -1) == 1 { TEAMS_TILES } else { CO_SELECT_TILES };
    let mut buf = vec![0u8; 128 * 32];
    for (row, line) in lines(core).iter().enumerate() {
        for (col, c) in line.chars().take(16).enumerate() {
            let Some(t) = glyph(c) else { continue };
            let block = (row / 4) * 2 + col / 8;
            let tile = block * 32 + (row % 4) * 8 + col % 8;
            core.raw_read_range(FONT + 32 * (t - 0x3C0), -1, &mut buf[tile * 32..tile * 32 + 32]);
        }
    }
    for b in buf.iter_mut() {
        if *b & 0x0F == 0 {
            *b |= PLATE;
        }
        if *b & 0xF0 == 0 {
            *b |= PLATE << 4;
        }
    }
    let at = OBJ_VRAM + 32 * first;
    let mut now = vec![0u8; buf.len()];
    core.raw_read_range(at, -1, &mut now);
    if now != buf {
        core.raw_write_range(at, -1, &buf);
    }
    for (i, colour) in COLOURS {
        for base in [PAL_BUFFER + 0x200, PAL_RAM + 0x200] {
            core.raw_write_16(base + 32 * PALETTE + 2 * i as u32, -1, colour);
        }
    }
    first
}

/// At the VBlank sprite flush (crate::branding::flush, last): the panel's
/// four sprites put first in the list (in front of every other sprite), the
/// game's moved down. Returns the list's new end.
pub fn flush(core: &mut Core, start: u32, at: u32, end: u32) -> u32 {
    if !is_open(core) || at + 4 * 8 > end {
        return at;
    }
    let first = render(core);
    let n = (at - start) as usize;
    let mut moved = vec![0u8; n];
    core.raw_read_range(start, -1, &mut moved);
    core.raw_write_range(start + 4 * 8, -1, &moved);
    let (x, y) = (56, 16);
    for block in 0..4u32 {
        let (bx, by) = (x + 64 * (block % 2), y + 32 * (block / 2));
        let s = start + 8 * block;
        // 64x32: shape wide, size 3; priority 0, palette 14.
        core.raw_write_16(s, -1, (by as u16 & 0xFF) | 0x4000);
        core.raw_write_16(s + 2, -1, (bx as u16 & 0x1FF) | 0xC000);
        core.raw_write_16(s + 4, -1, (first + 32 * block) as u16 | (PALETTE as u16) << 12);
        core.raw_write_16(s + 6, -1, 0);
    }
    at + 4 * 8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_fits() {
        assert!(STATE >= co_skills::VERSUS_RULE + 1 && STATE + STATE_LEN <= 0x0203_F600);
    }

    #[test]
    fn text() {
        assert_eq!(plain(b"Direct attack +5%"), "DIRECT ATTACK UP 5%");
        assert_eq!(wrap("DIRECT ATTACK UP 5%", 16), vec!["DIRECT ATTACK UP", "5%"]);
    }
}
