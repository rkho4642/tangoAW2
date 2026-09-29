//! The Design Room, extended (offline only): Black Hole as an army colour
//! and Black Hole's inventions as placeable structures.
//!
//! Black Hole is a fifth army next to the game's four, with its own HQ,
//! bases and units ([`crate::design5`]).
//!
//! Inventions are ordinary terrain tiles (their battle sprites come from
//! scanning the map when a battle loads), so placing one is writing its
//! footprint into the editor's map. The editor has no graphics for them,
//! so tangoAW2 draws them with the battle's own sprites
//! ([`crate::invention_art`]).
//!
//! Addresses and the tile/footprint tables come from the aw2bhr
//! decompilation and runtime probing; see docs/AW2.md.

use mgba::core::Core;

/// Title-menu mode 8 is the Design Room; the editor's own sub-mode is 5.
const GAME_MODE: u32 = 0x0300_33FC;
const DESIGN_ROOM: u8 = 8;
const SUB_MODE: u32 = 0x0300_3FC1;
const MAP_EDITOR: u8 = 5;

/// The editor's state block.
const EDITOR_PTR: u32 = 0x0200_B0B0;
const EDITOR: u32 = 0x0200_B000;
const E_STATE: u32 = EDITOR + 0x04; // 1 map, 2 a tool bar is open
const E_BAR: u32 = EDITOR + 0x07; // 0 terrain bar, else units
const E_CURSOR_X: u32 = EDITOR + 0x08;
const E_CURSOR_Y: u32 = EDITOR + 0x0A;
const E_TERRAIN: u32 = EDITOR + 0x2A; // class | owner << 5
/// First visible entry of the terrain bar; the highlighted one sits 4
/// further on, wrapping around the list. The bar's length: 17 in the game,
/// 27 or 29 with the inventions ([`crate::design_bar::entries`]).
const E_TERRAIN_WINDOW: u32 = EDITOR + 0x36;
/// The open bar's entries: (word, tile) pairs; word is the terrain class
/// byte (terrain bar) or the unit word (unit bar).
const BAR_LIST: u32 = crate::design_bar::LIST;

/// The editor's map: size, camera (pixels), tile IDs, class plane, unit
/// plane, and the row-offset table.
const MAP: u32 = 0x0201_E450;
const MAP_W: u32 = MAP;
const MAP_H: u32 = MAP + 2;
const CAMERA_X: u32 = MAP + 4;
const CAMERA_Y: u32 = MAP + 6;
const UNIT_PLANE: u32 = MAP + 0x12;
const TILES: u32 = MAP + 0xA22;
const CLASSES: u32 = MAP + 0x1432;
const ROWS: u32 = MAP + 0x417A;
/// Tile ID -> class byte (class | owner << 5), in ROM.
const CLASS_TABLE: u32 = 0x080C_1BC4;

const PLAIN_TILE: u16 = 0x001;
/// The terrain bar's Plain entry (its class).
const PLAIN_CLASS: u16 = 0x01;
const UNDERLAY: u16 = 0x1A4;
const VOLCANO_RIM: u16 = 0x1A5;
const FACTORY_ANCHOR: u16 = 0x18D;
const VOLCANO_ANCHOR: u16 = 0x1A7;
/// The game registers at most 16 inventions per map; keep one spare.
const MAX_INVENTIONS: usize = 15;

const KEY_A: u32 = 1;
const KEY_SELECT: u32 = 1 << 2;
const KEY_UP: u32 = 1 << 6;

/// One placeable invention: its label, the tile carrying its class (the
/// anchor), and its footprint rows starting at (anchor.x + dx, anchor.y + dy).
struct Invention {
    /// What the editor writes on the placed structure (the full label is
    /// shown while it is selected).
    anchor: u16,
    dx: i32,
    dy: i32,
    rows: &'static [&'static [u16]],
}

