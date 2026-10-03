//! Preview of the Dual Strike power animations
//! ([`tango_gamesupport_aw2::ds_power_art`]), for approval. For each effect:
//!
//! - `<effect>_frames.png`: every clip frame, once, on a 7x7-square field
//!   with the target square (outlined) in the middle.
//! - `<effect>_timeline.png`: one target's whole effect on a 240x160 screen
//!   (plains and AW2 Tanks), every 3rd frame, as it would be composed:
//!   sprites, the BG layer blended, the shake and the white flash.
//! - `<effect>_armies.png`: key frames over units of each army (rows Orange
//!   Star, Blue Moon, Green Earth, Yellow Comet, Black Hole): Dual Strike
//!   colours none of these effects by army, so this checks they read on
//!   all five.
//! - With `ds_<effect>_bg.rgb` (256x192 RGB) and `ds_<effect>_times.txt`
//!   (`target X Y` then one frame time a line) in the output directory:
//!   `ds_<effect>_conv.png`, the conversion drawn over that Dual Strike
//!   screen at those times, to set beside captures of the real game.
//!
//! Usage: TANGOAW2_DS_ROM=<Dual Strike ROM or pack> ds_power_art_preview <aw2 rom> <out dir>

use tango_gamesupport_aw2::ds_power_art::{effect, BgPlacement, Effect, PowerEffect};

const PLAINS: [u8; 3] = [136, 200, 72];
const GRID: [u8; 3] = [120, 184, 64];
const GAP: [u8; 3] = [24, 24, 24];

fn rgb(c: u16) -> [u8; 3] {
    let s = |v: u16| ((v & 31) * 255 / 31) as u8;
    [s(c), s(c >> 5), s(c >> 10)]
}

#[derive(Clone)]
struct Canvas {
    w: usize,
    h: usize,
    px: Vec<[u8; 3]>,
}

impl Canvas {
    fn new(w: usize, h: usize, fill: [u8; 3]) -> Self {
        Canvas {
            w,
            h,
            px: vec![fill; w * h],
        }
    }
    fn put(&mut self, x: i32, y: i32, c: [u8; 3]) {
        if (0..self.w as i32).contains(&x) && (0..self.h as i32).contains(&y) {
            self.px[y as usize * self.w + x as usize] = c;
        }
    }
    fn get(&self, x: i32, y: i32) -> [u8; 3] {
        self.px[y as usize * self.w + x as usize]
    }
    fn paste(&mut self, o: &Canvas, x: usize, y: usize) {
        for yy in 0..o.h {
            for xx in 0..o.w {
                self.put((x + xx) as i32, (y + yy) as i32, o.px[yy * o.w + xx]);
            }
        }
    }
    fn outline(&mut self, x: i32, y: i32, w: i32, h: i32, c: [u8; 3]) {
        for i in 0..w {
            self.put(x + i, y, c);
            self.put(x + i, y + h - 1, c);
        }
        for i in 0..h {
            self.put(x, y + i, c);
            self.put(x + w - 1, y + i, c);
        }
    }
    fn scaled(&self, s: usize) -> Canvas {
        let mut o = Canvas::new(self.w * s, self.h * s, GAP);
        for y in 0..o.h {
            for x in 0..o.w {
                o.px[y * o.w + x] = self.px[(y / s) * self.w + x / s];
            }
        }
        o
    }
    fn save(&self, path: &std::path::Path) {
        std::fs::write(path, png(self.w, self.h, &self.px)).unwrap();
        eprintln!("wrote {}", path.display());
    }
}

fn nibble(tiles: &[u8], tile: usize, x: usize, y: usize) -> u8 {
    tiles
        .get(32 * tile + 4 * y + x / 2)
        .map_or(0, |b| (b >> (4 * (x & 1))) & 15)
}

/// A clip frame with its anchor at screen `(ax, ay)`.
fn draw_frame(cv: &mut Canvas, e: &Effect, clip: usize, frame: usize, ax: i32, ay: i32) {
    let pal = &e.palette(0)[0];
    for p in e.clips[clip].frames[frame].pieces.iter().rev() {
        let (w, h) = p.dims();
        for y in 0..h as usize {
            for x in 0..w as usize {
                let sx = if p.hflip { w as usize - 1 - x } else { x };
                let sy = if p.vflip { h as usize - 1 - y } else { y };
                let t = p.tile as usize + (sy / 8) * (w as usize / 8) + sx / 8;
                let v = nibble(&e.tiles, t, sx % 8, sy % 8);
                if v != 0 {
                    cv.put(
                        ax + p.x as i32 + x as i32,
                        ay + p.y as i32 + y as i32,
                        rgb(pal[v as usize]),
                    );
                }
            }
        }
    }
}

