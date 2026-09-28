//! The save editor's view layer: its state ([`State`], [`Action`]) and
//! the shell ([`view`]) that hosts a game's editor behind the
//! streamer-mode cover. Nothing here is visible outside the gamesupport
//! layer — the app drives all of it through the shell in
//! [`crate::editor::shell`].

use crate::editor::loaded::OpenSave;
use iced::Element;
use unic_langid::LanguageIdentifier;

pub mod cover;

/// User-driven changes the embedded save view surfaces. The shell folds
/// each into the view's [`State`].
#[derive(Debug, Clone)]
pub enum Action {
    /// Leave the streamer-mode cover and reveal the regular save viewer.
    Review,
}

/// Per-save UI state for [`view`], minted with the save it draws.
#[derive(Clone)]
pub struct State {
    /// Streamer-mode privacy gate. A new loaded save starts covered;
    /// Review reveals the normal viewer.
    pub reviewing: bool,
    /// Entrance restarted when the save arrives (or is shown again) and
    /// when the cover is dismissed.
    pub enter: crate::anim::Enter,
    /// Starting offset for `enter`.
    pub enter_from: iced::Vector,
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    pub fn new() -> Self {
        let mut state = Self {
            reviewing: false,
            enter: crate::anim::Enter::default(),
            enter_from: iced::Vector::new(0.0, 20.0),
        };
        // A state is minted with the save it draws, so its own birth is
        // the save-switch entrance. Clock-reading here rather than taking
        // a `now`, because loading is not part of anyone's animation
        // bookkeeping — it happens mid-update, a frame before the draw
        // this animates.
        state.enter.start(iced::time::Instant::now());
        state
    }

    /// Replay the vertical save-switch entrance for an already-loaded view.
    pub fn restart_entrance(&mut self) {
        self.enter_from = iced::Vector::new(0.0, 20.0);
        self.enter.start(iced::time::Instant::now());
    }

    /// Take where `other` was looking — whether the reader had already
    /// dismissed the streamer-mode cover. Everything else stays this
    /// state's own: the animation belongs to the body coming in.
    pub fn carry_position_from(&mut self, other: &Self) {
        self.reviewing = other.reviewing;
    }

    /// Fold an `Action` into view-local state.
    pub fn apply(&mut self, action: &Action) {
        match action {
            Action::Review => {
                self.reviewing = true;
                self.restart_entrance();
            }
        }
    }
}

/// Wholesale save-view widget: the streamer-mode cover until the reader
/// asks to review the save, the game's own body otherwise. Embedders
/// just call this and `.map` it onto their messages.
pub fn view<'a>(
    lang: &'a LanguageIdentifier,
    loaded: &'a OpenSave,
    state: &'a State,
    streamer_mode: bool,
) -> Element<'a, Action> {
    if streamer_mode && !state.reviewing {
        let now = iced::time::Instant::now();
        let cover = cover::render_cover_gate(lang, loaded, Action::Review);
        return crate::anim::slide_in_opt(cover, state.enter.progress(now), state.enter_from);
    }
    loaded.save_editor.render(lang, loaded)
}
