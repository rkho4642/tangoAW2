//! Versus' Rules screen with the Dual Strike pack: two more rule rows,
//! **Skills** (the COs' CO skills, crate::co_skills::VERSUS_RULE) and **CO
//! Tag** (CO tag pairs, crate::tag::RULE), both ON/OFF and OFF at every boot,
//! after AW2's seven (Fog, Weather, Funds, Turn, Capt, Power, Visuals).
//!
//! AW2's rows are seven objects of the Teams/Rules record (`0x02017C50`:
//! +0x54 their pointers, +0x33 the cursor, +0x30 0 on the Rules stage,
//! +0x84.. their values), drawn by `RuleOption_Draw` (`0x080645AC`: the
//! label, sprite 0xC1 + column, 8 above; the diamond, 0xD1 + column, double
//! size and pulsing with affine matrix 0 when it is the cursor's) and their
//! value (`RuleValue_DrawOnOff`: 0xC8 "ON", 0xCB "OFF"); the record has
//! room for no more. So the two rows are drawn here as the game draws its
//! own, from the same sprites (`DrawOamObject`'s lookup: the graphic's
//! tile pool and its (tile, id) pairs, the layout by its size): the frame's
//! sprite-layer lists get their entries when they are pushed to OAM
//! (`PushSpriteLayerObjects`, `0x0801BF2C`), placed by the Visuals row
//! (they come and go with it), on a second line under Capt, Power and
//! Visuals. Their labels are drawn in AW2's font in the labels' colours.
//!
//! The cursor: RIGHT on Visuals goes on to Skills and CO Tag (and on to Fog),
//! LEFT on Fog to CO Tag; while it is on one of the two, the game's cursor
//! stays on Visuals with its arrows, pulse and help line taken off it, and
//! UP and DOWN change the row's value (with the game's cursor sound). The
//! help line is the row's (text ids 0x7302, 0x7303). Both seats' buttons
//! reach the Rules screen in netplay, so both peers set the same rules.
//! Without the pack the screen is AW2's own.

use mgba::core::Core;

use crate::ds_weather::is_on;

const RECORD: u32 = 0x0201_7C50;
const CURSOR: u32 = RECORD + 0x33;
const STAGE: u32 = RECORD + 0x30;
const OBJECTS: u32 = RECORD + 0x54;
const VISUALS: u32 = 6;
const OBJ_X: u32 = 0x28;
const OBJ_Y: u32 = 0x2A;
const OBJ_SELECTED: u32 = 0x46;

/// RAM (tangoAW2's tag state, crate::tag::UI + 0x08..): the row the cursor
/// is on (0 none, 1 Skills, 2 CO Tag), the Rules stage's input ran last
/// frame, a cursor sound to play, the sprite layers given their entries
/// this frame, the labels' tiles borrowed.
const VCURSOR: u32 = crate::tag::UI + 0x08;
const SEEN: u32 = crate::tag::UI + 0x09;
const SOUND: u32 = crate::tag::UI + 0x0A;
const LAYERS: u32 = crate::tag::UI + 0x0B;
const BORROWED: u32 = crate::tag::UI + 0x0C;
/// The labels' OBJ tiles as they were (16 tiles: 0x0203EE00..0x0203EFFF).
const SAVED: u32 = 0x0203_EE00;
const LABEL_TILE: u32 = 0x140;
const LABEL_TILES: u32 = 16;

/// The rows: label, diamond sprite, the rule's byte (1 on), help text id.
pub struct Row {
    pub label: &'static str,
    diamond: u16,
    pub value: u32,
    pub help: u16,
    help_text: &'static [u8],
}

pub const ROWS: [Row; 2] = [
    Row { label: "Skills", diamond: 0xD5, value: crate::co_skills::VERSUS_RULE, help: 0x7302, help_text: b"Select ON to use the COs' skills.\0" },
    Row {
        label: "CO Tag",
        diamond: 0xD2,
        value: crate::tag::RULE,
        help: 0x7303,
        help_text: b"Select ON for tag pairs (Teams: START).\0",
    },
];

/// Where each row goes from the Visuals row (its x, y).
const PLACES: [(i32, i32); 2] = [(-48, 56), (-16, 48)];

fn on(core: &Core, ds: bool) -> bool {
    ds && crate::pvp::in_versus(core) && !crate::five::active(core)
}

fn row_value(core: &Core, k: usize) -> bool {
    core.raw_read_8(ROWS[k].value, -1) == 1
}

/// The value as AW2's ON/OFF rows index it: 0 ON, 1 OFF.
fn index(core: &Core, k: usize) -> u8 {
    if row_value(core, k) {
        0
    } else {
        1
    }
}

