//! Dual Strike's narration pictures (its `rikiishi/` files: LZ77 tiles, an
//! LZ77 32x32 map, a 256-colour palette), shown on the world map's layer
//! behind its narration (the prologue, the narration after Victory or
//! Death!), as Dual Strike shows them behind its own.
//!
//! A picture is 256x192; AW2's screen shows 240x160 of it (the middle), in
//! the map layer's form (4bpp tiles, nine palettes, a tilemap), with a
//! light box drawn into it where AW2's narration text goes (BG0 rows 15..18,
//! columns 7..22). The palettes come from the picture: its cells are
//! grouped by colour (k-means on their mean colours), each group gets the 15
//! colours that draw it best (k-means on its pixels), each cell then takes
//! the palette that draws it best, a few rounds.

pub const WIDTH: usize = 240;
pub const HEIGHT: usize = 160;
const CELLS_X: usize = WIDTH / 8;
const CELLS_Y: usize = HEIGHT / 8;
const PALETTES: usize = 9;

/// The narration text's box (pixels, on the 240x160 screen): AW2 writes the
/// text in BG0 rows 15..18 from column 7, lines up to 176 pixels.
const BOX: (usize, usize, usize, usize) = (48, 114, 238, 158);

pub struct Picture {
    /// 4bpp tiles, tile 0 blank (the map screen's BG2 draws with it).
    pub tiles: Vec<u8>,
    /// 30x20 tilemap entries (palette from BG palette 6).
    pub tilemap: Vec<u16>,
    /// Nine palettes of 16 colours.
    pub palettes: Vec<u16>,
}

type Rgb = [i32; 3];

fn rgb(c: u16) -> Rgb {
    [(c & 31) as i32, ((c >> 5) & 31) as i32, ((c >> 10) & 31) as i32]
}

fn bgr(c: Rgb) -> u16 {
    (c[0].clamp(0, 31) | (c[1].clamp(0, 31) << 5) | (c[2].clamp(0, 31) << 10)) as u16
}

fn d2(a: Rgb, b: Rgb) -> i32 {
    (0..3).map(|k| (a[k] - b[k]).pow(2)).sum()
}

/// A Dual Strike picture (`rikiishi/` tiles, map and palette files, 4 or 8
/// bits a pixel) as 256x192 colours.
pub fn decode(tiles: &[u8], map: &[u8], palette: &[u8], bpp: u8) -> Option<Vec<Rgb>> {
    let tiles = crate::ds_art::lz10(tiles)?;
    let map = crate::ds_art::lz10(map)?;
    let pal: Vec<Rgb> = palette.chunks(2).map(|c| rgb(u16::from_le_bytes([c[0], c.get(1).copied().unwrap_or(0)]))).collect();
    let mut out = vec![[0; 3]; 256 * 192];
    let size = if bpp == 8 { 64 } else { 32 };
    for i in 0..(32 * 24) {
        let e = u16::from_le_bytes([*map.get(2 * i)?, *map.get(2 * i + 1)?]);
        let (k, hf, vf, pb) = ((e & 0x3FF) as usize, e & 0x400 != 0, e & 0x800 != 0, (e >> 12) as usize);
        let t = match tiles.get(size * k..size * k + size) {
            Some(t) => t,
            None => continue,
        };
        for y in 0..8 {
            for x in 0..8 {
                let (sx, sy) = (if hf { 7 - x } else { x }, if vf { 7 - y } else { y });
                let v = if bpp == 8 {
                    t[8 * sy + sx] as usize
                } else {
                    16 * pb + ((t[4 * sy + sx / 2] >> (4 * (sx & 1))) & 15) as usize
                };
                out[256 * (8 * (i / 32) + y) + 8 * (i % 32) + x] = *pal.get(v)?;
            }
        }
    }
    Some(out)
}

