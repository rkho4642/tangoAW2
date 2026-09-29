//! The new units' names in the map's unit panel, in AW2's own name font.
//!
//! The panel shows a 32x16 picture per unit (`sub_0802A838`: picture
//! `0x08108264 + 256 * index`, index per unit type from `0x0849A354`, 4
//! bytes each). The new units' pictures are put together at run time from
//! the letters of AW2's own name pictures (each letter's fill is one
//! connected piece of the picture, left to right in the order of the
//! name), outlined as the font is; O and z, which no AW2 name has, are
//! tangoAW2's own shapes in the same style. Nothing of the game's pictures
//! is kept in the source.

use mgba::core::Core;
use std::collections::BTreeMap;

pub const AW2_PICTURES: u32 = 0x0810_8264;
pub const PICTURE: usize = 256;
/// AW2's name pictures, and the letters each shows (a hyphen is a letter
/// here; spaces are not in the pictures).
const AW2_NAMES: [&str; 19] = [
    "Inftry", "Mech", "MdTank", "Neo", "Tank", "Recon", "APC", "Artly", "Rckts", "A-Air", "Mssls", "Fghtr", "Bmbr",
    "BCptr", "TCptr", "BShp", "Crsr", "Lndr", "Sub",
];
const OUTLINE: u8 = 15;

type Glyph = BTreeMap<(i32, i32), u8>;

fn grid(picture: &[u8]) -> [[u8; 32]; 16] {
    let mut g = [[0u8; 32]; 16];
    for (y, row) in g.iter_mut().enumerate() {
        for (x, v) in row.iter_mut().enumerate() {
            let t = (y / 8) * 4 + x / 8;
            *v = (picture[32 * t + 4 * (y % 8) + (x % 8) / 2] >> (4 * (x & 1))) & 15;
        }
    }
    g
}

fn is_fill(v: u8) -> bool {
    v != 0 && v != OUTLINE
}

/// The fill pieces of a picture, left to right (a piece of at most 4
/// pixels over another's columns, an i's dot, joins it).
fn pieces(g: &[[u8; 32]; 16]) -> Vec<Vec<(i32, i32)>> {
    let mut seen = [[false; 32]; 16];
    let mut out: Vec<Vec<(i32, i32)>> = Vec::new();
    for x in 0..32i32 {
        for y in 0..16i32 {
            if !is_fill(g[y as usize][x as usize]) || seen[y as usize][x as usize] {
                continue;
            }
            let mut stack = vec![(x, y)];
            seen[y as usize][x as usize] = true;
            let mut px = Vec::new();
            while let Some((a, b)) = stack.pop() {
                px.push((a, b));
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (a + dx, b + dy);
                    if (0..32).contains(&nx)
                        && (0..16).contains(&ny)
                        && !seen[ny as usize][nx as usize]
                        && is_fill(g[ny as usize][nx as usize])
                    {
                        seen[ny as usize][nx as usize] = true;
                        stack.push((nx, ny));
                    }
                }
            }
            out.push(px);
        }
    }
    out.sort_by_key(|p| p.iter().map(|q| q.0).min().unwrap_or(0));
    let cols = |p: &Vec<(i32, i32)>| p.iter().map(|q| q.0).collect::<std::collections::BTreeSet<_>>();
    let mut merged: Vec<Vec<(i32, i32)>> = Vec::new();
    for p in out {
        if let Some(last) = merged.last_mut() {
            let overlap = !cols(&p).is_disjoint(&cols(last));
            if overlap && (p.len() <= 4 || last.len() <= 4) {
                last.extend(p);
                continue;
            }
        }
        merged.push(p);
    }
    merged
}

fn mask(rows: &[&str], y0: i32) -> Glyph {
    let mut g = Glyph::new();
    for (i, row) in rows.iter().enumerate() {
        for (x, c) in row.bytes().enumerate() {
            match c {
                b'.' => g.insert((x as i32, y0 + i as i32), 1),
                b'7' => g.insert((x as i32, y0 + i as i32), 7),
                _ => None,
            };
        }
    }
    g
}

/// The font's letters, from AW2's pictures, plus O and z.
fn glyphs(core: &Core) -> BTreeMap<char, Glyph> {
    let mut out = BTreeMap::new();
    for (k, name) in AW2_NAMES.iter().enumerate() {
        let mut b = [0u8; PICTURE];
        core.raw_read_range(AW2_PICTURES + (PICTURE * k) as u32, -1, &mut b);
        let g = grid(&b);
        let ps = pieces(&g);
        if ps.len() != name.chars().count() {
            continue;
        }
        for (ch, p) in name.chars().zip(ps) {
            let x0 = p.iter().map(|q| q.0).min().unwrap_or(0);
            out.entry(ch)
                .or_insert_with(|| p.iter().map(|&(x, y)| ((x - x0, y), g[y as usize][x as usize])).collect());
        }
    }
    out.insert(
        'O',
        mask(&["7...7", ".. ..", ".. ..", ".. ..", ".. ..", ".. ..", ".. ..", ".. ..", "7...7"], 3),
    );
    out.insert('z', mask(&["....", "  . ", " .  ", "...."], 8));
    out
}

/// `name` in the font, centred in a 32x16 picture (GBA tiles, 4x2), or
/// `None` if a letter is missing or it does not fit.
pub fn picture(core: &Core, name: &str) -> Option<[u8; PICTURE]> {
    let font = glyphs(core);
    let mut px: Vec<((i32, i32), u8)> = Vec::new();
    let mut x = 0;
    for ch in name.chars() {
        if ch == ' ' {
            x += 2;
            continue;
        }
        let g = font.get(&ch)?;
        let w = g.keys().map(|k| k.0).max()? + 1;
        px.extend(g.iter().map(|(&(a, b), &v)| ((x + a, b), v)));
        x += w + 1;
    }
    let width = x - 1;
    if width > 31 {
        return None;
    }
    let off = (32 - width) / 2;
    let mut g = [[0u8; 32]; 16];
    for ((a, b), v) in px {
        g[b as usize][(a + off) as usize] = v;
    }
    let filled = g;
    for y in 0..16i32 {
        for x in 0..32i32 {
            if filled[y as usize][x as usize] != 0 {
                continue;
            }
            let near = (-1..=1).any(|dy| {
                (-1..=1).any(|dx| {
                    let (nx, ny) = (x + dx, y + dy);
                    (0..32).contains(&nx) && (0..16).contains(&ny) && is_fill(filled[ny as usize][nx as usize])
                })
            });
            if near {
                g[y as usize][x as usize] = OUTLINE;
            }
        }
    }
    let mut out = [0u8; PICTURE];
    for t in 0..8 {
        let (tx, ty) = (t % 4, t / 4);
        for y in 0..8 {
            for x in 0..8 {
                out[32 * t + 4 * y + x / 2] |= g[ty * 8 + y][tx * 8 + x] << (4 * (x & 1));
            }
        }
    }
    Some(out)
}
