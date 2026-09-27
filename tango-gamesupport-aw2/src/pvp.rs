//! Advance Wars 2 on the shared console: who drives the pad, and the
//! runtime patches (Slippi-style memory writes before every frame).
//!
//! Addresses are for the USA cartridge (`AW2E`, CRC32 5AD0E571). Sources:
//! Xenesis' RAM notes on Wars World News, the CodeBreaker code list, the
//! aw2bhr decompilation, and probing with `gba_probe`.

use mgba::core::Core;

/// One army's block: funds, CO, colour, ... `0x3C` bytes per player slot.
const PLAYER_BLOCK: u32 = 0x0202_32C0;
const PLAYER_STRIDE: u32 = 0x3C;
/// Army colour byte inside a player block: 1 Orange Star, 2 Blue Moon,
/// 3 Green Earth, 4 Yellow Comet, 5 Black Hole.
const COLOUR: u32 = 0x1A;

/// The army whose turn it is, 1..=4. Left over after a battle ends.
const CURRENT_PLAYER: u32 = 0x0300_33EC;
/// First of seven battle-scene function pointers, filled for the whole
/// time a battle is loaded (every turn, menus and hand-off screens on the
/// map) and zero in menus, results and save prompts.
const BATTLE_SCENE: u32 = 0x0300_0004;
/// The main-loop callback. The full-screen CO page (map menu → CO) unloads
/// the battle scene while it is up and runs this callback instead.
const MAIN_LOOP: u32 = 0x0300_0000;
const CO_PAGE_LOOP: u32 = 0x0804_3591;
/// Which mode was picked on the title menu: 1 Campaign, 3 Versus,
/// 5 War Room. Set on entering the mode, kept through its menus and
/// battles.
const GAME_MODE: u32 = 0x0300_33FC;
const VERSUS: u8 = 3;
/// The map menu (CO, Intel, Options, Save, End): its state, 5 once an item
/// is chosen, and its cursor, 4 on End. Both together mean the turn was
/// ended and the game waits on the fog-of-war "Next turn" screen, where the
/// current player has not changed yet but the incoming player should press A.
const MAP_MENU_STATE: u32 = 0x0300_14E2;
const MAP_MENU_CURSOR: u32 = 0x0300_14F0;
/// Fog of war for the battle in progress, nonzero when on.
const FOG: u32 = 0x0300_3FCD;

/// The unlock block the game keeps in EWRAM and saves to Flash.
const HARD_CAMPAIGN: u32 = 0x0202_8030;
const SOUND_ROOM: u32 = 0x0202_8031;
/// Battle Maps shop: 13 halfwords of "bought" bits.
const BATTLE_MAPS: (u32, u32) = (0x0202_8040, 26);
/// COs available (16 bits), then CO colour edits (32 bits).
const COS_AND_EDITS: (u32, u32) = (0x0202_805A, 6);

/// The five armies in the order players pick them.
pub const ARMIES: [(u8, &str); 5] = [
    (1, "Orange Star"),
    (2, "Blue Moon"),
    (4, "Yellow Comet"),
    (3, "Green Earth"),
    (5, "Black Hole"),
];

/// The match types a lobby offers: type = army 1's pick, subtype = army
/// 2's pick among the four left.
pub const MATCH_TYPES: &[usize] = &[4, 4, 4, 4, 4];

/// The army colours for player slots 1..=4 under a match type: slots 1 and
/// 2 as picked, 3 and 4 the remaining armies in pick order.
pub fn slot_colours(match_type: (u8, u8)) -> [u8; 4] {
    let first = (match_type.0 as usize).min(ARMIES.len() - 1);
    let rest: Vec<usize> = (0..ARMIES.len()).filter(|&i| i != first).collect();
    let second = rest[(match_type.1 as usize).min(rest.len() - 1)];
    let mut others = (0..ARMIES.len()).filter(|&i| i != first && i != second);
    [
        ARMIES[first].0,
        ARMIES[second].0,
        ARMIES[others.next().unwrap()].0,
        ARMIES[others.next().unwrap()].0,
    ]
}

/// Which seat drives army slot `slot` (1..=4): odd slots are the first
/// player's, even slots the second's.
pub fn seat_of(slot: u8) -> usize {
    ((slot - 1) % 2) as usize
}

