//! Dual Strike's power animations on AW2's map, with the Dual Strike pack
//! ([`crate::ds_power_art`] converts them; this plays them).
//!
//! The powers keep AW2's workings ([`crate::co_powers`]): Ex Machina and
//! Covering Fire run AW2's meteor script (the CPU's scorer picks the
//! target, the camera goes there, the strike hits, the map is redrawn),
//! Urban Blight AW2's mass damage (Black Wave's). Only the pictures change:
//!
//! - **Ex Machina, Covering Fire.** Two traps in AW2's meteor code, which
//!   act only in these powers (the army setting off Von Bolt's or Rachel's
//!   SCOP, the pack on; Sturm's Meteor Strike stays AW2's): where a strike
//!   draws its meteor (`sub_08044968`) the effect starts instead, with the
//!   target square the strike's proc holds (`proc+0x66`, a unit index, as
//!   AW2 takes it) and the meteor's sound; where the strike waits for the
//!   meteor to land (`sub_0806AAC4`, a `PROC_WHILE`) one frame of the
//!   effect plays and the answer is whether it goes on. The powers run
//!   tangoAW2's copies of the meteor script ([`install`]) without the
//!   64-frame fade from white that followed the meteor (the meteor had
//!   faded the screen to white). So the animation runs inside the
//!   emulated frame, from RAM ([`STATE`]), the same on both netplay peers
//!   and in replays, and for a CPU army as for a human one.
//! - **Urban Blight.** AW2's mass damage takes the overlay's map and
//!   palette as arguments (`sub_08044D70`): tangoAW2's function passes
//!   Dual Strike's (in ROM, [`URBAN_MAP`], [`URBAN_PALETTE`]); the
//!   overlay's tiles are fixed in AW2 (`0x08112704`, decompressed by
//!   `sub_08045358`), so a trap after that puts Dual Strike's 12 over the
//!   first of them. AW2's scroll (-12, +10 a frame) and fade are the ones
//!   Dual Strike kept, so they stay.
//!
//! **Screen space while a strike plays** (all of it AW2's meteor's or its
//! wave overlay's, so nothing else is on it then):
//! - OBJ tiles `0x1CA..0x209` (64; the meteor's, reloaded by the map
//!   afterwards) and OBJ palette 3 (the meteor's): each frame's sprites,
//!   at most 32 tiles, loaded when the frame changes.
//! - BG0 (the overlay layer the powers use, screen block from `BG0CNT`),
//!   its tiles at the character block + `0x5600` (the wave's 128), BG
//!   palette 8 (the wave's; put back after): the bolt, cut to 128 tiles
//!   ([`crate::ds_power_art::BgLayer::reduced`]); its map is cleared after.
//! - The display registers through the game's shadows (copied at VBlank,
//!   `sub_08012420`), saved at the start and put back at the end: BG0
//!   scroll, the offset every BG scrolls by (the shake), `DISPCNT` (BG0
//!   on), `BLDCNT` / `BLDALPHA` / `BLDY`.
//!
//! **Not as in Dual Strike.** Dual Strike's white flashes brighten the
//! whole screen while the bolt is blended; the GBA has one colour effect at
//! a time, so while the bolt is up only a flash near white (12/16 or more)
//! is shown (brightening everything, the bolt unblended), and the bolt's
//! additive blend takes the other frames. The shake moves the BG layers and the effect's sprites, not AW2's
//! unit sprites (as AW2's own meteor shake). Sounds are AW2's (the
//! meteor's, `0xC5`, at each strike).
//!
//! RAM: [`STATE`] (0x40 bytes, EWRAM `0x0203F7A0..0x0203F7DF`), zero when
//! no strike plays. ROM: `0x087C0000..0x087C0FFF` ([`ROM`]).

use mgba::core::Core;
use std::sync::OnceLock;

use crate::ds_power_art::{effect, BgLayer, Effect, PowerEffect};

// --- ROM ---------------------------------------------------------------------------

pub const ROM: u32 = 0x087C_0000;
pub const URBAN_MAP: u32 = ROM;
const URBAN_MAP_MAX: usize = 0x900;
pub const URBAN_PALETTE: u32 = ROM + 0x900;
pub const EXM_SCRIPT: u32 = ROM + 0x940;
pub const CF_SCRIPT: u32 = ROM + 0x9C0;
const SENTINEL: u32 = ROM + 0xFFC;
const MAGIC: u32 = 0x3141_5044; // "DPA1"

