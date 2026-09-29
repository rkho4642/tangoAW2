//! Preview of the Dual Strike CO conversion ([`tango_gamesupport_aw2::ds_co_art`]),
//! for approval. Each CO is one row, drawn the way AW2 lays the pictures
//! out: faces normal / happy / sad, mini portrait, HUD face, name (in AW2's
//! name palette) and full body, in colour scheme 0.
//!
//! - `ds_cos.png`: AW2's own Andy and Sturm (read from the AW2 ROM), then
//!   Dual Strike's Andy converted, then the nine new COs.
//! - `ds_co_schemes.png`: each new CO's normal face in its 8 colour schemes.
//!
//! Usage: TANGOAW2_DS_ROM=<Dual Strike ROM or pack> ds_co_art_preview <aw2 rom> <out dir>

use tango_gamesupport_aw2::ds_art::lz10;
use tango_gamesupport_aw2::ds_co_art::{co_art, CoArt, NEW_COS};

const SCALE: usize = 3;
const BG: [u8; 3] = [60, 90, 140];
const GAP: [u8; 3] = [24, 24, 24];

struct Rom(Vec<u8>);

impl Rom {
    fn at(&self, a: u32, n: usize) -> &[u8] {
        &self.0[(a - 0x0800_0000) as usize..][..n]
    }
    fn u32(&self, a: u32) -> u32 {
        u32::from_le_bytes(self.at(a, 4).try_into().unwrap())
    }
    fn lz(&self, a: u32) -> Vec<u8> {
        lz10(&self.0[(a - 0x0800_0000) as usize..]).unwrap()
    }
    /// AW2's own pictures of CO `co`, as a [`CoArt`].
    fn co(&self, co: u32) -> CoArt {
        let row = 0x084A_0090 + 0x44 * co;
        let body = self.u32(row);
        CoArt {
            face: [
                self.lz(self.u32(row + 0x0C)),
                self.lz(self.u32(row + 0x10)),
                self.lz(self.u32(row + 0x14)),
            ],
            mini: self.at(self.u32(row + 0x18), 384).to_vec(),
            hud: self.at(0x0810_2F64 + 0x100 * co, 256).to_vec(),
            body_top: self.lz(self.u32(body)),
            body_bottom: self.lz(self.u32(body + 4)),
            name: self.lz(self.u32(row + 4)),
            palette: self.at(self.u32(row + 8), 0x100).to_vec(),
        }
    }
}

