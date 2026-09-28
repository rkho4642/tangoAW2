//! The shared-console engine: ONE emulated GBA that both peers drive.
//!
//! The Battle Network games link two GBAs over a cable, so the rest of
//! this crate simulates a pair. Turn-based games with a hot-seat versus
//! mode (Advance Wars 2) do not need a cable at all: both players sit at
//! the same console and take turns with its one joypad. Netplay for them
//! is that console, simulated identically on both peers, with each tick's
//! joypad built from both players' rollback-confirmed inputs.
//!
//! How the two inputs become one joypad word is the game's business
//! ([`SharedGame::merge`]): a game that knows whose turn it is lets only
//! that player's buttons through; the default ORs them, which is what
//! two people on one controller amount to.
//!
//! The seam still speaks in pairs (`side(0)`, `side(1)`), so both seats
//! answer with the same console. Presentation calls arrive per seat and
//! are folded: the console renders if either seat wants it rendered.

use std::sync::atomic::AtomicBool;

use mgba_rollback::{LinkOptions, Peripheral, SideOptions};
use num_rational::Ratio;
use tango_match::{HostInput, Side};

use crate::gba::{to_rgba, JOYFLAGS_MASK};

/// Game support for a shared-console title.
pub trait SharedGame: Sync {
    /// The game half of [`Backend::sim_version`](tango_match::Backend::sim_version).
    fn sim_version(&self) -> u16;

    /// The joypad word the console sees this tick, from both players'
    /// sanitized inputs in absolute seat order. Must be a pure function
    /// of `core`'s state and `inputs`, so both peers (and every
    /// re-simulation) build the same word.
    fn merge(&self, core: &mgba::core::Core, inputs: [u32; 2]) -> u32 {
        let _ = core;
        inputs[0] | inputs[1]
    }

    /// Runs before every tick, with the console in hand: the place for a
    /// game's runtime patches (Slippi-style memory writes). `keys` is the
    /// joypad word about to reach the console (after [`merge`](Self::merge));
    /// what comes back is what the console actually sees, so a patch can
    /// take a button for itself. `mode` is the session's match type for a
    /// netplay match or its replay, and for a console played alone
    /// whatever its host picked (`None` for nothing). Must be a pure
    /// function of core state, `mode` and `keys`, like everything else
    /// that touches the simulation: state it keeps lives in the console's
    /// RAM, so snapshots carry it through rollback.
    fn before_tick(&self, core: &mut mgba::core::Core, mode: Option<(u8, u8)>, keys: u32) -> u32 {
        let _ = (core, mode);
        keys
    }

    /// Whether `seat` should be shown nothing right now: a hot-seat game
    /// hides one player's view from the other between turns, and on a
    /// networked shared console nothing else would. Presentation only,
    /// read after the tick, so it never touches the simulation.
    fn conceal(&self, core: &mgba::core::Core, seat: usize) -> bool {
        let _ = (core, seat);
        false
    }

    /// Draw game-specific help onto `seat`'s picture (RGBA8, 240x160):
    /// labels a patched mode shows that the cartridge cannot draw itself.
    /// Presentation only, read after the tick, like
    /// [`conceal`](Self::conceal); `mode` as for
    /// [`before_tick`](Self::before_tick).
    fn overlay(&self, core: &mgba::core::Core, mode: Option<(u8, u8)>, seat: usize, rgba: &mut [u8]) {
        let _ = (core, mode, seat, rgba);
    }

    /// Traps installed on the console at boot. Like the pair engine's
    /// primer traps they must be pure functions of emulation state.
    fn traps(&self) -> Vec<(u32, Box<dyn Fn(&mut mgba::core::Core)>)> {
        Vec::new()
    }

    /// Ticks to run with an idle pad right after power-on, before the
    /// session takes over (skipping boot logos costs nobody anything).
    fn boot_ticks(&self) -> u32 {
        0
    }
}

struct SharedSnapshot {
    snap: mgba_rollback::Snapshot,
}

/// One console, both seats.
pub struct SharedLink {
    inner: mgba_rollback::Link,
    game: &'static (dyn SharedGame + Send + Sync),
    mode: Option<(u8, u8)>,
    /// What each seat last asked for; the console renders if either does.
    render: [bool; 2],
}

