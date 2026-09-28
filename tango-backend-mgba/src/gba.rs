//! GBA constants and helpers the shared console builds on: the keypad
//! mask, the tick rate, the screen, and the BGR555 → RGBA expansion.

use num_rational::Ratio;
use tango_match::{Screen, ScreenLayout};

/// Bit mask of a joyflags value: the GBA keypad is 10 bits (A, B, Select,
/// Start, →, ←, ↑, ↓, R, L), occupying bits 0..=9. The top 6 bits are unused by
/// the hardware, so callers are free to repurpose them — e.g. the live core's r4
/// high bits, or the netplay wire's CONT/MARK entry tags.
pub const JOYFLAGS_MASK: u32 = 0x03ff;

/// Native ticks per second: one video frame spans 280,896 GBA clock cycles.
pub const TPS: Ratio<u32> = Ratio::new_raw(16_777_216, 280_896);

/// The GBA's single screen.
const SCREEN: Screen = Screen {
    width: 240,
    height: 160,
};

/// The screens this console presents, for a backend's
/// [`screen_layout`](tango_match::Backend::screen_layout).
pub fn screen_layout() -> ScreenLayout {
    ScreenLayout::new([SCREEN])
}

/// Expand mgba's native BGR555 to the RGBA8 the seam promises hosts.
pub(crate) fn to_rgba(src: &[u8]) -> Vec<u8> {
    let mut rgba = vec![0u8; src.len() * 2];
    mgba::gba::bgr555_to_rgba8(src, &mut rgba);
    rgba
}
