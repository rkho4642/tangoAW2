//! Dual Strike's unit stats and damage chart for AW2's own units, from the
//! Dual Strike pack ([`crate::ds_pack`]).
//!
//! With the pack on, each existing unit type takes Dual Strike's cost,
//! move, ammo, vision, range and fuel, and its two damage rows, as one
//! matching set, so the balance is Dual Strike's; with it off they are
//! AW2's own again. Dual Strike numbers its units like AW2 (Infantry 1 ...
//! Sub 24), so unit `n`'s Dual Strike record is AW2's record `n`.
//!
//! AW2's record (0x5C bytes at [`UNITS`]): cost/10 u16 +0x06, move +0x0A,
//! ammo +0x0B, vision +0x0C, range +0x0E/+0x0F, fuel +0x10, damage
//! u8[26] primary +0x1E and secondary +0x38 (column = defender id; 25 is
//! a dived Sub). Dual Strike's (0x6C bytes, overlay 0 0x47A58): cost/10
//! u32 +0x0C, move +0x10, ammo +0x11, vision +0x12, range +0x14/+0x15,
//! fuel +0x16, damage u8[32] +0x24/+0x44 (byte i = defender id i+1; 27 is
//! a submerged Sub). Every reader, the AI's included, goes through AW2's
//! table, so they all see the same numbers.

use mgba::core::Core;
use std::sync::OnceLock;

/// AW2's unit table.
pub const UNITS: u32 = 0x085D_5ABC;
const RECORD: u32 = 0x5C;
/// AW2's unit types (1..24, less the unused 4, 9, 12, 13 and 18).
pub const AW2_TYPES: [u32; 19] = [1, 2, 3, 5, 6, 7, 8, 10, 11, 14, 15, 16, 17, 19, 20, 21, 22, 23, 24];
/// Defender columns in AW2's damage rows (0 is no unit, 25 a dived Sub).
const COLUMNS: usize = 26;

/// Dual Strike's unit records in overlay 0.
const DS_OVERLAY_BASE: u32 = 0x022A_D560;
const DS_UNITS: u32 = DS_OVERLAY_BASE + 0x47A58;
const DS_RECORD: usize = 0x6C;
/// Dual Strike's defender slot for a submerged Sub.
const DS_SUBMERGED_SUB: usize = 27;

/// One run of bytes in the ROM image: AW2's and Dual Strike's.
struct Patch {
    at: u32,
    aw2: Vec<u8>,
    ds: Vec<u8>,
}

static PATCHES: OnceLock<Vec<Patch>> = OnceLock::new();

/// The Dual Strike values for one AW2 record, laid out as AW2's fields:
/// (offset in the record, bytes).
pub fn ds_fields(ds: &[u8]) -> Vec<(u32, Vec<u8>)> {
    let u32_at = |o: usize| u32::from_le_bytes(ds[o..o + 4].try_into().unwrap());
    let column = |row: usize, def: usize| -> u8 {
        let slot = if def == 25 { DS_SUBMERGED_SUB } else { def };
        if slot == 0 {
            0
        } else {
            ds[row + slot - 1]
        }
    };
    let row = |base: usize| (0..COLUMNS).map(|d| column(base, d)).collect::<Vec<u8>>();
    vec![
        (0x06, (u32_at(0x0C) as u16).to_le_bytes().to_vec()),
        (0x0A, vec![ds[0x10], ds[0x11], ds[0x12]]),
        (0x0E, vec![ds[0x14], ds[0x15], ds[0x16]]),
        (0x1E, row(0x24)),
        (0x38, row(0x44)),
    ]
}

fn build(core: &Core) -> Option<Vec<Patch>> {
    let pack = crate::ds_pack::pack()?;
    let mut out = Vec::new();
    for id in AW2_TYPES {
        let ds = pack.overlay_at(0, DS_OVERLAY_BASE, DS_UNITS + DS_RECORD as u32 * id, DS_RECORD)?;
        for (off, bytes) in ds_fields(ds) {
            let at = UNITS + RECORD * id + off;
            let mut aw2 = vec![0u8; bytes.len()];
            core.raw_read_range(at, -1, &mut aw2);
            out.push(Patch { at, aw2, ds: bytes });
        }
    }
    Some(out)
}

/// Every frame: the unit table as the pack says (`on`) or as AW2 has it.
/// Idempotent, and the same on every peer (the ROM image and `on` are).
pub fn apply(core: &mut Core, on: bool) {
    // AW2's bytes are read from the ROM image before anything is written
    // there, the first time the pack is seen.
    if PATCHES.get().is_none() {
        if !on {
            return;
        }
        match build(core) {
            Some(p) => {
                let _ = PATCHES.set(p);
            }
            None => return,
        }
    }
    for p in PATCHES.get().unwrap() {
        let want = if on { &p.ds } else { &p.aw2 };
        let mut now = vec![0u8; want.len()];
        core.raw_read_range(p.at, -1, &mut now);
        if &now != want {
            core.raw_write_range(p.at, -1, want);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_map_onto_aw2s_record() {
        let mut ds = vec![0u8; DS_RECORD];
        ds[0x0C..0x10].copy_from_slice(&700u32.to_le_bytes());
        ds[0x10] = 6; // move
        ds[0x11] = 9; // ammo
        ds[0x12] = 3; // vision
        ds[0x14] = 1;
        ds[0x15] = 1;
        ds[0x16] = 70;
        for i in 0..32 {
            ds[0x24 + i] = i as u8 + 1; // primary vs defender id i+1
            ds[0x44 + i] = 100 + i as u8;
        }
        let f = ds_fields(&ds);
        assert_eq!(f[0], (0x06, 700u16.to_le_bytes().to_vec()));
        assert_eq!(f[1], (0x0A, vec![6, 9, 3]));
        assert_eq!(f[2], (0x0E, vec![1, 1, 70]));
        let prim = &f[3].1;
        assert_eq!(prim.len(), COLUMNS);
        assert_eq!(prim[0], 0);
        assert_eq!(prim[5], 5); // defender id 5 (Tank)
        assert_eq!(prim[24], 24); // Sub
        assert_eq!(prim[25], 27); // dived Sub = DS slot 27
        assert_eq!(f[4].1[1], 100);
    }
}
