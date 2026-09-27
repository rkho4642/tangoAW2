//! A real two-peer Advance Wars 2 match over the actual network stack on
//! localhost: peer A runs `/host <port>`, peer B runs
//! `/connect 127.0.0.1:<port>`, both through `tango_lobby` exactly as the
//! desktop drives it (settings exchange with the AW2 game info, the
//! library's compatibility facts, match type, commit/reveal of the save,
//! StartMatch, handoff into `PreMatchData`). Each peer then builds its
//! `PvpSession` from the handoff the way the desktop's `spawn_pvp` does,
//! and the two sessions are driven at 60 Hz with the scripted per-seat
//! inputs from `aw2_rollback_sim` (power-on into a Versus match on Bean
//! Island, a turn each), then ~2 s idle. Both peers' final presented
//! frames must be identical.
//!
//! Usage: aw2_direct_netplay <rom> <out-dir> [port]
//! Set `AW2_LOG=info` (or debug) for the stack's own logs.

use futures::StreamExt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tango_library::storage::StdStorage;
use tango_lobby::{compat::Verdict, Event, Phase};
use tango_session::pvp::{PvpBoot, PvpSession, PvpSessionArgs, Seat};
use tango_session::{Drive as _, HostInput, Session as _};

const A: u32 = 1;
const SELECT: u32 = 1 << 2;
const START: u32 = 1 << 3;
const RIGHT: u32 = 1 << 4;
const UP: u32 = 1 << 6;
const DOWN: u32 = 1 << 7;

/// Idle ticks after the script, so both peers settle (~2 s).
const IDLE_TICKS: usize = 120;

// ---------- input script (same as tango-gamesupport-aw2's aw2_rollback_sim) ----------

struct Script {
    keys: [Vec<u32>; 2],
    t: usize,
    marks: Vec<(usize, &'static str)>,
}

impl Script {
    fn at(&mut self, seat: usize, t: usize, bits: u32) {
        for k in &mut self.keys {
            if k.len() <= t {
                k.resize(t + 1, 0);
            }
        }
        self.keys[seat][t] = bits;
    }
    fn wait(&mut self, n: usize) {
        self.t += n;
    }
    fn press(&mut self, seat: usize, bits: u32) {
        for i in 0..8 {
            self.at(seat, self.t + i, bits);
        }
        self.t += 14;
    }
    fn mark(&mut self, name: &'static str) {
        self.marks.push((self.t, name));
    }
    fn noise(&mut self, seat: usize, n: usize, seed: &mut u32) {
        let buttons = [A, 1 << 1, START, SELECT, RIGHT, 1 << 5, UP, DOWN, 1 << 8, 1 << 9];
        let mut i = 0;
        while i < n {
            *seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let b = buttons[(*seed >> 16) as usize % buttons.len()];
            for j in 0..4 {
                self.at(seat, self.t + i + j, b);
            }
            i += 11;
        }
    }
}

fn build_script() -> Script {
    let mut s = Script {
        keys: [vec![], vec![]],
        t: 0,
        marks: vec![],
    };
    let mut seed = 7u32;
    s.wait(700);
    s.press(0, START);
    s.wait(300);
    s.press(0, A);
    s.wait(150);
    s.press(0, DOWN);
    s.wait(60);
    s.press(0, A); // Versus
    s.wait(150);
    s.press(0, A); // New
    s.wait(150);
    s.press(0, A); // Bean Island
    s.wait(150);
    s.mark("teams");
    for b in [RIGHT, RIGHT, RIGHT, UP] {
        s.press(0, b); // slot 2 to "2P"
        s.wait(20);
    }
    s.press(0, A);
    s.wait(100);
    s.press(0, A); // rules accepted
    s.wait(400);
    s.mark("p1_turn");
    s.noise(1, 300, &mut seed);
    s.wait(300);
    s.press(0, A); // map menu on the HQ
    s.wait(40);
    s.press(0, UP);
    s.wait(30);
    s.press(0, A); // End
    s.wait(100);
    s.mark("next_turn");
    s.wait(300);
    s.press(1, A);
    s.wait(400);
    s.mark("p2_turn");
    s.noise(0, 300, &mut seed);
    s.wait(300);
    s.press(1, A);
    s.wait(40);
    s.press(1, UP);
    s.wait(30);
    s.press(1, A); // End
    s.wait(400);
    s.press(0, A);
    s.wait(400);
    s.mark("p1_day2");
    // Beyond aw2_rollback_sim: player 1 ends day 2 too, so the match
    // settles on the "Next turn" screen, which both seats are shown. On a
    // fog-of-war turn the waiting seat's picture is concealed (see
    // `SharedGame::conceal`), so a mid-turn frame differs between peers
    // by design and can't be the determinism check.
    s.press(0, A); // map menu on the HQ
    s.wait(40);
    s.press(0, UP);
    s.wait(30);
    s.press(0, A); // End
    s.wait(100);
    s.mark("next_turn_2");
    let total = s.t + 60 + IDLE_TICKS;
    for k in &mut s.keys {
        k.resize(total, 0);
    }
    s
}

fn write_bmp(path: &std::path::Path, rgba: &[u8]) {
    let (w, h) = (240u32, 160u32);
    assert_eq!(rgba.len(), (w * h * 4) as usize, "unexpected frame size");
    let mut out = Vec::new();
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(54 + w * h * 3).to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0, 54, 0, 0, 0, 40, 0, 0, 0]);
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(-(h as i32)).to_le_bytes());
    out.extend_from_slice(&[1, 0, 24, 0]);
    out.extend_from_slice(&[0u8; 24]);
    for px in rgba.chunks(4) {
        out.extend_from_slice(&[px[2], px[1], px[0]]);
    }
    std::fs::write(path, out).unwrap();
}

