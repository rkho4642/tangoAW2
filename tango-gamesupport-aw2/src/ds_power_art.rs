//! Dual Strike's own map animations for the new COs' powers, converted at
//! run time from the Dual Strike pack ([`crate::ds_pack`]) into GBA-ready
//! data: 4bpp 8x8 OBJ tiles, BGR555 palettes, OAM-shaped sprite frames with
//! their durations, BG layers (tiles, map, palette) and the screen effects
//! (white flashes, shakes, blending) with their timing. Converting only:
//! the caller decides where the tiles go and draws the frames.
//!
//! Only three new COs have a power animation of their own
//! ([`PowerEffect`]):
//!
//! - **Von Bolt, Ex Machina** (SCOP routine arm9 `0x020E4F38`): two white
//!   flashes, then a pink lightning bolt (a BG layer, additive) strikes the
//!   target square, the screen shakes and white-flashes, purple shock
//!   sparks spread from the square, the bolt fades out; then the damage.
//! - **Rachel, Covering Fire** (`0x020E4614`): three times, a missile falls
//!   from the top of the screen onto a target square, the screen shakes and
//!   flashes, and a ring and an explosion play there. It is the missile
//!   silo's strike (the same procedure and art), aimed three times.
//! - **Kindle, Urban Blight** (`0x020E3EB0`): a full-screen overlay of pink
//!   sparkles and streaks (a BG layer) scrolls diagonally while it fades in
//!   and out. It is Dual Strike's "wave power" overlay (Hawke's, Drake's
//!   and Olaf's powers use the same one with other pictures).
//!
//! The other new COs get Dual Strike's generic power effect, the same for
//! every CO (routine `0x020E2658`: small sparkles over the player's own
//! units), which AW2's own generic power effect stands in for: Jugger,
//! Koal, Grimm, Jake, Javier, Kindle's SCOP, Rachel's COP, and Von Bolt
//! has no COP. Sasha's Market Crash (`0x020E1D2C`) and War Bonds
//! (`0x020E1D0C`) show nothing on the map at all (a sound only).
//!
//! **Colours.** None of the three is army-coloured: every palette is a
//! fixed file loaded whoever plays (no army index anywhere in the loads),
//! so [`Effect::palette`] is the same for all five armies. The previews
//! draw each effect over units of all five armies to check it reads on
//! each.
//!
//! **Where Dual Strike keeps them** (all read from the pack):
//!
//! | effect | graphics | palette | animation |
//! |---|---|---|---|
//! | Ex Machina sparks | `syogun/1fb` (LZ10, 84 OBJ tiles) | `syogun/1fc` | arm9 `0x02134D68` sequence 0 |
//! | Ex Machina bolt | `syogun/1fe` (LZ10, 256 BG tiles) + `syogun/1fd` (32x32 map) | `syogun/1ff` | code |
//! | Covering Fire | `bmap/065` (LZ10, 245 OBJ tiles) | `bmap/066` | arm9 `0x0213DD38` sequences 2 (fall), 3 (impact) |
//! | Urban Blight | `syogun/1f1` (LZ10, 104 BG tiles) + `syogun/1f2` (32x32 map) | `syogun/1f8` | code |
//! | screen shake | - | - | arm9 `0x021574C0` (s16 x,y pairs, `0x7FFF` ends) |
//!
//! (`syogun/1fa` is the sparks again as a linear texture for the 3D
//! engine, which Dual Strike uses when the map is on the 3D screen; `1fb`
//! is the same pictures as 8x8 OBJ tiles.)
//!
//! **The animation format** (Dual Strike's 2D sprite animations,
//! `0x02021C24` / `0x02022478`): `u16` offset of the frame table, `u16`
//! offset of the sequence table (both from the start); each table holds
//! `u16` offsets from the table's own start. A frame is `u16 count` then
//! `count` pieces of three `u16`: GBA OAM attribute 0 (y as `s8`, shape),
//! attribute 1 (x as 9-bit signed, flips, size) and the tile number (4bpp
//! tiles, 1D mapping) with the palette in the top 4 bits. A sequence is
//! `(u16 duration, u16 frame)` pairs; duration 0 ends it (frame 1: end,
//! frame 255: loop). Pieces are placed from an anchor point the code
//! gives, and drawn first-listed on top (OAM order).
//!
//! **Positions** are map pixels: the target square `(x, y)` covers
//! `16x..16x+15`, `16y..16y+15` (both games have 16-pixel squares), so the
//! screen position is the map position minus the camera. Times are
//! frames at 60 Hz from the effect's start ([`Start`]); the cut-in picture
//! ("Ex Machina!") plays before and is not part of these. The timelines
//! follow the powers' procedures (`0x02168AD0` Ex Machina, `0x02168E78`
//! the missile) and were checked frame by frame in a running Dual Strike
//! (flash and blend registers, OAM); [`Effect::sprites_at`] and its
//! neighbours say what shows at a given frame.
//!
//! **Sizes for the GBA.** Ex Machina's sparks: 84 OBJ tiles in all, at
//! most 32 in one frame. Covering Fire: 180 (missile 16, impact 164), at
//! most 32 a frame. With little free OBJ space, load each frame's own tiles
//! ([`Effect::frame_tiles`]). The bolt (253 BG tiles, 12x32 tiles of
//! picture) and Urban Blight's overlay (12 BG tiles, a 32x32 map) are BG
//! layers and need one free BG with its own character block.
//!
//! **Not the same as Dual Strike.** Its white flashes brighten the whole
//! screen (`MASTER_BRIGHT`) while the bolt is alpha-blended; the GBA has
//! one colour effect at a time, so during Ex Machina's bolt flash one of
//! them has to be done another way (brightening the palettes, say). Dual
//! Strike draws the sparks with its 3D engine when the map is on the 3D
//! screen, where they land up to ~12 pixels towards the screen's centre
//! from the square; the 2D path (these frames) puts them on it.