const KEY_RIGHT: u32 = 1 << 4;
const KEY_LEFT: u32 = 1 << 5;
const KEY_UP: u32 = 1 << 6;
const KEY_DOWN: u32 = 1 << 7;
const DPAD: u32 = KEY_RIGHT | KEY_LEFT | KEY_UP | KEY_DOWN;

/// Every frame, before the game reads the pad. Returns the keys the game
/// gets.
pub fn tick(core: &mut Core, ds: bool, keys: u32, prev: u32) -> u32 {
    if !on(core, ds) {
        return keys;
    }
    install(core);
    core.raw_write_8(LAYERS, -1, 0);
    // (The game's frame can take two of the console's: a few frames' grace.)
    let left = core.raw_read_8(SEEN, -1);
    let seen = left > 0;
    if left > 0 {
        core.raw_write_8(SEEN, -1, left - 1);
    }
    let mut v = core.raw_read_8(VCURSOR, -1);
    if !seen || core.raw_read_8(STAGE, -1) != 0 {
        if v != 0 {
            core.raw_write_8(VCURSOR, -1, 0);
        }
        if core.raw_read_8(STAGE, -1) != 0 || !rows_shown(core) {
            restore(core);
        }
        return keys;
    }
    let pressed = keys & !prev;
    let cursor = core.raw_read_8(CURSOR, -1);
    let mut keys = keys;
    let mut sound = false;
    if v == 0 {
        if cursor as u32 == VISUALS && pressed & KEY_RIGHT != 0 {
            v = 1;
            sound = true;
            keys &= !KEY_RIGHT;
        } else if cursor == 0 && pressed & KEY_LEFT != 0 {
            v = 2;
            core.raw_write_8(CURSOR, -1, VISUALS as u8);
            sound = true;
            keys &= !KEY_LEFT;
        }
    } else {
        if core.raw_read_8(CURSOR, -1) as u32 != VISUALS {
            core.raw_write_8(CURSOR, -1, VISUALS as u8);
        }
        let k = (v - 1) as usize;
        if pressed & KEY_LEFT != 0 {
            v -= 1;
            sound = true;
        } else if pressed & KEY_RIGHT != 0 {
            if v == 2 {
                v = 0;
                core.raw_write_8(CURSOR, -1, 0);
            } else {
                v += 1;
            }
            sound = true;
        } else if pressed & KEY_UP != 0 && !row_value(core, k) {
            core.raw_write_8(ROWS[k].value, -1, 1);
            sound = true;
        } else if pressed & KEY_DOWN != 0 && row_value(core, k) {
            core.raw_write_8(ROWS[k].value, -1, 0);
            sound = true;
        }
        keys &= !DPAD;
    }
    core.raw_write_8(VCURSOR, -1, v);
    if sound {
        core.raw_write_8(SOUND, -1, 1);
    }
    keys
}

/// The help lines' strings (free ROM, after crate::tag's labels) and the
/// text table's entries for their ids.
const TEXT_TABLE: u32 = 0x0861_0A38;
const STRINGS: u32 = crate::tag::ROM + 0x400;
const SENTINEL: u32 = crate::tag::ROM + 0xFFF8;
const MAGIC: u32 = 0x3155_5256; // "VRU1"

fn install(core: &mut Core) {
    if core.raw_read_32(SENTINEL, -1) == MAGIC {
        return;
    }
    let mut at = STRINGS;
    for r in &ROWS {
        core.raw_write_range(at, -1, r.help_text);
        core.raw_write_32(TEXT_TABLE + 4 * r.help as u32, -1, at);
        at += (r.help_text.len() as u32 + 3) & !3;
    }
    core.raw_write_32(SENTINEL, -1, MAGIC);
}

// --- Traps -------------------------------------------------------------------------

/// `MatchSetupHandleRulesStageInput` (the Rules stage, every frame): seen.
const RULES_INPUT: u32 = 0x0806_6D74;
fn rules_input(core: &mut Core) {
    if is_on(core) {
        core.raw_write_8(SEEN, -1, 4);
    }
}

/// The same function past its prologue (its return address on the stack,
/// r0..r3 free): a cursor sound asked for is played first
/// (`PlayMusicOrSfx2(0x64)`, which returns here).
const RULES_INPUT_BODY: u32 = 0x0806_6D7E;
const PLAY_SOUND: u32 = 0x0803_B4DC;
const CURSOR_SOUND: i32 = 0x64;
fn rules_sound(core: &mut Core) {
    if is_on(core) && core.raw_read_8(SOUND, -1) == 1 {
        core.raw_write_8(SOUND, -1, 0);
        let cpu = core.gba_mut().cpu_mut();
        cpu.set_gpr(0, CURSOR_SOUND);
        cpu.set_gpr(14, (RULES_INPUT_BODY | 1) as i32);
        cpu.set_thumb_pc(PLAY_SOUND);
    }
}

