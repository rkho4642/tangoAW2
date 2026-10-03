//! A player updating tangoAW2 with a pack saved by an older version and
//! the .nds still in the ROMs folder: the first scan rebuilds the pack from
//! the .nds and saves it over the old one, with nothing for the player to
//! do. `TANGOAW2_DS_ROM` (the .nds) and `TANGOAW2_OLD_PACK` (an older saved
//! pack); `cargo test -p tango-library --features gamesupport-aw2 --test
//! ds_pack_update -- --ignored`.
#![cfg(feature = "gamesupport-aw2")]

mod ds_pack_common;
use ds_pack_common::*;
use tango_gamesupport_aw2::{ds_music, ds_pack};

#[test]
#[ignore]
fn an_older_pack_is_rebuilt_from_the_nds_in_the_folder() {
    let nds = std::env::var_os("TANGOAW2_DS_ROM").expect("TANGOAW2_DS_ROM");
    // (the pack only from the folder, as the app has it)
    std::env::remove_var("TANGOAW2_DS_ROM");
    let (old, old_version) = old_pack();
    let dir = folder("pack-update");
    std::fs::write(dir.join(PACK), &old).unwrap();
    let rom = dir.join("Advance Wars - Dual Strike (USA).nds");
    if std::fs::hard_link(&nds, &rom).is_err() {
        std::fs::copy(&nds, &rom).unwrap();
    }

    tango_library::rom::scan_roms(&tango_library::storage::StdStorage, &listing(&dir));

    assert!(ds_pack::pack().is_some());
    assert!(!ds_pack::outdated(), "the pack rebuilt from the .nds");
    assert!(!tango_library::ds_pack_outdated());
    let saved = version_of(&dir.join(PACK));
    assert!(
        saved > old_version,
        "saved over the old one: version {old_version} -> {saved}"
    );
    use ds_music::TagSe::*;
    for se in [Thunder, MeterUp, Count, Letter, Burst, Swap] {
        assert!(ds_music::tag_se(se).is_some(), "{se:?}: the tag screens' sound");
    }
    eprintln!(
        "pack version {old_version} -> {saved}; tag sounds: {:?}",
        ds_music::music().unwrap().tag_se
    );
    let _ = std::fs::remove_dir_all(&dir);
}