// ---------- a minimal stderr logger (AW2_LOG=error|warn|info|debug) ----------

struct Logger;
static LOGGER: Logger = Logger;
impl log::Log for Logger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::max_level()
    }
    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) {
            eprintln!("[{:5} {}] {}", r.level(), r.target(), r.args());
        }
    }
    fn flush(&self) {}
}

// ---------- the library, as the desktop resolves it ----------

struct Library {
    catalog: tango_library::Catalog,
    config: tango_library::config::Config,
    selection: tango_library::loadout::Selection,
    /// The save the selection resolved to, as bytes: what Ready commits.
    sram: Vec<u8>,
}

async fn open_library(rom: &std::path::Path, dir: &std::path::Path) -> Result<Library, String> {
    std::fs::create_dir_all(dir.join("roms")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir.join("saves")).map_err(|e| e.to_string())?;
    std::fs::copy(rom, dir.join("roms/Advance_wars_2.gba")).map_err(|e| format!("copy rom: {e}"))?;
    std::fs::write(
        dir.join("saves/aw2-blank.sav"),
        vec![0xffu8; tango_gamesupport_aw2::SAVE_SIZE],
    )
    .map_err(|e| e.to_string())?;

    let config = tango_library::config::Config::with_data_path(dir.to_path_buf());
    let catalog = tango_library::Catalog::new();
    let listings = tango_library::Catalog::list(&StdStorage, &config).await;
    catalog.rescan_library(&StdStorage, &config, &listings);

    let game = tango_library::game::find_by_family_and_variant("aw2", 0).ok_or("aw2 is not registered")?;
    if !catalog.roms.read().contains_key(&game) {
        return Err("the library scan did not recognize the AW2 ROM".into());
    }
    let mut selection = tango_library::loadout::Selection::default();
    selection.pick_game(game, &catalog, &config);
    let save = selection.save().ok_or("the library scan found no AW2 save")?.to_owned();
    let sram = catalog
        .resolver(&StdStorage, &config)
        .resolve(&selection, None)
        .map_err(|e| format!("resolve selection: {e}"))?
        .sram;
    println!(
        "library: {:?} v{} (sim_version {}), save {} ({} bytes)",
        game.family.id,
        game.variant,
        game.pvp.sim_version(),
        save.display(),
        sram.len()
    );
    Ok(Library {
        catalog,
        config,
        selection,
        sram,
    })
}

