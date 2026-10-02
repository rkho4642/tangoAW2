//! The DS Campaign's world map: AW2's own campaign map screen (the
//! `WorldMap*` procs, names from the aw2bhr decompilation) run on Dual
//! Strike's Omega Land, with Dual Strike's mission points, read from the
//! player's .nds at run time.
//!
//! - **Art.** Dual Strike draws its campaign map (`ohashi/res_gmap_map1`
//!   and `_map2`: each LZ77 4bpp tiles then an LZ77 32x32 tilemap, the left
//!   and right halves of a 480x240 picture; ten palettes at `res_gmap`
//!   +0x3B08). AW2's map layer is the same shape (two 32x32 screens of a
//!   64x32 layer), with room for 768 tiles (AW2's 704 and the block of
//!   BG1's tilemap, BG1 being off on the DS map) and nine palettes (BG
//!   6..14): the least used palette's tiles are drawn with the closest other
//!   one, a tile and its flips are one, the most alike tiles of a palette
//!   are folded together (shade weighted over detail: a shade change across
//!   a tile is what shows as an 8x8 patch) until 767 are left beside a
//!   blank one, and the kept tiles are refined to draw what was folded into
//!   them best ([`fit`]). Against Dual Strike's picture: 31.4 dB, colour
//!   jumps across tile edges +1.6 over its own (0.4.1's first fit, without
//!   flips, refining or the shade weighting: 29.3 dB, +3.2).
//! - **Mission points.** Dual Strike's table of them (ARM9 `0x0215BA04`, 12
//!   bytes: map record id, x, y, data) gives each mission's place on the
//!   map; AW2's mission table (`gUnknown_08615194`, 0x30 bytes a mission:
//!   map id, marker style, stars, flag x/y, ..., the CO screen's setup)
//!   gets a DS copy ([`MISSIONS`]) with every mission on map id
//!   [`crate::ds_campaign_data::MAP_ID`] (the header of the mission under
//!   the cursor is written there, [`crate::ds_campaign`]).
//! - **Unlocking.** AW2 reveals missions after a win from a table
//!   (`gUnknown_0861500C`: per mission, the missions to reveal and the
//!   cleared missions that must precede them); the DS copy's every record
//!   points at one list written before each return to the map: the
//!   missions the win has just made available.
//! - While a DS session is on, the literal-pool words of these tables and
//!   of the art point at the DS copies, and traps leave out AW2's story:
//!   the nation panel, its scenes after missions, its bonus missions, its
//!   alternative missions, the save prompt (the DS Campaign saves its own
//!   record). AW2's map state (`gUnknown_0202FDFC`, also its campaign
//!   save's) is kept aside for the session.

use mgba::core::Core;
use std::sync::OnceLock;

/// tangoAW2's world map data in the ROM image: past the DS Campaign's.
pub const BASE: u32 = 0x08FC_0000;
const MAGIC: u32 = 0x5041_4D57; // "WMAP"
const TILES: u32 = BASE + 0x100;
const TILEMAP: u32 = BASE + 0x8000;
const PALETTE: u32 = BASE + 0x9800;
const MISSION_TABLE: u32 = BASE + 0xA000;
const REVEAL_TABLE: u32 = BASE + 0xB000;
const REVEAL_LIST: u32 = BASE + 0xB200;
const CONDITION: u32 = BASE + 0xB220;
const CONDITION_IDS: u32 = BASE + 0xB230;
const ZEROS: u32 = BASE + 0xB400;
const AW2_STATE_BACKUP: u32 = BASE + 0xB800;
const BACKUP_MARK: u32 = BASE + 0xB900;
#[cfg(test)]
const END: u32 = BASE + 0x10000;

/// AW2's world map state (camera, cursor, mission, flags per mission,
/// markers): 0xFC bytes, as its profile saves them.
pub const STATE: u32 = 0x0202_FDFC;
const STATE_SIZE: u32 = 0xFC;
const S_CAMERA_X: u32 = STATE;
const S_CAMERA_Y: u32 = STATE + 2;
const S_CURSOR_X: u32 = STATE + 4;
const S_CURSOR_Y: u32 = STATE + 6;
pub const S_MISSION: u32 = STATE + 0x0C;
const S_SNAPPED: u32 = STATE + 0x10;
pub const S_WON: u32 = STATE + 0x11;
pub const S_FLAGS: u32 = STATE + 0x12;
const S_MARKERS: u32 = STATE + 0x3C;
/// unk12 bits: shown (a marker), cleared.
pub const SHOWN: u8 = 1;
pub const CLEARED: u8 = 2;

/// The map layer's graphics: AW2's 704 tiles (0x06008000 up to BG1's
/// tilemap at 0x0600D800) and 64 more where BG1's tilemap is (BG1, AW2's
/// sea and grid, is off on the DS map: [`tick`]); tile 0 is blank. Past
/// them (0x0600E000) are the mission panel's (BG2's) tiles.
const MAX_TILES: usize = 768;
const MISSION_RECORD: u32 = 0x30;
const AW2_MISSIONS: u32 = 0x2A;
/// Dual Strike's picture (two 240x240 halves) in AW2's 512x256 map layer.
const MAP_WIDTH: i32 = 480;
const MAP_HEIGHT: i32 = 240;

