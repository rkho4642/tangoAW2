//! The Dual Strike pack: everything tangoAW2's Dual Strike features read,
//! taken once from the player's own Advance Wars: Dual Strike ROM.
//!
//! tangoAW2 ships nothing from Dual Strike. When a Dual Strike (USA) ROM
//! is in the ROMs folder, the library scan offers it here
//! ([`crate::ds_art::offer`]); the pack keeps its ARM9 code image, its
//! overlays and its file system (every file but the sound archive, of
//! which only the new COs' themes, the heal sounds, the DS Campaign's
//! songs and the staff roll's stream are kept:
//! [`crate::ds_music`]), and
//! is saved next to the ROMs ([`CACHE_NAME`]) so the .nds is needed only
//! once. Features built on it read their tables and pictures from here at
//! run time.
//!
//! Only the USA ROM is accepted (game code `AWRE`, header CRC16 `0xB586`):
//! the offsets the features use are that release's.

use std::collections::HashMap;
use std::sync::OnceLock;

/// The saved pack's file in the ROMs folder.
pub const CACHE_NAME: &str = "Dual Strike pack.tangoaw2";
const MAGIC: &[u8; 8] = b"TAW2DSPK";
/// Bumped when the pack's contents change; an older pack is rebuilt from
/// the .nds on the next scan (3: the heal sounds; 4: the DS Campaign's
/// songs; 5: the staff roll's stream). Without the .nds, a pack of version
/// [`OLDEST`] or later is still used (what it lacks falls back to AW2's
/// own).
const VERSION: u32 = 5;
const OLDEST: u32 = 3;
/// The header CRC16 of Advance Wars: Dual Strike (USA).
const HEADER_CRC: u16 = 0xB586;

/// Directories not kept (the sound archive: 18 MB, of which
/// [`crate::ds_music::keep`] takes the few files the music reads).
const SKIPPED_DIRS: [&str; 1] = ["data/"];
const SOUND_ARCHIVE: &str = "data/sound_data.sdat";

pub struct Pack {
    /// The ARM9 code image (loaded at 0x02000000, uncompressed in this ROM).
    pub arm9: Vec<u8>,
    /// Overlay images by overlay id.
    pub overlays: Vec<Vec<u8>>,
    /// File-system files by path (`bmap/015`, `battle/0dd`, ...).
    files: HashMap<String, Vec<u8>>,
}

impl Pack {
    /// A file by its path in the DS file system.
    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(|v| v.as_slice())
    }

    /// Bytes of the ARM9 image at a DS address.
    pub fn arm9_at(&self, addr: u32, len: usize) -> Option<&[u8]> {
        let o = addr.checked_sub(0x0200_0000)? as usize;
        self.arm9.get(o..o + len)
    }

    /// Bytes of overlay `id` at a DS address, given the overlay's load address.
    pub fn overlay_at(&self, id: usize, base: u32, addr: u32, len: usize) -> Option<&[u8]> {
        let o = addr.checked_sub(base)? as usize;
        self.overlays.get(id)?.get(o..o + len)
    }
}

static PACK: OnceLock<Pack> = OnceLock::new();
/// A saved pack of an older version: used until (unless) the .nds gives a
/// current one.
static OLDER: OnceLock<Pack> = OnceLock::new();

/// The pack, if one has been offered (a DS ROM, a saved pack, or the file
/// `TANGOAW2_DS_ROM` names).
pub fn pack() -> Option<&'static Pack> {
    if let Some(p) = PACK.get() {
        return Some(p);
    }
    if let Some(path) = std::env::var_os("TANGOAW2_DS_ROM") {
        if let Ok(buf) = std::fs::read(&path) {
            crate::ds_art::offer(&buf);
        }
    }
    PACK.get().or_else(|| OLDER.get())
}

/// Whether the Dual Strike features are on: for a netplay match or its
/// replay, as the match says ([`crate::ds_art::SHARED_ART`]), so both peers
/// and every replay agree; played alone, when this player has the pack.
pub fn features(mode: Option<(u8, u8)>) -> bool {
    match mode {
        Some((_, subtype)) => subtype & crate::ds_art::SHARED_ART != 0,
        None => pack().is_some(),
    }
}