// ---------- one peer's lobby, as tango/src/app/{play,lobby}.rs drives it ----------

type Built = (PvpSession, PvpBoot, tango_session::audio::Stream);

async fn run_lobby(name: &'static str, command: String, lib: &Library, match_type: (u8, u8)) -> Result<Built, String> {
    let ident = tango_lobby::LinkIdent::parse(&command).ok_or_else(|| format!("{name}: bad command {command}"))?;
    let tango_lobby::LinkIdent::Direct(role) = ident else {
        return Err(format!("{name}: not a direct command"));
    };
    let mut state = tango_lobby::State::new();
    let (cancel, progress) = state.begin_direct(&role);
    tokio::spawn(tango_lobby::connect_direct(role, cancel, progress));
    // E::Connect: default match type for the family; then E::SetMatchType.
    let game = lib.selection.game().unwrap();
    let family = game.family_and_variant().0;
    state.apply_default_match_type(family, game.family.match_types, None);
    state.pick_match_type(Some(family), match_type);
    let mut incoming = state.take_incoming().ok_or("no progress channel")?;

    let facts = |l: &tango_net_protocol::control::Settings, r: &tango_net_protocol::control::Settings| {
        lib.catalog.compatibility_facts(l, r)
    };
    let local_settings = |lobby: &tango_lobby::LobbyState| tango_net_protocol::control::Settings {
        nickname: name.to_owned(),
        match_type: lobby.match_type,
        game_info: lib.selection.game_info(),
        blind_setup: lobby.blind_setup,
    };

    let deadline = Instant::now() + Duration::from_secs(30);
    let mut last_phase = String::new();
    let mut last_verdict: Option<Verdict> = None;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let next = match tokio::time::timeout(remaining, incoming.next()).await {
            Ok(Some(next)) => next,
            Ok(None) => return Err(format!("{name}: lobby progress channel closed in {last_phase}")),
            Err(_) => {
                return Err(format!(
                    "{name}: lobby timed out after 30 s in phase {last_phase}, verdict {last_verdict:?}, ready {:?}",
                    state.ready_view()
                ))
            }
        };
        let mut event = state.apply(next);
        let phase = format!("{:?}", state.phase);
        if phase != last_phase {
            println!("{name}: phase {phase}");
            last_phase = phase;
        }
        if let Phase::Failed { error } = &state.phase {
            return Err(format!("{name}: lobby failed: {error} ({error:?})"));
        }
        // App::resend_settings_if_lobby after every report.
        if let Some((patch, version)) = state.reconcile(local_settings, facts) {
            return Err(format!("{name}: lobby wants patch {patch} {version}"));
        }
        let verdict = state.verdict(facts);
        if verdict != last_verdict {
            println!(
                "{name}: verdict {verdict:?} (local {:?}, remote {:?})",
                state.lobby.local.as_ref().map(|s| (&s.game_info, s.match_type)),
                state.lobby.remote.as_ref().map(|s| (&s.game_info, s.match_type)),
            );
            last_verdict = verdict.clone();
        }
        // The Ready button is primary only on a Compatible verdict.
        if verdict == Some(Verdict::Compatible) && !state.local_ready() {
            println!("{name}: ready (committing {} save bytes)", lib.sram.len());
            if let Some(e) = state.commit(lib.sram.clone()) {
                event = Some(e);
            }
        }
        if let Some(Event::MatchReady) = event {
            let (ticket, pre_match) = state
                .take_pre_match()
                .ok_or_else(|| format!("{name}: MatchReady but take_pre_match returned None"))?;
            let t = &pre_match.terms;
            println!(
                "{name}: handoff: offerer {}, match_type {:?}, match_ts {}, saves {}/{} bytes, remote nickname {:?}",
                t.is_offerer,
                t.match_type,
                t.match_ts,
                t.local_save_data.len(),
                t.remote_save_data.len(),
                t.remote_settings.nickname
            );
            let built = build_session(lib, pre_match).await.map_err(|e| e.to_string());
            let err = built.as_ref().err().cloned();
            return state
                .complete_handoff(ticket, built)
                .ok_or_else(|| format!("{name}: session build failed: {}", err.unwrap_or_default()));
        }
    }
}

