//! The loadout strip: the family and save pickers over the App-level
//! [`Selection`], and the messages they emit.
//!
//! The selection policy — what each pick does to the rest of the
//! selection, and what's remembered per family and per save — is
//! [`tango_library::loadout::Selection`]'s, shared with the browser
//! host. What's here is only the iced side: option lists and pickers.

use crate::config;
use crate::i18n::t;
use crate::library::Catalog;
use crate::library::{game, rom};
use crate::ui::widgets;
use iced::{Element, Length};
use tango_library::loadout::Selection;
use unic_langid::LanguageIdentifier;

#[derive(Debug, Clone)]
pub enum Message {
    FamilySelected(FamilyOption),
    SaveSelected(SaveOption),
}

/// Side-effects bubble-up, mirroring the tab modules' convention:
/// pure state mutations happen inside [`update`]; anything
/// that needs App-level collaborators comes back as an `Effect`.
#[derive(Debug, Clone, Copy)]
pub enum Effect {
    /// Selection (family / game / save) changed.
    /// App should rebuild its `LoadedSave` cache, persist config, and
    /// resend lobby settings if one is live.
    SelectionChanged,
}

/// Apply a strip message to the selection.
pub fn update(selection: &mut Selection, msg: Message, scanners: &Catalog, config: &config::Config) -> Option<Effect> {
    match msg {
        Message::FamilySelected(f) => selection.pick_family(f.family, scanners, config),
        Message::SaveSelected(s) => selection.pick_save(s.game, s.path, scanners, config),
    }
    Some(Effect::SelectionChanged)
}

/// Single source of truth for the local side's `protocol::Settings`.
/// App calls this when actually sending settings on the wire; the lobby
/// view calls it as the "You" slot fallback during
/// Connecting/Negotiating (before `lobby.local` has been populated by
/// the netplay loop).
pub fn local_settings(
    selection: &Selection,
    config: &config::Config,
    lobby: &crate::netplay::LobbyState,
) -> tango_net_protocol::control::Settings {
    tango_net_protocol::control::Settings {
        nickname: config.nickname.clone().unwrap_or_default(),
        match_type: (
            lobby.match_type.0,
            lobby.match_type.1 | tango_library::shared_content_flag(),
        ),
        game_info: selection.game_info(),
        blind_setup: lobby.blind_setup,
    }
}

// ---------- Family / Save pick_list options ----------

#[derive(Clone)]
pub struct FamilyOption {
    /// Region-specific gamedb family string (e.g. `"bn3"`).
    pub family: &'static str,
    pub display: String,
    /// `false` unless *every* game in this family has a ROM in the scan
    /// results. Drives sweeten's `.disabled()` closure on the picker so
    /// the row renders greyed out and refuses clicks.
    pub available: bool,
}

impl PartialEq for FamilyOption {
    fn eq(&self, o: &Self) -> bool {
        self.family == o.family
    }
}
impl Eq for FamilyOption {}
impl std::hash::Hash for FamilyOption {
    fn hash<H: std::hash::Hasher>(&self, s: &mut H) {
        self.family.hash(s);
    }
}
impl std::fmt::Display for FamilyOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.display)
    }
}
impl std::fmt::Debug for FamilyOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.display)
    }
}

#[derive(Clone, Debug)]
pub struct SaveOption {
    pub path: std::path::PathBuf,
    /// Pre-computed display label: the save's path relative to the
    /// saves dir, forward-slash separated (so nested folders show up
    /// in the picker), behind the short variant tag when the family has
    /// more than one variant to tell apart. Built when the option list
    /// is constructed because `Display::fmt` gets neither the saves root
    /// nor the language as input.
    pub display: String,
    /// The concrete game this save resolves to *within its family*
    /// (White/Blue picked from the save's own contents). Selecting the
    /// save sets `game` to this.
    pub game: rom::GameRef,
    /// `false` when `game`'s ROM isn't owned — the row greys out and
    /// can't be selected.
    pub available: bool,
}

// Identity is the path: a save is the same option regardless of which
// game/availability the family aggregation tagged it with, so picker
// selection-matching and de-dup stay path-based.
impl PartialEq for SaveOption {
    fn eq(&self, o: &Self) -> bool {
        self.path == o.path
    }
}
impl Eq for SaveOption {}
impl std::hash::Hash for SaveOption {
    fn hash<H: std::hash::Hasher>(&self, s: &mut H) {
        self.path.hash(s);
    }
}

impl SaveOption {
    /// `variant` is the save's short variant tag (e.g. "Blue Moon"),
    /// `None` for families with only one variant — nothing to tell apart
    /// there, so the row stays bare.
    pub fn new(
        saves_path: &std::path::Path,
        path: std::path::PathBuf,
        game: rom::GameRef,
        available: bool,
        variant: Option<&str>,
    ) -> Self {
        let name = path
            .strip_prefix(saves_path)
            .ok()
            .map(|rel| {
                rel.components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .or_else(|| path.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| path.display().to_string());
        // Variant first: the list intermingles a family's variants, and a
        // save's file name seldom says which one it belongs to. Same
        // "<variant> – <name>" shape the new-save template picker uses.
        let display = match variant {
            Some(variant) => format!("{variant} \u{2013} {name}"),
            None => name,
        };
        Self {
            path,
            display,
            game,
            available,
        }
    }
}

impl std::fmt::Display for SaveOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.display)
    }
}

// ---------- Option builders ----------

