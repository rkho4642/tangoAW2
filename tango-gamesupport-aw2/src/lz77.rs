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