use crate::ds_art::lz10;

/// The power animations Dual Strike has for the new COs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerEffect {
    /// Von Bolt's SCOP.
    ExMachina,
    /// Rachel's SCOP.
    CoveringFire,
    /// Kindle's COP.
    UrbanBlight,
}

impl PowerEffect {
    pub const ALL: [PowerEffect; 3] = [
        PowerEffect::ExMachina,
        PowerEffect::CoveringFire,
        PowerEffect::UrbanBlight,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PowerEffect::ExMachina => "Ex Machina",
            PowerEffect::CoveringFire => "Covering Fire",
            PowerEffect::UrbanBlight => "Urban Blight",
        }
    }
}

/// One sprite of a frame, as GBA OAM describes it. `x`, `y` are the
/// sprite's top-left corner from the clip's anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    pub x: i16,
    pub y: i16,
    /// OAM shape: 0 square, 1 wide, 2 tall.
    pub shape: u8,
    /// OAM size 0..3.
    pub size: u8,
    /// First tile (1D mapping: the sprite's tiles follow in rows).
    pub tile: u16,
    /// Index into [`Effect::palette`]'s list (always 0 here).
    pub palette: u8,
    pub hflip: bool,
    pub vflip: bool,
}

impl Piece {
    /// Width and height in pixels.
    pub fn dims(&self) -> (u32, u32) {
        const SIZES: [[(u32, u32); 4]; 3] = [
            [(8, 8), (16, 16), (32, 32), (64, 64)],
            [(16, 8), (32, 8), (32, 16), (64, 32)],
            [(8, 16), (8, 32), (16, 32), (32, 64)],
        ];
        SIZES[self.shape as usize % 3][self.size as usize & 3]
    }

    /// How many tiles the sprite uses.
    pub fn tile_count(&self) -> u16 {
        let (w, h) = self.dims();
        (w * h / 64) as u16
    }

    /// GBA OAM attributes 0..2 with the anchor at screen `(ax, ay)`, the
    /// effect's tile 0 at OBJ tile `tile_base` and its palette at OBJ
    /// palette `palette_base`.
    pub fn oam(&self, ax: i32, ay: i32, tile_base: u16, palette_base: u8) -> [u16; 3] {
        let (x, y) = (ax + self.x as i32, ay + self.y as i32);
        [
            (y & 0xFF) as u16 | (self.shape as u16) << 14,
            (x & 0x1FF) as u16 | (self.hflip as u16) << 12 | (self.vflip as u16) << 13 | (self.size as u16) << 14,
            (tile_base + self.tile) & 0x3FF | ((palette_base + self.palette) as u16 & 15) << 12,
        ]
    }

    fn from_oam(a0: u16, a1: u16, a2: u16) -> Piece {
        let x = (a1 & 0x1FF) as i16;
        Piece {
            x: if x >= 0x100 { x - 0x200 } else { x },
            y: (a0 & 0xFF) as u8 as i8 as i16,
            shape: (a0 >> 14) as u8,
            size: (a1 >> 14) as u8,
            tile: a2 & 0x3FF,
            palette: (a2 >> 12) as u8,
            hflip: a1 & 0x1000 != 0,
            vflip: a1 & 0x2000 != 0,
        }
    }
}

/// A frame: its sprites (first on top) and how long it shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub pieces: Vec<Piece>,
    pub duration: u8,
}

/// When something starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    /// This many frames after the effect starts.
    At(u16),
    /// This many frames after clip `.0` ends (a clip with motion ends when
    /// the motion does; a clip without, after its last frame's duration).
    After(usize, u16),
}

/// Where a clip's anchor is, from the target square `(x, y)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    /// The square's centre, `(16x + 8, 16y + 8)`.
    Centre,
    /// The middle of its top edge, `(16x + 8, 16y)`.
    Top,
    /// The middle of its bottom edge, `(16x + 8, 16y + 16)`.
    Bottom,
}

