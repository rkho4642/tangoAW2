//! The on-disk game library, as this frontend sees it.
//!
//! The library itself lives in the [`tango_library`] crate. Its core
//! operations are independent of UI and reach storage and the network only
//! through its `Storage` / `Http` traits — so a browser build can reuse
//! the registry, the scanners, and the patch catalog over IndexedDB and
//! `fetch`. This module re-exports that surface (so `crate::library::*`
//! keeps resolving), binds the native filesystem as its storage, and
//! adds the parts that are genuinely host-side:
//!
//! * [`replays`]: the crate's replay index, plus the re-simulation that
//!   produces match stats — that one needs an emulator core and the
//!   analysis engine, so it stays out of the library.

pub mod replays;
pub use tango_library::{game, rom, save, Catalog};

use tango_library::storage::Storage;

/// The filesystem, as the library sees it. Stateless, so this is a
/// plain static.
pub fn storage() -> &'static dyn Storage {
    &tango_library::storage::StdStorage
}
