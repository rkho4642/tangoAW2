//! Headless save preparation and session snapshots.

pub use tango_gamesupport::GameRef;

pub use tango_gamesupport::AppliedPatch;

/// A committed game + save, with the assets derived from the pair.
pub struct SaveModel {
    pub game: GameRef,
    pub save_path: std::path::PathBuf,
    pub save: Box<dyn crate::save::Save + Send + Sync>,
    /// Patch+version baked into this SaveModel, if any. `None` = raw ROM.
    pub patch: Option<AppliedPatch>,
    pub assets: Box<dyn crate::rom::Assets + Send + Sync>,
}

/// Prepare the save/ROM pair before validation and presentation. The ROM is
/// already patched; this derives its assets (with the patch's charset
/// override, if any).
pub fn prepare(
    game: GameRef,
    rom: &[u8],
    save_path: std::path::PathBuf,
    save: tango_gamesupport::BoxedSave,
    applied_patch: Option<AppliedPatch>,
) -> tango_gamesupport::PreparedSave {
    let save = crate::unwrap_save(save);
    let wram = save.as_raw_wram().into_owned();
    let charset_owned: Option<Vec<&str>> = applied_patch
        .as_ref()
        .and_then(|p| p.rom_overrides.charset.as_ref())
        .map(|c| c.iter().map(|s| s.as_str()).collect());
    // A netplay-only game has no ROM assets behind its save — bake from
    // empty ones, and the editor shell renders its empty state.
    let assets: Box<dyn crate::rom::Assets + Send + Sync> =
        match game.load_rom_assets(rom, &wram, charset_owned.as_deref()) {
            Some(assets) => crate::unwrap_assets(assets),
            None => Box::new(crate::rom::EmptyAssets),
        };

    tango_gamesupport::PreparedSave {
        game,
        save_path,
        patch: applied_patch,
        save: crate::wrap_save(save),
        assets: crate::wrap_assets(assets),
    }
}

/// Convert the concrete prepared envelope into the editor's mutable model.
pub fn from_prepared(prepared: tango_gamesupport::PreparedSave) -> SaveModel {
    SaveModel {
        game: prepared.game,
        save_path: prepared.save_path,
        save: crate::unwrap_save(prepared.save),
        patch: prepared.patch,
        assets: crate::unwrap_assets(prepared.assets),
    }
}

/// Serialize a save for a session without mutating the editor's staged copy.
///
/// Edits are applied to that copy as they happen, while its checksum is only
/// rebuilt when the user saves. A match can start before then, so session
/// snapshots must repair a clone rather than serializing the checksum-stale
/// editor value (or implicitly committing it to disk).
pub fn session_sram(save: &(dyn crate::save::Save + Send + Sync)) -> Vec<u8> {
    let mut snapshot = save.clone_box();
    snapshot.rebuild_checksum();
    snapshot.to_sram_dump()
}

#[cfg(test)]
mod tests {
    use super::session_sram;
    use crate::save::Save as _;
    use std::borrow::Cow;

    #[derive(Clone)]
    struct TestSave {
        value: u8,
        checksum: u8,
    }

    impl crate::save::Save for TestSave {
        fn to_sram_dump(&self) -> Vec<u8> {
            vec![self.value, self.checksum]
        }

        fn as_raw_wram(&self) -> Cow<'_, [u8]> {
            Cow::Owned(self.to_sram_dump())
        }

        fn rebuild_checksum(&mut self) {
            self.checksum = self.value;
        }
    }

    #[test]
    fn session_sram_repairs_a_clone_without_committing_the_editor_copy() {
        let staged = TestSave {
            value: 0x42,
            checksum: 0x11,
        };

        assert_eq!(session_sram(&staged), vec![0x42, 0x42]);
        assert_eq!(staged.to_sram_dump(), vec![0x42, 0x11]);
    }
}