/// AW2's world map camera stops at (192, 96) (its picture, 432x256): two
/// clamping helpers, each tests the x then the y against it (`cmp rN,
/// #0xC0` / `#0x60`, then `movs rN` of the limit). On the DS map they stop
/// at Dual Strike's picture's edges ([`MAP_WIDTH`], [`MAP_HEIGHT`]): the
/// value is clamped here and the game's test skipped.
const CAMERA_CLAMPS: [(u32, u32, usize, i32); 4] = [
    (0x0807_4C0A, 0x0807_4C10, 2, MAP_WIDTH - 240),
    (0x0807_4C4A, 0x0807_4C50, 2, MAP_HEIGHT - 160),
    (0x0807_4C68, 0x0807_4C6E, 0, MAP_WIDTH - 240),
    (0x0807_4C7C, 0x0807_4C82, 0, MAP_HEIGHT - 160),
];
/// AW2's cursor moves (`sub_08076CAC` across, `sub_08076D68` up and down,
/// delta in r4): the cursor's map position may not pass 415 x 239 (`cmp r1`
/// with the literal 0x19F, `cmp r1, #0xEF`), the camera 191 x 95 (`cmp r1,
/// #0xBF` / `#0x5F` before scrolling it). On the DS map: Dual Strike's
/// picture's (463 x 223, 240 x 80); the tests are worked out here and the
/// game's branch taken.
const CURSOR_X_LIMIT: u32 = 0x0807_6CD4;
const CAMERA_X_TEST: (u32, u32, u32) = (0x0807_6D02, 0x0807_6D06, 0x0807_6D1A);
const CURSOR_Y_TEST: (u32, u32, u32) = (0x0807_6D8E, 0x0807_6D92, 0x0807_6DF2);
const CAMERA_Y_TEST: (u32, u32, u32) = (0x0807_6DBA, 0x0807_6DBE, 0x0807_6DD2);
fn cursor_x_limit(core: &mut Core) {
    if crate::ds_campaign::active(core) {
        core.gba_mut().cpu_mut().set_gpr(0, MAP_WIDTH - 17);
    }
}
/// `cmp r1, #limit` then a branch when r1 is past it (`bgt` signed, `bhi`
/// unsigned): with the DS map's limit instead.
fn limit_test(core: &mut Core, (_, within, past): (u32, u32, u32), limit: i32, unsigned: bool) {
    if !crate::ds_campaign::active(core) {
        return;
    }
    let cpu = core.gba_mut().cpu_mut();
    let v = cpu.gpr(1);
    let over = if unsigned { v as u32 > limit as u32 } else { v > limit };
    cpu.set_thumb_pc(if over { past } else { within });
}

fn camera_clamp(core: &mut Core, past: u32, reg: usize, max: i32) {
    if !crate::ds_campaign::active(core) {
        return;
    }
    let cpu = core.gba_mut().cpu_mut();
    let v = cpu.gpr(reg);
    if v > max {
        cpu.set_gpr(reg, max);
    }
    cpu.set_thumb_pc(past);
}

/// AW2's literal-pool words of the world map's art and tables.
const TILES_POOLS: [u32; 1] = [0x0807_69B4];
const AW2_TILES: u32 = 0x081C_C5F0;
const TILEMAP_POOLS: [u32; 3] = [0x0807_6AD0, 0x0807_6B14, 0x0807_6BB4];
const AW2_TILEMAP: u32 = 0x081D_0BAC;
const PALETTE_POOLS: [u32; 3] = [0x0807_5C94, 0x0807_5E08, 0x0807_69BC];
const AW2_PALETTE: u32 = 0x081D_1504;
const MISSION_POOLS: [u32; 18] = [
    0x0806_1890, 0x0806_190C, 0x0807_47EC, 0x0807_4A24, 0x0807_4A98, 0x0807_5920, 0x0807_59C8, 0x0807_6C3C, 0x0807_6C60,
    0x0807_6FF4, 0x0807_72F8, 0x0807_73A0, 0x0807_7DD8, 0x0807_7ED4, 0x0807_7F18, 0x0807_7F68, 0x0807_7FD8, 0x0807_8234,
];
const AW2_MISSION_TABLE: u32 = 0x0861_5194;
const REVEAL_POOLS: [u32; 2] = [0x0807_6C14, 0x0807_8350];
const AW2_REVEAL_TABLE: u32 = 0x0861_500C;
/// Words that point at AW2's world map script from the menu
/// (`gUnknown_0861485C`): `StartWorldMapForResume`'s pools, the Continue
/// proc's wait for it, the main loop's way back to it.
const MAP_SCRIPT_POOLS: [u32; 5] = [0x0807_814C, 0x0807_817C, 0x0807_81E4, 0x0849_EB90, 0x0861_4750];
/// The mission panel's rank (AW2's results by mission, `gUnknown_0200C2D0`;
/// in a session the DS Campaign's own, crate::ds_campaign::RECORDS).
const RANK_POOLS: [u32; 1] = [0x0807_758C];
const AW2_RANKS: u32 = 0x0200_C2D0;

/// Dual Strike's mission points (ARM9).
const DS_POINTS: u32 = 0x0215_BA04;

pub struct WorldMap {
    tiles: Vec<u8>,
    tilemap: Vec<u8>,
    palette: Vec<u8>,
    /// Per DS mission (record index 0..27): x, y, Dual Strike's flags
    /// (0x1 a lab mission, 0x2 the last).
    pub points: Vec<(i16, i16, u16)>,
}

static BUILT: OnceLock<Option<WorldMap>> = OnceLock::new();

pub fn world_map() -> Option<&'static WorldMap> {
    BUILT.get_or_init(build).as_ref()
}

fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?))
}

/// The byte length of an LZ77 stream at `start` (to find what follows it).
fn lz_len(b: &[u8], start: usize) -> Option<usize> {
    let size = u32::from_le_bytes([*b.get(start + 1)?, *b.get(start + 2)?, *b.get(start + 3)?, 0]) as usize;
    let (mut out, mut p) = (0, start + 4);
    while out < size {
        let flags = *b.get(p)?;
        p += 1;
        for bit in 0..8 {
            if out >= size {
                break;
            }
            if flags & (0x80 >> bit) != 0 {
                out += (*b.get(p)? as usize >> 4) + 3;
                p += 2;
            } else {
                out += 1;
                p += 1;
            }
        }
    }
    Some(p - start)
}

