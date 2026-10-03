//! Intel > General: Dual Strike's ally posture, on two fronts (docs/AW2.md
//! "Two fronts", "General").
//!
//! In Dual Strike's two-front missions the main front's Intel menu has a
//! "General" item (bank 0xC0 texts 98..101, the AI icon): one of four
//! postures, Strike ("Set ally to aggressive posture."), Assault
//! ("offensive"), General ("general, all-purpose") and Defense
//! ("defensive"), shown one at a time; A turns it to the next (General,
//! Defense, Strike, Assault). It is a byte of the army's player record
//! (+0x2E) that Dual Strike's CPU reads on each of its turns. Here it is a
//! posture per army in [`crate::two_front`]'s state; this module gives its
//! labels, help lines and icon (from the .nds), and its effect on AW2's
//! CPU:
//!
//! AW2's CPU gives each unit a role (its unit record's +0x0B: the
//! deployment's AI byte, or what the CPU's production gives a unit it
//! buys), and after its attack check moves the unit by that role
//! (`AiRunRoleMove`, `0x0805F4CC`, through the table at `0x085768E0`).
//! While an army has a posture other than General, its units are moved by
//! the role that matches it instead of their own:
//!
//! - Strike: 4, toward the nearest enemy units (and into them, whatever the
//!   odds: Dual Strike's Strike does not mark down a trade that costs it).
//! - Assault: 3, onto the enemy's properties, each unit given its own
//!   (Dual Strike's Assault pushes every advance to the top of its range).
//! - General: the units' own roles (Dual Strike's: its ranges at random).
//! - Defense: 0, where it stands (Dual Strike's Defense skips its advance
//!   steps); a unit still fires at what comes into its reach.
//!
//! Everything the posture changes is decided from emulated RAM (the
//! posture, the current army), so it is the same on both netplay peers
//! and in replays.

use mgba::core::Core;

use crate::two_front::{ASSAULT, DEFENSE, STRIKE};

/// `AiRunRoleMove`: the role move of the CPU's current unit
/// (`gUnknown_030040D8`, its +0x0B role), through [`ROLE_MOVES`].
pub const ROLE_MOVE: u32 = 0x0805_F4CC;
const ROLE_MOVES: u32 = 0x0857_68E0;
const CURRENT_ARMY: u32 = 0x0300_33EC;

/// AW2's role for each posture (None: the unit's own).
pub fn role(posture: u8) -> Option<u32> {
    match posture {
        STRIKE => Some(4),
        ASSAULT => Some(3),
        DEFENSE => Some(0),
        _ => None,
    }
}

/// `AiRunRoleMove`, trapped: in a two-front battle, the army's posture's
/// role instead of the unit's (a tail call into that role's move).
fn role_move(core: &mut Core) {
    let army = core.raw_read_16(CURRENT_ARMY, -1) as u32;
    let Some(r) = role(crate::two_front::cpu_posture(core, army)) else {
        return;
    };
    let f = core.raw_read_32(ROLE_MOVES + 4 * r, -1);
    if (0x0800_0000..0x0A00_0000).contains(&f) {
        core.gba_mut().cpu_mut().set_thumb_pc(f & !1);
    }
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![(ROLE_MOVE, Box::new(role_move))]
}

// --- From the .nds: labels, help lines, icon -----------------------------------------

/// Dual Strike's texts (bank 0xC0): the items' labels, by posture, and
/// their help lines.
const DS_LABELS: u32 = 0xC000_0000 | 98;
const DS_HELP: u32 = 0xC000_0000 | 707;

/// The icon's code in a label (`\x09` and a byte: AW2's menus draw the 16x16
/// icon of BG tiles `0x1B4 + 4 * (code - 0x80)`, BG palette 10). AW2's
/// icons are 0x80..0x95; Dual Strike's AI icon (its code 0xA1) gets 0xE6,
/// whose four tiles (0x34C..0x34F of BG0's characters) the battle map
/// leaves empty: they hold the icon while the Intel menu is up
/// ([`icon_tick`]).
pub const ICON_CODE: u8 = 0xE6;
const ICON_TILES: u32 = 0x0600_0000 + 32 * (0x1B4 + 4 * (ICON_CODE as u32 - 0x80));
const BG0CNT: u32 = 0x0400_0008;
/// Dual Strike's icon sheet (16x16 icons of four 4-bit tiles: top left, top
/// right, bottom left, bottom right, as AW2's) and the AI icon in it. Its
/// palette (`icon/res_icon_cl`, palette 0) is AW2's menu palette (BG 10),
/// colour for colour, so the pixels go in as they are.
const DS_ICONS: &str = "icon/res_icon0_cg_E";
const DS_AI_ICON: usize = 0x5F;

const FONT_WIDTHS: u32 = 0x084C_36E4;

