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

/// The Versus Teams screen's record: how many armies the map has, each
/// army's controller (1 human, 2 computer) and colour (same codes as the
/// player blocks), and the cursor (two stops per army: its CO, then its
/// human/computer marker). The game builds each army's player block from
/// this record when the battle loads, so a colour set here is the army's
/// colour for the whole battle.
const TEAMS: u32 = 0x0201_7C50;
const TEAMS_ARMIES: u32 = TEAMS + 0x08;
const TEAMS_COLOUR: u32 = TEAMS + 0x0D;
const TEAMS_CURSOR: u32 = TEAMS + 0x32;
/// The Teams screen's task function. It sits in the task table while the
/// Teams screen is up and nowhere else.
const TEAMS_TASK: u32 = 0x0806_4E5D;
const TASK_TABLE: (u32, u32) = (0x0300_1500, 0x0300_1A00);

/// Sprite groups the game loads per screen (0x88 bytes each): a VRAM base
/// pointer, palette slot, count, then (tile, sprite id) pairs. Group 1 is
/// the army emblems, sprite ids 0x3E..=0x42 (Orange Star, Blue Moon, Green
/// Earth, Yellow Comet, Black Hole); the Teams screen loads only the first
/// four. Sprite sizes (width, height in tiles) are a 4-byte table in ROM,
/// and each group's graphics a ROM row of (base, palette, first id).
const SPRITE_GROUPS: u32 = 0x0200_F920;
const SPRITE_GROUP_SIZE: u32 = 0x88;
pub(crate) const EMBLEM_GROUP: u32 = 1;
const SPRITE_SIZES: u32 = 0x0848_B780;
const SPRITE_ROWS: u32 = 0x0848_B738;
pub(crate) const EMBLEM_BASE_ID: u32 = 0x3D;
const BLACK_HOLE: u8 = 5;

/// The Versus map being played; design maps are 0xB4..=0xB7.
const MAP_ID: u32 = 0x0300_3FC2;
const DESIGN_MAPS: std::ops::RangeInclusive<u8> = 0xB4..=0xB7;
/// A design map's own colour per army slot (index 1..=4), from its save.
const DESIGN_COLOURS: u32 = 0x0300_3FF3;
/// The game's working palettes (copied to palette RAM every frame) and
/// palette RAM itself. Army slot n's buildings use BG row 11 + n and its
/// units OBJ row 8 + n.
const PAL_BUFFER: u32 = 0x0300_20C0;
const PAL_RAM: u32 = 0x0500_0000;
/// ROM palettes: buildings per colour (1..=5, the last entry's high byte is
/// a flag the game clears), units per colour 1..=4, and Black Hole's units.
const BUILDING_PALETTES: u32 = 0x0810_E6E0;
const UNIT_PALETTES: u32 = 0x080D_3F04;
const BLACK_HOLE_UNIT_PALETTE: u32 = 0x080D_3E84;

/// tangoAW2's own state, in the last bytes of EWRAM (unused by the game
/// in every mode probed). Kept in console RAM so rollback snapshots carry
/// it: the previous frame's joypad word.
const PREV_KEYS: u32 = 0x0203_FFF0;
/// Buttons tangoAW2 took for itself on their press: hidden from the game
/// for as long as they stay held, so a long press is not a fresh one for
/// the game on its second frame.
const CLAIMED_KEYS: u32 = 0x0203_FFF6;
/// Whether this match's Teams screen has been seen (its original colours
/// recorded and any design-map preset applied).
const DESIGN_PRESET_DONE: u32 = 0x0203_FFF5;
/// Each army's colour when this match's Teams screen opened: what the map
/// itself gives them, and what the game loads palettes for.
const ORIGINAL_COLOURS: u32 = 0x0203_FFF8;
/// The design-map list's cache of each saved map (0x1C bytes per slot);
/// + 0x14 holds the map's saved marker byte (5 marks a five-army map, see crate::five::DESIGN_FIVE).
const DESIGN_CACHE: u32 = 0x0202_80C0;

const KEY_SELECT: u32 = 1 << 2;
const KEY_R: u32 = 1 << 8;
const KEY_L: u32 = 1 << 9;

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

/// One mode, Versus: armies are picked on the game's own Teams screen.
pub const MATCH_TYPES: &[usize] = &[1];

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