/// One half of the map: (4bpp tiles, 32x32 tilemap entries).
fn half(file: &[u8]) -> Option<(Vec<u8>, Vec<u16>)> {
    let tiles = crate::ds_art::lz10(file)?;
    let at = (lz_len(file, 0)? + 3) & !3;
    let map = crate::ds_art::lz10(file.get(at..)?)?;
    let ents = (0..1024).map(|i| u16_at(&map, 2 * i)).collect::<Option<Vec<_>>>()?;
    Some((tiles, ents))
}

type Pixels = [u8; 64];

fn rgb(c: u16) -> [i32; 3] {
    [(c & 31) as i32, ((c >> 5) & 31) as i32, ((c >> 10) & 31) as i32]
}

fn build() -> Option<WorldMap> {
    let pack = crate::ds_pack::pack()?;
    let res = pack.file("ohashi/res_gmap")?;
    let colours: Vec<u16> = (0..160).map(|i| u16_at(res, 0x3B08 + 2 * i)).collect::<Option<_>>()?;
    let mut cells: Vec<(Pixels, u8)> = Vec::with_capacity(2048);
    for name in ["ohashi/res_gmap_map1", "ohashi/res_gmap_map2"] {
        let (tiles, ents) = half(pack.file(name)?)?;
        for e in ents {
            let (k, hf, vf, pal) = ((e & 0x3FF) as usize, e & 0x400 != 0, e & 0x800 != 0, (e >> 12) as u8);
            let mut px = [0u8; 64];
            for y in 0..8 {
                for x in 0..8 {
                    let (sx, sy) = (if hf { 7 - x } else { x }, if vf { 7 - y } else { y });
                    let b = *tiles.get(32 * k + 4 * sy + sx / 2)?;
                    px[8 * y + x] = (b >> (4 * (sx & 1))) & 15;
                }
            }
            cells.push((px, pal.min(9)));
        }
    }
    let (tiles, tilemap, palette) = fit(&cells, &colours, MAX_TILES - 1, 6);
    // Mission points.
    let mut points = Vec::new();
    for i in 0..crate::ds_campaign_data::MISSIONS as u32 {
        let id = crate::ds_campaign_data::FIRST_RECORD + i;
        let mut found = None;
        for k in 0..40 {
            let r = pack.arm9_at(DS_POINTS + 12 * k, 12)?;
            let rid = u32::from_le_bytes(r[0..4].try_into().ok()?);
            if rid == 0 {
                break;
            }
            if rid & 0xFFFF == id {
                found = Some((i16::from_le_bytes([r[4], r[5]]), i16::from_le_bytes([r[6], r[7]]), (rid >> 16) as u16));
            }
        }
        points.push(found?);
    }
    Some(WorldMap { tiles, tilemap, palette, points })
}

/// How much more a tile's shade (its four 4x4 means) counts than its detail
/// when tiles are folded together: a shade change across a whole tile is
/// what shows as an 8x8 patch (the seas' gradients), a detail change much
/// less.
const SHADE_WEIGHT: f64 = 4.0;
/// Rounds of refining the kept tiles after folding.
const REFINE: usize = 4;

/// A tile flipped: 0 as is, 1 left-right, 2 top-bottom, 3 both.
fn flipped(px: &Pixels, f: usize) -> Pixels {
    let mut out = [0u8; 64];
    for y in 0..8 {
        for x in 0..8 {
            let (sx, sy) = (if f & 1 != 0 { 7 - x } else { x }, if f & 2 != 0 { 7 - y } else { y });
            out[8 * y + x] = px[8 * sy + sx];
        }
    }
    out
}

/// A tile as the folding compares it: its colours' four 4x4 means
/// (weighted) and what is left of each pixel.
fn feature(rgb: &[[i32; 3]; 64]) -> Vec<f64> {
    let mut means = [[0f64; 3]; 4];
    for y in 0..8 {
        for x in 0..8 {
            for k in 0..3 {
                means[(y / 4) * 2 + x / 4][k] += rgb[8 * y + x][k] as f64 / 16.0;
            }
        }
    }
    let w = SHADE_WEIGHT.sqrt();
    let mut out = Vec::with_capacity(64 * 6);
    for y in 0..8 {
        for x in 0..8 {
            let m = means[(y / 4) * 2 + x / 4];
            for k in 0..3 {
                out.push(w * m[k]);
                out.push(rgb[8 * y + x][k] as f64 - m[k]);
            }
        }
    }
    out
}