const METEOR_SCRIPT: u32 = 0x084A_0858;
/// The meteor script's commands: 3 to start, 7 for one strike (pick the
/// target, move there, draw, wait for the meteor, hit, redraw), 5 to end.
const METEOR_START: u32 = 3;
const METEOR_STRIKE: u32 = 7;
const METEOR_END: u32 = 5;
const OP_SLEEP: u16 = 0x0E;
const OP_FADE_FROM_WHITE: u16 = 0x26;

/// The meteor script with `strikes` strikes and without the fade from
/// white that followed the meteor (it had faded the screen to white).
fn script(core: &Core, strikes: u32) -> Vec<u8> {
    let read = |k: u32| {
        let mut c = [0u8; 8];
        core.raw_read_range(METEOR_SCRIPT + 8 * k, -1, &mut c);
        if u16::from_le_bytes([c[0], c[1]]) == OP_FADE_FROM_WHITE {
            c = [0; 8];
            c[..2].copy_from_slice(&OP_SLEEP.to_le_bytes());
            c[2] = 1;
        }
        c
    };
    let mut s = Vec::new();
    for k in 0..METEOR_START {
        s.extend_from_slice(&read(k));
    }
    for _ in 0..strikes {
        for k in METEOR_START..METEOR_START + METEOR_STRIKE {
            s.extend_from_slice(&read(k));
        }
    }
    for k in METEOR_START + METEOR_STRIKE..METEOR_START + METEOR_STRIKE + METEOR_END {
        s.extend_from_slice(&read(k));
    }
    s
}

/// The converted effects, once.
struct Art {
    exm: Effect,
    cf: Effect,
    /// Ex Machina's bolt, in the 128 tiles of the wave's space.
    bolt: BgLayer,
    /// Urban Blight: its tiles, and its map LZ77-compressed for AW2.
    urban_tiles: Vec<u8>,
    urban_map: Vec<u8>,
    urban_palette: [u16; 16],
}

static ART: OnceLock<Option<Art>> = OnceLock::new();

fn art() -> Option<&'static Art> {
    ART.get_or_init(|| {
        let exm = effect(PowerEffect::ExMachina)?;
        let cf = effect(PowerEffect::CoveringFire)?;
        let ub = effect(PowerEffect::UrbanBlight)?;
        let bolt = exm.bg.as_ref()?.reduced(BG_TILES);
        let ubg = ub.bg?;
        let map: Vec<u8> = ubg.map.iter().flat_map(|m| m.to_le_bytes()).collect();
        let urban_map = crate::lz77::compress(&map);
        if urban_map.len() > URBAN_MAP_MAX || ubg.tiles.len() > 32 * BG_TILES {
            return None;
        }
        Some(Art { exm, cf, bolt, urban_tiles: ubg.tiles, urban_map, urban_palette: ubg.palette })
    })
    .as_ref()
}

/// Whether the animations are there (the pack and its pictures).
pub fn available() -> bool {
    art().is_some()
}

/// Writes the functions, scripts and Urban Blight's map once (the same
/// bytes on every peer). `false` without the pictures.
pub fn install(core: &mut Core) -> bool {
    let Some(art) = art() else { return false };
    if core.raw_read_32(SENTINEL, -1) == MAGIC {
        return true;
    }
    core.raw_write_range(URBAN_MAP, -1, &art.urban_map);
    let pal: Vec<u8> = art.urban_palette.iter().flat_map(|c| c.to_le_bytes()).collect();
    core.raw_write_range(URBAN_PALETTE, -1, &pal);
    let exm = script(core, 1);
    let cf = script(core, 3);
    core.raw_write_range(EXM_SCRIPT, -1, &exm);
    core.raw_write_range(CF_SCRIPT, -1, &cf);
    core.raw_write_32(SENTINEL, -1, MAGIC);
    true
}

// --- RAM and the display -------------------------------------------------------------