/// tango/src/session/launch.rs `spawn_pvp`, minus the UI panes, replay
/// recording and stats cache.
async fn build_session(lib: &Library, pre_match: tango_session::pvp::PreMatchData) -> Result<Built, String> {
    let prepared = lib
        .catalog
        .resolver(&StdStorage, &lib.config)
        .prepare_match(
            &pre_match.terms.local_settings,
            &pre_match.terms.remote_settings,
            [&pre_match.terms.local_save_data, &pre_match.terms.remote_save_data],
        )
        .map_err(|e| format!("prepare_match: {e}"))?;
    let local = Seat {
        game: prepared.local.prepared.game,
        rom: prepared.local.rom.clone(),
        sram: prepared.local.match_sram(),
    };
    let remote = Seat {
        game: prepared.remote.prepared.game,
        rom: prepared.remote.rom.clone(),
        sram: prepared.remote.match_sram(),
    };
    PvpSession::new(PvpSessionArgs {
        local,
        remote,
        pre_match,
        frame_delay: tango_session::pvp::DEFAULT_FRAME_DELAY,
        disable_bgm: false,
        replays: None,
        stats_sink: None,
        sample_rate: 48_000,
    })
    .await
    .map_err(|e| format!("PvpSession::new: {e}"))
}

// ---------- driving a session, as the desktop's drive thread does ----------

