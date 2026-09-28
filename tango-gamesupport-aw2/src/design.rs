//! The Design Room, extended (offline only): Black Hole as an army colour
//! and Black Hole's inventions as placeable structures.
//!
//! Advance Wars 2 has four army slots; Black Hole is always one of them
//! shown in colour 5 (the campaign does the same). Here Black Hole shares
//! Yellow Comet's slot, 4: a design map has one or the other. The choice is
//! the map-wide marker [`MARKER`] (4 = slot 4 is Black Hole), a spare byte
//! the game saves with each design map and restores on load.
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
const E_TERRAIN_SLOT: u32 = EDITOR + 0x2E; // 0..=4
/// First visible entry of each bar; the highlighted one sits 4 (terrain)
/// or 3 (units) further on, wrapping around the list.
const E_TERRAIN_WINDOW: u32 = EDITOR + 0x36;
const E_UNIT_WINDOW: u32 = EDITOR + 0x38;
// The terrain bar's length: 17 in the game, 27 or 29 with the inventions
// ([`crate::design_bar::entries`]).
const UNIT_ENTRIES: u32 = 20;
/// The unit bar's "Del" (eraser) entry.
const UNIT_DELETE_INDEX: u32 = 2;
/// The open bar's entries: (word, tile) pairs; word is the terrain class
/// byte (terrain bar) or the unit word (unit bar).
const BAR_LIST: u32 = crate::design_bar::LIST;
const E_UNIT_SLOT: u32 = EDITOR + 0x2F; // 1..=4

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

/// Per-slot army colours of the map being edited; index 0 is unused by the
/// game, saved and restored with the map, and is tangoAW2's Black Hole
/// marker.
pub const MARKER: u32 = 0x0300_3FF3;
const BLACK_HOLE_SLOT: u8 = 4;

/// Editor player blocks (slot n at + 0x3C * n), colour at + 0x1A.
const EDITOR_PLAYERS: u32 = 0x0202_3284;

/// Slot 4's own CO while it is shown as Black Hole (0 = not swapped).
const SAVED_CO: u32 = 0x0203_FFFD;

/// The game draws units and HQs in the style of the army's CO's country
/// (`0x08042DE0`: CO at player + 0x1D -> country); the editor gives slot 4
/// Kanbei, so Yellow Comet's designs. While slot 4 is Black Hole it gets
/// Flak, so Black Hole's own units and HQ.
const CO: u32 = 0x1D;
const BLACK_HOLE_CO: u8 = 11;
fn is_black_hole_co(co: u8) -> bool {
    (10..=14).contains(&co)
}
/// HQ sprite tops, 0x100 bytes per country (1 Orange Star .. 5 Black Hole),
/// copied into OBJ VRAM when a map loads.
const HQ_SPRITES: u32 = 0x080D_16C4;

const PLAIN_TILE: u16 = 0x001;
const UNDERLAY: u16 = 0x1A4;
const VOLCANO_RIM: u16 = 0x1A5;
const FACTORY_ANCHOR: u16 = 0x18D;
const VOLCANO_ANCHOR: u16 = 0x1A7;
/// The game registers at most 16 inventions per map; keep one spare.
const MAX_INVENTIONS: usize = 15;

const KEY_A: u32 = 1;
const KEY_SELECT: u32 = 1 << 2;
const KEY_UP: u32 = 1 << 6;
const KEY_DOWN: u32 = 1 << 7;

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

