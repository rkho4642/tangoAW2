//! The user's game library and its persistence, frontend-agnostic.
//!
//! * [`game`]: the registry of supported games everything else is keyed
//!   by, and the game-name localizer.
//! * [`scanner`]: the shared fingerprint-gated rescan machinery the
//!   content scans below build on.
//! * [`rom`] / [`save`] / [`patch`] / [`replays`]: one module per kind of
//!   content the library folders hold.
//! * [`catalog`]: the four scanners bundled, with the rescan pipeline
//!   and the preparation helpers that read them.
//! * [`config`]: the persisted settings model.
//!
//! The optional `ui` feature attaches save editors to registered games.
//! Core library operations use no UI toolkit and never access the
//! filesystem or the network directly: [`storage::Storage`] and
//! [`http::Http`] are the two seams, and a frontend supplies both. The
//! `native` feature (on by default) provides `std::fs` and reqwest
//! implementations; a browser build turns it off and hands in IndexedDB and
//! `fetch` instead.

pub mod catalog;
pub mod config;
pub mod game;
pub mod http;
pub mod lang;
pub mod loadout;
pub mod marker;
pub mod patch;
pub mod replays;
pub mod rom;
pub mod save;
pub mod scanner;
pub mod stats;
pub mod storage;

pub use catalog::Catalog;

/// This player's [`SHARED_CONTENT`](tango_net_protocol::control::SHARED_CONTENT)
/// bit for their match subtype: set when they can show the content their
/// game gates on (tangoAW2: the Dual Strike pack imported).
pub fn shared_content_flag() -> u8 {
    #[cfg(feature = "gamesupport-aw2")]
    if tango_gamesupport_aw2::ds_pack::pack().is_some() {
        return tango_net_protocol::control::SHARED_CONTENT;
    }
    0
}
pub use storage::Storage;

#[cfg(test)]
mod test_support;