/// The BG layer with scroll `(hofs, vofs)`, blended as GBA BLDALPHA does.
fn draw_bg(cv: &mut Canvas, e: &Effect, hofs: i32, vofs: i32, (eva, evb): (u8, u8)) {
    let bg = e.bg.as_ref().unwrap();
    let wrap_x = matches!(bg.placement, BgPlacement::Scrolling { .. });
    for y in 0..cv.h as i32 {
        for x in 0..cv.w as i32 {
            let (mut bx, by) = (x + hofs, (y + vofs).rem_euclid(256));
            bx = if wrap_x { bx.rem_euclid(256) } else { bx.rem_euclid(512) };
            if bx >= 256 {
                continue;
            }
            let m = bg.map[(by / 8 * 32 + bx / 8) as usize];
            let (mut tx, mut ty) = ((bx % 8) as usize, (by % 8) as usize);
            if m & 0x400 != 0 {
                tx = 7 - tx;
            }
            if m & 0x800 != 0 {
                ty = 7 - ty;
            }
            let v = nibble(&bg.tiles, (m & 0x3FF) as usize, tx, ty);
            if v == 0 {
                continue;
            }
            let (a, b) = (rgb(bg.palette[v as usize]), cv.get(x, y));
            let c =
                std::array::from_fn(|i| ((eva as u32 * a[i] as u32 + evb as u32 * b[i] as u32) / 16).min(255) as u8);
            cv.put(x, y, c);
        }
    }
}

/// The map under the effect: plains, a grid, and Tanks of `army` on some
/// squares near the target.
struct Field {
    tank: Vec<u8>,
    palettes: Vec<[u16; 16]>,
}

impl Field {
    fn background(
        &self,
        w: usize,
        h: usize,
        cam: (i32, i32),
        target: (u8, u8),
        army: usize,
        shake: (i8, i8),
    ) -> Canvas {
        let mut cv = Canvas::new(w, h, PLAINS);
        let (ox, oy) = (shake.0 as i32 - cam.0, shake.1 as i32 - cam.1);
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                if (x - ox).rem_euclid(16) == 0 || (y - oy).rem_euclid(16) == 0 {
                    cv.put(x, y, GRID);
                }
            }
        }
        let (tx, ty) = (target.0 as i32, target.1 as i32);
        for (dx, dy) in [(0, 0), (-1, 0), (1, 1), (0, -2), (2, 0), (-2, 1), (1, -1), (-1, 2)] {
            let (sx, sy) = (16 * (tx + dx) + ox, 16 * (ty + dy) + oy);
            let pal = &self.palettes[army];
            for (k, (qx, qy)) in [(0, 0), (8, 0), (0, 8), (8, 8)].into_iter().enumerate() {
                for y in 0..8 {
                    for x in 0..8 {
                        let v = nibble(&self.tank, k, x, y);
                        if v != 0 {
                            let px = if army == 1 || army == 3 { 15 - (qx + x) } else { qx + x };
                            cv.put(sx + px as i32, sy + (qy + y) as i32, rgb(pal[v as usize]));
                        }
                    }
                }
            }
        }
        cv
    }
}

/// One target's effect at `t` over `base` (drawn with the shake already).
fn compose(base: &Canvas, e: &Effect, t: u32, target: (u8, u8), cam: (i32, i32)) -> Canvas {
    let mut cv = base.clone();
    let (sx, sy) = e.shake_at(t, target, cam.1);
    if let (Some(blend), Some((h, v))) = (e.blend_at(t, target, cam.1), e.scroll_at(t, target, cam)) {
        draw_bg(&mut cv, e, h - sx as i32, v - sy as i32, blend);
    }
    for p in e.sprites_at(t, target, cam.1).iter().rev() {
        draw_frame(
            &mut cv,
            e,
            p.clip,
            p.frame,
            p.x - cam.0 + sx as i32,
            p.y - cam.1 + sy as i32,
        );
    }
    let lvl = e.flash_at(t, target, cam.1) as u32;
    for c in &mut cv.px {
        *c = std::array::from_fn(|i| (c[i] as u32 + (255 - c[i] as u32) * lvl / 16) as u8);
    }
    cv
}

fn sheet(cells: &[Canvas], cols: usize) -> Canvas {
    let (w, h) = (cells[0].w + 2, cells[0].h + 2);
    let rows = cells.len().div_ceil(cols);
    let mut s = Canvas::new(w * cols.min(cells.len()), h * rows, GAP);
    for (i, c) in cells.iter().enumerate() {
        s.paste(c, (i % cols) * w + 1, (i / cols) * h + 1);
    }
    s
}