/// Whether `rom` is the Dual Strike release the pack is built from.
pub fn is_supported_rom(rom: &[u8]) -> bool {
    rom.len() >= 0x200 && &rom[0x0C..0x10] == b"AWRE" && u16::from_le_bytes([rom[0x15E], rom[0x15F]]) == HEADER_CRC
}

/// Build the pack from a Dual Strike ROM and keep it. `false` if the ROM is
/// not the supported release or is damaged.
pub(crate) fn offer_rom(rom: &[u8]) -> bool {
    if !is_supported_rom(rom) {
        return false;
    }
    match from_rom(rom) {
        Some(p) => {
            let _ = PACK.set(p);
            true
        }
        None => false,
    }
}

/// Load a saved pack and keep it. `false` if `buf` is not one (or is an
/// older version).
pub(crate) fn offer_saved(buf: &[u8]) -> bool {
    match decode(buf) {
        Some((p, true)) => {
            let _ = PACK.set(p);
            true
        }
        Some((p, false)) => {
            let _ = OLDER.set(p);
            true
        }
        None => false,
    }
}

/// Whether `buf` looks like a saved pack (of any version).
pub(crate) fn is_saved_pack(buf: &[u8]) -> bool {
    buf.len() >= 8 && &buf[..8] == MAGIC
}

/// The kept pack as a file to save next to the ROMs.
pub fn cache() -> Option<Vec<u8>> {
    PACK.get().map(encode)
}

fn u32_at(b: &[u8], o: usize) -> Option<usize> {
    b.get(o..o + 4)
        .map(|x| u32::from_le_bytes(x.try_into().unwrap()) as usize)
}

fn u16_at(b: &[u8], o: usize) -> Option<usize> {
    b.get(o..o + 2)
        .map(|x| u16::from_le_bytes(x.try_into().unwrap()) as usize)
}

fn from_rom(rom: &[u8]) -> Option<Pack> {
    let (arm9_off, arm9_len) = (u32_at(rom, 0x20)?, u32_at(rom, 0x2C)?);
    let arm9 = rom.get(arm9_off..arm9_off + arm9_len)?.to_vec();
    let (fnt, fat) = (u32_at(rom, 0x40)?, u32_at(rom, 0x48)?);
    let fat_file = |id: usize| -> Option<Vec<u8>> {
        let (a, b) = (u32_at(rom, fat + 8 * id)?, u32_at(rom, fat + 8 * id + 4)?);
        rom.get(a..b).map(|s| s.to_vec())
    };
    // Overlay table: 32-byte entries, the file id at +0x18.
    let (ovt, ovt_len) = (u32_at(rom, 0x50)?, u32_at(rom, 0x54)?);
    let mut overlays = Vec::new();
    for i in 0..ovt_len / 32 {
        overlays.push(fat_file(u32_at(rom, ovt + 32 * i + 0x18)?)?);
    }
    let mut files = HashMap::new();
    let mut sound = None;
    let mut stack = vec![(0usize, String::new())];
    while let Some((dir, path)) = stack.pop() {
        let mut p = fnt + u32_at(rom, fnt + 8 * dir)?;
        let mut id = u16_at(rom, fnt + 8 * dir + 4)?;
        loop {
            let len = *rom.get(p)? as usize;
            p += 1;
            if len == 0 {
                break;
            }
            let name = String::from_utf8_lossy(rom.get(p..p + (len & 0x7F))?).into_owned();
            p += len & 0x7F;
            if len & 0x80 != 0 {
                let child = u16_at(rom, p)? & 0xFFF;
                p += 2;
                stack.push((child, format!("{path}{name}/")));
            } else {
                let full = format!("{path}{name}");
                if full == SOUND_ARCHIVE {
                    sound = fat_file(id);
                } else if !SKIPPED_DIRS.iter().any(|d| full.starts_with(d)) {
                    files.insert(full, fat_file(id)?);
                }
                id += 1;
            }
        }
    }
    // The new COs' themes, with their instruments and samples.
    for (name, data) in crate::ds_music::keep(&sound?, &arm9)? {
        files.insert(name, data);
    }
    Some(Pack { arm9, overlays, files })
}