fn ds() -> Option<crate::ds_campaign_data::Ds<'static>> {
    crate::ds_campaign_data::Ds::from_pack(crate::ds_pack::pack()?)
}

/// A Dual Strike text as one line of plain characters.
fn ds_line(ds: &crate::ds_campaign_data::Ds, r: u32) -> String {
    let t = ds.text(r).unwrap_or_default();
    let s: String = t
        .iter()
        .map(|&c| if c == b'\r' || c == b'\n' { ' ' } else { c as char })
        .collect();
    s.trim().to_string()
}

/// The items' names (Strike, Assault, General, Defense): Dual Strike's
/// labels without their icon (`\t`, `\xC2\xA1`) and padding.
pub fn names() -> Vec<String> {
    let Some(ds) = ds() else { return vec![String::new(); 4] };
    (0..4)
        .map(|k| {
            let t = ds.text(DS_LABELS + k).unwrap_or_default();
            t.iter()
                .filter(|c| c.is_ascii_alphanumeric())
                .map(|&c| c as char)
                .collect()
        })
        .collect()
}

/// The four labels in AW2's menu text: the icon, the name, padded with
/// AW2's narrow spaces (0x18..0x1F: 0..7 pixels and the gap) to the widest
/// so the menu keeps its width as the posture changes (Dual Strike pads
/// its labels the same way).
pub fn labels(core: &Core) -> Vec<Vec<u8>> {
    let names = names();
    let width = |s: &str| {
        s.bytes()
            .map(|c| core.raw_read_8(FONT_WIDTHS + c as u32, -1) as u32 + 1)
            .sum::<u32>()
    };
    let widest = names.iter().map(|n| width(n)).max().unwrap_or(0);
    names
        .iter()
        .map(|n| {
            let mut l = vec![0x09, ICON_CODE];
            l.extend(n.bytes());
            let mut left = widest - width(n);
            while left > 0 {
                let k = (left - 1).min(7);
                l.push(0x18 + k as u8);
                left -= k + 1;
            }
            l.push(0);
            l
        })
        .collect()
}

static HELP: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

/// The posture's help line (Dual Strike's texts 707..710, one line).
pub fn help(posture: u8) -> &'static str {
    let h = HELP.get_or_init(|| match ds() {
        Some(ds) => (0..4).map(|k| ds_line(&ds, DS_HELP + k)).collect(),
        None => vec![String::new(); 4],
    });
    h.get(posture as usize).map(|s| s.as_str()).unwrap_or("")
}

/// Dual Strike's AI icon: its four tiles (128 bytes).
pub fn icon() -> Option<&'static [u8]> {
    let f = crate::ds_pack::pack()?.file(DS_ICONS)?;
    f.get(128 * DS_AI_ICON..128 * (DS_AI_ICON + 1))
}

/// Every frame of a two-front battle: the icon's tiles hold Dual Strike's
/// AI icon while the Intel menu (our copy) is up, if the map left them
/// empty; empty again once it is gone.
pub fn icon_tick(core: &mut Core, menu_up: bool) {
    let Some(icon) = icon() else { return };
    if (core.raw_read_16(BG0CNT, -1) >> 2) & 3 != 0 {
        return;
    }
    let mut now = [0u8; 128];
    core.raw_read_range(ICON_TILES, -1, &mut now);
    if menu_up && now.iter().all(|&b| b == 0) {
        core.raw_write_range(ICON_TILES, -1, icon);
    } else if !menu_up && now[..] == *icon {
        core.raw_write_range(ICON_TILES, -1, &[0u8; 128]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::two_front::GENERAL;

    /// From the .nds (with TANGOAW2_DS_ROM): the four labels and help lines
    /// are Dual Strike's and the AI icon is its sheet's 0x5F (its colours
    /// against AW2's menu palette: tools/aw2test's
    /// `two_front_general_menu`).
    #[test]
    #[ignore]
    fn from_the_nds() {
        let Some(path) = std::env::var_os("TANGOAW2_DS_ROM") else {
            return;
        };
        let Ok(rom) = std::fs::read(path) else { return };
        crate::ds_art::offer(&rom);
        assert!(crate::ds_pack::pack().is_some(), "the pack");
        assert_eq!(names(), vec!["Strike", "Assault", "General", "Defense"]);
        assert_eq!(help(STRIKE), "Set ally to aggressive posture.");
        assert_eq!(help(ASSAULT), "Set ally to offensive posture.");
        assert_eq!(help(GENERAL), "Set ally to general, all-purpose posture.");
        assert_eq!(help(DEFENSE), "Set ally to defensive posture.");
        let icon = icon().unwrap();
        assert_eq!(icon.len(), 128);
        assert!(icon.iter().any(|&b| b != 0));
    }

    #[test]
    fn roles() {
        assert_eq!(
            [STRIKE, ASSAULT, GENERAL, DEFENSE].map(role),
            [Some(4), Some(3), None, Some(0)]
        );
    }
}