impl Anchor {
    /// The anchor in map pixels.
    pub fn at(self, x: u8, y: u8) -> (i32, i32) {
        let (cx, top) = (16 * x as i32 + 8, 16 * y as i32);
        match self {
            Anchor::Centre => (cx, top + 8),
            Anchor::Top => (cx, top),
            Anchor::Bottom => (cx, top + 16),
        }
    }
}

/// How a clip's anchor moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    /// It stays on the anchor.
    Still,
    /// It starts at the top edge of the screen straight above the anchor
    /// (anchor y = the camera's y) and moves down `speed` pixels a frame;
    /// it is drawn at each position up to and including the last one not
    /// below the anchor, and ends there. Dual Strike: `0x020ED114`.
    FallFromScreenTop { speed: u8 },
}

/// A sprite animation placed on the map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clip {
    pub name: &'static str,
    pub start: Start,
    pub anchor: Anchor,
    pub motion: Motion,
    /// Loop the frames (until the motion ends) instead of playing once.
    pub looping: bool,
    pub frames: Vec<Frame>,
}

impl Clip {
    /// Frames the clip takes to play once.
    pub fn length(&self) -> u32 {
        self.frames.iter().map(|f| f.duration as u32).sum()
    }

    /// The frame showing `t` frames after the clip starts (wrapping when
    /// looping), or `None` once a clip that does not loop has ended.
    pub fn frame_at(&self, t: u32) -> Option<usize> {
        let len = self.length();
        let mut t = if self.looping && len > 0 { t % len } else { t };
        for (i, f) in self.frames.iter().enumerate() {
            if t < f.duration as u32 {
                return Some(i);
            }
            t -= f.duration as u32;
        }
        None
    }
}

/// Where a BG layer sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BgPlacement {
    /// Fixed on the target: the layer's pixel `(x, y)` is on the target
    /// square's top-left corner, so the scroll registers are
    /// `(x - (16tx - camera_x), y - (16ty - camera_y))`. Dual Strike uses a
    /// 512x256 layer whose right half is empty, so the picture never
    /// repeats sideways; with a 256-wide layer, blank the other copy. The
    /// picture is as tall as the layer, so it would wrap round below the
    /// target: draw [`BgLayer::map_for_row`] instead of the map.
    OnTarget { x: i16, y: i16 },
    /// Tiled over the whole screen, the scroll registers moving by
    /// `(dx, dy)` every frame (the picture moves by minus that).
    Scrolling { dx: i8, dy: i8 },
}

/// A full-screen BG layer (text mode, 4bpp, 32x32 map).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BgLayer {
    /// The tiles the map uses, and only those (4bpp, 32 bytes each).
    pub tiles: Vec<u8>,
    /// 32x32 map entries in GBA text-BG format (tile, flips, palette 0).
    pub map: Vec<u16>,
    pub palette: [u16; 16],
    pub start: Start,
    pub placement: BgPlacement,
    /// Per frame from `start`, the blend weights `(eva, evb)` 0..16: the
    /// layer is the first target (weight `eva`) over everything else
    /// (`evb`), as GBA `BLDALPHA` does. The layer ends after the last.
    pub blend: Vec<(u8, u8)>,
}

impl BgLayer {
    /// The map with the top `2 * (screen_rows - row)` tile rows blanked,
    /// for a target square `row` squares below the top of a screen
    /// `screen_rows` squares tall (Dual Strike: 12; the GBA: 10), so the
    /// part of the picture that wraps round below the target is empty
    /// (Dual Strike, `0x020E4C0C`). Tile 0 is blank.
    pub fn map_for_row(&self, row: i32, screen_rows: i32) -> Vec<u16> {
        let rows = (2 * (screen_rows - row)).clamp(0, 32) as usize;
        let mut map = self.map.clone();
        map[..32 * rows].fill(0);
        map
    }

