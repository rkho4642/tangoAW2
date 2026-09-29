//! Preview of the Dual Strike unit conversion ([`tango_gamesupport_aw2::ds_unit_art`]):
//! five PNGs for approval, drawn with AW2's own palettes next to AW2's own units.
//!
//! - `ds_units_map.png`: AW2's Tank, Fighter and Lander, then the seven new
//!   units, idle frame 0, in each army's palette (rows 1..5) and greyed (6..10).
//! - `ds_units_idle_frames.png`: the three idle frames of each (Orange Star).
//! - `ds_units_move.png`: the moving sheets, AW2's and the new units'.
//! - `ds_validate_map.png`: units both games have, AW2's own beside the
//!   conversion of Dual Strike's, per army.
//! - `ds_validate_move.png`: the same for the moving sheets.
//!
//! Usage: TANGOAW2_DS_ROM=<Dual Strike ROM or pack> ds_unit_art_preview <aw2 rom> <out dir>

use tango_gamesupport_aw2::ds_art::lz10;
use tango_gamesupport_aw2::ds_unit_art::{map_tiles, move_sheet, NEW_UNITS};

const SCALE: usize = 4;
const BG: [u8; 3] = [88, 152, 88];
const GAP: [u8; 3] = [40, 40, 40];

struct Rom(Vec<u8>);

impl Rom {
    fn at(&self, a: u32, n: usize) -> &[u8] {
        &self.0[(a - 0x0800_0000) as usize..][..n]
    }
    fn u32(&self, a: u32) -> u32 {
        u32::from_le_bytes(self.at(a, 4).try_into().unwrap())
    }
    fn palette(&self, a: u32) -> [[u8; 3]; 16] {
        let b = self.at(a, 32);
        std::array::from_fn(|i| {
            let c = u16::from_le_bytes([b[2 * i], b[2 * i + 1]]);
            let s = |v: u16| ((v & 31) * 255 / 31) as u8;
            [s(c), s(c >> 5), s(c >> 10)]
        })
    }
    /// AW2's map tiles of a unit type (Orange Star's slot), idle frame `f`.
    fn map_tiles(&self, t: u32, f: u32) -> Vec<u8> {
        let slot = u16::from_le_bytes(self.at(0x0849_9608 + 2 * t, 2).try_into().unwrap()) as u32;
        self.at(0x0810_BE60 + 0xD80 * f + 128 * slot, 128).to_vec()
    }
    /// AW2's moving sheet of a unit type (Orange Star's).
    fn move_sheet(&self, t: u32) -> Vec<u8> {
        let p = self.u32(0x0849_CD88 + 36 * t);
        lz10(&self.0[(p - 0x0800_0000) as usize..]).unwrap()
    }
}

struct Image {
    w: usize,
    h: usize,
    px: Vec<[u8; 3]>,
}

