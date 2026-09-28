//! Shared UI metrics: the typographic scale, row/pane spacing, and the
//! monospace face — the layout constants both the app and the
//! save-editor layer measure against, so a row built on either side of
//! that boundary comes out the same height. Metrics only one of them
//! uses live in its own `style` module, which re-exports this one.

// Typographic scale. Everything that renders text picks from this
// list; one-off sizes outside it tend to look like UI bugs
// (random 12px next to 11px next to 13px). If you need a new
// size, add it here and update the audit.
//
//   TITLE   — section headers ("tab-settings", empty-state cards).
//   HEADING — sub-section labels (nickname on side cards).
//   BODY    — default body copy. Same value as the iced default.
//   CAPTION — muted hints, status lines, metadata labels.
pub const TEXT_TITLE: f32 = 18.0;
pub const TEXT_HEADING: f32 = 15.0;
pub const TEXT_BODY: f32 = 13.0;
pub const TEXT_CAPTION: f32 = 11.0;

/// List rows and whole-row buttons (library entries, zebra rows).
pub const ROW_PADDING: [f32; 2] = [6.0, 10.0];

/// Standard internal padding for [`crate::widgets::pane`] containers.
/// Use this on `.padding(...)` so every demarcation pane has the same
/// gap between its edge and its content.
pub const PANE_PADDING: f32 = 12.0;
/// Standard outer gap (column spacing / row spacing / outer padding)
/// between sibling panes.
pub const PANE_GAP: f32 = 8.0;

/// The bundled monospace face. Most widgets inherit the app's default
/// font for free; the ones that build their own text styles have to
/// name a face explicitly, and anything tabular wants this one.
pub const MONOSPACE_FONT: iced::Font = iced::Font::with_name("Noto Sans Mono");

/// The name of tangoAW2's Advance Wars theme (`Theme::custom`); styles
/// that dress up for it test with [`is_advance_wars`].
pub const ADVANCE_WARS_THEME: &str = "Advance Wars";

/// Whether `theme` is the Advance Wars look: the game's cream menu boxes
/// with red frames over a sepia field.
pub fn is_advance_wars(theme: &iced::Theme) -> bool {
    theme.to_string() == ADVANCE_WARS_THEME
}

/// The Advance Wars look's colors, taken from the game's own menus.
pub mod aw {
    use iced::Color;
    /// Menu box fill.
    pub const CREAM: Color = Color::from_rgb8(0xf8, 0xf0, 0xd8);
    /// The faint stripes across a menu box.
    pub const CREAM_STRIPE: Color = Color::from_rgb8(0xf0, 0xe4, 0xc2);
    /// Input and picker fill: a touch brighter than the box.
    pub const PAPER: Color = Color::from_rgb8(0xff, 0xf8, 0xe6);
    /// Input and picker edge.
    pub const TAN: Color = Color::from_rgb8(0xc9, 0xb5, 0x8a);
    /// Menu box frame and the Fight button.
    pub const RED: Color = Color::from_rgb8(0xc8, 0x40, 0x2c);
    pub const RED_DARK: Color = Color::from_rgb8(0x8e, 0x2a, 0x1c);
    /// Headings.
    pub const BLUE: Color = Color::from_rgb8(0x28, 0x48, 0xa8);
    /// Body text.
    pub const INK: Color = Color::from_rgb8(0x2a, 0x24, 0x18);
    /// The selection cursor.
    pub const GOLD: Color = Color::from_rgb8(0xf0, 0xc0, 0x40);
    /// The top and bottom bars: an olive-brown plate.
    pub const PLATE_TOP: Color = Color::from_rgb8(0x3b, 0x33, 0x26);
    pub const PLATE_BOTTOM: Color = Color::from_rgb8(0x22, 0x1d, 0x15);
    /// Text on the plates.
    pub const PLATE_TEXT: Color = Color::from_rgb8(0xe9, 0xdf, 0xc3);
}