/// Every supported family — not just the ones we have ROMs for, so
/// users can see what tango knows about. sweeten's `.disabled()` greys
/// out families that don't have every game's ROM owned; available
/// families stable-sort to the top (then own-region first) so the live
/// ones lead.
///
/// What survives both of those is the order the library itself lists
/// the games in, because the sort is stable and that is the order they
/// were collected in. It is the series' own order, which is the one a
/// player already knows them by — alphabetical on the family string
/// sorted BN1..BN6 by accident and would have sorted the next family
/// wherever its letters happened to fall.
pub fn family_options(lang: &LanguageIdentifier, scanners: &Catalog) -> Vec<FamilyOption> {
    let roms = scanners.roms.read();
    let mut families: Vec<&'static str> = Vec::new();
    for g in crate::library::game::GAMES.iter() {
        let fam = g.family_and_variant().0;
        if !families.contains(&fam) {
            families.push(fam);
        }
    }
    let mut family_options: Vec<FamilyOption> = families
        .iter()
        .map(|fam| FamilyOption {
            family: fam,
            display: game::family_display_name(lang, fam),
            available: game::games_in_family(fam).all(|g| roms.contains_key(&g)),
        })
        .collect();
    family_options.sort_by(|a, b| {
        (!a.available).cmp(&(!b.available)).then_with(|| {
            let ar = !game::family_matches_language(lang, a.family);
            let br = !game::family_matches_language(lang, b.family);
            ar.cmp(&br)
        })
    });
    family_options
}

/// Every save across the selected family's color variants, grouped by
/// variant. Each save is tagged with the concrete game it resolves to
/// and whether that game's ROM is owned (so the row can grey out), and —
/// for families with more than one variant — labelled with that
/// variant's short name. A path appears under exactly one variant within
/// a family, but de-dup defensively. The list itself isn't trimmed by
/// the active patch — `save_picker` instead greys out (disables) saves
/// the active patch can't run, so the set stays stable while the
/// patch comes and goes.
pub fn save_options(
    loadout: &Selection,
    lang: &LanguageIdentifier,
    scanners: &Catalog,
    config: &config::Config,
) -> Vec<SaveOption> {
    let saves_path = config.saves_path();
    let roms = scanners.roms.read();
    let saves = scanners.saves.read();
    let mut save_options: Vec<SaveOption> = Vec::new();
    if let Some(family) = loadout.family() {
        // Single-variant families (bn1, bn2, exe45) have nothing to tell
        // apart, so their rows carry no tag.
        let multi_variant = game::games_in_family(family).count() > 1;
        let mut seen: std::collections::HashSet<std::path::PathBuf> = std::collections::HashSet::new();
        for g in game::games_in_family(family) {
            let available = roms.contains_key(&g);
            let variant = multi_variant.then(|| game::variant_short_name(lang, g));
            if let Some(saves_for_game) = saves.get(&g) {
                for s in saves_for_game {
                    if seen.insert(s.path.clone()) {
                        save_options.push(SaveOption::new(
                            &saves_path,
                            s.path.clone(),
                            g,
                            available,
                            variant.as_deref(),
                        ));
                    }
                }
            }
        }
    }
    // One block per variant, folders first — see `save::picker_order`.
    save_options.sort_by(|a, b| crate::library::save::picker_order(&saves_path, (a.game, &a.path), (b.game, &b.path)));
    save_options
}

// ---------- Views ----------

/// The game row for the Play tab's selector strip: the family picker,
/// taking the whole row. No rescan button — scans re-run on their own
/// (tab entry, session close).
pub fn game_row<'a>(
    loadout: &'a Selection,
    lang: &'a LanguageIdentifier,
    scanners: &'a Catalog,
) -> Element<'a, Message> {
    family_picker(loadout, lang, scanners).width(Length::Fill).into()
}

fn family_picker<'a>(
    loadout: &'a Selection,
    lang: &'a LanguageIdentifier,
    scanners: &'a Catalog,
) -> sweeten::widget::PickList<'a, FamilyOption, Vec<FamilyOption>, FamilyOption, Message> {
    let options = family_options(lang, scanners);
    let selected = loadout
        .family()
        .and_then(|fam| options.iter().find(|opt| opt.family == fam).cloned());
    widgets::picker(options, selected, Message::FamilySelected)
        .disabled(|opts: &[FamilyOption]| opts.iter().map(|o| !o.available).collect())
        .placeholder(t!(lang, "play-no-game"))
}

/// The save picker on its own — the Play tab embeds it in its
/// save-action row (next to the rename / delete / new buttons), which
/// is that tab's own furniture.
pub fn save_picker<'a>(
    loadout: &'a Selection,
    lang: &'a LanguageIdentifier,
    scanners: &'a Catalog,
    config: &'a config::Config,
) -> sweeten::widget::PickList<'a, SaveOption, Vec<SaveOption>, SaveOption, Message> {
    let options = save_options(loadout, lang, scanners, config);
    let selected = loadout
        .save()
        .and_then(|p| options.iter().find(|s| s.path == p).cloned());
    // Grey out saves the active patch can't run (alongside saves whose
    // ROM isn't owned) so an incompatible save can't be picked under a
    // patch — switch/clear the patch first. `None` (no patch) disables
    // nothing on this axis.
    let patch_supported = loadout.patch_supported_games(scanners);
    widgets::picker(options, selected, Message::SaveSelected)
        .disabled(move |opts: &[SaveOption]| {
            opts.iter()
                .map(|o| !o.available || patch_supported.as_ref().map(|s| !s.contains(&o.game)).unwrap_or(false))
                .collect()
        })
        .placeholder(t!(lang, "play-no-save"))
}
