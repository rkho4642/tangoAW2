//! Headless save validation: a game's own typed findings, without display
//! state. Frontends adapt them separately.

use crate::{rom, save};

/// A game's typed findings for one save, if it reported any.
#[derive(Default)]
pub struct Validation {
    pub game: Option<Box<dyn std::any::Any + Send + Sync>>,
}
impl tango_gamesupport::Validation for Validation {
    fn is_empty(&self) -> bool {
        self.game.is_none()
    }
}
impl Validation {
    pub fn game<T: 'static>(&self) -> Option<&T> {
        self.game.as_ref()?.downcast_ref()
    }
}
pub fn validate(save: &dyn save::Save, assets: &dyn rom::Assets) -> Validation {
    Validation {
        game: save.game_violations(assets),
    }
}