#[derive(Default)]
struct DriveReport {
    seat: usize,
    advances: usize,
    boot_time: Duration,
    peer_wait: Duration,
    run_time: Duration,
    rollbacks: u32,
    deepest: u32,
    max_lead: i32,
    min_skew: i32,
    max_skew: i32,
    tps: f32,
    fps_target: f32,
    latency: Option<Duration>,
    reconnect_seen: bool,
    marks: Vec<(&'static str, Vec<u8>)>,
    final_frame: Vec<u8>,
}

fn drive(
    name: &'static str,
    session: &PvpSession,
    boot: &mut PvpBoot,
    keys: &[Vec<u32>; 2],
    marks: &[(usize, &'static str)],
) -> Result<DriveReport, String> {
    let seat = session.local_player_index() as usize;
    let total = keys[seat].len();
    let mut r = DriveReport {
        seat,
        ..Default::default()
    };
    let started = Instant::now();
    let mut primed_at = None;
    let mut running_at = None;
    let mut next_tick = Instant::now();
    let mut n = 0usize;
    while n < total {
        if let Some(e) = session.prime_error() {
            return Err(format!("{name}: boot failed: {e}"));
        }
        if session.is_ended() {
            return Err(format!(
                "{name}: session ended at advance {n} (remote_disconnected {})",
                session.remote_disconnected()
            ));
        }
        if started.elapsed() > Duration::from_secs(300) {
            return Err(format!("{name}: gave up after 300 s at advance {n}"));
        }
        let booting = session.is_booting();
        if !booting && primed_at.is_none() {
            primed_at = Some(Instant::now());
        }
        // Ticks that can't advance (waiting at the ready gate, paused for a
        // reconnect) are not handed to the driver, so every tick that is
        // counts as exactly one advance and both peers index the script by
        // the same pair tick.
        if !booting && (session.waiting_for_peer() || session.is_reconnecting()) {
            r.reconnect_seen |= session.is_reconnecting();
            std::thread::sleep(Duration::from_millis(1));
            next_tick = Instant::now();
            continue;
        }
        if !booting && running_at.is_none() {
            running_at = Some(Instant::now());
        }
        let stats = session.round_stats().unwrap();
        if !booting && stats.lead as usize >= tango_net::data::RECONNECT_QUEUE_LENGTH {
            return Err(format!(
                "{name}: local queue at the stall limit ({}) at advance {n}",
                stats.lead
            ));
        }
        session.set_input(HostInput::keys(keys[seat][n]));
        if !boot.tick() {
            return Err(format!("{name}: driver stopped at advance {n}"));
        }
        // The boot tick advances too if the peer was already primed; the
        // framebuffer starts zeroed and is written only by an advance.
        let advanced = !booting || session.frame().iter().any(|&b| b != 0);
        if !advanced {
            std::thread::sleep(Duration::from_millis(1));
            next_tick = Instant::now();
            continue;
        }
        if running_at.is_none() {
            running_at = Some(Instant::now());
        }
        let stats = session.round_stats().unwrap();
        if stats.depth > 0 {
            r.rollbacks += 1;
            r.deepest = r.deepest.max(stats.depth);
        }
        r.max_lead = r.max_lead.max(stats.lead);
        r.min_skew = r.min_skew.min(stats.skew);
        r.max_skew = r.max_skew.max(stats.skew);
        for &(t, mark) in marks {
            if t == n {
                r.marks.push((mark, session.frame()));
            }
        }
        n += 1;
        if n.is_multiple_of(600) {
            println!(
                "{name}: advance {n}/{total}, tps {:.1}, latency {:?}, lead {}, skew {}",
                session.tps(),
                session.latency(),
                stats.lead,
                stats.skew
            );
        }
        // Pacer::wait
        let fps = boot.fps_target();
        let fps = if fps > 0.0 { fps } else { 60.0 };
        next_tick += Duration::from_secs_f64(1.0 / fps as f64);
        let now = Instant::now();
        if next_tick > now {
            std::thread::sleep(next_tick - now);
        } else if now - next_tick > Duration::from_millis(250) {
            next_tick = now;
        }
    }
    let primed_at = primed_at.unwrap_or(started);
    let running_at = running_at.unwrap_or(primed_at);
    r.advances = n;
    r.boot_time = primed_at - started;
    r.peer_wait = running_at.saturating_duration_since(primed_at);
    r.run_time = running_at.elapsed();
    r.tps = session.tps();
    r.fps_target = session.fps_target();
    r.latency = session.latency();
    r.final_frame = session.frame();
    Ok(r)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: {} <rom> <out-dir> [port]", args[0]);
        std::process::exit(2);
    }
    let rom = std::path::PathBuf::from(&args[1]);
    let out = std::path::PathBuf::from(&args[2]);
    let port: u16 = args.get(3).map(|p| p.parse().expect("port")).unwrap_or(24690);
    let level = std::env::var("AW2_LOG")
        .ok()
        .and_then(|l| l.parse().ok())
        .unwrap_or(log::LevelFilter::Warn);
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(level);
    std::fs::create_dir_all(&out).expect("out dir");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let ok = rt.block_on(run(rom, out, port));
    // Don't wait on the transport's background threads.
    rt.shutdown_timeout(Duration::from_secs(2));
    std::process::exit(if ok { 0 } else { 1 });
}

async fn run(rom: std::path::PathBuf, out: std::path::PathBuf, port: u16) -> bool {
    let lib = match open_library(&rom, &out.join("library")).await {
        Ok(lib) => Arc::new(lib),
        Err(e) => {
            println!("library: {e}");
            return false;
        }
    };
    let script = Arc::new(build_script());
    println!(
        "script: {} ticks ({} scripted + {} idle), marks {:?}",
        script.keys[0].len(),
        script.keys[0].len() - IDLE_TICKS,
        IDLE_TICKS,
        script.marks
    );

    // Player 1 Black Hole, player 2 Orange Star (as in aw2_rollback_sim).
    // tangoAW2 has one mode; armies are picked in the game.
    let match_type = (0u8, 0u8);
    let lobby_start = Instant::now();
    let host = run_lobby("A", format!("/host {port}"), &lib, match_type);
    let client = async {
        tokio::time::sleep(Duration::from_millis(300)).await;
        run_lobby("B", format!("/connect 127.0.0.1:{port}"), &lib, match_type).await
    };
    let (a, b) = tokio::join!(host, client);
    let (a, b) = match (a, b) {
        (Ok(a), Ok(b)) => (a, b),
        (a, b) => {
            if let Err(e) = &a {
                println!("peer A: {e}");
            }
            if let Err(e) = &b {
                println!("peer B: {e}");
            }
            return false;
        }
    };
    println!("lobby + session build took {:.2?}", lobby_start.elapsed());

    let spawn = |name: &'static str, (session, mut boot, audio): Built| {
        let script = script.clone();
        tokio::task::spawn_blocking(move || {
            println!("{name}: local player index {}", session.local_player_index());
            let report = drive(name, &session, &mut boot, &script.keys, &script.marks);
            (session, boot, audio, report)
        })
    };
    let (ja, jb) = tokio::join!(spawn("A", a), spawn("B", b));
    let (sa, ba, _audio_a, ra) = ja.unwrap();
    let (sb, bb, _audio_b, rb) = jb.unwrap();

    let mut ok = true;
    let reports = match (ra, rb) {
        (Ok(ra), Ok(rb)) => Some((ra, rb)),
        (ra, rb) => {
            for e in [ra.err(), rb.err()].into_iter().flatten() {
                println!("{e}");
            }
            ok = false;
            None
        }
    };

    // Both loops are done: only now close, so neither side's Goodbye ends
    // the other's match early.
    sa.request_close();
    sb.request_close();
    let (done_a, done_b) = (sa.supervisor_done(), sb.supervisor_done());
    let _ = tokio::task::spawn_blocking(move || {
        ba.finish();
        bb.finish();
    })
    .await;
    let _ = tokio::time::timeout(Duration::from_secs(3), async {
        done_a.cancelled().await;
        done_b.cancelled().await;
    })
    .await;

    let Some((ra, rb)) = reports else { return false };
    for (name, r) in [("A", &ra), ("B", &rb)] {
        println!(
            "peer {name}: player {} | {} advances | boot {:.2?}, waited for peer {:.2?}, ran {:.2?} | \
             tps {:.2}, fps target {:.2} | latency {:?} | advances that rolled back {}, deepest {} ticks | \
             max lead {} | skew {}..{} | reconnect seen {}",
            r.seat + 1,
            r.advances,
            r.boot_time,
            r.peer_wait,
            r.run_time,
            r.tps,
            r.fps_target,
            r.latency,
            r.rollbacks,
            r.deepest,
            r.max_lead,
            r.min_skew,
            r.max_skew,
            r.reconnect_seen
        );
    }
    // What a concealed seat is shown (tango-backend-mgba shared.rs `concealed`).
    let concealed = |f: &[u8]| {
        f.chunks(4).enumerate().all(|(i, px)| {
            let v = if (76..84).contains(&(i / 240)) { 40 } else { 8 };
            px == [v, v, v + 12, 255]
        })
    };
    let describe = |f: &[u8]| if concealed(f) { "concealed" } else { "picture" };
    for ((mark, fa), (_, fb)) in ra.marks.iter().zip(rb.marks.iter()) {
        write_bmp(&out.join(format!("{mark}_A.bmp")), fa);
        write_bmp(&out.join(format!("{mark}_B.bmp")), fb);
        println!(
            "mark {mark}: A (player {}) {}, B (player {}) {}, identical: {}",
            ra.seat + 1,
            describe(fa),
            rb.seat + 1,
            describe(fb),
            fa == fb
        );
    }
    write_bmp(&out.join("final_A.bmp"), &ra.final_frame);
    write_bmp(&out.join("final_B.bmp"), &rb.final_frame);
    let same = ra.final_frame == rb.final_frame;
    println!(
        "final: A {}, B {}",
        describe(&ra.final_frame),
        describe(&rb.final_frame)
    );
    let lit = ra.final_frame.chunks(4).filter(|p| p[..3] != [0, 0, 0]).count();
    println!("final frames written to {}", out.display());
    println!("final frame non-black pixels: {lit} / {}", 240 * 160);
    println!("peer A and peer B final frames identical: {same}");
    ok && same
}