/// Where the battle's army colours live, for tools reading them through
/// [`tango_match::Link::peek`]: `PLAYER_STRIDE` bytes apart.
pub const ARMY_COLOUR_ADDRS: [u32; 4] = [
    PLAYER_BLOCK + COLOUR,
    PLAYER_BLOCK + PLAYER_STRIDE + COLOUR,
    PLAYER_BLOCK + PLAYER_STRIDE * 2 + COLOUR,
    PLAYER_BLOCK + PLAYER_STRIDE * 3 + COLOUR,
];

/// The Teams record's colour for each army (Teams screen), for tests.
pub fn teams_colours(core: &Core) -> [u8; 4] {
    [0, 1, 2, 3].map(|s| core.raw_read_8(TEAMS_COLOUR + s, -1))
}

pub struct Aw2;

pub static AW2E: Aw2 = Aw2;

fn in_battle(core: &Core) -> bool {
    core.raw_read_32(BATTLE_SCENE, -1) != 0 || core.raw_read_32(MAIN_LOOP, -1) == CO_PAGE_LOOP
}

pub(crate) fn in_versus(core: &Core) -> bool {
    core.raw_read_8(GAME_MODE, -1) == VERSUS
}

fn on_teams_screen(core: &Core) -> bool {
    (TASK_TABLE.0..TASK_TABLE.1)
        .step_by(4)
        .any(|a| core.raw_read_32(a, -1) == TEAMS_TASK)
}

pub(crate) fn sprite_tiles(core: &Core, id: u32) -> u32 {
    let w = core.raw_read_8(SPRITE_SIZES + id * 4, -1) as u32;
    let h = core.raw_read_8(SPRITE_SIZES + id * 4 + 1, -1) as u32;
    w * h
}

/// Where sprite `id`'s graphics start in ROM (the game's own lookup).
pub(crate) fn sprite_source(core: &Core, id: u32, group: u32) -> u32 {
    let row = SPRITE_ROWS + group * 12;
    let base = core.raw_read_32(row, -1);
    let first = core.raw_read_32(row + 8, -1) & 0xFFFF;
    let before: u32 = (first..id).map(|i| sprite_tiles(core, i)).sum();
    base + (before & 0x3FF) * 32
}

/// The Teams screen shows each army's emblem, but only loads the four
/// standard ones. With five colours and at most four armies at least one
/// standard emblem is always unused: when an army is Black Hole, its
/// emblem is drawn into that unused emblem's tiles and registered there.
/// Every standard emblem is restored from ROM each frame first, so a
/// borrowed one comes back the moment it is needed again.
fn show_emblems(core: &mut Core, armies: u32) {
    let group = SPRITE_GROUPS + EMBLEM_GROUP * SPRITE_GROUP_SIZE;
    let vram = core.raw_read_32(group, -1);
    let count = core.raw_read_8(group + 5, -1) as u32;
    let entries: Vec<(u32, u32)> = (0..count.min(31))
        .map(|i| {
            let e = group + 8 + i * 4;
            (core.raw_read_16(e, -1) as u32, core.raw_read_16(e + 2, -1) as u32)
        })
        .collect();
    let tile_of = |id: u32| entries.iter().find(|(_, s)| *s == id).map(|(t, _)| *t);
    let copy = |core: &mut Core, id: u32, tile: u32| {
        let len = (sprite_tiles(core, id) * 32) as usize;
        let mut buf = vec![0u8; len];
        core.raw_read_range(sprite_source(core, id, EMBLEM_GROUP), -1, &mut buf);
        core.raw_write_range(vram + (tile & 0x3FF) * 32, -1, &buf);
    };
    for colour in 1..=4u32 {
        if let Some(tile) = tile_of(EMBLEM_BASE_ID + colour) {
            copy(core, EMBLEM_BASE_ID + colour, tile);
        }
    }
    let colours: Vec<u8> = (0..armies).map(|s| core.raw_read_8(TEAMS_COLOUR + s, -1)).collect();
    if !colours.contains(&BLACK_HOLE) {
        return;
    }
    let Some(donor) = (1..=4u8).find(|c| !colours.contains(c)) else {
        return;
    };
    let Some(tile) = tile_of(EMBLEM_BASE_ID + donor as u32) else {
        return;
    };
    let bh = EMBLEM_BASE_ID + BLACK_HOLE as u32;
    copy(core, bh, tile);
    match entries.iter().position(|(_, s)| *s == bh) {
        Some(i) => core.raw_write_16(group + 8 + i as u32 * 4, -1, tile as u16),
        None => {
            let e = group + 8 + count * 4;
            core.raw_write_16(e, -1, tile as u16);
            core.raw_write_16(e + 2, -1, bh as u16);
            core.raw_write_8(group + 5, -1, (count + 1) as u8);
        }
    }
}

