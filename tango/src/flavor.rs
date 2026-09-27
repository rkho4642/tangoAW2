//! What this build shows. tangoAW2 is Tango cut down to one game,
//! Advance Wars 2, and the Battle Network conveniences that come with
//! the shared code stay compiled but are kept off screen here, so the
//! upstream code paths keep building and can be turned back on in one
//! place.

/// The patch server: the Patches tab, the patch and version pickers on
/// the Play tab and the patch settings. tangoAW2 applies its Advance
/// Wars 2 changes in memory and never fetches Battle Network patches.
pub const PATCHES: bool = false;

/// Controls that only mean something for the Battle Network games:
/// the Legacy Collection border behind the emulator and the Nintendo
/// DS screen layout.
pub const BATTLE_NETWORK_EXTRAS: bool = false;