/// Dual Strike's 2048 map cells (left screen then right, each 32x32, as
/// pixels and a palette 0..9) as AW2's layer: at most `max_tiles` 4bpp
/// tiles, nine palettes from BG palette `first_pal`, a 64x32 tilemap in
/// two screens (with flips). Returns (tiles, tilemap bytes, palette bytes).
///
/// Dual Strike's least used palette goes (its cells take the palette that
/// draws them best); tiles that are the same but for a flip are one; then,
/// while there are too many, the tile whose nearest one of its palette
/// costs least to stand in for it (the distance, shade weighted, times the
/// cells that use it) is folded into that one.
pub fn fit(cells: &[(Pixels, u8)], colours: &[u16], max_tiles: usize, first_pal: u16) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let col = |p: u8, i: u8| rgb(colours[16 * p as usize + i as usize]);
    let mut use_ = [0usize; 10];
    for c in cells {
        use_[c.1 as usize] += 1;
    }
    let drop = (0..10).min_by_key(|&p| use_[p]).unwrap() as u8;
    let kept: Vec<u8> = (0..10).filter(|&p| p != drop).collect();
    let near = |a: [i32; 3], b: [i32; 3]| (0..3).map(|k| (a[k] - b[k]).pow(2)).sum::<i32>();
    // Dual Strike draws colour 0 of these palettes (AW2 shows the backdrop
    // through it): such pixels take the palette's closest other colour.
    let cells: Vec<(Pixels, u8)> = cells
        .iter()
        .map(|&(mut px, p)| {
            let zero = col(p, 0);
            let sub = (1..16u8).min_by_key(|&j| near(zero, col(p, j))).unwrap();
            for v in px.iter_mut() {
                if *v == 0 {
                    *v = sub;
                }
            }
            (px, p)
        })
        .collect();
    let cells: Vec<(Pixels, u8)> = cells
        .iter()
        .map(|&(px, p)| {
            if p != drop {
                return (px, p);
            }
            let mut best: Option<(i32, u8, Pixels)> = None;
            for &q in &kept {
                let mut out = [0u8; 64];
                let mut err = 0;
                for (i, &v) in px.iter().enumerate() {
                    let c = col(p, v);
                    let (j, e) = (1..16u8).map(|j| (j, near(c, col(q, j)))).min_by_key(|x| x.1).unwrap();
                    out[i] = j;
                    err += e;
                }
                if best.as_ref().map_or(true, |b| err < b.0) {
                    best = Some((err, q, out));
                }
            }
            let b = best.unwrap();
            (b.2, b.1)
        })
        .collect();
    // Unique tiles (a tile and its flips are one: the cell keeps its flip)
    // and how many cells use each.
    let mut uniq: Vec<(Pixels, u8)> = Vec::new();
    let mut weight: Vec<f64> = Vec::new();
    let mut of_cell = Vec::with_capacity(cells.len());
    {
        let mut index = std::collections::HashMap::new();
        for &(px, p) in &cells {
            let (f, canon) = (0..4).map(|f| (f, flipped(&px, f))).min_by_key(|x| x.1).unwrap();
            let id = *index.entry((canon, p)).or_insert_with(|| {
                uniq.push((canon, p));
                weight.push(0.0);
                uniq.len() - 1
            });
            weight[id] += 1.0;
            of_cell.push((id, f));
        }
    }
    let n = uniq.len();
    let feat: Vec<Vec<f64>> = uniq
        .iter()
        .map(|(px, p)| {
            let mut c = [[0i32; 3]; 64];
            for (i, &v) in px.iter().enumerate() {
                c[i] = col(*p, v);
            }
            feature(&c)
        })
        .collect();
    let dist = |a: usize, b: usize| -> f64 {
        if uniq[a].1 != uniq[b].1 {
            return f64::INFINITY;
        }
        feat[a].iter().zip(&feat[b]).map(|(x, y)| (x - y) * (x - y)).sum()
    };
    let mut alive = vec![true; n];
    let mut rep: Vec<usize> = (0..n).collect();
    let nearest = |i: usize, alive: &[bool]| -> (usize, f64) {
        let mut best = (usize::MAX, f64::INFINITY);
        for j in 0..n {
            if j != i && alive[j] {
                let d = dist(i, j);
                if d < best.1 {
                    best = (j, d);
                }
            }
        }
        best
    };
    let mut nn: Vec<(usize, f64)> = (0..n).map(|i| nearest(i, &alive)).collect();
    let mut count = n;
    while count > max_tiles {
        let i = (0..n)
            .filter(|&i| alive[i] && nn[i].0 != usize::MAX)
            .min_by(|&a, &b| (nn[a].1 * weight[a]).partial_cmp(&(nn[b].1 * weight[b])).unwrap())
            .unwrap();
        let j = nn[i].0;
        alive[i] = false;
        rep[i] = j;
        weight[j] += weight[i];
        count -= 1;
        for k in 0..n {
            if alive[k] && nn[k].0 == i {
                nn[k] = nearest(k, &alive);
            }
        }
    }
    let root = |mut i: usize| {
        while rep[i] != i {
            i = rep[i];
        }
        i
    };
    // Then, a few times: each kept tile becomes the pattern (in its
    // palette) that draws the tiles folded into it best, pixel by pixel
    // (each weighted by its cells), and each tile goes to the kept tile of
    // its palette nearest to it.
    let mut assign: Vec<usize> = (0..n).map(root).collect();
    let reps: Vec<usize> = (0..n).filter(|&i| alive[i]).collect();
    let mut pats: Vec<Pixels> = uniq.iter().map(|u| u.0).collect();
    let cells_of: Vec<f64> = {
        let mut w = vec![0f64; n];
        for &(id, _) in &of_cell {
            w[id] += 1.0;
        }
        w
    };
    for _ in 0..REFINE {
        let mut members: Vec<Vec<usize>> = vec![Vec::new(); n];
        for u in 0..n {
            members[assign[u]].push(u);
        }
        for &r in &reps {
            if members[r].is_empty() {
                continue;
            }
            let p = uniq[r].1;
            for i in 0..64 {
                let best = (1..16u8)
                    .min_by_key(|&k| {
                        let c = col(p, k);
                        members[r].iter().map(|&u| near(c, col(p, uniq[u].0[i])) as i64 * cells_of[u] as i64).sum::<i64>()
                    })
                    .unwrap();
                pats[r][i] = best;
            }
        }
        let rf: Vec<(usize, Vec<f64>)> = reps
            .iter()
            .map(|&r| {
                let mut c = [[0i32; 3]; 64];
                for (i, &v) in pats[r].iter().enumerate() {
                    c[i] = col(uniq[r].1, v);
                }
                (r, feature(&c))
            })
            .collect();
        for u in 0..n {
            let mut best = (assign[u], f64::INFINITY);
            for (r, f) in &rf {
                if uniq[*r].1 != uniq[u].1 {
                    continue;
                }
                let d: f64 = f.iter().zip(&feat[u]).map(|(x, y)| (x - y) * (x - y)).sum();
                if d < best.1 {
                    best = (*r, d);
                }
            }
            assign[u] = best.0;
        }
    }
    // Number the kept tiles, write them out. Tile 0 stays blank: the map
    // screen's other layers on these graphics (BG2) are drawn with it.
    let mut number = vec![usize::MAX; n];
    let mut tiles = vec![0u8; 32];
    for &i in &reps {
        number[i] = tiles.len() / 32;
        let px = &pats[i];
        for y in 0..8 {
            for x in (0..8).step_by(2) {
                tiles.push(px[8 * y + x] | (px[8 * y + x + 1] << 4));
            }
        }
    }
    let pal_of = |p: u8| first_pal + kept.iter().position(|&k| k == p).unwrap() as u16;
    let mut tilemap = Vec::with_capacity(4096);
    for &(id, f) in &of_cell {
        let r = assign[id];
        // The cell is its tile's canonical form flipped back (flips undo
        // themselves): 0x400 left-right, 0x800 top-bottom.
        let e = (pal_of(uniq[r].1) << 12) | ((f as u16 & 1) << 10) | ((f as u16 >> 1) << 11) | number[r] as u16;
        tilemap.extend_from_slice(&e.to_le_bytes());
    }
    let mut palette = Vec::with_capacity(0x120);
    for &p in &kept {
        for i in 0..16 {
            palette.extend_from_slice(&colours[16 * p as usize + i].to_le_bytes());
        }
    }
    (tiles, tilemap, palette)
}