/// 240x160 of a 256x192 picture (from row `y0`, the columns in the
/// middle), with the narration's box.
pub fn screen(pic: &[Rgb], y0: usize) -> Vec<Rgb> {
    let x0 = (256 - WIDTH) / 2;
    let mut out = vec![[0; 3]; WIDTH * HEIGHT];
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            out[WIDTH * y + x] = pic[256 * (y + y0) + x + x0];
        }
    }
    let (bx0, by0, bx1, by1) = BOX;
    for y in by0..by1 {
        for x in bx0..bx1 {
            let edge = x == bx0 || x == bx1 - 1 || y == by0 || y == by1 - 1;
            // (corners cut off: a rounded box)
            let corner = (x == bx0 || x == bx1 - 1) && (y == by0 || y == by1 - 1);
            if corner {
                continue;
            }
            out[WIDTH * y + x] = if edge { [8, 8, 12] } else { [30, 30, 27] };
        }
    }
    out
}

/// k-means of colours (`init` the starting centres): the centres.
fn kmeans(points: &[(Rgb, u32)], init: Vec<Rgb>, rounds: usize) -> Vec<Rgb> {
    let mut cent = init;
    for _ in 0..rounds {
        let mut sum = vec![[0i64; 4]; cent.len()];
        for &(p, w) in points {
            let k = (0..cent.len()).min_by_key(|&k| d2(p, cent[k])).unwrap();
            for c in 0..3 {
                sum[k][c] += p[c] as i64 * w as i64;
            }
            sum[k][3] += w as i64;
        }
        for (k, s) in sum.iter().enumerate() {
            if s[3] > 0 {
                cent[k] = [(s[0] + s[3] / 2) / s[3], (s[1] + s[3] / 2) / s[3], (s[2] + s[3] / 2) / s[3]].map(|v| v as i32);
            }
        }
    }
    cent
}

/// Distinct colours (with counts), the starting centres spread over them
/// (every n-th by brightness).
fn spread(points: &[(Rgb, u32)], n: usize) -> Vec<Rgb> {
    let mut v: Vec<Rgb> = points.iter().map(|p| p.0).collect();
    v.sort_by_key(|c| (c[0] * 3 + c[1] * 6 + c[2], c[0], c[1]));
    if v.is_empty() {
        return vec![[0; 3]; n];
    }
    (0..n).map(|k| v[((k * v.len()) / n + (v.len() / n) / 2).min(v.len() - 1)]).collect()
}

fn histogram(px: impl Iterator<Item = Rgb>) -> Vec<(Rgb, u32)> {
    let mut h: std::collections::BTreeMap<Rgb, u32> = std::collections::BTreeMap::new();
    for p in px {
        *h.entry(p).or_default() += 1;
    }
    h.into_iter().collect()
}