    /// The layer with at most `max` tiles: the tile that costs least to
    /// drop (its difference from the closest other tile, allowing flips,
    /// times how often the map uses it) is repeatedly replaced by that
    /// tile. Tile 0 stays (blank). For the GBA's smaller free BG space
    /// (the bolt's 253 tiles into the 128 AW2's wave overlays use); under
    /// additive blending the merged tiles are hard to tell apart.
    pub fn reduced(&self, max: usize) -> BgLayer {
        let n = self.tiles.len() / 32;
        if n <= max {
            return self.clone();
        }
        let rgb = |c: u16| [(c & 31) as i32, (c >> 5 & 31) as i32, (c >> 10 & 31) as i32];
        let pal: Vec<[i32; 3]> = self.palette.iter().map(|&c| rgb(c)).collect();
        let px = |t: usize, x: usize, y: usize| (self.tiles[32 * t + 4 * y + x / 2] >> (4 * (x & 1))) & 15;
        let diff = |a: u8, b: u8| -> i32 {
            match (a, b) {
                (0, 0) => 0,
                (0, _) | (_, 0) => 48,
                _ => (0..3).map(|k| (pal[a as usize][k] - pal[b as usize][k]).abs()).sum(),
            }
        };
        // dist[i][j] = (difference, flip) of tile i drawn as tile j flipped.
        let mut dist = vec![vec![(i32::MAX, 0u16); n]; n];
        for (i, row) in dist.iter_mut().enumerate() {
            for (j, d_ij) in row.iter_mut().enumerate() {
                if i == j {
                    continue;
                }
                for f in 0..4u16 {
                    let mut d = 0;
                    for y in 0..8 {
                        for x in 0..8 {
                            let fx = if f & 1 != 0 { 7 - x } else { x };
                            let fy = if f & 2 != 0 { 7 - y } else { y };
                            d += diff(px(i, x, y), px(j, fx, fy));
                        }
                    }
                    if d < d_ij.0 {
                        *d_ij = (d, f);
                    }
                }
            }
        }
        let mut uses = vec![0i64; n];
        for &m in &self.map {
            uses[(m & 0x3FF) as usize] += 1;
        }
        // to[i] = (tile, flip) tile i is drawn as.
        let mut to: Vec<(usize, u16)> = (0..n).map(|i| (i, 0)).collect();
        let mut alive = vec![true; n];
        let mut left = n;
        while left > max {
            let mut best = (i64::MAX, 0, 0);
            for i in 1..n {
                if !alive[i] {
                    continue;
                }
                for j in 0..n {
                    if j != i && alive[j] {
                        let c = dist[i][j].0 as i64 * uses[i].max(1);
                        if c < best.0 {
                            best = (c, i, j);
                        }
                    }
                }
            }
            let (_, i, j) = best;
            alive[i] = false;
            uses[j] += uses[i];
            left -= 1;
            let f = dist[i][j].1;
            for t in to.iter_mut() {
                if t.0 == i {
                    *t = (j, t.1 ^ f);
                }
            }
        }
        let keep: Vec<usize> = (0..n).filter(|&i| alive[i]).collect();
        let mut tiles = Vec::with_capacity(32 * keep.len());
        for &k in &keep {
            tiles.extend_from_slice(&self.tiles[32 * k..32 * k + 32]);
        }
        let map = self
            .map
            .iter()
            .map(|&m| {
                let (t, f) = to[(m & 0x3FF) as usize];
                let flips = ((m >> 10) & 3) ^ f;
                keep.iter().position(|&k| k == t).unwrap() as u16 | flips << 10
            })
            .collect();
        BgLayer { tiles, map, ..self.clone() }
    }
}

/// The whole screen brightened towards white: per frame from `start`,
/// 0 (none) .. 16 (white), as DS `MASTER_BRIGHT` / GBA `BLDY`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flash {
    pub start: Start,
    pub levels: Vec<u8>,
}

/// The whole map picture (BG layers and sprites, the effect's own too)
/// offset by `(x, y)` pixels, one entry per frame from `start`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shake {
    pub start: Start,
    pub offsets: Vec<(i8, i8)>,
}

/// A converted power animation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Effect {
    pub which: PowerEffect,
    /// The clips' OBJ tiles (4bpp, 32 bytes each); pieces number into it.
    pub tiles: Vec<u8>,
    /// The clips' OBJ palette (colour 0 transparent).
    pub palettes: Vec<[u16; 16]>,
    pub clips: Vec<Clip>,
    pub bg: Option<BgLayer>,
    pub flashes: Vec<Flash>,
    pub shakes: Vec<Shake>,
    /// How many target squares play the effect, one after another (the
    /// game picks them). Dual Strike moves the camera to each first.
    pub targets: u8,
    /// When one target's effect is over (the damage is shown after).
    pub end: Start,
}

impl Effect {
    /// The OBJ palettes for an army (0 Orange Star .. 4 Black Hole). Dual
    /// Strike colours none of these effects by army, so this is the same
    /// for all five.
    pub fn palette(&self, _army: u8) -> &[[u16; 16]] {
        &self.palettes
    }

    /// OBJ tiles for all clips at once.
    pub fn tile_count(&self) -> usize {
        self.tiles.len() / 32
    }

    /// The tiles one frame needs, packed, and its pieces renumbered into
    /// them: what to load for that frame alone.
    pub fn frame_tiles(&self, clip: usize, frame: usize) -> (Vec<u8>, Vec<Piece>) {
        let mut tiles = Vec::new();
        let mut placed: Vec<(u16, u16, u16)> = Vec::new(); // (old, count, new)
        let mut pieces = Vec::new();
        for p in &self.clips[clip].frames[frame].pieces {
            let n = p.tile_count();
            let new = match placed.iter().find(|&&(o, c, _)| o == p.tile && c >= n) {
                Some(&(_, _, new)) => new,
                None => {
                    let new = (tiles.len() / 32) as u16;
                    let from = 32 * p.tile as usize;
                    tiles.extend_from_slice(&self.tiles[from..from + 32 * n as usize]);
                    placed.push((p.tile, n, new));
                    new
                }
            };
            pieces.push(Piece { tile: new, ..*p });
        }
        (tiles, pieces)
    }