fn installed(core: &Core) -> bool {
    core.raw_read_32(BASE, -1) == MAGIC
}

/// Writes the world map's art and tables into the ROM image (once).
pub fn install(core: &mut Core, cos_pick: &[bool], co_setup: u32, info_texts: &[u16], after_win: &[(usize, u32)]) -> bool {
    if installed(core) {
        return true;
    }
    let Some(w) = world_map() else { return false };
    let tiles = crate::lz77::compress(&w.tiles);
    let map = crate::lz77::compress(&w.tilemap);
    assert!(TILES + tiles.len() as u32 <= TILEMAP && TILEMAP + map.len() as u32 <= PALETTE);
    core.raw_write_range(TILES, -1, &tiles);
    core.raw_write_range(TILEMAP, -1, &map);
    core.raw_write_range(PALETTE, -1, &w.palette);
    // The missions.
    let mut table = vec![0u8; (MISSION_RECORD * AW2_MISSIONS) as usize];
    for (i, &(x, y, flags)) in w.points.iter().enumerate() {
        let r = &mut table[i * MISSION_RECORD as usize..(i + 1) * MISSION_RECORD as usize];
        r[0..2].copy_from_slice(&(crate::ds_campaign_data::MAP_ID as u16).to_le_bytes());
        // Marker style: Dual Strike's lab missions and its last stand out.
        r[2] = if flags & 2 != 0 { 8 } else if flags & 1 != 0 { 4 } else { 0 };
        // The stars beside LEVEL (Normal, Hard): [`stars`].
        let (normal, hard) = stars(i);
        r[3] = normal;
        r[4] = hard;
        r[6..8].copy_from_slice(&x.to_le_bytes());
        r[8..10].copy_from_slice(&y.to_le_bytes());
        // The mission panel's text (its +0x10 is a text id).
        r[0x10..0x12].copy_from_slice(&info_texts.get(i).copied().unwrap_or(0).to_le_bytes());
        if cos_pick.get(i).copied().unwrap_or(false) {
            r[0x20..0x24].copy_from_slice(&co_setup.to_le_bytes());
        }
        // The story Dual Strike plays after this mission's win (AW2 plays
        // +0x18 on the map after a won mission, `StartWorldMapAfterMissionScript`).
        if let Some(&(_, s)) = after_win.iter().find(|&&(m, _)| m == i) {
            r[0x18..0x1C].copy_from_slice(&s.to_le_bytes());
        }
    }
    core.raw_write_range(MISSION_TABLE, -1, &table);
    // Reveals: every record (mission + 0..3) points at the one list and
    // condition the end of a mission writes.
    let mut reveal = Vec::new();
    for _ in 0..AW2_MISSIONS + 4 {
        reveal.extend_from_slice(&REVEAL_LIST.to_le_bytes());
        reveal.extend_from_slice(&CONDITION.to_le_bytes());
    }
    core.raw_write_range(REVEAL_TABLE, -1, &reveal);
    core.raw_write_range(REVEAL_LIST, -1, &[0xFF; 16]);
    core.raw_write_range(CONDITION, -1, &[0; 16]);
    core.raw_write_range(ZEROS, -1, &[0; 0x400]);
    core.raw_write_32(BASE, -1, MAGIC);
    true
}

/// A mission's difficulty stars beside LEVEL (AW2's mission table +3
/// Normal, +4 Hard). Dual Strike has none (no field in its mission records
/// or map points, and its map shows none), so they follow the mission's
/// place in the campaign, over AW2's own ranges (its campaign: Normal 1..7,
/// Hard 1..10): Normal one more every four missions (1..7), Hard one to
/// three more than Normal, further in (up to 10).
pub fn stars(mission: usize) -> (u8, u8) {
    let order = &crate::ds_campaign_data::ORDER;
    let step = order.iter().position(|&m| m as usize == mission).unwrap_or(0);
    let n = order.len().max(1);
    let normal = 1 + (step * 7 / n) as u8;
    let hard = (normal + 1 + (step * 3 / n) as u8).min(10);
    (normal, hard)
}

fn set32(core: &mut Core, at: u32, v: u32) {
    if core.raw_read_32(at, -1) != v {
        core.raw_write_32(at, -1, v);
    }
}