/// A 240x160 picture in the map layer's form.
pub fn fit(img: &[Rgb]) -> Picture {
    let cell = |c: usize| -> Vec<Rgb> {
        let (cx, cy) = (c % CELLS_X, c / CELLS_X);
        (0..64).map(|i| img[WIDTH * (8 * cy + i / 8) + 8 * cx + i % 8]).collect()
    };
    let cells: Vec<Vec<Rgb>> = (0..CELLS_X * CELLS_Y).map(cell).collect();
    // Groups by mean colour.
    let means: Vec<(Rgb, u32)> = cells
        .iter()
        .map(|c| {
            let s = c.iter().fold([0; 3], |a, p| [a[0] + p[0], a[1] + p[1], a[2] + p[2]]);
            ([s[0] / 64, s[1] / 64, s[2] / 64], 1)
        })
        .collect();
    let gcent = kmeans(&means, spread(&means, PALETTES), 8);
    let mut group: Vec<usize> = means.iter().map(|m| (0..PALETTES).min_by_key(|&k| d2(m.0, gcent[k])).unwrap()).collect();
    let mut pals: Vec<Vec<Rgb>> = vec![vec![[0; 3]; 15]; PALETTES];
    let draw = |c: &Vec<Rgb>, pal: &Vec<Rgb>| -> i64 { c.iter().map(|&p| pal.iter().map(|&q| d2(p, q)).min().unwrap() as i64).sum() };
    for _ in 0..4 {
        for (g, pal) in pals.iter_mut().enumerate() {
            let h = histogram(cells.iter().zip(&group).filter(|(_, &k)| k == g).flat_map(|(c, _)| c.iter().copied()));
            if h.is_empty() {
                continue;
            }
            *pal = kmeans(&h, spread(&h, 15), 10);
        }
        for (c, gr) in cells.iter().zip(group.iter_mut()) {
            *gr = (0..PALETTES).min_by_key(|&k| draw(c, &pals[k])).unwrap();
        }
    }
    // Tiles (a tile and its flips one), the tilemap.
    let mut tiles = vec![0u8; 32];
    let mut index: std::collections::HashMap<[u8; 64], usize> = std::collections::HashMap::new();
    let mut tilemap = Vec::with_capacity(cells.len());
    for (c, &g) in cells.iter().zip(&group) {
        let mut px = [0u8; 64];
        for (i, &p) in c.iter().enumerate() {
            px[i] = 1 + (0..15).min_by_key(|&k| d2(p, pals[g][k])).unwrap() as u8;
        }
        let flip = |f: usize| -> [u8; 64] {
            let mut o = [0u8; 64];
            for y in 0..8 {
                for x in 0..8 {
                    let (sx, sy) = (if f & 1 != 0 { 7 - x } else { x }, if f & 2 != 0 { 7 - y } else { y });
                    o[8 * y + x] = px[8 * sy + sx];
                }
            }
            o
        };
        let (f, canon) = (0..4).map(|f| (f, flip(f))).min_by_key(|x| x.1).unwrap();
        let n = *index.entry(canon).or_insert_with(|| {
            let n = tiles.len() / 32;
            for y in 0..8 {
                for x in (0..8).step_by(2) {
                    tiles.push(canon[8 * y + x] | (canon[8 * y + x + 1] << 4));
                }
            }
            n
        });
        tilemap.push(((6 + g as u16) << 12) | ((f as u16 & 1) << 10) | ((f as u16 >> 1) << 11) | n as u16);
    }
    let mut palettes = Vec::with_capacity(16 * PALETTES);
    for pal in &pals {
        palettes.push(0);
        palettes.extend(pal.iter().map(|&c| bgr(c)));
    }
    Picture { tiles, tilemap, palettes }
}

/// The narration's pictures: (text reference, tiles, map, palette, bits a
/// pixel), as Dual Strike's story overlay pairs them (overlay 5: the
/// prologue's three, `0x0235_2790`.., and the one after Victory or Death!,
/// `0x0235_789C`).
/// The last field: the first row shown (the tanks fill their picture's top).
pub const NARRATION: [(u32, &str, &str, &str, u8, usize); 4] = [
    (0x2100_0000, "rikiishi/002", "rikiishi/003", "rikiishi/004", 4, 0),
    (0x2100_0001, "rikiishi/005", "rikiishi/006", "rikiishi/009", 4, 16),
    (0x2100_0002, "rikiishi/00d", "rikiishi/00e", "rikiishi/00f", 8, 16),
    (0x2100_0003, "rikiishi/01c", "rikiishi/01d", "rikiishi/01e", 4, 16),
];

/// The narration pictures from the pack, in [`NARRATION`]'s order.
pub fn narration_pictures() -> Vec<Option<Picture>> {
    let Some(pack) = crate::ds_pack::pack() else { return Vec::new() };
    NARRATION
        .iter()
        .map(|&(_, t, m, p, bpp, y0)| {
            let pic = decode(pack.file(t)?, pack.file(m)?, pack.file(p)?, bpp)?;
            Some(fit(&screen(&pic, y0)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flat_picture_is_one_tile() {
        let img = vec![[10, 20, 30]; WIDTH * HEIGHT];
        let p = fit(&img);
        assert_eq!(p.tilemap.len(), CELLS_X * CELLS_Y);
        assert_eq!(p.tiles.len() / 32, 2);
        assert_eq!(p.palettes.len(), 16 * PALETTES);
    }
}
