//! A player updating tangoAW2 with a pack saved by an older version and
//! the .nds gone from the ROMs folder: the old pack is used (what it has
//! still works), the new sounds are left out, and the app says so
//! (`tango_library::ds_pack_outdated`, the play tab's note).
//! `TANGOAW2_OLD_PACK`; `cargo test -p tango-library --features
//! gamesupport-aw2 --test ds_pack_update_no_nds -- --ignored`.
#![cfg(feature = "gamesupport-aw2")]

mod ds_pack_common;
use ds_pack_common::*;
use tango_gamesupport_aw2::{ds_music, ds_pack};

#[test]
#[ignore]
fn an_older_pack_without_the_nds_is_used_and_said_to_be_old() {
    std::env::remove_var("TANGOAW2_DS_ROM");
    let (old, old_version) = old_pack();
    let dir = folder("pack-update-no-nds");
    std::fs::write(dir.join(PACK), &old).unwrap();

    tango_library::rom::scan_roms(&tango_library::storage::StdStorage, &listing(&dir));

    assert!(ds_pack::pack().is_some(), "the old pack still used");
    assert!(ds_pack::outdated());
    assert!(tango_library::ds_pack_outdated(), "the note shows");
    assert_eq!(version_of(&dir.join(PACK)), old_version, "the old pack left as it was");
    assert!(ds_music::music().is_some(), "its music still plays");
    assert!(ds_music::tag_se(ds_music::TagSe::Thunder).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