/// `RuleOption_DrawArrows(index)`: none on Visuals while the cursor is on
/// one of ours.
const ARROWS: u32 = 0x0806_6B8C;
fn arrows(core: &mut Core) {
    if is_on(core) && core.raw_read_8(VCURSOR, -1) != 0 {
        let cpu = core.gba_mut().cpu_mut();
        let lr = cpu.gpr(14) as u32;
        cpu.set_thumb_pc(lr & !1);
    }
}

/// `MatchSetupHighlightSelectedRuleOption`'s end: Visuals not pulsing while
/// the cursor is on one of ours.
const HIGHLIGHT_END: u32 = 0x0806_6D6A;
fn highlight_end(core: &mut Core) {
    if is_on(core) && core.raw_read_8(VCURSOR, -1) != 0 {
        let obj = core.raw_read_32(OBJECTS + 4 * VISUALS, -1);
        if (0x0200_0000..0x0400_0000).contains(&obj) {
            core.raw_write_8(obj + OBJ_SELECTED, -1, 0);
        }
    }
}

/// `MatchSetupShowHelpText`, its text id in r5: the row's help line.
const HELP_ID: u32 = 0x0806_6F8A;
fn help_id(core: &mut Core) {
    let v = core.raw_read_8(VCURSOR, -1);
    if is_on(core) && v != 0 && core.raw_read_8(STAGE, -1) == 0 {
        core.gba_mut().cpu_mut().set_gpr(5, ROWS[v as usize - 1].help as i32);
    }
}

// --- Drawing -----------------------------------------------------------------------

/// The sprite-layer lists (`gUnknown_0200D510`, 0x10 a layer: next, oam1,
/// oam0, oam2, object) and the pool their entries come from
/// (`gUnknown_03002B24`, the next free entry).
const LAYER_HEADS: u32 = 0x0200_D510;
const NODE_POOL: u32 = 0x0300_2B24;
const NODE_POOL_END: u32 = 0x0200_D510;
const PUSH_LAYER: u32 = 0x0801_BF2C;

/// `DrawOamObject`'s lookup: the graphic's tile pool (`GetTilePoolForGraphic`)
/// and its (tile, id) pairs (`0x0200F920` + 0x88 a pool: +4 the palette bank
/// biased by 0x10, +5 the count, +8 the pairs); the layout by its size
/// (`0x0848B780` + 4 id: width, height in tiles; `0x0848BAE4` + 4 (h * 17 + w)).
const POOLS: u32 = 0x0200_F920;
const SIZES: u32 = 0x0848_B780;
const LAYOUTS: u32 = 0x0848_BAE4;

fn pool_of(id: u16) -> u32 {
    match id {
        0xBC.. => 5,
        0xB8..=0xBB => 4,
        0xAC..=0xB7 => 3,
        0x43..=0xAB => 2,
        0x3E..=0x42 => 1,
        _ => 0,
    }
}

fn layout(core: &Core, id: u16) -> u32 {
    let w = core.raw_read_8(SIZES + 4 * id as u32, -1) as u32;
    let h = core.raw_read_8(SIZES + 4 * id as u32 + 1, -1) as u32;
    core.raw_read_32(LAYOUTS + 4 * (h * 17 + w), -1)
}

/// (layout, oam2) for graphic `id`, as `DrawOamObject` puts it.
fn sprite(core: &Core, id: u16) -> Option<(u32, u16)> {
    let pool = POOLS + 0x88 * pool_of(id);
    let n = core.raw_read_8(pool + 5, -1) as u32;
    let bank = core.raw_read_8(pool + 4, -1) as u16;
    (0..n.min(31)).find_map(|k| {
        let e = pool + 8 + 4 * k;
        (core.raw_read_16(e + 2, -1) == id).then(|| (layout(core, id), core.raw_read_16(e, -1).wrapping_add(bank.wrapping_sub(0x10) << 12)))
    })
}