    /// The most OBJ tiles any single frame needs.
    pub fn max_frame_tiles(&self) -> usize {
        (0..self.clips.len())
            .flat_map(|c| (0..self.clips[c].frames.len()).map(move |f| (c, f)))
            .map(|(c, f)| self.frame_tiles(c, f).0.len() / 32)
            .max()
            .unwrap_or(0)
    }

    /// Each clip's `[start, end)` in frames for one target, with the
    /// camera's top at map pixel `camera_y` (only a falling clip depends
    /// on it).
    pub fn clip_times(&self, target: (u8, u8), camera_y: i32) -> Vec<(u32, u32)> {
        let mut times: Vec<(u32, u32)> = Vec::new();
        for c in &self.clips {
            let start = resolve(c.start, &times);
            let length = match c.motion {
                Motion::Still => c.length(),
                Motion::FallFromScreenTop { speed } => {
                    let (_, ay) = c.anchor.at(target.0, target.1);
                    ((ay - camera_y).max(0) / speed.max(1) as i32) as u32 + 1
                }
            };
            times.push((start, start + length));
        }
        times
    }

    /// When one target's effect ends.
    pub fn length(&self, target: (u8, u8), camera_y: i32) -> u32 {
        resolve(self.end, &self.clip_times(target, camera_y))
    }

    /// What to draw `t` frames into one target's effect: every clip frame
    /// showing, with its anchor in map pixels (shake not added), in clip
    /// order (earlier clips on top).
    pub fn sprites_at(&self, t: u32, target: (u8, u8), camera_y: i32) -> Vec<Placed> {
        let times = self.clip_times(target, camera_y);
        let mut out = Vec::new();
        for (i, (c, &(start, end))) in self.clips.iter().zip(&times).enumerate() {
            if t < start || t >= end {
                continue;
            }
            let k = t - start;
            let Some(frame) = c.frame_at(k) else { continue };
            let (x, mut y) = c.anchor.at(target.0, target.1);
            if let Motion::FallFromScreenTop { speed } = c.motion {
                y = camera_y + speed as i32 * k as i32;
            }
            out.push(Placed { clip: i, frame, x, y });
        }
        out
    }

    /// The white flash level (0..16) at `t`.
    pub fn flash_at(&self, t: u32, target: (u8, u8), camera_y: i32) -> u8 {
        let times = self.clip_times(target, camera_y);
        self.flashes
            .iter()
            .filter_map(|f| at(&f.levels, t, resolve(f.start, &times)).copied())
            .max()
            .unwrap_or(0)
    }

    /// The shake offset at `t`.
    pub fn shake_at(&self, t: u32, target: (u8, u8), camera_y: i32) -> (i8, i8) {
        let times = self.clip_times(target, camera_y);
        self.shakes
            .iter()
            .find_map(|s| at(&s.offsets, t, resolve(s.start, &times)).copied())
            .unwrap_or((0, 0))
    }

    /// The BG layer's blend weights at `t`, or `None` while it is off.
    pub fn blend_at(&self, t: u32, target: (u8, u8), camera_y: i32) -> Option<(u8, u8)> {
        let bg = self.bg.as_ref()?;
        at(&bg.blend, t, resolve(bg.start, &self.clip_times(target, camera_y))).copied()
    }

    /// The BG layer's scroll registers at `t`, the camera's top-left at map
    /// pixel `camera` (shake not added).
    pub fn scroll_at(&self, t: u32, target: (u8, u8), camera: (i32, i32)) -> Option<(i32, i32)> {
        let bg = self.bg.as_ref()?;
        let k = t.checked_sub(resolve(bg.start, &self.clip_times(target, camera.1)))? as i32;
        Some(match bg.placement {
            BgPlacement::OnTarget { x, y } => (
                x as i32 - (16 * target.0 as i32 - camera.0),
                y as i32 - (16 * target.1 as i32 - camera.1),
            ),
            BgPlacement::Scrolling { dx, dy } => (dx as i32 * k, dy as i32 * k),
        })
    }
}

/// A clip frame to draw: its anchor in map pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    pub clip: usize,
    pub frame: usize,
    pub x: i32,
    pub y: i32,
}

fn resolve(s: Start, times: &[(u32, u32)]) -> u32 {
    match s {
        Start::At(t) => t as u32,
        Start::After(c, plus) => times.get(c).map_or(0, |&(_, end)| end) + plus as u32,
    }
}

fn at<T>(v: &[T], t: u32, start: u32) -> Option<&T> {
    v.get(t.checked_sub(start)? as usize)
}

