//! Baking an [`OpenSave`]: every image handle the save view draws from,
//! derived once per game+save so the per-frame `view()` only clones
//! handles.

use iced::widget::image as iced_image;

/// A loaded save's model plus this frontend's baked art for it.
pub struct OpenSave {
    /// Game, parsed save, assets, applied patch. Reachable directly
    /// through `Deref`, so `loaded.save` works.
    pub model: crate::model::SaveModel,
    /// The game's own save-editor UI. Resolved by the app's per-family
    /// registry at construction (the registry owns the feature gates).
    pub save_editor: &'static dyn crate::editor::GameSaveEditor,
    /// Logos for the streamer-mode Cover, as `(width, height, handle)` —
    /// one per variant in the loaded game's family that has one, in the
    /// family's own order (so a family with two logos fans both out, the
    /// same way round whichever one is loaded). Built once here so the
    /// per-frame view() just clones the handles.
    pub logos: Vec<(u32, u32, iced_image::Handle)>,
}

/// Reopen the private model/art bundle carried by the public loaded-save
/// envelope. Only the editor consumes it; save validation happens against the
/// prepared save before this bundle exists.
pub(crate) fn open(data: &tango_gamesupport::LoadedSave) -> &OpenSave {
    (&*data.payload as &dyn std::any::Any)
        .downcast_ref::<OpenSave>()
        .expect("LoadedSave payload must be this crate's OpenSave")
}

impl std::ops::Deref for OpenSave {
    type Target = crate::model::SaveModel;
    fn deref(&self) -> &Self::Target {
        &self.model
    }
}

impl std::ops::DerefMut for OpenSave {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.model
    }
}

/// Bake every image handle the save view draws from, once per
/// game+save, so the per-frame `view()` only clones handles.
pub fn from_model(model: crate::model::SaveModel, save_editor: &'static dyn crate::editor::GameSaveEditor) -> OpenSave {
    // Logos for the streamer-mode Cover: every variant in this game's family, in
    // the family's own order. The per-game `LazyImage` caches the PNG
    // decode; `to_rgba8` + `from_rgba` run once here so the per-frame
    // view() just clones handles.
    let logos: Vec<(u32, u32, iced_image::Handle)> = model
        .game
        .family
        .games
        .iter()
        // A game with no logo (netplay-only) simply contributes none.
        .filter_map(|gi| {
            let img = gi.logo_image?.to_rgba8();
            let (w, h) = img.dimensions();
            Some((w, h, iced_image::Handle::from_rgba(w, h, img.into_raw())))
        })
        .collect();

    OpenSave {
        model,
        save_editor,
        logos,
    }
}
