//! The mgba engine for tangoAW2: one emulated GBA that both netplay
//! peers simulate together ([`shared`]), with its rollback, solo and
//! replay support.
//!
//! It lives outside `tango-match` so that crate — the engine-neutral seam
//! the sessions speak — builds without an emulator.

pub mod gba;
pub mod shared;

pub use gba::JOYFLAGS_MASK;
pub use shared::{SharedBackend, SharedGame, SharedLink};

/// Simulation failure, as this engine reports it. Converts into the
/// seam's [`tango_match::Error`], which cannot name mgba's error type.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Mgba(#[from] mgba::Error),

    /// The caller's cancel flag flipped mid-simulation.
    #[error("cancelled")]
    Cancelled,
}

impl From<Error> for tango_match::Error {
    fn from(error: Error) -> Self {
        match error {
            Error::Cancelled => tango_match::Error::Cancelled,
            Error::Mgba(e) => tango_match::Error::Backend(Box::new(e)),
        }
    }
}