impl Image {
    fn new(w: usize, h: usize) -> Self {
        Image {
            w,
            h,
            px: vec![GAP; w * h],
        }
    }
    /// A block of tiles `tw` x `th`, tile k at `place(k)` (in tiles), at
    /// cell position (x, y) in unscaled pixels.
    fn tiles(
        &mut self,
        x: usize,
        y: usize,
        tiles: &[u8],
        tw: usize,
        th: usize,
        place: impl Fn(usize) -> (usize, usize),
        pal: &[[u8; 3]; 16],
    ) {
        for k in 0..tw * th {
            let (cx, cy) = place(k);
            for py in 0..8 {
                for pxx in 0..8 {
                    let b = tiles[k * 32 + py * 4 + pxx / 2];
                    let v = if pxx & 1 == 1 { b >> 4 } else { b & 15 } as usize;
                    let c = if v == 0 { BG } else { pal[v] };
                    let (sx, sy) = ((x + cx * 8 + pxx) * SCALE, (y + cy * 8 + py) * SCALE);
                    for a in 0..SCALE {
                        for b in 0..SCALE {
                            self.px[(sy + b) * self.w + sx + a] = c;
                        }
                    }
                }
            }
        }
    }
    fn map(&mut self, x: usize, y: usize, t: &[u8], pal: &[[u8; 3]; 16]) {
        self.tiles(x, y, t, 2, 2, |k| (k % 2, k / 2), pal);
    }
    /// A moving sheet: frames of 9 tiles, tile k at (2 - k%3, 2 - k/3).
    fn sheet(&mut self, x: usize, y: usize, s: &[u8], pal: &[[u8; 3]; 16]) {
        for (f, frame) in s.chunks(9 * 32).enumerate() {
            self.tiles(x + f * 26, y, frame, 3, 3, |k| (2 - k % 3, 2 - k / 3), pal);
        }
    }
    fn save(&self, path: &std::path::Path) {
        std::fs::write(path, png(self.w, self.h, &self.px)).unwrap();
        eprintln!("wrote {}", path.display());
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

/// AW2 types shown beside the new units: Tank, Fighter, Lander.
const ORIGINALS: [u32; 3] = [5, 16, 23];
/// The AW2 type whose moving animation each new unit borrows in the preview.
const LIKE: [(u8, u8); 7] = [(4, 8), (9, 11), (12, 17), (13, 16), (18, 23), (25, 21), (26, 5)];
/// Units both games have, for the comparison.
const SHARED: [u8; 8] = [3, 5, 6, 8, 11, 16, 21, 23];

fn main() {
    let mut args = std::env::args().skip(1);
    let rom = Rom(std::fs::read(args.next().expect("aw2 rom")).unwrap());
    let out = std::path::PathBuf::from(args.next().expect("out dir"));
    tango_gamesupport_aw2::ds_pack::pack().expect("TANGOAW2_DS_ROM: a Dual Strike ROM or pack");
    let map_pal = |row: u32| rom.palette(0x0810_E6E0 + 32 * row);
    let move_pal = |row: u32| rom.palette(0x0810_EA60 + 32 * row);
    let cell = 18;

    // 1: every army, normal and greyed.
    let cols = ORIGINALS.len() + 1 + NEW_UNITS.len();
    let mut im = Image::new(cols * cell * SCALE, 10 * cell * SCALE);
    for row in 0..10u32 {
        let pal = map_pal(row);
        for (i, &t) in ORIGINALS.iter().enumerate() {
            im.map(i * cell + 1, row as usize * cell + 1, &rom.map_tiles(t, 0), &pal);
        }
        for (i, &id) in NEW_UNITS.iter().enumerate() {
            im.map(
                (ORIGINALS.len() + 1 + i) * cell + 1,
                row as usize * cell + 1,
                &map_tiles(id, 0).unwrap(),
                &pal,
            );
        }
    }
    im.save(&out.join("ds_units_map.png"));

    // 2: idle frames.
    let rows = ORIGINALS.len() + NEW_UNITS.len();
    let mut im = Image::new(3 * cell * SCALE, rows * cell * SCALE);
    let pal = map_pal(0);
    for f in 0..3 {
        for (r, &t) in ORIGINALS.iter().enumerate() {
            im.map(f * cell + 1, r * cell + 1, &rom.map_tiles(t, f as u32), &pal);
        }
        for (i, &id) in NEW_UNITS.iter().enumerate() {
            im.map(
                f * cell + 1,
                (ORIGINALS.len() + i) * cell + 1,
                &map_tiles(id, f).unwrap(),
                &pal,
            );
        }
    }
    im.save(&out.join("ds_units_idle_frames.png"));

    // 3: moving sheets (Orange Star's and Blue Moon's palettes).
    let mv = 26;
    let mut im = Image::new(2 * 9 * mv * SCALE + 8 * SCALE, rows * mv * SCALE);
    for (p, x0) in [(0u32, 0usize), (1, 9 * mv + 8)] {
        let pal = move_pal(p);
        for (r, &t) in ORIGINALS.iter().enumerate() {
            im.sheet(x0 + 1, r * mv + 1, &rom.move_sheet(t), &pal);
        }
        for (i, &(id, like)) in LIKE.iter().enumerate() {
            im.sheet(
                x0 + 1,
                (ORIGINALS.len() + i) * mv + 1,
                &move_sheet(id, like).unwrap(),
                &pal,
            );
        }
    }
    im.save(&out.join("ds_units_move.png"));

    // 4: AW2's own beside the conversion, per army.
    let mut im = Image::new(SHARED.len() * (2 * cell + 4) * SCALE, 5 * cell * SCALE);
    for row in 0..5u32 {
        let pal = map_pal(row);
        for (i, &id) in SHARED.iter().enumerate() {
            let x = i * (2 * cell + 4);
            im.map(x + 1, row as usize * cell + 1, &rom.map_tiles(id as u32, 0), &pal);
            im.map(x + cell + 1, row as usize * cell + 1, &map_tiles(id, 0).unwrap(), &pal);
        }
    }
    im.save(&out.join("ds_validate_map.png"));

    // 5: moving sheets, AW2's own above the conversion.
    let shared_moves = [5u8, 16, 23, 19];
    let mut im = Image::new(9 * mv * SCALE, shared_moves.len() * (2 * mv + 4) * SCALE);
    let pal = move_pal(0);
    for (i, &id) in shared_moves.iter().enumerate() {
        let y = i * (2 * mv + 4);
        im.sheet(1, y + 1, &rom.move_sheet(id as u32), &pal);
        im.sheet(1, y + mv + 1, &move_sheet(id, id).unwrap(), &pal);
    }
    im.save(&out.join("ds_validate_move.png"));
}