/// What plays: +0 kind (0 none, 1 Ex Machina, 2 Covering Fire), +1/+2 the
/// target square, +3 the sprite frame loaded (1 + clip * 32 + frame),
/// +4 u16 frames played, +6/+8 s16 the camera at the start, +0x0A the
/// saved shadows ([`SHADOWS`], u16 each), +0x20 BG palette 8 saved,
/// +0x1E 1 once the bolt's map is written.
pub const STATE: u32 = 0x0203_F7A0;
const KIND: u32 = STATE;
const TARGET: u32 = STATE + 1;
const LOADED: u32 = STATE + 3;
const TIME: u32 = STATE + 4;
const CAMERA: u32 = STATE + 6;
const SAVED: u32 = STATE + 0x0A;
const SAVED_PALETTE: u32 = STATE + 0x20;
const BOLT_SHOWN: u32 = STATE + 0x1E;
const STATE_LEN: u32 = 0x40;

/// The display shadows `sub_08012420` copies to the registers at VBlank.
const DISPCNT: u32 = 0x0300_30CC;
const BG0CNT: u32 = 0x0300_2B6C;
const BG0HOFS: u32 = 0x0300_1FF8;
const BG0VOFS: u32 = 0x0300_1418;
/// Subtracted from every BG's scroll.
const SHIFT_X: u32 = 0x0300_30D0;
const SHIFT_Y: u32 = 0x0300_2B20;
const BLDCNT: u32 = 0x0300_30E0;
const EVA: u32 = 0x0300_2020;
const EVB: u32 = 0x0300_2B28;
const BLDY: u32 = 0x0300_1FFC;
const SHADOWS: [u32; 10] = [DISPCNT, BG0CNT, BG0HOFS, BG0VOFS, SHIFT_X, SHIFT_Y, BLDCNT, EVA, EVB, BLDY];

const PAL_BUFFER: u32 = 0x0300_20C0;
const PAL_RAM: u32 = 0x0500_0000;
const BG_PALETTE: u32 = 8;
const OBJ_PALETTE: u16 = 3;
const OBJ_VRAM: u32 = 0x0601_0000;
const OBJ_TILE: u16 = 0x1CA;
const OBJ_TILES: usize = 64;
/// The wave overlay's tiles: character block + 0x5600, 128 of them.
const BG_TILE_OFFSET: u32 = 0x5600;
const BG_TILES: usize = 128;
const BG0_ON: u16 = 0x0100;
/// Brighten everything; BG0 blended over everything else.
const BLEND_BRIGHTEN: u16 = 0x00BF;
const BLEND_BOLT: u16 = 0x3E41;
const FLASH_OVER_BOLT: u8 = 12;

const MAP_POINTER: u32 = 0x0849_9590;
const BG0_MAP_BUFFER: u32 = 0x0849_9578;
const BG_FLUSH: u32 = 0x0300_2F00;
const UNITS_POINTER: u32 = 0x0849_9594;
const UNIT: u32 = 12;
const CURRENT_ARMY: u32 = 0x0300_33EC;
const OAM_NEXT: u32 = 0x0300_141C;
const SPRITE_LIST: u32 = 0x0300_0278;
const SPRITE_LIST_SIZE: u32 = 0x400;
/// Sprite-list entries before the map's own sprites, and the y of an
/// entry nothing uses this frame.
const RESERVED: u32 = 16;
const PARKED_Y: u16 = 160;
const SCREEN_W: i32 = 240;
const SCREEN_H: i32 = 160;

fn camera(core: &Core) -> (i32, i32) {
    let m = core.raw_read_32(MAP_POINTER, -1);
    (core.raw_read_16(m + 4, -1) as i16 as i32, core.raw_read_16(m + 6, -1) as i16 as i32)
}

fn write_if_changed(core: &mut Core, at: u32, bytes: &[u8]) {
    let mut now = vec![0u8; bytes.len()];
    core.raw_read_range(at, -1, &mut now);
    if now != bytes {
        core.raw_write_range(at, -1, bytes);
    }
}

fn set16(core: &mut Core, at: u32, v: u16) {
    if core.raw_read_16(at, -1) != v {
        core.raw_write_16(at, -1, v);
    }
}

fn palette_bytes(p: &[u16; 16]) -> Vec<u8> {
    p.iter().flat_map(|c| c.to_le_bytes()).collect()
}

/// BG0's character block.
fn bg0_chars(core: &Core) -> u32 {
    0x0600_0000 + 0x4000 * ((core.raw_read_16(BG0CNT, -1) as u32 >> 2) & 3)
}

// --- Traps ------------------------------------------------------------------------