/// Every frame: the pool words point at the DS copies during a session,
/// at AW2's otherwise (and AW2's map state comes back after a session).
pub fn tick(core: &mut Core, session: bool, aw2_map_script: u32, ds_map_script: u32) {
    if !installed(core) {
        return;
    }
    let pick = |ds: u32, aw2: u32| if session { ds } else { aw2 };
    for at in TILES_POOLS {
        set32(core, at, pick(TILES, AW2_TILES));
    }
    for at in TILEMAP_POOLS {
        set32(core, at, pick(TILEMAP, AW2_TILEMAP));
    }
    for at in PALETTE_POOLS {
        set32(core, at, pick(PALETTE, AW2_PALETTE));
    }
    for at in MISSION_POOLS {
        set32(core, at, pick(MISSION_TABLE, AW2_MISSION_TABLE));
    }
    for at in REVEAL_POOLS {
        set32(core, at, pick(REVEAL_TABLE, AW2_REVEAL_TABLE));
    }
    for at in RANK_POOLS {
        set32(core, at, pick(crate::ds_campaign::RECORDS, AW2_RANKS));
    }
    // The world map from the menu: the session's copy of its script
    // (crate::ds_campaign: the prologue before the first mission).
    for at in MAP_SCRIPT_POOLS {
        set32(core, at, pick(ds_map_script, aw2_map_script));
    }
    if !session && core.raw_read_8(BACKUP_MARK, -1) == 1 {
        restore_aw2_state(core);
    }
    // AW2's sea and grid layer (BG1, blended over the map) is AW2's art:
    // on the DS map it is left off (the map layer, BG3, is its own sea).
    let on_map = session && map_screen(core);
    let d = core.raw_read_16(DISP_CT, -1);
    if on_map {
        if d & BG1_ON != 0 {
            core.raw_write_16(DISP_CT, -1, d & !BG1_ON);
            core.raw_write_8(BG1_HIDDEN, -1, 1);
        }
        // The map's tiles past AW2's 704 lie where BG1's tilemap is, which
        // the screen's setup writes too: they are put back.
        if let Some(w) = world_map() {
            let ours = &w.tiles[(704 * 32).min(w.tiles.len())..];
            if !ours.is_empty() {
                let mut now = vec![0u8; ours.len()];
                core.raw_read_range(BG1_TILEMAP, -1, &mut now);
                if now != ours {
                    core.raw_write_range(BG1_TILEMAP, -1, ours);
                }
            }
        }
    } else if core.raw_read_8(BG1_HIDDEN, -1) == 1 {
        // Off the map, BG1 is the next screen's (the mission card's): on.
        core.raw_write_16(DISP_CT, -1, d | BG1_ON);
        core.raw_write_8(BG1_HIDDEN, -1, 0);
    }
}

/// AW2's flag with Orange Star's star (OBJ tiles 40..43 of the map
/// screen's own sprites, palette 0), 16x16, drawn where AW2 draws a
/// mission's flag.
const CLEARED_FLAG_TILE: u16 = 40;
const FLAG_DX: i32 = -5;
const FLAG_DY: i32 = -5;

/// At the sprite flush (`crate::branding::flush`): on the DS map, a won
/// mission's point keeps a flag. AW2 marks a won mission by painting its
/// part of the continent in Orange Star's colours; Omega Land's picture has
/// no such parts, so the point keeps AW2's starred flag instead. The flags
/// go after the game's sprites: the cursor and the open missions' flags
/// draw over them. Returns the end of the sprite list.
pub fn flush_sprites(core: &mut Core, mut at: u32, end: u32) -> u32 {
    if !installed(core) || !crate::ds_campaign::active(core) || !map_screen(core) {
        return at;
    }
    let Some(w) = world_map() else { return at };
    let (cam_x, cam_y) = (core.raw_read_16(S_CAMERA_X, -1) as i16 as i32, core.raw_read_16(S_CAMERA_Y, -1) as i16 as i32);
    for (m, &(px, py, _)) in w.points.iter().enumerate() {
        if core.raw_read_8(S_FLAGS + m as u32, -1) & CLEARED == 0 || at + 8 > end {
            continue;
        }
        let (x, y) = (px as i32 - cam_x + FLAG_DX, py as i32 - cam_y + FLAG_DY);
        if !(-16..240).contains(&x) || !(-16..160).contains(&y) {
            continue;
        }
        core.raw_write_16(at, -1, (y as u16) & 0xFF);
        core.raw_write_16(at + 2, -1, ((x as u16) & 0x1FF) | (1 << 14));
        core.raw_write_16(at + 4, -1, CLEARED_FLAG_TILE | (3 << 10));
        core.raw_write_16(at + 6, -1, 0);
        at += 8;
    }
    at
}

/// The map screen is up (its layers as it sets them).
fn map_screen(core: &Core) -> bool {
    core.raw_read_16(BG3CNT, -1) == WORLD_MAP_BG3 && core.raw_read_16(BG1CNT, -1) == WORLD_MAP_BG1
}

/// gDispIo's DISPCNT shadow, BG1's bit, and the world map's BG3 control
/// (char block 2, screen 30, 512x256): the map screen is up.
const DISP_CT: u32 = 0x0300_30CC;
const BG1_ON: u16 = 1 << 9;
const BG3CNT: u32 = 0x0400_000E;
const WORLD_MAP_BG3: u16 = 0x5E0B;
/// BG1 as the map screen sets it (its sea and grid: screen 27).
const BG1CNT: u32 = 0x0400_000A;
/// 1 while BG1 is held off ([`crate::ds_campaign`]'s RAM block).
const BG1_HIDDEN: u32 = 0x0203_FD16;
const WORLD_MAP_BG1: u16 = 0x1B02;
/// BG1's tilemap on the map screen (screen 27), tiles 704.. of BG3's.
const BG1_TILEMAP: u32 = 0x0600_D800;