/// Dual Strike's animation for `which`, converted; `None` without the pack
/// (or if the pack is not the release these offsets are for).
pub fn effect(which: PowerEffect) -> Option<Effect> {
    let pack = crate::ds_pack::pack()?;
    let file = |p: &str| pack.file(p);
    let lz = |p: &str| lz10(pack.file(p)?);
    let palette = |p: &str| -> Option<[u16; 16]> {
        let b = file(p)?.get(..32)?;
        Some(std::array::from_fn(|i| u16::from_le_bytes([b[2 * i], b[2 * i + 1]])))
    };
    let arm9 = |a: u32, n: usize| pack.arm9_at(a, n);
    match which {
        PowerEffect::ExMachina => {
            let anim = Anim::parse(arm9(SPARKS_ANIM, 0x200)?)?;
            let sparks = anim.clip(
                "shock sparks",
                0,
                Start::At(SPARKS_AT),
                Anchor::Centre,
                Motion::Still,
                false,
            )?;
            let (tiles, clips) = compact(&lz("syogun/1fb")?, vec![sparks])?;
            let bolt = bg_layer(&lz("syogun/1fe")?, &lz("syogun/1fd")?, palette("syogun/1ff")?, true)?;
            let mut blend = vec![(0, 16), (3, 16), (6, 16), (9, 16), (12, 16)];
            blend.extend(std::iter::repeat_n((16, 16), 26));
            for eva in (0..16).rev() {
                blend.extend(std::iter::repeat_n((eva, 16), 5));
            }
            Some(Effect {
                which,
                tiles,
                palettes: vec![palette("syogun/1fc")?],
                clips,
                bg: Some(BgLayer {
                    start: Start::At(BOLT_AT),
                    placement: BgPlacement::OnTarget { x: 0x28, y: 0xD0 },
                    blend,
                    ..bolt
                }),
                flashes: vec![
                    Flash {
                        start: Start::At(0),
                        levels: SHORT_FLASH.to_vec(),
                    },
                    Flash {
                        start: Start::At(14),
                        levels: SHORT_FLASH.to_vec(),
                    },
                    Flash {
                        start: Start::At(BOLT_AT + 1),
                        levels: BOLT_FLASH.to_vec(),
                    },
                ],
                shakes: vec![Shake {
                    start: Start::At(BOLT_AT),
                    offsets: shake(pack, 90)?,
                }],
                targets: 1,
                end: Start::At(BOLT_AT + 108),
            })
        }
        PowerEffect::CoveringFire => {
            let anim = Anim::parse(arm9(MISSILE_ANIM, 0x400)?)?;
            let fall = anim.clip(
                "missile",
                2,
                Start::At(0),
                Anchor::Top,
                Motion::FallFromScreenTop { speed: 8 },
                true,
            )?;
            let blast = anim.clip("impact", 3, Start::After(0, 1), Anchor::Bottom, Motion::Still, false)?;
            let (tiles, clips) = compact(&lz("bmap/065")?, vec![fall, blast])?;
            Some(Effect {
                which,
                tiles,
                palettes: vec![palette("bmap/066")?],
                clips,
                bg: None,
                flashes: vec![Flash {
                    start: Start::After(0, 0),
                    levels: SHORT_FLASH.to_vec(),
                }],
                shakes: vec![Shake {
                    start: Start::After(0, 0),
                    offsets: shake(pack, 60)?,
                }],
                targets: 3,
                end: Start::After(0, 61),
            })
        }
        PowerEffect::UrbanBlight => {
            let layer = bg_layer(&lz("syogun/1f1")?, &lz("syogun/1f2")?, palette("syogun/1f8")?, false)?;
            // Measured in Dual Strike (BLDALPHA every frame): the weight
            // rises one step every 3 frames to 10/16, holds, and falls.
            let mut blend = vec![(0u8, 16u8); 2];
            for eva in 1..=10 {
                blend.extend(std::iter::repeat_n((eva, 16 - eva), if eva == 10 { 19 } else { 3 }));
            }
            for eva in (0..10).rev() {
                blend.extend(std::iter::repeat_n((eva, 16 - eva), 3));
            }
            blend.extend(std::iter::repeat_n((0, 16), 2));
            let length = blend.len() as u16;
            Some(Effect {
                which,
                tiles: Vec::new(),
                palettes: Vec::new(),
                clips: Vec::new(),
                bg: Some(BgLayer {
                    start: Start::At(0),
                    placement: BgPlacement::Scrolling { dx: 10, dy: -12 },
                    blend,
                    ..layer
                }),
                flashes: Vec::new(),
                shakes: Vec::new(),
                targets: 1,
                end: Start::At(length),
            })
        }
    }
}