/// `sub_08044968(proc)`, a strike draws its meteor, just after its
/// `push {lr}`: in Ex Machina and Covering Fire the effect starts instead
/// (the target square from the strike's proc as AW2 takes it,
/// `proc+0x66`, a unit index), the display state is kept, and the meteor's
/// sound plays (`sub_0803B4DC(0xC5)`, returning to the function's end).
const DRAW_METEOR: u32 = 0x0804_496A;
const DRAW_METEOR_END: u32 = 0x0804_498D;
const PLAY_SOUND: u32 = 0x0803_B4DC;
const METEOR_SOUND: i32 = 0xC5;
fn draw_meteor(core: &mut Core) {
    if art().is_none() || !crate::ds_weather::is_on(core) {
        return;
    }
    use crate::co_powers::{activating, RACHEL, SCOP, VON_BOLT};
    let army = core.raw_read_16(CURRENT_ARMY, -1) as u32;
    let kind = match activating(core, army) {
        (VON_BOLT, SCOP) => 1,
        (RACHEL, SCOP) => 2,
        _ => return,
    };
    let proc = core.gba().cpu().gpr(0) as u32;
    let i = core.raw_read_16(proc + 0x66, -1) as i16;
    for k in 0..STATE_LEN {
        core.raw_write_8(STATE + k, -1, 0);
    }
    if i > 0 {
        let u = core.raw_read_32(UNITS_POINTER, -1) + UNIT * i as u32;
        let (x, y) = (core.raw_read_8(u + 2, -1), core.raw_read_8(u + 3, -1));
        let (cx, cy) = camera(core);
        core.raw_write_8(KIND, -1, kind);
        core.raw_write_8(TARGET, -1, x);
        core.raw_write_8(TARGET + 1, -1, y);
        core.raw_write_16(CAMERA, -1, cx as u16);
        core.raw_write_16(CAMERA + 2, -1, cy as u16);
        for (k, &a) in SHADOWS.iter().enumerate() {
            let v = core.raw_read_16(a, -1);
            core.raw_write_16(SAVED + 2 * k as u32, -1, v);
        }
        let mut pal = [0u8; 32];
        core.raw_read_range(PAL_BUFFER + 32 * BG_PALETTE, -1, &mut pal);
        core.raw_write_range(SAVED_PALETTE, -1, &pal);
    }
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, METEOR_SOUND);
    cpu.set_gpr(14, DRAW_METEOR_END as i32);
    cpu.set_thumb_pc(PLAY_SOUND);
}

/// `sub_0806AAC4`, "the meteor is still falling", just after its
/// `push {lr}`: while a strike's effect plays, it plays one more frame
/// and the function returns whether it goes on.
const METEOR_FALLING: u32 = 0x0806_AAC6;
const METEOR_FALLING_END: u32 = 0x0806_AAD2;
fn meteor_falling(core: &mut Core) {
    let Some(art) = art() else { return };
    if core.raw_read_8(KIND, -1) == 0 {
        return;
    }
    let going = frame(core, art);
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, going as i32);
    cpu.set_thumb_pc(METEOR_FALLING_END);
}