/// The title-menu mode and every army slot's colour byte, for tests.
pub fn army_state(core: &Core) -> (u8, [u8; 4]) {
    let colour = |slot: u32| core.raw_read_8(PLAYER_BLOCK + PLAYER_STRIDE * slot + COLOUR, -1);
    (
        core.raw_read_8(GAME_MODE, -1),
        [colour(0), colour(1), colour(2), colour(3)],
    )
}

pub struct Aw2;

pub static AW2E: Aw2 = Aw2;

fn in_battle(core: &Core) -> bool {
    core.raw_read_32(BATTLE_SCENE, -1) != 0 || core.raw_read_32(MAIN_LOOP, -1) == CO_PAGE_LOOP
}

fn in_versus(core: &Core) -> bool {
    core.raw_read_8(GAME_MODE, -1) == VERSUS
}

fn turn_ended(core: &Core) -> bool {
    core.raw_read_8(MAP_MENU_STATE, -1) == 5 && core.raw_read_8(MAP_MENU_CURSOR, -1) == 4
}

fn set_bits(core: &mut Core, (start, len): (u32, u32)) {
    for a in start..start + len {
        core.raw_write_8(a, -1, 0xff);
    }
}

impl tango_backend_mgba::SharedGame for Aw2 {
    fn sim_version(&self) -> u16 {
        3
    }

    /// On the battlefield only the army whose turn it is moves, so only
    /// its seat's buttons reach the pad; the other seat watches. Anywhere
    /// else (title, map and CO select, results, the hand-off between
    /// turns) both seats share the pad.
    fn merge(&self, core: &Core, inputs: [u32; 2]) -> u32 {
        let current = core.raw_read_8(CURRENT_PLAYER, -1);
        if in_battle(core) && (1..=4).contains(&current) && !turn_ended(core) {
            inputs[seat_of(current)]
        } else {
            inputs[0] | inputs[1]
        }
    }

    /// With fog of war on, the waiting player sees nothing of the other
    /// army's turn: the game shows the mover's own fog, which on a shared
    /// console would give the mover's vision away. The "Next turn" screen
    /// stays visible so the incoming player can press A.
    fn conceal(&self, core: &Core, seat: usize) -> bool {
        let current = core.raw_read_8(CURRENT_PLAYER, -1);
        in_battle(core)
            && core.raw_read_8(FOG, -1) != 0
            && (1..=4).contains(&current)
            && seat_of(current) != seat
            && !turn_ended(core)
    }

    fn before_tick(&self, core: &mut Core, mode: Option<(u8, u8)>) {
        // Everything unlocked: every CO (Sturm and Hachi included), every
        // CO colour edit, every Battle Map, Hard Campaign and the Sound
        // Room. The game saves this block, so a save made here keeps it.
        core.raw_write_8(HARD_CAMPAIGN, -1, 1);
        core.raw_write_8(SOUND_ROOM, -1, 1);
        set_bits(core, BATTLE_MAPS);
        set_bits(core, COS_AND_EDITS);

        // Versus only (Campaign and War Room keep their story armies):
        // every army slot takes the picked colour, Black Hole included.
        // Palettes follow the colour byte. `mode` is the lobby's pick
        // online and the Armies picker's offline.
        if let Some(mode) = mode.filter(|_| in_versus(core)) {
            for (slot, colour) in slot_colours(mode).into_iter().enumerate() {
                core.raw_write_8(PLAYER_BLOCK + PLAYER_STRIDE * slot as u32 + COLOUR, -1, colour);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_match_type_gives_four_distinct_armies() {
        for t in 0..MATCH_TYPES.len() {
            for s in 0..MATCH_TYPES[t] {
                let mut c = slot_colours((t as u8, s as u8)).to_vec();
                c.sort();
                c.dedup();
                assert_eq!(c.len(), 4, "type {t} subtype {s}");
            }
        }
    }

    #[test]
    fn black_hole_can_be_either_army() {
        assert_eq!(slot_colours((4, 0))[0], 5);
        assert_eq!(slot_colours((0, 3))[1], 5);
    }

    #[test]
    fn seats_alternate_by_slot() {
        assert_eq!([1, 2, 3, 4].map(seat_of), [0, 1, 0, 1]);
    }
}