/// Ex Machina's spark animation and Covering Fire's missile animation.
const SPARKS_ANIM: u32 = 0x0213_4D68;
const MISSILE_ANIM: u32 = 0x0213_DD38;
/// Dual Strike's shake pattern 2 (`0x02003FA0(2, frames)`).
const SHAKE_TABLE: u32 = 0x0215_74C0;
/// Ex Machina's timeline, from its procedure (`0x02168AD0`) and a frame-by-
/// frame capture: flashes at 0 and 14; at 54 the bolt layer and the shake
/// start, the flash the frame after; the sparks show from 82.
const BOLT_AT: u16 = 54;
const SPARKS_AT: u16 = 82;
/// `0x020041E4(4, 0, 2)`: a white flash, as captured.
const SHORT_FLASH: [u8; 7] = [4, 8, 12, 16, 16, 8, 0];
/// `0x020041E4(4, 0, 20)`: up, then down over 20 frames.
const BOLT_FLASH: [u8; 25] = [
    4, 8, 12, 16, 16, 16, 15, 14, 13, 12, 12, 11, 10, 9, 8, 8, 7, 6, 5, 4, 4, 3, 2, 1, 0,
];

/// `frames` offsets of Dual Strike's shake pattern (repeated as it does).
fn shake(pack: &crate::ds_pack::Pack, frames: usize) -> Option<Vec<(i8, i8)>> {
    let b = pack.arm9_at(SHAKE_TABLE, 0x200)?;
    let table: Vec<(i8, i8)> = b
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| (i16::from_le_bytes([c[0], c[1]]), i16::from_le_bytes([c[2], c[3]])))
        .take_while(|&(x, _)| x != 0x7FFF)
        .map(|(x, y)| (x as i8, y as i8))
        .collect();
    if table.is_empty() {
        return None;
    }
    Some(table.iter().copied().cycle().take(frames).collect())
}

/// A BG picture with only the tiles its map uses, renumbered in map order.
/// With `blank`, tile 0 is an empty tile (added if the map has none).
fn bg_layer(tiles: &[u8], map: &[u8], palette: [u16; 16], blank: bool) -> Option<BgLayer> {
    if map.len() < 2048 {
        return None;
    }
    let mut used: Vec<u16> = Vec::new();
    let mut out_tiles = Vec::new();
    if blank {
        let empty = (0..tiles.len() / 32).find(|&t| tiles[32 * t..32 * t + 32].iter().all(|&b| b == 0));
        out_tiles.extend_from_slice(&[0; 32]);
        used.push(empty.map_or(u16::MAX, |t| t as u16));
    }
    let mut out_map = Vec::with_capacity(1024);
    for e in map[..2048].as_chunks::<2>().0.iter().map(|&c| u16::from_le_bytes(c)) {
        let t = e & 0x3FF;
        let new = match used.iter().position(|&u| u == t) {
            Some(i) => i,
            None => {
                out_tiles.extend_from_slice(tiles.get(32 * t as usize..32 * t as usize + 32)?);
                used.push(t);
                used.len() - 1
            }
        };
        out_map.push(new as u16 | (e & 0x0C00));
    }
    Some(BgLayer {
        tiles: out_tiles,
        map: out_map,
        palette,
        start: Start::At(0),
        placement: BgPlacement::OnTarget { x: 0, y: 0 },
        blend: Vec::new(),
    })
}

/// Keep only the tiles the clips use, in their order (a sprite's tiles
/// stay consecutive), and renumber the pieces.
fn compact(all: &[u8], mut clips: Vec<Clip>) -> Option<(Vec<u8>, Vec<Clip>)> {
    let mut keep = vec![false; all.len() / 32];
    for p in clips.iter().flat_map(|c| &c.frames).flat_map(|f| &f.pieces) {
        for t in p.tile..p.tile + p.tile_count() {
            *keep.get_mut(t as usize)? = true;
        }
    }
    let mut new = vec![0u16; keep.len()];
    let mut tiles = Vec::new();
    for (t, _) in keep.iter().enumerate().filter(|(_, &k)| k) {
        new[t] = (tiles.len() / 32) as u16;
        tiles.extend_from_slice(&all[32 * t..32 * t + 32]);
    }
    for p in clips.iter_mut().flat_map(|c| &mut c.frames).flat_map(|f| &mut f.pieces) {
        p.tile = new[p.tile as usize];
    }
    Some((tiles, clips))
}

/// A Dual Strike sprite animation (see the module notes for the format).
struct Anim {
    frames: Vec<Vec<Piece>>,
    /// `(duration, frame)`, the closing opcode left out.
    sequences: Vec<Vec<(u8, u16)>>,
}

impl Anim {
    fn parse(b: &[u8]) -> Option<Anim> {
        let u16_at = |o: usize| b.get(o..o + 2).map(|s| u16::from_le_bytes([s[0], s[1]]) as usize);
        let (ft, st) = (u16_at(0)?, u16_at(2)?);
        let mut frames = Vec::new();
        let mut first = usize::MAX;
        for i in 0..st.checked_sub(ft)? / 2 {
            let a = ft + u16_at(ft + 2 * i)?;
            first = first.min(a);
            let mut pieces = Vec::new();
            for k in 0..u16_at(a)? {
                let p = a + 2 + 6 * k;
                pieces.push(Piece::from_oam(
                    u16_at(p)? as u16,
                    u16_at(p + 2)? as u16,
                    u16_at(p + 4)? as u16,
                ));
            }
            frames.push(pieces);
        }
        let mut sequences = Vec::new();
        for i in 0..first.checked_sub(st)? / 2 {
            let mut p = st + u16_at(st + 2 * i)?;
            let mut seq = Vec::new();
            loop {
                let (d, f) = (u16_at(p)?, u16_at(p + 2)?);
                p += 4;
                if d == 0 {
                    break;
                }
                if f >= frames.len() || seq.len() > 64 {
                    return None;
                }
                seq.push((d as u8, f as u16));
            }
            sequences.push(seq);
        }
        Some(Anim { frames, sequences })
    }