/// Stamp invention `i` at the cursor, unless it would leave the map, cover
/// a property or a unit, or exceed the game's invention limit.
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
        if property || core.raw_read_8(UNIT_PLANE + c, -1) != 0 {
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

fn highlighted_unit_index(core: &Core) -> u32 {
    (core.raw_read_8(E_UNIT_WINDOW, -1) as u32 + 3) % UNIT_ENTRIES
}

fn is_property_class(class: u8) -> bool {
    matches!(class, 0x06 | 0x08 | 0x0A | 0x0B | 0x0E)
}

/// One Design Room frame: the Black Hole colour step, the invention picker
/// on the Silo entry, placing inventions, and keeping the editor's slot 4
/// drawn as Black Hole. Returns the joypad word the editor should see.
pub fn editor_tick(core: &mut Core, keys: u32, prev: u32) -> u32 {
    let mut keys = keys;
    let mut pressed = keys & !prev;
    let bar_open = core.raw_read_8(E_STATE, -1) == 2;
    // In a tool bar SELECT only repeated L/R (switch bars); tangoAW2 makes
    // it "next army" like UP, as on Versus' Teams screen: Yellow Comet ->
    // Black Hole included.
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
    let class = word as u8 & 0x1F;
    if bar_open && terrain_bar && pressed & KEY_A != 0 {
        core.raw_write_16(PICKED, -1, word);
    }
    let unit_is_delete = bar_open && !terrain_bar && highlighted_unit_index(core) == UNIT_DELETE_INDEX;
    let marker = core.raw_read_8(MARKER, -1);

    if bar_open {
        {
            // The colour arrows: Yellow Comet and Black Hole share slot 4.
            let (slot_addr, coloured) = if terrain_bar {
                (E_TERRAIN_SLOT, is_property_class(class))
            } else {
                (E_UNIT_SLOT, !unit_is_delete)
            };
            let slot = core.raw_read_8(slot_addr, -1);
            if coloured && slot == BLACK_HOLE_SLOT {
                if pressed & KEY_UP != 0 && marker != BLACK_HOLE_SLOT {
                    // Yellow Comet -> Black Hole.
                    core.raw_write_8(MARKER, -1, BLACK_HOLE_SLOT);
                    keys &= !KEY_UP;
                } else if pressed & KEY_DOWN != 0 && marker == BLACK_HOLE_SLOT {
                    // Black Hole -> Yellow Comet.
                    core.raw_write_8(MARKER, -1, 0);
                    keys &= !KEY_DOWN;
                }
            } else if coloured {
                // Arriving at slot 4 from either side shows the army next
                // in line: going forward (UP/SELECT) Yellow Comet comes
                // before Black Hole, going back (DOWN) Black Hole comes
                // first. So one button walks all six: ... Green Earth,
                // Yellow Comet, Black Hole, Neutral ...
                let below = BLACK_HOLE_SLOT - 1;
                let above = if terrain_bar { 0 } else { 1 };
                if pressed & KEY_UP != 0 && slot == below {
                    core.raw_write_8(MARKER, -1, 0);
                } else if pressed & KEY_DOWN != 0 && slot == above {
                    core.raw_write_8(MARKER, -1, BLACK_HOLE_SLOT);
                }
            }
        }
    } else if pressed & KEY_A != 0 && terrain_bar {
        // An invention picked from the terrain bar: A on the map places its
        // whole footprint (the game alone would place just one tile).
        if let Some(i) = crate::design_bar::invention_of(word) {
            stamp(core, i);
            keys &= !KEY_A;
        }
    }

    clear_orphans(core);
    let volcano_on_map = placed(core).iter().any(|&(i, _, _)| i == VOLCANO_INDEX);
    crate::invention_art::tick(core, volcano_on_map);
    show_slot4_as(
        core,
        if core.raw_read_8(MARKER, -1) == BLACK_HOLE_SLOT {
            5
        } else {
            4
        },
    );
    if core.raw_read_8(MARKER, -1) == BLACK_HOLE_SLOT {
        core.raw_write_8(EDITOR_PLAYERS + 0x3C * BLACK_HOLE_SLOT as u32 + 0x1A, -1, 5);
        // Slot 4's rows belong to slot 4 alone in the editor, so they can
        // be held to Black Hole's even while the game animates a slot change.
        crate::pvp::force_slot_palettes(core, BLACK_HOLE_SLOT as u32 - 1, 5);
        // The army list's emblem for slot 4: Black Hole's, drawn into
        // Yellow Comet's emblem tiles.
        crate::pvp::draw_emblem_as(core, 4, 5);
    } else if core.raw_read_8(MARKER, -1) != 0 {
        // Only 0 and 4 are ours; anything else is not a marker.
        core.raw_write_8(MARKER, -1, 0);
    } else {
        // Back to Yellow Comet: put its colour and palettes back (the game
        // only reloads them when the slot changes).
        let colour = EDITOR_PLAYERS + 0x3C * BLACK_HOLE_SLOT as u32 + 0x1A;
        if core.raw_read_8(colour, -1) == 5 {
            core.raw_write_8(colour, -1, BLACK_HOLE_SLOT);
        }
        crate::pvp::swap_slot_palettes(core, BLACK_HOLE_SLOT as u32 - 1, 5, 4);
        crate::pvp::draw_emblem_as(core, 4, 4);
    }
    keys
}

/// The tool bar's item ring: 11 entries of 0x1C bytes, flags first; bit 0
/// = shown, bit 3 = reload the entry's sprite graphics this frame.
const DESIGN_RING: u32 = 0x0200_B0D0;

/// Draws slot 4 in `country`'s own designs (4 Yellow Comet, 5 Black Hole):
/// its CO, and the HQ sprite already in OBJ VRAM. When it switches, the
/// tool bar reloads its sprites, which were drawn for the old CO.
fn show_slot4_as(core: &mut Core, country: u8) {
    let before = core.raw_read_8(SAVED_CO, -1);
    let co = EDITOR_PLAYERS + 0x3C * BLACK_HOLE_SLOT as u32 + CO;
    let saved = core.raw_read_8(SAVED_CO, -1);
    if country == 5 {
        let now = core.raw_read_8(co, -1);
        if !is_black_hole_co(now) {
            core.raw_write_8(SAVED_CO, -1, now.wrapping_add(1));
            core.raw_write_8(co, -1, BLACK_HOLE_CO);
        }
    } else if saved != 0 {
        core.raw_write_8(co, -1, saved - 1);
        core.raw_write_8(SAVED_CO, -1, 0);
    }
    if core.raw_read_8(SAVED_CO, -1) != before {
        for i in 0..11 {
            let flags = DESIGN_RING + 0x1C * i;
            let f = core.raw_read_32(flags, -1);
            if f & 1 != 0 {
                core.raw_write_32(flags, -1, f | 8);
            }
        }
        core.raw_write_8(REDRAW, -1, REDRAW_PENDING);
    }
    let (from, to) = if country == 5 { (4, 5) } else { (5, 4) };
    let mut want = [0u8; 0x100];
    let mut have = [0u8; 0x100];
    core.raw_read_range(HQ_SPRITES + 0x100 * (from - 1), -1, &mut have);
    core.raw_read_range(HQ_SPRITES + 0x100 * (to - 1), -1, &mut want);
    let mut vram = vec![0u8; 0x8000];
    core.raw_read_range(0x0601_0000, -1, &mut vram);
    let mut at = 0;
    while at + 0x100 <= vram.len() {
        if vram[at..at + 0x100] == have {
            core.raw_write_range(0x0601_0000 + at as u32, -1, &want);
        }
        at += 0x20;
    }
}

// ---------- Redrawing placed units ----------

/// Placed units are background tiles picked by their army's CO country when
/// they are drawn, so after the Black Hole switch they must be drawn again.
/// The editor's per-frame input handler (`sub_08005F4C`, void, no
/// arguments): at its entry nothing but LR is live, so a pending redraw
/// detours through the game's visible-map unit redraw (`sub_08022580`) and
/// comes back to it.
pub const EDITOR_FRAME: u32 = 0x0800_5F4C;
const UNIT_REDRAW: u32 = 0x0802_2580;
/// 0 nothing to do, 1 pending, 2 in flight.
const REDRAW: u32 = 0x0203_FFF4;
const REDRAW_PENDING: u8 = 1;
const REDRAW_IN_FLIGHT: u8 = 2;
/// The handler's return address while the detour runs.
const REDRAW_LR: u32 = 0x0203_FFEC;

/// Trap at [`EDITOR_FRAME`].
pub fn editor_frame(core: &mut Core) {
    match core.raw_read_8(REDRAW, -1) {
        REDRAW_PENDING if in_map_editor(core) => {
            let lr = core.gba().cpu().gpr(14) as u32;
            core.raw_write_32(REDRAW_LR, -1, lr);
            core.raw_write_8(REDRAW, -1, REDRAW_IN_FLIGHT);
            let cpu = core.gba_mut().cpu_mut();
            cpu.set_gpr(14, (EDITOR_FRAME | 1) as i32);
            cpu.set_thumb_pc(UNIT_REDRAW);
        }
        REDRAW_IN_FLIGHT => {
            // Back from the redraw: the handler runs with its own caller.
            let lr = core.raw_read_32(REDRAW_LR, -1);
            core.raw_write_8(REDRAW, -1, 0);
            core.gba_mut().cpu_mut().set_gpr(14, lr as i32);
        }
        _ => {}
    }
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