fn encode(p: &Pack) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&VERSION.to_le_bytes());
    let mut entries: Vec<(String, &[u8])> = vec![("#arm9".into(), &p.arm9)];
    for (i, o) in p.overlays.iter().enumerate() {
        entries.push((format!("#ov{i}"), o));
    }
    let mut names: Vec<&String> = p.files.keys().collect();
    names.sort();
    for n in names {
        entries.push((n.clone(), &p.files[n]));
    }
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for (name, data) in entries {
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
    }
    out
}

/// A saved pack, and whether it is of the current version.
fn decode(buf: &[u8]) -> Option<(Pack, bool)> {
    let version = u32_at(buf, 8)? as u32;
    if !is_saved_pack(buf) || !(OLDEST..=VERSION).contains(&version) {
        return None;
    }
    let count = u32_at(buf, 12)?;
    let mut p = 16;
    let (mut arm9, mut overlays, mut files) = (None, Vec::new(), HashMap::new());
    for _ in 0..count {
        let n = u16_at(buf, p)?;
        let name = std::str::from_utf8(buf.get(p + 2..p + 2 + n)?).ok()?.to_string();
        p += 2 + n;
        let len = u32_at(buf, p)?;
        let data = buf.get(p + 4..p + 4 + len)?.to_vec();
        p += 4 + len;
        if name == "#arm9" {
            arm9 = Some(data);
        } else if let Some(i) = name.strip_prefix("#ov") {
            let i: usize = i.parse().ok()?;
            if overlays.len() <= i {
                overlays.resize(i + 1, Vec::new());
            }
            overlays[i] = data;
        } else {
            files.insert(name, data);
        }
    }
    Some((
        Pack {
            arm9: arm9?,
            overlays,
            files,
        },
        version == VERSION,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With `TANGOAW2_DS_ROM` naming a Dual Strike (USA) ROM: the pack
    /// builds, saves and loads back unchanged, and holds what the features
    /// read. `cargo test -p tango-gamesupport-aw2 -- --ignored`.
    #[test]
    #[ignore]
    fn a_real_rom_builds_a_pack() {
        let rom = std::fs::read(std::env::var_os("TANGOAW2_DS_ROM").expect("TANGOAW2_DS_ROM")).unwrap();
        assert!(is_supported_rom(&rom));
        let p = from_rom(&rom).unwrap();
        let saved = encode(&p);
        let q = decode(&saved).unwrap().0;
        assert_eq!(q.arm9, p.arm9);
        assert_eq!(q.overlays, p.overlays);
        assert_eq!(q.files.len(), p.files.len());
        for f in ["bmap/015", "bmap/00e", "bmap/001", "bmap/009", "bmap/01d", "bmap/06c"] {
            assert!(q.file(f).is_some(), "{f}");
        }
        assert!(!q.files.keys().any(|k| k.starts_with("data/")));
        assert!(q.files.keys().any(|k| k.starts_with("sound/seq/")));
        // Overlay 0 holds the unit records: Infantry (id 1) costs 1000.
        let inf = q
            .overlay_at(0, 0x022A_D560, 0x022A_D560 + 0x47A58 + 0x6C, 0x6C)
            .unwrap();
        eprintln!(
            "pack: {} KB, {} files, {} overlays; infantry record starts {:02x?}",
            saved.len() / 1024,
            q.files.len(),
            q.overlays.len(),
            &inf[..16]
        );
    }

    #[test]
    fn a_pack_round_trips() {
        let mut files = HashMap::new();
        files.insert("bmap/015".to_string(), vec![1, 2, 3]);
        files.insert("battle/0dd".to_string(), vec![]);
        let p = Pack {
            arm9: vec![9; 10],
            overlays: vec![vec![0; 4], vec![1]],
            files,
        };
        let q = decode(&encode(&p)).unwrap().0;
        assert_eq!(q.arm9, p.arm9);
        assert_eq!(q.overlays, p.overlays);
        assert_eq!(q.file("bmap/015"), Some(&[1u8, 2, 3][..]));
        assert_eq!(q.file("battle/0dd"), Some(&[][..]));
        assert!(decode(b"TAW2DSPK\x02\0\0\0\0\0\0\0").is_none());
        // A pack of the previous version still loads, as an older one.
        let mut old = encode(&p);
        old[8..12].copy_from_slice(&OLDEST.to_le_bytes());
        assert!(!decode(&old).unwrap().1);
        assert!(decode(&encode(&p)).unwrap().1);
    }
}