fn frame(core: &mut Core, art: &Art) -> bool {
    let kind = core.raw_read_8(KIND, -1);
    let e = match kind {
        1 => &art.exm,
        2 => &art.cf,
        _ => return false,
    };
    let target = (core.raw_read_8(TARGET, -1), core.raw_read_8(TARGET + 1, -1));
    let t = core.raw_read_16(TIME, -1) as u32;
    let start_cam_y = core.raw_read_16(CAMERA + 2, -1) as i16 as i32;
    if t >= e.length(target, start_cam_y) {
        finish(core);
        return false;
    }
    let cam = camera(core);
    let saved = |core: &Core, a: u32| {
        let k = SHADOWS.iter().position(|&s| s == a).unwrap() as u32;
        core.raw_read_16(SAVED + 2 * k, -1)
    };

    // Shake: every BG, and the effect's sprites.
    let (dx, dy) = e.shake_at(t, target, start_cam_y);
    let (sx0, sy0) = (saved(core, SHIFT_X), saved(core, SHIFT_Y));
    set16(core, SHIFT_X, sx0.wrapping_add(dx as i16 as u16));
    set16(core, SHIFT_Y, sy0.wrapping_add(dy as i16 as u16));

    // The bolt on BG0.
    let blend = e.blend_at(t, target, start_cam_y);
    if let (Some(_), Some((h, v)), 1) = (blend, e.scroll_at(t, target, cam), kind) {
        if core.raw_read_8(BOLT_SHOWN, -1) == 0 {
            show_bolt(core, &art.bolt, h, v);
            core.raw_write_8(BOLT_SHOWN, -1, 1);
        }
        set16(core, BG0HOFS, h as u16);
        set16(core, BG0VOFS, v as u16);
        let d = saved(core, DISPCNT) | BG0_ON;
        set16(core, DISPCNT, d);
    } else if core.raw_read_8(BOLT_SHOWN, -1) == 1 {
        hide_bolt(core);
    }

    // Flash, else the bolt's blend, else as it was.
    // One colour effect at a time: while the bolt is up, only a flash
    // near white wins over its blend (the bolt unblended shows its dark
    // glow, which additive blending hides).
    let flash = e.flash_at(t, target, start_cam_y);
    let bolt_up = core.raw_read_8(BOLT_SHOWN, -1) == 1;
    if flash >= if bolt_up { FLASH_OVER_BOLT } else { 1 } {
        set16(core, BLDCNT, BLEND_BRIGHTEN);
        set16(core, BLDY, flash as u16);
    } else if let (Some((a, b)), 1) = (blend, core.raw_read_8(BOLT_SHOWN, -1)) {
        set16(core, BLDCNT, BLEND_BOLT);
        set16(core, EVA, a as u16);
        set16(core, EVB, b as u16);
    } else {
        for a in [BLDCNT, EVA, EVB, BLDY] {
            let v = saved(core, a);
            set16(core, a, v);
        }
    }

    // Sprites.
    let pal = palette_bytes(&e.palette(0)[0]);
    for base in [PAL_BUFFER, PAL_RAM] {
        write_if_changed(core, base + 0x200 + 32 * OBJ_PALETTE as u32, &pal);
    }
    // As the heal effect does: the list's first 16 entries are this pass's,
    // the map's own sprites follow; take entries still parked this frame
    // (y = 160) and leave the others alone.
    let base = core.raw_read_32(SPRITE_LIST, -1);
    let (reserved_end, list_end) = (base + 8 * RESERVED, base + SPRITE_LIST_SIZE);
    let mut oam = core.raw_read_32(OAM_NEXT, -1);
    let mut next = oam;
    for p in e.sprites_at(t, target, start_cam_y) {
        let key = 1 + (p.clip * 32 + p.frame) as u8;
        let (tiles, pieces) = e.frame_tiles(p.clip, p.frame);
        if core.raw_read_8(LOADED, -1) != key && tiles.len() <= 32 * OBJ_TILES {
            write_if_changed(core, OBJ_VRAM + 32 * OBJ_TILE as u32, &tiles);
            core.raw_write_8(LOADED, -1, key);
        }
        let (ax, ay) = (p.x - cam.0 + dx as i32, p.y - cam.1 + dy as i32);
        for piece in &pieces {
            let (w, h) = piece.dims();
            let (x, y) = (ax + piece.x as i32, ay + piece.y as i32);
            if x + w as i32 <= 0 || x >= SCREEN_W || y + h as i32 <= 0 || y >= SCREEN_H {
                continue;
            }
            while oam + 8 <= list_end && oam >= reserved_end && core.raw_read_16(oam, -1) & 0xFF != PARKED_Y {
                oam += 8;
            }
            if oam + 8 > list_end {
                break;
            }
            let a = piece.oam(ax, ay, OBJ_TILE, OBJ_PALETTE as u8);
            core.raw_write_16(oam, -1, a[0]);
            core.raw_write_16(oam + 2, -1, a[1]);
            core.raw_write_16(oam + 4, -1, a[2]);
            oam += 8;
            if oam <= reserved_end {
                next = oam;
            }
        }
    }
    core.raw_write_32(OAM_NEXT, -1, next);
    core.raw_write_16(TIME, -1, (t + 1) as u16);
    true
}