fn slug(which: PowerEffect) -> &'static str {
    match which {
        PowerEffect::ExMachina => "ex_machina",
        PowerEffect::CoveringFire => "covering_fire",
        PowerEffect::UrbanBlight => "urban_blight",
        PowerEffect::BlackOnyx => "black_onyx",
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let aw2 = std::fs::read(&args[1]).expect("AW2 ROM");
    let out = std::path::PathBuf::from(&args[2]);
    std::fs::create_dir_all(&out).unwrap();
    let at = |a: u32, n: usize| &aw2[(a - 0x0800_0000) as usize..][..n];
    // AW2's idle Tank (map-unit sheet slot 2, frame 0) and its army palettes.
    let field = Field {
        tank: at(0x0810_BE60 + 2 * 128, 128).to_vec(),
        palettes: (0..5)
            .map(|a| {
                let b = at(0x0810_E6E0 + 32 * a, 32);
                std::array::from_fn(|i| u16::from_le_bytes([b[2 * i], b[2 * i + 1]]))
            })
            .collect(),
    };
    let (target, cam) = ((7u8, 5u8), (0, 0));
    for which in PowerEffect::ALL {
        let e = effect(which).expect("TANGOAW2_DS_ROM");
        let name = slug(which);
        let len = e.length(target, cam.1);
        eprintln!(
            "{}: {} frames a target, {} OBJ tiles ({} at most a frame), BG {:?} tiles",
            which.name(),
            len,
            e.tile_count(),
            e.max_frame_tiles(),
            e.bg.as_ref().map(|b| b.tiles.len() / 32)
        );

        // Every clip frame once.
        let mut cells = Vec::new();
        for (c, clip) in e.clips.iter().enumerate() {
            for f in 0..clip.frames.len() {
                let mut cv = Canvas::new(112, 112, PLAINS);
                cv.outline(48, 48, 16, 16, [255, 255, 255]);
                let (ax, ay) = clip.anchor.at(3, 3);
                draw_frame(&mut cv, &e, c, f, ax, ay);
                cells.push(cv.scaled(2));
            }
        }
        if !cells.is_empty() {
            sheet(&cells, 8).save(&out.join(format!("{name}_frames.png")));
        }

        // The timeline.
        let cells: Vec<Canvas> = (0..len)
            .step_by(3)
            .map(|t| {
                let base = field.background(240, 160, cam, target, 0, e.shake_at(t, target, cam.1));
                compose(&base, &e, t, target, cam)
            })
            .collect();
        sheet(&cells, 8).save(&out.join(format!("{name}_timeline.png")));

        // Five armies.
        let keys: &[u32] = match which {
            PowerEffect::ExMachina => &[3, 57, 62, 84, 90, 96, 104, 112, 124, 150],
            PowerEffect::CoveringFire => &[4, 12, 14, 17, 20, 24, 28, 36, 46, 60],
            PowerEffect::UrbanBlight => &[6, 14, 22, 30, 40, 50, 60, 68, 74, 78],
            PowerEffect::BlackOnyx => &[2, 6, 10, 12, 16, 22, 30, 45, 60, 80],
        };
        let mut cells = Vec::new();
        for army in 0..5 {
            for &t in keys {
                let base = field.background(96, 96, (16 * 4, 16 * 2), target, army, e.shake_at(t, target, 32));
                cells.push(compose(&base, &e, t, target, (16 * 4, 16 * 2)).scaled(2));
            }
        }
        sheet(&cells, keys.len()).save(&out.join(format!("{name}_armies.png")));

        // Over a Dual Strike screen, for the comparison with captures.
        if let (Ok(bg), Ok(times)) = (
            std::fs::read(out.join(format!("ds_{name}_bg.rgb"))),
            std::fs::read_to_string(out.join(format!("ds_{name}_times.txt"))),
        ) {
            let mut lines = times.lines();
            let t: Vec<i32> = lines
                .next()
                .unwrap()
                .split_whitespace()
                .skip(1)
                .map(|v| v.parse().unwrap())
                .collect();
            let base = Canvas {
                w: 256,
                h: 192,
                px: bg.as_chunks::<3>().0.to_vec(),
            };
            let cells: Vec<Canvas> = lines
                .map(|l| {
                    let t0: u32 = l.trim().parse().unwrap();
                    compose(&base, &e, t0, (t[0] as u8, t[1] as u8), (0, t[2]))
                })
                .collect();
            sheet(&cells, cells.len()).save(&out.join(format!("ds_{name}_conv.png")));
        }
    }
}

/// A PNG (RGB, zlib with stored blocks).
fn png(w: usize, h: usize, px: &[[u8; 3]]) -> Vec<u8> {
    fn crc(data: &[u8]) -> u32 {
        let mut c = !0u32;
        for &b in data {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc(&body).to_be_bytes());
    }
    let mut raw = Vec::new();
    for y in 0..h {
        raw.push(0);
        for x in 0..w {
            raw.extend_from_slice(&px[y * w + x]);
        }
    }
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, b) in blocks.iter().enumerate() {
        z.push((i + 1 == blocks.len()) as u8);
        z.extend_from_slice(&(b.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(b.len() as u16)).to_le_bytes());
        z.extend_from_slice(b);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&(b << 16 | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = (w as u32).to_be_bytes().to_vec();
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}