    fn clip(
        &self,
        name: &'static str,
        seq: usize,
        start: Start,
        anchor: Anchor,
        motion: Motion,
        looping: bool,
    ) -> Option<Clip> {
        let frames = self
            .sequences
            .get(seq)?
            .iter()
            .map(|&(duration, f)| Frame {
                pieces: self.frames[f as usize].clone(),
                duration,
            })
            .collect();
        Some(Clip {
            name,
            start,
            anchor,
            motion,
            looping,
            frames,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oam_round_trip() {
        let p = Piece::from_oam(0x00F0, 0x91F0, 0x0024);
        assert_eq!(
            (p.x, p.y, p.dims(), p.hflip, p.vflip, p.tile),
            (-16, -16, (32, 32), true, false, 0x24)
        );
        assert_eq!(p.oam(0, 0, 0, 0), [0x00F0, 0x91F0, 0x0024]);
        assert_eq!(p.oam(100, 50, 0x1F9, 7)[2], (0x1F9 + 0x24) & 0x3FF | 7 << 12);
    }

    /// With `TANGOAW2_DS_ROM`: every effect converts, at the sizes the
    /// notes give. `cargo test -p tango-gamesupport-aw2 ds_power_art -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn the_three_effects_convert() {
        crate::ds_pack::pack().expect("TANGOAW2_DS_ROM");
        for which in PowerEffect::ALL {
            let e = effect(which).unwrap_or_else(|| panic!("{which:?}"));
            eprintln!(
                "{}: {} OBJ tiles, at most {} a frame, {} clips ({} frames), BG {:?} tiles",
                which.name(),
                e.tile_count(),
                e.max_frame_tiles(),
                e.clips.len(),
                e.clips.iter().map(|c| c.frames.len()).sum::<usize>(),
                e.bg.as_ref().map(|b| b.tiles.len() / 32)
            );
            assert_eq!(e.tiles.len() % 32, 0);
            for c in &e.clips {
                for f in &c.frames {
                    assert!(f.duration > 0);
                    for p in &f.pieces {
                        assert!(p.tile as usize + p.tile_count() as usize <= e.tile_count());
                        assert_eq!(p.palette, 0);
                    }
                }
            }
            if let Some(bg) = &e.bg {
                assert_eq!(bg.map.len(), 1024);
                assert!(bg.map.iter().all(|&m| ((m & 0x3FF) as usize) < bg.tiles.len() / 32));
                assert!(bg.blend.iter().all(|&(a, b)| a <= 16 && b <= 16));
            }
            assert!(e
                .shakes
                .iter()
                .all(|s| s.offsets.iter().all(|&(x, y)| x.abs() <= 8 && y.abs() <= 8)));
        }
        let ex = effect(PowerEffect::ExMachina).unwrap();
        assert_eq!(
            (ex.tile_count(), ex.max_frame_tiles(), ex.clips[0].frames.len()),
            (84, 32, 8)
        );
        assert_eq!(ex.clips[0].length(), 42);
        assert_eq!(ex.bg.as_ref().unwrap().tiles.len() / 32, 253);
        let cf = effect(PowerEffect::CoveringFire).unwrap();
        assert_eq!((cf.tile_count(), cf.max_frame_tiles()), (180, 32));
        assert_eq!(
            (cf.clips[0].frames.len(), cf.clips[1].frames.len(), cf.clips[1].length()),
            (2, 16, 54)
        );
        // As captured: the missile at y 0, 8 .. 96 over 13 frames for the
        // square (14, 6), then the flash and shake, the impact a frame on.
        assert_eq!(cf.clip_times((14, 6), 0), vec![(0, 13), (14, 68)]);
        assert_eq!(
            cf.sprites_at(12, (14, 6), 0)[0],
            Placed {
                clip: 0,
                frame: 0,
                x: 232,
                y: 96
            }
        );
        assert_eq!((cf.flash_at(13, (14, 6), 0), cf.flash_at(16, (14, 6), 0)), (4, 16));
        assert_eq!(cf.length((14, 6), 0), 74);
        let ub = effect(PowerEffect::UrbanBlight).unwrap();
        assert_eq!(ub.bg.as_ref().unwrap().tiles.len() / 32, 12);
        assert_eq!(ub.bg.as_ref().unwrap().blend.len(), 80);
    }
}