const INVENTIONS: &[Invention] = &[
    Invention {
        anchor: 0x182,
        dx: 0,
        dy: 0,
        rows: &[&[0x182]],
    },
    Invention {
        anchor: 0x183,
        dx: 0,
        dy: 0,
        rows: &[&[0x183]],
    },
    Invention {
        anchor: 0x184,
        dx: 0,
        dy: 0,
        rows: &[&[0x184]],
    },
    Invention {
        anchor: 0x185,
        dx: 0,
        dy: 0,
        rows: &[&[0x185]],
    },
    Invention {
        anchor: 0x181,
        dx: 0,
        dy: 0,
        rows: &[&[0x181]],
    },
    Invention {
        anchor: 0x187,
        dx: -1,
        dy: -1,
        rows: &[&[UNDERLAY; 3], &[0x186, 0x187, 0x188], &[UNDERLAY; 3]],
    },
    Invention {
        anchor: 0x18A,
        dx: -1,
        dy: -1,
        rows: &[&[UNDERLAY; 3], &[0x189, 0x18A, 0x18B], &[UNDERLAY; 3]],
    },
    Invention {
        anchor: 0x18D,
        dx: -1,
        dy: -2,
        rows: &[
            &[UNDERLAY, 0x143, UNDERLAY],
            &[UNDERLAY; 3],
            &[0x18C, 0x18D, 0x18E],
            &[UNDERLAY; 3],
        ],
    },
    Invention {
        anchor: 0x1A7,
        dx: -1,
        dy: -2,
        rows: &[
            &[VOLCANO_RIM; 4],
            &[VOLCANO_RIM, UNDERLAY, UNDERLAY, VOLCANO_RIM],
            &[0x1A6, 0x1A7, 0x1A8, 0x1A9],
            &[UNDERLAY; 4],
        ],
    },
    Invention {
        anchor: 0x190,
        dx: -1,
        dy: -1,
        rows: &[&[UNDERLAY; 3], &[0x18F, 0x190, 0x191], &[UNDERLAY; 3]],
    },
    // The Black Crystal and Black Obelisk (crate::obelisk).
    Invention {
        anchor: crate::obelisk::CRYSTAL_TILE,
        dx: 0,
        dy: 0,
        rows: &[&[crate::obelisk::CRYSTAL_TILE]],
    },
    Invention {
        anchor: crate::obelisk::OBELISK_TILE,
        dx: -1,
        dy: -1,
        rows: &[
            &[UNDERLAY; 3],
            &[UNDERLAY, crate::obelisk::OBELISK_TILE, UNDERLAY],
            &[UNDERLAY; 3],
        ],
    },
];

/// The Design Room's map editor is loaded, whatever it is showing (its
/// bars and menus included).
pub fn in_map_editor(core: &Core) -> bool {
    core.raw_read_8(GAME_MODE, -1) == DESIGN_ROOM
        && core.raw_read_8(SUB_MODE, -1) == MAP_EDITOR
        && core.raw_read_32(EDITOR_PTR, -1) == EDITOR
}

pub fn in_editor(core: &Core) -> bool {
    core.raw_read_8(GAME_MODE, -1) == DESIGN_ROOM
        && core.raw_read_8(SUB_MODE, -1) == MAP_EDITOR
        && core.raw_read_32(EDITOR_PTR, -1) == EDITOR
        && matches!(core.raw_read_8(E_STATE, -1), 1 | 2)
}

fn size(core: &Core) -> (i32, i32) {
    (core.raw_read_16(MAP_W, -1) as i32, core.raw_read_16(MAP_H, -1) as i32)
}

fn cell(core: &Core, x: i32, y: i32) -> u32 {
    core.raw_read_16(ROWS + y as u32 * 2, -1) as u32 + x as u32
}

fn tile(core: &Core, x: i32, y: i32) -> u16 {
    core.raw_read_16(TILES + cell(core, x, y) * 2, -1) & 0x1FF
}

fn set_tile(core: &mut Core, x: i32, y: i32, t: u16) {
    let c = cell(core, x, y);
    let class = core.raw_read_8(CLASS_TABLE + t as u32, -1);
    core.raw_write_16(TILES + c * 2, -1, t);
    core.raw_write_8(CLASSES + c, -1, class);
}

/// Every cell of `inv` placed with its anchor at (x, y).
fn footprint(inv: &Invention, x: i32, y: i32) -> impl Iterator<Item = (i32, i32, u16)> + '_ {
    inv.rows.iter().enumerate().flat_map(move |(ry, row)| {
        row.iter()
            .enumerate()
            .map(move |(rx, &t)| (x + inv.dx + rx as i32, y + inv.dy + ry as i32, t))
    })
}

/// Placed inventions: (index into INVENTIONS, anchor x, y).
fn placed(core: &Core) -> Vec<(usize, i32, i32)> {
    let (w, h) = size(core);
    let mut out = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let t = tile(core, x, y);
            if let Some(i) = INVENTIONS.iter().position(|inv| inv.anchor == t) {
                out.push((i, x, y));
            }
        }
    }
    out
}

