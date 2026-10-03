//! Shared by the Dual Strike pack update tests (each its own process: the
//! pack is kept for the process's life).

use std::path::{Path, PathBuf};
use tango_library::storage::{Entry, Listing};

pub const PACK: &str = "Dual Strike pack.tangoaw2";

/// A pack saved by an older tangoAW2 (`TANGOAW2_OLD_PACK`, e.g. 0.5.0's,
/// version 5), its version.
pub fn old_pack() -> (Vec<u8>, u32) {
    let path = std::env::var_os("TANGOAW2_OLD_PACK").expect("TANGOAW2_OLD_PACK: a pack saved by an older tangoAW2");
    let buf = std::fs::read(path).unwrap();
    assert_eq!(&buf[..8], b"TAW2DSPK");
    let v = u32::from_le_bytes(buf[8..12].try_into().unwrap());
    (buf, v)
}

/// A fresh ROMs folder.
pub fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tangoaw2-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn listing(dir: &Path) -> Listing {
    let entries = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            let m = e.metadata().unwrap();
            Entry {
                path: e.path(),
                len: m.len(),
                modified: None,
            }
        })
        .collect();
    Listing::new(entries)
}

pub fn version_of(path: &Path) -> u32 {
    let buf = std::fs::read(path).unwrap();
    u32::from_le_bytes(buf[8..12].try_into().unwrap())
}