/// BG3's scroll shadows (`SetBgScrollShadow(3, ..)`), the palette buffer
/// the game copies to palette RAM, OBJ's bit in the DISPCNT shadow.
const BG3_SCROLL_X: u32 = 0x0300_200C;
const BG3_SCROLL_Y: u32 = 0x0300_2000;
const PAL_BUFFER: u32 = 0x0300_20C0;
const OBJ_ON: u16 = 1 << 12;
const BG3_CHARS: u32 = 0x0600_8000;
const BG3_MAP: u32 = 0x0600_F000;

fn write_palettes(core: &mut Core, at: u32, bytes: &[u8]) {
    for base in [PAL_BUFFER, 0x0500_0000] {
        core.raw_write_range(base + at, -1, bytes);
    }
}

/// A narration picture on the map's layer (its tiles, its 30x20 tilemap
/// in the top left of the first screen, its palettes 6..14), unscrolled,
/// the map's sprites (flags, cursor) off.
pub fn show_picture(core: &mut Core, p: &crate::ds_story_art::Picture) {
    core.raw_write_range(BG3_CHARS, -1, &p.tiles);
    let mut map = vec![0u8; 0x1000];
    for (i, &e) in p.tilemap.iter().enumerate() {
        let (x, y) = (i % 30, i / 30);
        map[2 * (32 * y + x)..2 * (32 * y + x) + 2].copy_from_slice(&e.to_le_bytes());
    }
    core.raw_write_range(BG3_MAP, -1, &map);
    let pal: Vec<u8> = p.palettes.iter().flat_map(|c| c.to_le_bytes()).collect();
    write_palettes(core, 0xC0, &pal);
    core.raw_write_16(BG3_SCROLL_X, -1, 0);
    core.raw_write_16(BG3_SCROLL_Y, -1, 0);
    let d = core.raw_read_16(DISP_CT, -1);
    core.raw_write_16(DISP_CT, -1, d & !OBJ_ON);
}

/// The map back on its layer after a picture (tiles, tilemap, palettes,
/// the camera's scroll, the sprites).
pub fn restore_map(core: &mut Core) {
    let Some(w) = world_map() else { return };
    core.raw_write_range(BG3_CHARS, -1, &w.tiles);
    core.raw_write_range(BG3_MAP, -1, &w.tilemap);
    write_palettes(core, 0xC0, &w.palette);
    let (x, y) = (core.raw_read_16(S_CAMERA_X, -1), core.raw_read_16(S_CAMERA_Y, -1));
    core.raw_write_16(BG3_SCROLL_X, -1, x);
    core.raw_write_16(BG3_SCROLL_Y, -1, y);
    let d = core.raw_read_16(DISP_CT, -1);
    core.raw_write_16(DISP_CT, -1, d | OBJ_ON);
}

/// AW2's map state is put aside when a DS session starts.
pub fn backup_aw2_state(core: &mut Core) {
    if core.raw_read_8(BACKUP_MARK, -1) == 1 {
        return;
    }
    let mut b = vec![0u8; STATE_SIZE as usize];
    core.raw_read_range(STATE, -1, &mut b);
    core.raw_write_range(AW2_STATE_BACKUP, -1, &b);
    core.raw_write_8(BACKUP_MARK, -1, 1);
}

fn restore_aw2_state(core: &mut Core) {
    let mut b = vec![0u8; STATE_SIZE as usize];
    core.raw_read_range(AW2_STATE_BACKUP, -1, &mut b);
    core.raw_write_range(STATE, -1, &b);
    core.raw_write_8(BACKUP_MARK, -1, 0);
}

/// The DS session's map state: `available` missions shown (with markers),
/// `won` ones cleared, the cursor on `focus`.
pub fn write_state(core: &mut Core, available: &[u8], won: &[u8], focus: u8) {
    let Some(w) = world_map() else { return };
    core.raw_write_range(STATE, -1, &[0u8; STATE_SIZE as usize]);
    for &m in won {
        let v = core.raw_read_8(S_FLAGS + m as u32, -1);
        core.raw_write_8(S_FLAGS + m as u32, -1, v | CLEARED);
    }
    for &m in available {
        let v = core.raw_read_8(S_FLAGS + m as u32, -1);
        core.raw_write_8(S_FLAGS + m as u32, -1, v | SHOWN);
    }
    // The marker list starts empty (the map rebuilds it from the flags).
    core.raw_write_16(S_MARKERS, -1, 0xFFFF);
    let _ = w;
    point_at(core, focus);
    core.raw_write_32(S_MISSION, -1, focus as u32);
    core.raw_write_8(S_SNAPPED, -1, 0);
}

/// The camera and cursor on mission `m`'s point.
pub fn point_at(core: &mut Core, m: u8) {
    let Some(w) = world_map() else { return };
    let (x, y, _) = w.points.get(m as usize).copied().unwrap_or((240, 120, 0));
    let cam_x = (x as i32 - 120).clamp(0, MAP_WIDTH - 240);
    let cam_y = (y as i32 - 80).clamp(0, MAP_HEIGHT - 160);
    core.raw_write_16(S_CAMERA_X, -1, cam_x as u16);
    core.raw_write_16(S_CAMERA_Y, -1, cam_y as u16);
    core.raw_write_16(S_CURSOR_X, -1, (x as i32 - cam_x) as u16);
    core.raw_write_16(S_CURSOR_Y, -1, (y as i32 - cam_y) as u16);
}

