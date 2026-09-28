//! The private gamesupport layer's save editor and its supporting
//! cast: [`editor`] (the per-game interface, the shell implementing the
//! public `tango_gamesupport::SaveEditor` embedding API, the view, and
//! the `OpenSave` baking), [`model`] (the save model it operates on),
//! and [`i18n`] (the editor's fluent bundle). Drawing substrate comes
//! from the game-agnostic `tango-ui` toolkit: [`widgets`] and [`style`]
//! re-export it and [`anim`] passes straight through, so module paths
//! inside read `crate::widgets` / `crate::style` / `crate::anim`.
//! Likewise [`dataview`] passes the parsing substrate crate through.

pub use tango_gamesupport_common_dataview as dataview;
pub use tango_ui::anim;

pub mod editor;
pub mod i18n;
pub mod model;
pub mod style;
pub mod widgets;