fn palette(core: &Core, rom: u32, building: bool) -> [u8; 32] {
    let mut p = [0u8; 32];
    core.raw_read_range(rom, -1, &mut p);
    if building {
        p[30] = 0;
        p[31] = 0;
    }
    p
}

fn building_palette(core: &Core, colour: u8) -> [u8; 32] {
    palette(core, BUILDING_PALETTES + 0x20 * (colour as u32 - 1), true)
}

fn unit_palette(core: &Core, colour: u8) -> [u8; 32] {
    let rom = if colour == BLACK_HOLE {
        BLACK_HOLE_UNIT_PALETTE
    } else {
        UNIT_PALETTES + 0x20 * (colour as u32 - 1)
    };
    palette(core, rom, false)
}

/// Where army slot `slot` (0-based)'s building and unit palette rows still
/// hold exactly colour `from`'s palettes, swap in colour `to`'s. A row the
/// game is fading or has changed is left alone; a reload is caught on the
/// next frame. The game's working buffer and palette RAM are each swapped
/// where they hold `from`, so the swap shows this frame and survives the
/// next copy.
pub fn swap_slot_palettes(core: &mut Core, slot: u32, from: u8, to: u8) {
    if from == to || !(1..=5).contains(&from) || !(1..=5).contains(&to) {
        return;
    }
    let rows = [
        (11 + slot + 1, building_palette(core, from), building_palette(core, to)),
        (16 + 8 + slot + 1, unit_palette(core, from), unit_palette(core, to)),
    ];
    for (row, from, to) in rows {
        // The buffer and palette RAM are checked on their own: the game
        // sometimes writes palette RAM directly, after the buffer was swapped.
        for base in [PAL_BUFFER, PAL_RAM] {
            let mut now = [0u8; 32];
            core.raw_read_range(base + row * 32, -1, &mut now);
            if now == from {
                core.raw_write_range(base + row * 32, -1, &to);
            }
        }
    }
}

/// Writes colour `to`'s building and unit palettes into army slot `slot`
/// (0-based)'s rows, whatever they hold now (the buffer and palette RAM).
pub fn force_slot_palettes(core: &mut Core, slot: u32, to: u8) {
    let rows = [
        (11 + slot + 1, building_palette(core, to)),
        (16 + 8 + slot + 1, unit_palette(core, to)),
    ];
    for (row, pal) in rows {
        for base in [PAL_BUFFER, PAL_RAM] {
            core.raw_write_range(base + row * 32, -1, &pal);
        }
    }
}

/// Design maps load each army's palettes from the map's own colours, not
/// the Teams pick; swap each army to its picked colour.
fn recolour_design_armies(core: &mut Core, armies: u32) {
    for army in 0..armies {
        let picked = core.raw_read_8(TEAMS_COLOUR + army, -1);
        let original = core.raw_read_8(ORIGINAL_COLOURS + army, -1);
        swap_slot_palettes(core, army, original, picked);
    }
    // The CO panel's palettes are loaded at the start of each turn from the
    // army's colour; the first turn's are loaded before the colours are
    // held to the pick, so they can still be the map's colour.
    let army = core.raw_read_16(CURRENT_PLAYER, -1) as u32;
    if (1..=armies).contains(&army) {
        let picked = core.raw_read_8(TEAMS_COLOUR + army - 1, -1);
        let original = core.raw_read_8(ORIGINAL_COLOURS + army - 1, -1);
        if picked != original && (1..=5).contains(&picked) && (1..=5).contains(&original) {
            for (table, row) in CO_PANEL_PALETTES {
                let pal = |c: u8| {
                    let mut p = [0u8; 32];
                    core.raw_read_range(table + (c as u32 - 1) * 32, -1, &mut p);
                    p
                };
                let (from, to) = (pal(original), pal(picked));
                for base in [PAL_BUFFER, PAL_RAM] {
                    let mut now = [0u8; 32];
                    core.raw_read_range(base + row * 32, -1, &mut now);
                    if now == from {
                        core.raw_write_range(base + row * 32, -1, &to);
                    }
                }
            }
        }
    }
}