fn palette(b: &[u8]) -> [[u8; 3]; 16] {
    std::array::from_fn(|i| {
        let c = u16::from_le_bytes([b[2 * i], b[2 * i + 1]]);
        let s = |v: u16| ((v & 31) * 255 / 31) as u8;
        [s(c), s(c >> 5), s(c >> 10)]
    })
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
            px: vec![GAP; w * h * SCALE * SCALE],
        }
    }
    fn fill(&mut self, x: usize, y: usize, w: usize, h: usize) {
        for yy in y * SCALE..(y + h) * SCALE {
            for xx in x * SCALE..(x + w) * SCALE {
                self.px[yy * self.w * SCALE + xx] = BG;
            }
        }
    }
    /// Tiles in rows `tw` wide, from `tiles`, tile k at `place(k)`.
    fn tiles(&mut self, x: usize, y: usize, tiles: &[u8], place: impl Fn(usize) -> (usize, usize), pal: &[[u8; 3]; 16]) {
        for k in 0..tiles.len() / 32 {
            let (cx, cy) = place(k);
            for py in 0..8 {
                for pxx in 0..8 {
                    let b = tiles[k * 32 + py * 4 + pxx / 2];
                    let v = if pxx & 1 == 1 { b >> 4 } else { b & 15 } as usize;
                    if v == 0 {
                        continue;
                    }
                    let (sx, sy) = ((x + cx * 8 + pxx) * SCALE, (y + cy * 8 + py) * SCALE);
                    for a in 0..SCALE {
                        for b in 0..SCALE {
                            self.px[(sy + b) * self.w * SCALE + sx + a] = pal[v];
                        }
                    }
                }
            }
        }
    }
    fn rows(&mut self, x: usize, y: usize, w: usize, h: usize, t: &[u8], pal: &[[u8; 3]; 16]) {
        self.fill(x, y, w, h);
        let tw = w / 8;
        self.tiles(x, y, t, |k| (k % tw, k / tw), pal);
    }
    /// A full body: six sprites as `0x084A0756` places them.
    fn body(&mut self, x: usize, y: usize, a: &CoArt, pal: &[[u8; 3]; 16]) {
        self.fill(x, y, 128, 160);
        let all: Vec<u8> = a.body_top.iter().chain(&a.body_bottom).copied().collect();
        let sprites = [(0, 0, 8, 8), (64, 0, 8, 8), (0, 64, 8, 8), (64, 64, 8, 8), (0, 128, 8, 4), (64, 128, 8, 4)];
        let mut t = 0;
        for (sx, sy, tw, th) in sprites {
            let n = tw * th;
            self.tiles(x + sx, y + sy, &all[t * 32..(t + n) * 32], |k| (k % tw, k / tw), pal);
            t += n;
        }
    }
    /// A name graphic: six 8x16 columns.
    fn name(&mut self, x: usize, y: usize, t: &[u8], pal: &[[u8; 3]; 16]) {
        self.fill(x, y, 48, 16);
        self.tiles(x, y, t, |k| (k / 2, k % 2), pal);
    }
    fn co(&mut self, y: usize, a: &CoArt, name_pal: &[[u8; 3]; 16], scheme: usize) {
        let pal = palette(&a.palette[0x20 * scheme..]);
        for (i, f) in a.face.iter().enumerate() {
            self.rows(4 + i * 52, y, 48, 48, f, &pal);
        }
        self.fill(4, y + 56, 32, 24);
        self.tiles(4, y + 56, &a.mini, |k| (k / 6 * 2 + k % 2, k % 6 / 2), &pal);
        self.rows(40, y + 56, 32, 16, &a.hud, &pal);
        self.name(76, y + 56, &a.name, name_pal);
        self.body(164, y, a, &pal);
    }
    fn save(&self, path: &std::path::Path) {
        std::fs::write(path, png(self.w * SCALE, self.h * SCALE, &self.px)).unwrap();
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

fn main() {
    let mut args = std::env::args().skip(1);
    let rom = Rom(std::fs::read(args.next().expect("aw2 rom")).unwrap());
    let out = std::path::PathBuf::from(args.next().expect("out dir"));
    tango_gamesupport_aw2::ds_pack::pack().expect("TANGOAW2_DS_ROM: a Dual Strike ROM or pack");
    // The name palette `sub_08043B44` loads.
    let name_pal = palette(rom.at(0x080F_6164, 32));

    // AW2's Andy (1) and Sturm (10), Dual Strike's Andy (2), the new nine.
    let mut cos = vec![rom.co(1), rom.co(10), co_art(2).unwrap()];
    cos.extend(NEW_COS.iter().map(|&id| co_art(id).unwrap()));
    let row = 164;
    let mut im = Image::new(296, row * cos.len());
    for (i, a) in cos.iter().enumerate() {
        im.co(i * row + 2, a, &name_pal, 0);
    }
    im.save(&out.join("ds_cos.png"));

    let mut im = Image::new(8 * 52 + 4, 52 * NEW_COS.len());
    for (i, &id) in NEW_COS.iter().enumerate() {
        let a = co_art(id).unwrap();
        for s in 0..8 {
            im.rows(4 + s * 52, i * 52 + 2, 48, 48, &a.face[0], &palette(&a.palette[0x20 * s..]));
        }
    }
    im.save(&out.join("ds_co_schemes.png"));
}