/// The structure covering map cell (x, y), if any: (index into
/// INVENTIONS, anchor x, y).
fn structure_at(core: &Core, x: i32, y: i32) -> Option<(usize, i32, i32)> {
    placed(core)
        .into_iter()
        .find(|&(i, ax, ay)| footprint(&INVENTIONS[i], ax, ay).any(|(cx, cy, _)| (cx, cy) == (x, y)))
}

/// Stamp invention `i` at the cursor, unless it would leave the map, cover
/// a property, a unit or another structure, or exceed the game's invention
/// limit.
fn stamp(core: &mut Core, i: usize) -> bool {
    let inv = &INVENTIONS[i];
    let (w, h) = size(core);
    let x = core.raw_read_16(E_CURSOR_X, -1) as i32;
    let y = core.raw_read_16(E_CURSOR_Y, -1) as i32;
    let existing = placed(core);
    if existing.len() >= MAX_INVENTIONS {
        return false;
    }
    // The Black Factory and the Volcano draw from the same graphics
    // memory in battle (no campaign map has both); a map with both shows
    // one of them garbled, so a map gets one or the other.
    let exclusive = |a: u16| a == FACTORY_ANCHOR || a == VOLCANO_ANCHOR;
    if exclusive(inv.anchor)
        && existing
            .iter()
            .any(|&(j, _, _)| exclusive(INVENTIONS[j].anchor) && INVENTIONS[j].anchor != inv.anchor)
    {
        return false;
    }
    let cells: Vec<_> = footprint(inv, x, y).collect();
    for &(cx, cy, _) in &cells {
        if cx < 0 || cy < 0 || cx >= w || cy >= h {
            return false;
        }
        let c = cell(core, cx, cy);
        let owner_class = core.raw_read_8(CLASSES + c, -1) & 0x1F;
        let property = matches!(owner_class, 0x06 | 0x08 | 0x0A | 0x0B | 0x0E | 0x10 | 0x11);
        if property || core.raw_read_8(UNIT_PLANE + c, -1) != 0 || structure_at(core, cx, cy).is_some() {
            return false;
        }
    }
    for (cx, cy, t) in cells {
        set_tile(core, cx, cy, t);
    }
    true
}

/// A structure whose anchor was painted over leaves its other cells behind
/// as blank underlay; turn those back into plain.
fn clear_orphans(core: &mut Core) {
    let (w, h) = size(core);
    let mut covered = vec![false; (w * h).max(0) as usize];
    for (i, x, y) in placed(core) {
        for (cx, cy, _) in footprint(&INVENTIONS[i], x, y) {
            if cx >= 0 && cy >= 0 && cx < w && cy < h {
                covered[(cy * w + cx) as usize] = true;
            }
        }
    }
    for y in 0..h {
        for x in 0..w {
            let t = tile(core, x, y);
            if (t == UNDERLAY || t == VOLCANO_RIM) && !covered[(y * w + x) as usize] {
                set_tile(core, x, y, PLAIN_TILE);
            }
        }
    }
}

/// The highlighted terrain entry's word (class, plus
/// [`crate::design_bar::OURS`] for the Crystal and Obelisk).
fn highlighted_terrain_word(core: &Core) -> u16 {
    let i = (core.raw_read_8(E_TERRAIN_WINDOW, -1) as u32 + 4) % crate::design_bar::entries(core);
    core.raw_read_16(BAR_LIST + 4 * i, -1) & (crate::design_bar::OURS | 0x1F)
}

/// The terrain word last picked from the bar: the editor keeps only the
/// class (`E_TERRAIN`), which the Crystal shares with the minicannon and the
/// Obelisk with the Black Cannon.
const PICKED: u32 = 0x0203_FF7C;