/// The CO panel's per-colour palettes (colour - 1 rows into each table) and
/// the palette row each is loaded into at the start of a turn: the panel
/// (`0x0801A548` -> `0x0802D5CC`, BG row 8) and its header with the funds
/// (`0x08043834`, OBJ row 7).
const CO_PANEL_PALETTES: [(u32, u32); 2] = [(0x080D_4188, 8), (0x0810_4264, 23)];

/// The first frame of a match's Teams screen: remember every army's
/// original colour, and give a design map saved with Black Hole (the
/// Design Room marker, kept in its map-select cache) its Black Hole army.
fn first_teams_frame(core: &mut Core, armies: u32) {
    if core.raw_read_8(DESIGN_PRESET_DONE, -1) != 0 {
        return;
    }
    core.raw_write_8(DESIGN_PRESET_DONE, -1, 1);
    for army in 0..4 {
        let c = core.raw_read_8(TEAMS_COLOUR + army, -1);
        core.raw_write_8(ORIGINAL_COLOURS + army, -1, c);
    }
    let id = core.raw_read_8(MAP_ID, -1);
    if !DESIGN_MAPS.contains(&id) {
        return;
    }
    let k = (id - DESIGN_MAPS.start()) as u32;
    let marker = core.raw_read_8(DESIGN_CACHE + 0x1C * k + 0x14, -1) as u32;
    if !(1..=4).contains(&marker) {
        return;
    }
    // The army standing in the marked slot is the one whose map colour is
    // that slot's.
    let slot_colour = core.raw_read_8(DESIGN_COLOURS + marker, -1);
    let taken = (0..armies).any(|a| core.raw_read_8(TEAMS_COLOUR + a, -1) == BLACK_HOLE);
    if let Some(army) = (0..armies).find(|&a| core.raw_read_8(TEAMS_COLOUR + a, -1) == slot_colour) {
        if !taken {
            core.raw_write_8(TEAMS_COLOUR + army, -1, BLACK_HOLE);
        }
    }
}

/// Draw colour `show`'s emblem into the tiles where the loaded emblem group
/// keeps colour `slot_colour`'s, if that emblem is loaded on this screen.
pub fn draw_emblem_as(core: &mut Core, slot_colour: u8, show: u8) {
    let group = SPRITE_GROUPS + EMBLEM_GROUP * SPRITE_GROUP_SIZE;
    let vram = core.raw_read_32(group, -1);
    let count = core.raw_read_8(group + 5, -1) as u32;
    let want = EMBLEM_BASE_ID + slot_colour as u32;
    let Some(tile) = (0..count.min(31)).find_map(|i| {
        let e = group + 8 + i * 4;
        (core.raw_read_16(e + 2, -1) as u32 == want).then(|| core.raw_read_16(e, -1) as u32)
    }) else {
        return;
    };
    let id = EMBLEM_BASE_ID + show as u32;
    let mut buf = vec![0u8; (sprite_tiles(core, id) * 32) as usize];
    core.raw_read_range(sprite_source(core, id, EMBLEM_GROUP), -1, &mut buf);
    core.raw_write_range(vram + (tile & 0x3FF) * 32, -1, &buf);
}