fn push(core: &mut Core, layer: u32, x: u16, y: u16, object: u32, oam2: u16) {
    let node = core.raw_read_32(NODE_POOL, -1);
    if !(0x0200_0000..0x0300_8000).contains(&node) || (node < NODE_POOL_END && node + 0x10 > NODE_POOL_END) {
        return;
    }
    let head = LAYER_HEADS + 0x10 * layer;
    core.raw_write_32(node, -1, core.raw_read_32(head, -1));
    core.raw_write_16(node + 4, -1, x);
    core.raw_write_16(node + 6, -1, y);
    core.raw_write_16(node + 8, -1, oam2);
    core.raw_write_32(node + 0xC, -1, object);
    core.raw_write_32(head, -1, node);
    core.raw_write_32(NODE_POOL, -1, node + 0x10);
}

/// A rule row's object script (`gUnknown_0858096C`): an object running it
/// is on screen (a freed one has 0 there).
const RULE_SCRIPT: u32 = 0x0858_096C;

fn visuals(core: &Core) -> Option<(i32, i32)> {
    let obj = core.raw_read_32(OBJECTS + 4 * VISUALS, -1);
    if !(0x0200_0000..0x0400_0000).contains(&obj) || core.raw_read_32(obj, -1) != RULE_SCRIPT {
        return None;
    }
    Some((core.raw_read_16(obj + OBJ_X, -1) as i16 as i32, core.raw_read_16(obj + OBJ_Y, -1) as i16 as i32))
}

/// The rows are on screen: the Rules stage, its rows out (Visuals placed).
fn rows_shown(core: &Core) -> bool {
    core.raw_read_8(STAGE, -1) == 0 && visuals(core).is_some_and(|(_, y)| (-32..176).contains(&y))
}

/// `PushSpriteLayerObjects(layer)`: our rows' entries join the layer's list
/// (labels, values and arrows on layer 0; diamonds on layer 3), once a frame.
fn push_layer(core: &mut Core) {
    if !is_on(core) || !crate::pvp::in_versus(core) || crate::five::active(core) {
        return;
    }
    // The lists are one chain (`ClearSprites`): head 0 -> layer 0 -> head 1
    // ... -> layer 4, then 5..15 from head 5: the push of layer 0 takes
    // layers 0..4.
    if core.gba().cpu().gpr(0) != 0 {
        return;
    }
    if core.raw_read_8(LAYERS, -1) != 0 || !rows_shown(core) {
        return;
    }
    core.raw_write_8(LAYERS, -1, 1);
    let Some((vx, vy)) = visuals(core) else { return };
    borrow(core);
    for layer in [3, 0] {
        push_rows(core, layer, vx, vy);
    }
}

fn push_rows(core: &mut Core, layer: u32, vx: i32, vy: i32) {
    let v = core.raw_read_8(VCURSOR, -1) as usize;
    for (k, row) in ROWS.iter().enumerate() {
        let (x, y) = (vx + PLACES[k].0, vy + PLACES[k].1);
        let selected = v == k + 1;
        if layer == 3 {
            let Some((obj, oam2)) = sprite(core, row.diamond) else { continue };
            if selected {
                push(core, 3, (x - 16) as u16 & 0x1FF, ((y - 16) as u16 & 0xFF) | 0x300, obj, oam2);
            } else {
                push(core, 3, x as u16 & 0x1FF, y as u16 & 0xFF, obj, oam2);
            }
            continue;
        }
        // The label: our tiles, as the game's (32x16, palette bank of the
        // words' pool).
        let bank = core.raw_read_8(POOLS + 0x88 * 5 + 4, -1) as u16;
        let label_obj = layout(core, 0xC1);
        push(core, 0, x as u16 & 0x1FF, (y - 8) as u16 & 0xFF, label_obj, (LABEL_TILE + 8 * k as u32) as u16 | (bank.wrapping_sub(0x10) << 12));
        let word = if index(core, k) == 0 { 0xC8 } else { 0xCB };
        if let Some((obj, oam2)) = sprite(core, word) {
            push(core, 0, (x + 8) as u16 & 0x1FF, (y + 0xC) as u16 & 0xFF, obj, oam2);
        }
        if selected {
            if index(core, k) != 0 {
                if let Some((obj, oam2)) = sprite(core, 0x43) {
                    push(core, 0, (x + 10) as u16 & 0x1FF, (y - 16) as u16 & 0xFF, obj, oam2);
                }
            }
            if index(core, k) == 0 {
                if let Some((obj, oam2)) = sprite(core, 0x44) {
                    push(core, 0, (x + 10) as u16 & 0x1FF, (y + 31) as u16 & 0xFF, obj, oam2);
                }
            }
        }
    }
}

/// AW2's proportional font (crate::skills_panel's).
const GLYPHS: u32 = 0x084C_32E4;
const WIDTHS: u32 = 0x084C_36E4;
/// The labels' colours: white letters, a dark outline (as AW2's labels).
const INK: u8 = 1;
const OUTLINE: u8 = 15;