/// One Design Room frame: SELECT as "next army" in the bars, placing
/// inventions, and Black Hole as army 5 ([`crate::design5::editor_tick`]).
/// Returns the joypad word the editor should see.
pub fn editor_tick(core: &mut Core, keys: u32, prev: u32) -> u32 {
    let mut keys = keys;
    let mut pressed = keys & !prev;
    let bar_open = core.raw_read_8(E_STATE, -1) == 2;
    // In a tool bar SELECT only repeated L/R (switch bars); tangoAW2 makes
    // it "next army" like UP, as on Versus' Teams screen.
    if bar_open && pressed & KEY_SELECT != 0 {
        keys = (keys & !KEY_SELECT) | KEY_UP;
        pressed = (pressed & !KEY_SELECT) | KEY_UP;
    } else if bar_open {
        keys &= !KEY_SELECT;
    }
    let terrain_bar = core.raw_read_8(E_BAR, -1) == 0;
    // While a bar is open the tool is only chosen on A: read the
    // highlighted entry. On the map, the chosen tool.
    let word = if bar_open && terrain_bar {
        highlighted_terrain_word(core)
    } else {
        let class = core.raw_read_8(E_TERRAIN, -1) as u16 & 0x1F;
        let picked = core.raw_read_16(PICKED, -1);
        if picked & 0x1F == class {
            picked
        } else {
            class
        }
    };
    if bar_open && terrain_bar && pressed & KEY_A != 0 {
        core.raw_write_16(PICKED, -1, word);
    }
    // In the bars UP/DOWN (and SELECT) step through the armies: neutral,
    // Orange Star, Blue Moon, Green Earth, Yellow Comet and Black Hole
    // (crate::design5 gives the bars a fifth army).
    if !bar_open && pressed & KEY_A != 0 && terrain_bar {
        // An invention picked from the terrain bar: A on the map places its
        // whole footprint (the game alone would place just one tile).
        if let Some(i) = crate::design_bar::invention_of(word) {
            stamp(core, i);
            keys &= !KEY_A;
        }
    }
    // One thing per cell: nothing goes onto a structure's cells, except
    // plain, which clears the whole structure (the editor's way to remove
    // things). Checked while A is held, so painting by dragging stops at a
    // structure too.
    if !bar_open && keys & KEY_A != 0 && !(terrain_bar && crate::design_bar::invention_of(word).is_some()) {
        let x = core.raw_read_16(E_CURSOR_X, -1) as i32;
        let y = core.raw_read_16(E_CURSOR_Y, -1) as i32;
        if let Some((i, ax, ay)) = structure_at(core, x, y) {
            if terrain_bar && word & 0x1F == PLAIN_CLASS && pressed & KEY_A != 0 {
                for (cx, cy, _) in footprint(&INVENTIONS[i], ax, ay).collect::<Vec<_>>() {
                    set_tile(core, cx, cy, PLAIN_TILE);
                }
            } else {
                keys &= !KEY_A;
            }
        }
    }

    clear_orphans(core);
    let volcano_on_map = placed(core).iter().any(|&(i, _, _)| i == VOLCANO_INDEX);
    crate::invention_art::tick(core, volcano_on_map);
    crate::design5::editor_tick(core);
    keys
}

// ---------- The inventions' own pictures ----------

/// At the game's VBlank sprite flush: the placed inventions, drawn with the
/// battle's own sprites.
pub fn flush_sprites(core: &mut Core, at: u32, end: u32) -> u32 {
    if !in_editor(core) {
        return at;
    }
    let placed = placed(core);
    let volcano_on_map = placed.iter().any(|&(i, _, _)| i == VOLCANO_INDEX);
    let cam_x = core.raw_read_16(CAMERA_X, -1) as i32;
    let cam_y = core.raw_read_16(CAMERA_Y, -1) as i32;
    let mut list = Vec::new();
    for (i, x, y) in placed {
        let inv = &INVENTIONS[i];
        list.push(crate::invention_art::Placed {
            i,
            x: (x + inv.dx) * 16 - cam_x,
            y: (y + inv.dy) * 16 - cam_y,
            priority: 3,
        });
    }
    // While a tool bar is open it covers the bottom of the screen.
    let bottom = if core.raw_read_8(E_STATE, -1) == 2 {
        BAR_TOP
    } else {
        160
    };
    let at = crate::design5::append_emblem(core, at, end);
    crate::invention_art::append(core, &list, volcano_on_map, bottom, at, end)
}

const VOLCANO_INDEX: usize = 8;
/// The top of an open tool bar, on screen.
const BAR_TOP: i32 = 120;

/// Whether the map being edited has a Volcano (its picture then takes the
/// free sprite palette).
pub fn volcano_on_map(core: &Core) -> bool {
    in_editor(core) && placed(core).iter().any(|&(i, _, _)| i == VOLCANO_INDEX)
}
