//! What sits behind the tabs, welcome and results screens: the player's
//! own background image when they picked one in Settings, otherwise the
//! drawn backdrop ([`crate::ui::widgets::map_backdrop`]). The image is
//! read from where it lies on disk; tangoAW2 never copies or shares it.

use iced::widget::image::Handle;
use iced::{Element, Length};

/// The picked image, if there is one and it is still on disk. A file
/// that has gone falls back to the drawn backdrop rather than a blank.
pub fn load(path: Option<&std::path::Path>) -> Option<Handle> {
    let path = path?;
    if !path.is_file() {
        log::warn!(
            "background image {} not found; using the drawn backdrop",
            path.display()
        );
        return None;
    }
    Some(Handle::from_path(path))
}

/// The backdrop: `image` filling the window (cropped to cover it),
/// or the drawn one.
pub fn view<'a, M: 'a>(image: Option<&Handle>) -> Element<'a, M> {
    match image {
        Some(handle) => iced::widget::image(handle.clone())
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(iced::ContentFit::Cover)
            .into(),
        None => crate::ui::widgets::map_backdrop(),
    }
}
