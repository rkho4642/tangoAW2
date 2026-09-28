//! The save editor — everything that loads and draws a save.
//!
//! [`GameSaveEditor`] is the per-game interface, wrapped in a
//! [`SaveEditorShell`]: the one implementation of the public
//! `tango_gamesupport::SaveEditor` embedding API (in [`shell`], along
//! with the opaque-envelope plumbing). [`view`] holds the shell view
//! (the streamer-mode cover gate around the game's body); [`loaded`]
//! bakes the model into the render-ready [`loaded::OpenSave`] the whole
//! editor reads from.

pub mod loaded;
pub(crate) mod shell;
pub mod view;

pub use shell::SaveEditorShell;

use crate::editor::loaded::OpenSave;
use iced::Element;
use unic_langid::LanguageIdentifier;

pub use crate::editor::view::Action;
pub use tango_gamesupport::{BuildWarnings, OpaqueBuildWarnings};

pub type Save = dyn crate::dataview::save::Save + Send + Sync;
pub type Assets = dyn crate::dataview::rom::Assets + Send + Sync;

pub trait GameSaveEditor: Send + Sync {
    /// Format a save's headless validation findings (see
    /// [`crate::dataview::build::validate`]) without loading editor
    /// state, for the opponent's build-warning advisory. The default is
    /// a game that reports none.
    fn build_warnings(
        &self,
        _save: &Save,
        _assets: &Assets,
        _validation: &crate::dataview::build::Validation,
    ) -> Vec<tango_gamesupport::OpaqueBuildWarnings> {
        vec![]
    }

    /// The save view's body, drawn whenever the streamer-mode cover
    /// isn't. The default is nothing at all: a game that models nothing
    /// behind its save has nothing to show, and an empty "no data" card
    /// would only suggest something is missing.
    fn render<'a>(&self, lang: &'a LanguageIdentifier, loaded: &'a OpenSave) -> Element<'a, Action> {
        let _ = (lang, loaded);
        iced::widget::column![].width(iced::Fill).into()
    }
}

/// The editor of a game that models nothing behind its save (netplay
/// only): nothing to show and nothing editable. Such a game still
/// reaches a session through the same editor path as any other, and
/// streamer mode still covers it.
pub struct EmptyEditor;

/// Editor registered for a game with no editable save model.
pub static EMPTY_SAVE_EDITOR: SaveEditorShell<EmptyEditor> = SaveEditorShell(EmptyEditor);

impl GameSaveEditor for EmptyEditor {}
