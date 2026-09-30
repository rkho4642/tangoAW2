//! Advance Wars 2: Black Hole Rising (USA, `AW2E`).
//!
//! Netplay runs the game's own hot-seat Versus mode on one console that
//! both peers simulate (see `tango_backend_mgba::shared`). The save is
//! the cartridge's 64 KiB Flash, carried as an opaque image: seat 0's
//! save boots the shared console, so the host's unlocks are the match's.

#[cfg(feature = "ui")]
pub mod ui {
    pub use tango_gamesupport_common_ui::editor::EMPTY_SAVE_EDITOR as SAVE_EDITOR;
}

pub mod branding;
pub mod design;
pub mod design5;
pub mod design_bar;
pub mod ds_art;
pub mod ds_battle;
pub mod ds_co_art;
pub mod ds_pack;
pub mod ds_power_art;
pub mod ds_unit_art;
pub mod ds_unit_pictures;
pub mod ds_units;
pub mod ds_weather;
pub mod sandstorm;
pub mod wasteland;
pub mod com_tower;
pub mod co_grid;
pub mod co_new;
pub mod cpu_tactics;
pub mod co_powers;
pub mod co_roster;
pub mod heal_effect;
pub mod lz77;
pub mod map_anim;
pub mod roster;
pub mod unit_names;
pub mod unit_actions;
pub mod factory;
pub mod five;
mod five_art;
mod five_map;
mod five_map_data;
mod five_patches;
pub mod invention_art;
pub mod obelisk;
mod obelisk_art;
pub mod power_anim;
pub mod pvp;
mod volcano;

use std::sync::LazyLock;
use tango_gamesupport::{Family, Game, Region, SaveTemplates};

/// The cartridge's Flash chip: 512 Kbit.
pub const SAVE_SIZE: usize = 0x10000;

/// The Flash image, opaque: AW2 checks its own save and there is no
/// editor, so nothing here interprets it.
#[derive(Clone)]
pub struct Save(Vec<u8>);

impl Save {
    pub fn new(data: &[u8]) -> Result<Self, tango_gamesupport_common_dataview::save::Error> {
        if data.len() != SAVE_SIZE {
            return Err(tango_gamesupport_common_dataview::save::Error::InvalidSize(data.len()));
        }
        Ok(Save(data.to_vec()))
    }
}

impl tango_gamesupport_common_dataview::save::Save for Save {
    fn to_sram_dump(&self) -> Vec<u8> {
        self.0.clone()
    }
    fn as_raw_wram(&self) -> std::borrow::Cow<'_, [u8]> {
        self.0.as_slice().into()
    }
    fn rebuild_checksum(&mut self) {}
}

/// A fresh cartridge. Everything unlocks at runtime (see `pvp`), so no
/// completed save is needed.
static AW2_T: SaveTemplates = LazyLock::new(|| {
    vec![(
        "",
        tango_gamesupport_common_dataview::wrap_save(Box::new(Save(vec![0xff; SAVE_SIZE]))),
    )]
});

static ENGINE: tango_backend_mgba::SharedBackend = tango_backend_mgba::SharedBackend::new(&pvp::AW2E);

pub static AW2: Game = Game {
    family: &AW2_FAMILY,
    variant: 0,
    rom_code: b"AW2E",
    revision: 0x00,
    crc32: 0x5ad0e571,
    rom_size: 0x800000,
    region: Region::US,
    parse_save_fn: |data| Ok(tango_gamesupport_common_dataview::wrap_save(Box::new(Save::new(data)?))),
    load_rom_assets_fn: None,
    pvp: &ENGINE,
    save_templates: Some(&AW2_T),
    logo_image: None,
};

pub static AW2_FAMILY: Family = Family {
    id: "aw2",
    games: &[&AW2],
    match_types: pvp::MATCH_TYPES,
    players_colored_by_seat: true,
    translations: &[("en-US", include_str!("../locales/en-US/aw2.ftl"))],
};

pub static FAMILIES: &[&Family] = &[&AW2_FAMILY];