/// Before the map returns after mission `mission`: the missions to reveal
/// (`newly`) and the condition (the mission itself cleared).
pub fn write_reveal(core: &mut Core, mission: u8, newly: &[u8]) {
    let mut list = [0xFFu8; 16];
    for (k, &m) in newly.iter().take(15).enumerate() {
        list[k] = m;
    }
    core.raw_write_range(REVEAL_LIST, -1, &list);
    core.raw_write_range(CONDITION_IDS, -1, &[mission, 0xFF]);
    core.raw_write_32(CONDITION, -1, CONDITION_IDS);
    core.raw_write_8(CONDITION + 4, -1, 1);
    core.raw_write_8(CONDITION + 5, -1, 0xFF);
}

// --- Traps -----------------------------------------------------------------------

fn ret(core: &mut Core) {
    let cpu = core.gba_mut().cpu_mut();
    let lr = cpu.gpr(14) as u32;
    cpu.set_thumb_pc(lr & !1);
}

/// `WorldMapReturn_Init`'s switch on AW2's story missions (7, 0xF, 0x17,
/// 0x29): a DS mission takes the plain path (label 2).
const RETURN_SWITCH: u32 = 0x0807_827E;
const RETURN_PLAIN: u32 = 0x0807_82B0;
/// `WorldMapReturn_RevealBonusMissions`, `sub_08076B20` (AW2's alternative
/// missions), `StartWorldMapNationPanel`: AW2's story, left out.
const BONUS: u32 = 0x0807_8358;
const ALTERNATIVES: u32 = 0x0807_6B20;
const NATION_PANEL: u32 = 0x0807_639C;
/// `SaveScreenCampaign_StartMessage`: AW2's "save?" after a mission.
pub const SAVE_PROMPT: u32 = 0x0803_D92C;

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    let mut t: Vec<(u32, Box<dyn Fn(&mut Core)>)> = vec![
        (
            RETURN_SWITCH,
            Box::new(|core: &mut Core| {
                if crate::ds_campaign::active(core) {
                    core.gba_mut().cpu_mut().set_thumb_pc(RETURN_PLAIN);
                }
            }),
        ),
        (BONUS, Box::new(|core: &mut Core| if crate::ds_campaign::active(core) { ret(core) })),
        (ALTERNATIVES, Box::new(|core: &mut Core| if crate::ds_campaign::active(core) { ret(core) })),
        (NATION_PANEL, Box::new(|core: &mut Core| if crate::ds_campaign::active(core) { ret(core) })),
        (PROFILE_SERIALIZED, Box::new(profile_serialized)),
    ];
    for (at, past, reg, max) in CAMERA_CLAMPS {
        t.push((at, Box::new(move |core: &mut Core| camera_clamp(core, past, reg, max))));
    }
    t.push((CURSOR_X_LIMIT, Box::new(cursor_x_limit)));
    t.push((CAMERA_X_TEST.0, Box::new(|core: &mut Core| limit_test(core, CAMERA_X_TEST, MAP_WIDTH - 240, false))));
    t.push((CURSOR_Y_TEST.0, Box::new(|core: &mut Core| limit_test(core, CURSOR_Y_TEST, MAP_HEIGHT - 17, true))));
    t.push((CAMERA_Y_TEST.0, Box::new(|core: &mut Core| limit_test(core, CAMERA_Y_TEST, MAP_HEIGHT - 160, false))));
    t
}

/// AW2's profile serializer (`0x08016B2C`, RAM to the save buffer), at its
/// end (r6 the buffer). Its last part is AW2's world map state (its
/// campaign: the Continue), which holds the DS map's while AW2's is in the
/// backup. AW2's save writer serializes the profile with every slot it
/// writes (the profile's header lists the other slots' sectors), the DS
/// Campaign's own record too: the buffer gets AW2's state from the backup,
/// so the profile in Flash stays AW2's.
const PROFILE_SERIALIZED: u32 = 0x0801_6BA0;
const PROFILE_STATE_AT: u32 = 0x4D0;
fn profile_serialized(core: &mut Core) {
    if core.raw_read_8(BACKUP_MARK, -1) != 1 {
        return;
    }
    let buffer = core.gba().cpu().gpr(6) as u32;
    if !(0x0200_0000..0x0204_0000).contains(&buffer) {
        return;
    }
    let mut b = vec![0u8; STATE_SIZE as usize];
    core.raw_read_range(AW2_STATE_BACKUP, -1, &mut b);
    core.raw_write_range(buffer + PROFILE_STATE_AT, -1, &b);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stars_rise_with_the_campaign() {
        assert_eq!(stars(0), (1, 2));
        assert_eq!(stars(24), (7, 10));
        let order = crate::ds_campaign_data::ORDER;
        for w in order.windows(2) {
            assert!(stars(w[0] as usize).0 <= stars(w[1] as usize).0);
        }
    }

    #[test]
    fn layout_fits() {
        assert!(BACKUP_MARK < END);
        assert!(AW2_STATE_BACKUP + STATE_SIZE <= BACKUP_MARK);
        assert!(REVEAL_TABLE + 8 * (AW2_MISSIONS + 4) <= REVEAL_LIST);
        assert!(MISSION_TABLE + MISSION_RECORD * AW2_MISSIONS <= REVEAL_TABLE);
        assert!(END <= 0x0900_0000);
    }

    #[test]
    fn fit_keeps_a_small_map_whole() {
        // Two palettes, three distinct tiles: nothing to fold.
        let colours: Vec<u16> = (0..160).map(|i| i as u16 * 7).collect();
        let mut cells = Vec::new();
        for k in 0..2048 {
            let mut px = [0u8; 64];
            px[0] = (k % 3) as u8;
            cells.push((px, (k % 2) as u8));
        }
        let (tiles, map, pal) = fit(&cells, &colours, 896, 6);
        assert_eq!(tiles.len() / 32, 7);
        assert_eq!(map.len(), 4096);
        assert_eq!(pal.len(), 0x120);
    }
}