impl SharedLink {
    pub fn boot(
        rom: &[u8],
        save: Option<&[u8]>,
        rtc: Option<std::time::SystemTime>,
        game: &'static (dyn SharedGame + Send + Sync),
        mode: Option<(u8, u8)>,
        cancel: Option<&AtomicBool>,
    ) -> Result<Self, crate::Error> {
        let mut inner = mgba_rollback::Link::with_options(LinkOptions {
            sides: vec![SideOptions {
                rom: rom.to_vec(),
                save: save.map(<[u8]>::to_vec),
            }],
            rtc,
            peripheral: Peripheral::Cable,
        })?;
        inner.set_traps(0, game.traps());
        for _ in 0..game.boot_ticks() {
            if cancel.is_some_and(|c| c.load(std::sync::atomic::Ordering::Relaxed)) {
                return Err(crate::Error::Cancelled);
            }
            let word = game.before_tick(inner.core_mut(0), mode, 0);
            inner.try_tick(&[word])?;
        }
        let core = inner.core_mut(0);
        core.set_audio_buffer_size(16384);
        core.audio_buffer().clear();
        Ok(SharedLink {
            inner,
            game,
            mode,
            render: [true, true],
        })
    }

    /// The console, for tests and tools that read its memory.
    pub fn core(&self) -> &mgba::core::Core {
        self.inner.core(0)
    }

    /// The console, for tools that poke its memory (never the simulation's
    /// own path: that goes through [`SharedGame::before_tick`]).
    pub fn core_mut(&mut self) -> &mut mgba::core::Core {
        self.inner.core_mut(0)
    }

    fn apply_render(&mut self) {
        let on = self.render[0] || self.render[1];
        self.inner.set_frameskip(0, if on { 0 } else { i32::MAX });
    }
}

impl tango_match::Link for SharedLink {
    fn sanitize(&self, input: HostInput) -> HostInput {
        HostInput::keys(input.keys & JOYFLAGS_MASK)
    }

    fn tick(&mut self, inputs: [HostInput; 2]) {
        let keys = inputs.map(|i| i.keys & JOYFLAGS_MASK);
        let word = self.game.merge(self.inner.core(0), keys) & JOYFLAGS_MASK;
        let word = self.game.before_tick(self.inner.core_mut(0), self.mode, word) & JOYFLAGS_MASK;
        self.inner.tick(&[word]);
    }

    fn snapshot(
        &mut self,
        _recycled: Option<tango_match::Snapshot>,
    ) -> Result<tango_match::Snapshot, tango_match::Error> {
        let snap = self.inner.save().map_err(crate::Error::from)?;
        Ok(Box::new(SharedSnapshot { snap }))
    }

    fn restore(&mut self, snapshot: &tango_match::Snapshot) -> Result<(), tango_match::Error> {
        let snapshot = snapshot
            .downcast_ref::<SharedSnapshot>()
            .expect("a shared link can only restore its own snapshots");
        self.inner.load(&snapshot.snap).map_err(crate::Error::from)?;
        Ok(())
    }

    fn side(&mut self, player: usize) -> Box<dyn Side + '_> {
        Box::new(SharedSide { link: self, player })
    }

    fn peek(&mut self, addr: u32, buf: &mut [u8]) -> bool {
        self.inner.core(0).raw_read_range(addr, -1, buf);
        true
    }
}

struct SharedSide<'a> {
    link: &'a mut SharedLink,
    player: usize,
}

impl Side for SharedSide<'_> {
    fn frame(&mut self) -> Option<Vec<u8>> {
        let mut frame = self.link.inner.video_buffer(0).map(to_rgba)?;
        let core = self.link.inner.core(0);
        if self.link.game.conceal(core, self.player) {
            return Some(concealed(&frame));
        }
        self.link.game.overlay(core, self.link.mode, self.player, &mut frame);
        Some(frame)
    }

    fn set_render(&mut self, on: bool) {
        self.link.render[self.player] = on;
        self.link.apply_render();
    }

    fn export_save(&mut self) -> Option<Vec<u8>> {
        self.link.inner.export_save(0)
    }

    fn audio_sample_rate(&mut self) -> Ratio<u32> {
        Ratio::from_integer(self.link.inner.core(0).audio_sample_rate())
    }

    fn drain_audio(&mut self, out: &mut [i16]) -> usize {
        let buf = self.link.inner.core_mut(0).audio_buffer();
        let available = buf.available();
        buf.read(out, (out.len() / 2).min(available));
        available
    }
}