/// The bolt's tiles, palette and map on BG0; map cells whose place is off
/// the screen are left blank, so the layer's wrap-round shows nothing.
fn show_bolt(core: &mut Core, bolt: &BgLayer, hofs: i32, vofs: i32) {
    let chars = bg0_chars(core);
    write_if_changed(core, chars + BG_TILE_OFFSET, &bolt.tiles);
    let pal = palette_bytes(&bolt.palette);
    for base in [PAL_BUFFER, PAL_RAM] {
        write_if_changed(core, base + 32 * BG_PALETTE, &pal);
    }
    let first = (BG_TILE_OFFSET / 32) as u16;
    let mut map = Vec::with_capacity(2048);
    for (k, &m) in bolt.map.iter().enumerate() {
        let (x, y) = (8 * (k % 32) as i32 - hofs, 8 * (k / 32) as i32 - vofs);
        let off = x + 8 <= -8 || x >= SCREEN_W + 8 || y + 8 <= -8 || y >= SCREEN_H + 8;
        let e = if off || m & 0x3FF == 0 { 0 } else { (m & 0xFFF) + first | (BG_PALETTE as u16) << 12 };
        map.extend_from_slice(&e.to_le_bytes());
    }
    write_bg0_map(core, &map);
}

/// BG0's map goes through the game's buffer (`*0x08499578`, copied to
/// VRAM at VBlank when bit 0 of `0x03002F00` is set, `sub_08013AEC`),
/// which would otherwise overwrite it; the wave overlay does the same.
fn write_bg0_map(core: &mut Core, map: &[u8]) {
    let buffer = core.raw_read_32(BG0_MAP_BUFFER, -1);
    write_if_changed(core, buffer, map);
    let f = core.raw_read_16(BG_FLUSH, -1);
    core.raw_write_16(BG_FLUSH, -1, f | 1);
}

fn hide_bolt(core: &mut Core) {
    write_bg0_map(core, &[0u8; 2048]);
    core.raw_write_8(BOLT_SHOWN, -1, 0);
}

/// The strike is over: the display as it was, the state cleared.
fn finish(core: &mut Core) {
    if core.raw_read_8(BOLT_SHOWN, -1) == 1 {
        hide_bolt(core);
    }
    let mut pal = [0u8; 32];
    core.raw_read_range(SAVED_PALETTE, -1, &mut pal);
    if core.raw_read_8(KIND, -1) == 1 {
        for base in [PAL_BUFFER, PAL_RAM] {
            write_if_changed(core, base + 32 * BG_PALETTE, &pal);
        }
    }
    for (k, &a) in SHADOWS.iter().enumerate() {
        let v = core.raw_read_16(SAVED + 2 * k as u32, -1);
        set16(core, a, v);
    }
    for k in 0..STATE_LEN {
        core.raw_write_8(STATE + k, -1, 0);
    }
}

/// `sub_08045358`, just after it decompressed the wave overlay's fixed
/// tiles (r5 = the overlay's proc, +0x4C its map): in Urban Blight, Dual
/// Strike's tiles go over them.
const WAVE_TILES_LOADED: u32 = 0x0804_5370;
fn wave_tiles_loaded(core: &mut Core) {
    let Some(art) = art() else { return };
    let proc = core.gba().cpu().gpr(5) as u32;
    if !crate::ds_weather::is_on(core) || core.raw_read_32(proc + 0x4C, -1) != URBAN_MAP {
        return;
    }
    let chars = bg0_chars(core);
    write_if_changed(core, chars + BG_TILE_OFFSET, &art.urban_tiles);
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![
        (DRAW_METEOR, Box::new(draw_meteor)),
        (METEOR_FALLING, Box::new(meteor_falling)),
        (WAVE_TILES_LOADED, Box::new(wave_tiles_loaded)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room() {
        assert!(URBAN_MAP + URBAN_MAP_MAX as u32 <= URBAN_PALETTE);
        assert!(EXM_SCRIPT + 8 * 15 <= CF_SCRIPT);
        assert!(CF_SCRIPT + 8 * 29 <= SENTINEL);
        assert!(STATE + STATE_LEN <= 0x0203_F800);
    }

    /// With `TANGOAW2_DS_ROM`: the bolt fits the wave's tiles.
    #[test]
    #[ignore]
    fn the_bolt_fits() {
        crate::ds_pack::pack().expect("TANGOAW2_DS_ROM");
        let art = art().expect("art");
        assert!(art.bolt.tiles.len() <= 32 * BG_TILES);
        assert!(art.bolt.map.iter().all(|&m| ((m & 0x3FF) as usize) < BG_TILES));
        assert!(art.urban_map.len() <= URBAN_MAP_MAX);
    }
}
