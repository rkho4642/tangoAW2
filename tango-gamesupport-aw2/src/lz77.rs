//! LZ77 (type 0x10) compression, as the GBA BIOS decompresses it: for
//! pictures tangoAW2 puts in the ROM image where the game expects
//! compressed data ([`crate::ds_art::lz10`] is the other way).
//!
//! Matches keep at least 2 bytes back, so the data also decompresses
//! safely to VRAM (16-bit writes).

const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 18;
const MIN_DISP: usize = 2;
const MAX_DISP: usize = 4096;

pub fn compress(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x10, data.len() as u8, (data.len() >> 8) as u8, (data.len() >> 16) as u8];
    let mut p = 0;
    while p < data.len() {
        let flag_at = out.len();
        out.push(0);
        for bit in 0..8 {
            if p >= data.len() {
                break;
            }
            let (mut best_len, mut best_disp) = (0, 0);
            let start = p.saturating_sub(MAX_DISP);
            let mut q = if p >= MIN_DISP { p - MIN_DISP + 1 } else { start };
            while q > start {
                q -= 1;
                let mut n = 0;
                while n < MAX_MATCH && p + n < data.len() && data[q + n] == data[p + n] {
                    n += 1;
                }
                if n > best_len {
                    best_len = n;
                    best_disp = p - q;
                    if n == MAX_MATCH {
                        break;
                    }
                }
            }
            if best_len >= MIN_MATCH {
                out[flag_at] |= 0x80 >> bit;
                let (len, disp) = (best_len - MIN_MATCH, best_disp - 1);
                out.push(((len << 4) | (disp >> 8)) as u8);
                out.push(disp as u8);
                p += best_len;
            } else {
                out.push(data[p]);
                p += 1;
            }
        }
    }
    while out.len() % 4 != 0 {
        out.push(0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let mut data: Vec<u8> = (0..5000u32).map(|i| ((i * 7) % 13) as u8).collect();
        data.extend((0..300u32).map(|i| (i * 2654435761u32 >> 24) as u8));
        data.extend(std::iter::repeat(0).take(700));
        let c = compress(&data);
        assert!(c.len() < data.len());
        assert_eq!(crate::ds_art::lz10(&c).unwrap(), data);
        assert_eq!(crate::ds_art::lz10(&compress(&[])).unwrap(), Vec::<u8>::new());
    }
}

#[cfg(test)]
mod pack_tests {
    /// Every new CO picture round-trips (needs `TANGOAW2_DS_ROM`).
    #[test]
    #[ignore]
    fn co_pictures_round_trip() {
        for ds in crate::ds_co_art::NEW_COS {
            let a = crate::ds_co_art::co_art(ds).unwrap();
            for p in [&a.name, &a.body_top, &a.body_bottom, &a.face[0], &a.face[1], &a.face[2]] {
                assert_eq!(&crate::ds_art::lz10(&super::compress(p)).unwrap(), p, "CO {ds}");
            }
        }
    }
}

#[cfg(test)]
mod dump_names {
    #[test]
    #[ignore]
    fn dump() {
        for ds in [23u8, 25] {
            let a = crate::ds_co_art::co_art(ds).unwrap();
            let mut s = String::new();
            for y in 0..16 {
                for x in 0..48 {
                    let t = (x / 8) * 2 + y / 8;
                    let b = a.name[32 * t + 4 * (y % 8) + (x % 8) / 2];
                    let v = (b >> (4 * (x & 1))) & 15;
                    s.push(char::from_digit(v as u32, 16).unwrap());
                }
                s.push('\n');
            }
            println!("{ds}\n{s}");
        }
    }
}