/// What a concealed seat sees: the picture blacked out except a faint
/// band, so it reads as "the other player is moving" rather than a crash.
fn concealed(frame: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; frame.len()];
    for (i, px) in out.chunks_mut(4).enumerate() {
        let y = i / 240;
        let v = if (76..84).contains(&y) { 40 } else { 8 };
        px.copy_from_slice(&[v, v, v + 12, 255]);
    }
    out
}

/// A shared-console game as the engine-neutral backend its registration
/// holds.
pub struct SharedBackend {
    game: &'static (dyn SharedGame + Send + Sync),
}

impl SharedBackend {
    pub const fn new(game: &'static (dyn SharedGame + Send + Sync)) -> Self {
        SharedBackend { game }
    }
}

/// Engine half of the version: shared-console engine revisions.
const SHARED_SIM_VERSION: u16 = 0x5a00;

impl tango_match::Backend for SharedBackend {
    fn sim_version(&self) -> u32 {
        ((SHARED_SIM_VERSION as u32) << 16) | self.game.sim_version() as u32
    }

    fn screen_layout(&self, _mode: tango_match::SessionMode) -> tango_match::ScreenLayout {
        crate::gba::screen_layout()
    }

    fn keys_mask(&self) -> u32 {
        JOYFLAGS_MASK
    }

    fn tps(&self) -> Ratio<u32> {
        crate::gba::TPS
    }

    /// Both peers boot the same console from seat 0's ROM and save (the
    /// host's), so the match is one machine, not two.
    fn start(&self, config: tango_match::StartConfig) -> Result<tango_match::Match, tango_match::Error> {
        let link = SharedLink::boot(
            config.roms[0],
            config.saves[0],
            Some(config.rtc),
            self.game,
            Some(config.match_type),
            config.cancel,
        )?;
        tango_match::Match::new(link, config.local_player, config.present_delay, config.audio)
    }

    /// Played alone, the console still gets the game's runtime patches.
    fn start_solo(&self, config: tango_match::SoloConfig) -> Result<tango_match::Solo, tango_match::Error> {
        let link = SharedLink::boot(config.rom, config.save, config.rtc, self.game, config.match_type, None)?;
        Ok(tango_match::Solo::new(SharedSolo(link), config.audio))
    }

    fn open_replay(&self, config: tango_match::ReplayConfig) -> Result<tango_match::ReplaySet, tango_match::Error> {
        let boot = SharedBoot {
            rom: config.roms[0].clone(),
            save: config.saves[0].clone(),
            rtc: config.rtc,
            game: self.game,
            mode: config.match_type,
        };
        Ok(tango_match::ReplaySet::new(&config, boot))
    }
}

struct SharedBoot {
    rom: Vec<u8>,
    save: Vec<u8>,
    rtc: std::time::SystemTime,
    game: &'static (dyn SharedGame + Send + Sync),
    mode: (u8, u8),
}

impl SharedBoot {
    fn link(&self, cancel: Option<&AtomicBool>) -> Result<tango_match::BootedReplay, tango_match::Error> {
        let save = (!self.save.is_empty()).then_some(self.save.as_slice());
        let link = SharedLink::boot(&self.rom, save, Some(self.rtc), self.game, Some(self.mode), cancel)?;
        Ok(tango_match::BootedReplay {
            link: Box::new(link),
            telemetry: None,
        })
    }
}

impl tango_match::ReplayBoot for SharedBoot {
    fn boot(&self, _observe: bool, cancel: &AtomicBool) -> Result<tango_match::BootedReplay, tango_match::Error> {
        self.link(Some(cancel))
    }

    fn boot_unprimed(&self, _observe: bool) -> Result<tango_match::BootedReplay, tango_match::Error> {
        self.link(None)
    }
}

/// The shared console played by one person: the same console, the same
/// runtime patches, one pad.
struct SharedSolo(SharedLink);

impl tango_match::Console for SharedSolo {
    fn tick(&mut self, input: HostInput) -> Result<(), tango_match::Error> {
        let link = &mut self.0;
        let word = link
            .game
            .before_tick(link.inner.core_mut(0), link.mode, input.keys & JOYFLAGS_MASK)
            & JOYFLAGS_MASK;
        link.inner
            .try_tick(&[word])
            .map_err(|e| tango_match::Error::Backend(Box::new(crate::Error::from(e))))?;
        Ok(())
    }

    fn side(&mut self) -> Box<dyn Side + '_> {
        Box::new(SharedSide {
            link: &mut self.0,
            player: 0,
        })
    }
}