/// The next colour (`step` +1 or -1 in [`ARMIES`] order) for army `slot`
/// that no other army on this map already has.
fn cycle_colour(core: &Core, slot: u32, armies: u32, step: isize) -> u8 {
    let current = core.raw_read_8(TEAMS_COLOUR + slot, -1);
    let taken: Vec<u8> = (0..armies)
        .filter(|&s| s != slot)
        .map(|s| core.raw_read_8(TEAMS_COLOUR + s, -1))
        .collect();
    let n = ARMIES.len() as isize;
    let start = ARMIES.iter().position(|(c, _)| *c == current).unwrap_or(0) as isize;
    (1..n)
        .map(|k| ARMIES[(start + step * k).rem_euclid(n) as usize].0)
        .find(|c| !taken.contains(c))
        .unwrap_or(current)
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
        13
    }

    fn traps(&self) -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
        crate::factory::traps()
    }

    /// On a Versus battlefield only the army whose turn it is moves, so
    /// only its seat's buttons reach the pad; the other seat watches. Anywhere
    /// else (title, map and CO select, results, the hand-off between
    /// turns) both seats share the pad.
    fn merge(&self, core: &Core, inputs: [u32; 2]) -> u32 {
        let current = core.raw_read_8(CURRENT_PLAYER, -1);
        if in_versus(core) && in_battle(core) && (1..=5).contains(&current) && !turn_ended(core) {
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
            && (1..=5).contains(&current)
            && seat_of(current) != seat
            && !turn_ended(core)
    }

    fn before_tick(&self, core: &mut Core, mode: Option<(u8, u8)>, keys: u32) -> u32 {
        // Everything unlocked: every CO (Sturm and Hachi included), every
        // CO colour edit, every Battle Map, Hard Campaign and the Sound
        // Room. The game saves this block, so a save made here keeps it.
        core.raw_write_8(HARD_CAMPAIGN, -1, 1);
        core.raw_write_8(SOUND_ROOM, -1, 1);
        set_bits(core, BATTLE_MAPS);
        set_bits(core, COS_AND_EDITS);

        crate::five::sync(core);
        crate::obelisk::install(core);
        // The Black Crystal and Black Obelisk appear only with their art,
        // imported from the player's Dual Strike ROM (see ds_art.rs).
        let ds_features = crate::ds_art::features(mode);
        crate::five_map::show_obelisk_maps(core, ds_features);
        crate::branding::tick(core);
        // Dual Strike's unit stats and damage chart, with the pack on.
        let ds = crate::ds_pack::features(mode);
        crate::design_bar::patch_rom(core, ds_features, ds);
        crate::design5::patch_rom(core);
        crate::roster::tick(core, ds);
        crate::co_roster::tick(core, ds);
        crate::co_new::tick(core, ds);
        crate::co_powers::tick(core, ds);
        crate::unit_actions::tick(core, ds);
        crate::cpu_tactics::tick(core, ds);
        crate::ds_weather::tick(core, ds);
        crate::design5::sync(core);

        let prev = core.raw_read_16(PREV_KEYS, -1) as u32;
        core.raw_write_16(PREV_KEYS, -1, keys as u16);
        let held = keys;
        let claimed = core.raw_read_16(CLAIMED_KEYS, -1) as u32 & held;
        let mut keys = keys;

        // The Design Room's Black Hole colour and inventions: offline only.
        if mode.is_none() && crate::design::in_editor(core) {
            keys = crate::design::editor_tick(core, keys, prev);
        } else {
            crate::invention_art::put_back(core);
        }
        if in_battle(core) {
            core.raw_write_8(DESIGN_PRESET_DONE, -1, 0);
        }

        // Army colours are picked on Versus' own Teams screen: SELECT/R and
        // L cycle the highlighted army through the five armies, Black Hole
        // included. Campaign and War Room never reach this screen, so their
        // story armies are untouched.
        if in_versus(core) && on_teams_screen(core) && !crate::five::active(core) {
            let armies = (core.raw_read_8(TEAMS_ARMIES, -1) as u32).clamp(1, 4);
            first_teams_frame(core, armies);
            let pressed = keys & !prev;
            let step = if pressed & (KEY_SELECT | KEY_R) != 0 {
                1
            } else if pressed & KEY_L != 0 {
                -1
            } else {
                0
            };
            if step != 0 {
                let slot = (core.raw_read_8(TEAMS_CURSOR, -1) as u32 / 2).min(armies - 1);
                let colour = cycle_colour(core, slot, armies, step);
                core.raw_write_8(TEAMS_COLOUR + slot, -1, colour);
            }
            show_emblems(core, armies);
            // The Teams screen has no use for these; keep them ours.
            keys &= !(KEY_SELECT | KEY_L | KEY_R);
        }

        // Every Versus battle comes from a Teams screen, and its record
        // keeps the picked colours. Built-in maps build the armies from it
        // already; design maps take theirs from the map's save instead, so
        // hold the battle's armies to the pick.
        if in_versus(core) && in_battle(core) && !crate::five::active(core) {
            let armies = (core.raw_read_8(TEAMS_ARMIES, -1) as u32).clamp(1, 4);
            for slot in 0..armies {
                let colour = core.raw_read_8(TEAMS_COLOUR + slot, -1);
                if (1..=5).contains(&colour) {
                    core.raw_write_8(
                        crate::five::players(core) + PLAYER_STRIDE * (slot + 1) + COLOUR,
                        -1,
                        colour,
                    );
                }
            }
            if DESIGN_MAPS.contains(&core.raw_read_8(MAP_ID, -1)) {
                recolour_design_armies(core, armies);
            }
        }

        let claimed = claimed | (held & !keys);
        core.raw_write_16(CLAIMED_KEYS, -1, claimed as u16);
        keys & !claimed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seats_alternate_by_slot() {
        assert_eq!([1, 2, 3, 4].map(seat_of), [0, 1, 0, 1]);
    }
}