fn label_tiles(core: &Core, s: &str) -> Vec<u8> {
    let mut px = [[0u8; 32]; 16];
    // A pixel between letters, none around a space (its own width).
    let bytes = s.as_bytes();
    let advance = |k: usize| -> i32 {
        let c = bytes[k];
        let cw = core.raw_read_8(WIDTHS + c as u32, -1) as i32;
        if c == b' ' {
            return cw.max(3);
        }
        let gap = k + 1 < bytes.len() && bytes[k + 1] != b' ';
        cw + gap as i32
    };
    let w: i32 = (0..bytes.len()).map(advance).sum();
    let mut x = ((32 - w) / 2).max(1).min(32 - w - 1).max(0);
    for (k, &c) in bytes.iter().enumerate() {
        let cw = core.raw_read_8(WIDTHS + c as u32, -1) as usize;
        let at = core.raw_read_32(GLYPHS + 4 * c as u32, -1);
        if (0x0800_0000..0x0A00_0000).contains(&at) {
            let stride = cw.div_ceil(2);
            for r in 0..12usize {
                for cx in 0..cw {
                    let b = core.raw_read_8(at + (stride * (3 + r) + cx / 2) as u32, -1);
                    let v = (b >> (4 * (cx & 1))) & 15;
                    let (xx, yy) = (x + cx as i32, 2 + r as i32);
                    if v == 0xA && (0..32).contains(&xx) && (0..16).contains(&yy) {
                        px[yy as usize][xx as usize] = INK;
                    }
                }
            }
        }
        x += advance(k);
    }
    let ink = px;
    for y in 0..16i32 {
        for x in 0..32i32 {
            if ink[y as usize][x as usize] == INK {
                continue;
            }
            let near = (-1..=1).any(|dy| {
                (-1..=1).any(|dx| {
                    let (xx, yy) = (x + dx, y + dy);
                    (0..32).contains(&xx) && (0..16).contains(&yy) && ink[yy as usize][xx as usize] == INK
                })
            });
            if near {
                px[y as usize][x as usize] = OUTLINE;
            }
        }
    }
    let mut out = Vec::with_capacity(256);
    for ty in 0..2 {
        for tx in 0..4 {
            for y in 0..8 {
                for x in (0..8).step_by(2) {
                    let a = px[8 * ty + y][8 * tx + x];
                    let b = px[8 * ty + y][8 * tx + x + 1];
                    out.push(a | b << 4);
                }
            }
        }
    }
    out
}

const OBJ_VRAM: u32 = 0x0601_0000;

fn borrow(core: &mut Core) {
    if core.raw_read_8(BORROWED, -1) != 1 {
        let mut t = vec![0u8; (32 * LABEL_TILES) as usize];
        core.raw_read_range(OBJ_VRAM + 32 * LABEL_TILE, -1, &mut t);
        core.raw_write_range(SAVED, -1, &t);
        core.raw_write_8(BORROWED, -1, 1);
    }
    for (k, row) in ROWS.iter().enumerate() {
        let t = label_tiles(core, row.label);
        let at = OBJ_VRAM + 32 * (LABEL_TILE + 8 * k as u32);
        let mut now = vec![0u8; t.len()];
        core.raw_read_range(at, -1, &mut now);
        if now != t {
            core.raw_write_range(at, -1, &t);
        }
    }
}

fn restore(core: &mut Core) {
    if core.raw_read_8(BORROWED, -1) != 1 {
        return;
    }
    let mut t = vec![0u8; (32 * LABEL_TILES) as usize];
    core.raw_read_range(SAVED, -1, &mut t);
    core.raw_write_range(OBJ_VRAM + 32 * LABEL_TILE, -1, &t);
    core.raw_write_8(BORROWED, -1, 0);
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![
        (RULES_INPUT, Box::new(rules_input)),
        (RULES_INPUT_BODY, Box::new(rules_sound)),
        (ARROWS, Box::new(arrows)),
        (HIGHLIGHT_END, Box::new(highlight_end)),
        (HELP_ID, Box::new(help_id)),
        (PUSH_LAYER, Box::new(push_layer)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ram_fits() {
        assert!(BORROWED < crate::tag::UI + 0x10);
        assert!(SAVED + 32 * LABEL_TILES <= 0x0203_F000);
        assert_eq!(pool_of(0xC8), 5);
        assert_eq!(pool_of(0x43), 2);
        for r in &ROWS {
            assert!(r.help_text.ends_with(b"\0") && r.help_text.len() <= 48);
        }
    }
}
