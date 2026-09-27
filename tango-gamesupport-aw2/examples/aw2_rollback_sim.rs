//! Two rollback peers in one process, joined by a fake network with delay
//! and jitter, playing Advance Wars 2 from power-on into a Versus match and
//! through a turn each. Checks that both peers end on the same picture, and
//! that re-running the confirmed inputs without any rollback lands there too.
//!
//! Usage: aw2_rollback_sim <rom> [out-dir]

use std::collections::VecDeque;
use tango_match::{HostInput, Link};

const A: u32 = 1;
const SELECT: u32 = 1 << 2;
const START: u32 = 1 << 3;
const RIGHT: u32 = 1 << 4;
const UP: u32 = 1 << 6;
const DOWN: u32 = 1 << 7;

/// Per-seat button timelines built from probe-style steps.
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
    /// Random presses by `seat` over the next `n` frames, without moving the clock.
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

fn write_bmp(path: &std::path::Path, rgba: &[u8]) {
    let (w, h) = (240u32, 160u32);
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let rom = std::fs::read(&args[1]).expect("rom");
    let out = std::path::PathBuf::from(args.get(2).cloned().unwrap_or_else(|| ".".into()));
    let save = vec![0xffu8; tango_gamesupport_aw2::SAVE_SIZE];
    let rtc = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
    // Player 1 Black Hole, player 2 Orange Star.
    let match_type = (4u8, 0u8);

    let mut s = Script {
        keys: [vec![], vec![]],
        t: 0,
        marks: vec![],
    };
    let mut seed = 7u32;
    // Both seats share the pad in menus; seat 0 drives them.
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
    // Seat 1 mashes during player 1's turn: must change nothing.
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
    let total = s.t + 60;
    for k in &mut s.keys {
        k.resize(total, 0);
    }

    let game = &tango_gamesupport_aw2::AW2;
    let start = |local_player| {
        game.pvp
            .start(tango_match::StartConfig {
                roms: [&rom, &rom],
                saves: [Some(&save), Some(&save)],
                rng_seed: [0; 16],
                rtc,
                match_type,
                peer_rom: tango_match::PeerRom {
                    code: *b"AW2E",
                    revision: 0,
                },
                local_player,
                present_delay: 2,
                disable_bgm: false,
                audio: None,
                cancel: None,
            })
            .expect("start")
    };
    let mut peers = [start(0), start(1)];

    // In-flight packets: (deliver_at, input, advantage) per direction.
    let mut wire: [VecDeque<(usize, HostInput, i16)>; 2] = [VecDeque::new(), VecDeque::new()];
    let mut confirmed: Vec<[HostInput; 2]> = Vec::new();
    let mut rollbacks = 0u32;
    let mut deepest = 0u32;
    let mut jitter = 99u32;
    for t in 0..total {
        for p in 0..2 {
            // Deliver whatever has arrived for peer p (sent by the other).
            while wire[1 - p].front().is_some_and(|&(at, _, _)| at <= t) {
                let (_, input, adv) = wire[1 - p].pop_front().unwrap();
                peers[p].add_remote_input(input, adv);
            }
            let adv = peers[p].advance(HostInput::keys(s.keys[p][t])).expect("advance");
            if p == 0 {
                confirmed.extend(adv.confirmed_inputs);
            }
            let depth = peers[p].last_rollback_depth();
            if depth > 0 {
                rollbacks += 1;
                deepest = deepest.max(depth);
            }
            jitter = jitter.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let delay = 7 + (jitter >> 16) as usize % 4;
            let at = (t + delay).max(wire[p].back().map_or(0, |b| b.0));
            wire[p].push_back((at, adv.outgoing, adv.tick_advantage));
        }
        for &(mt, name) in &s.marks {
            if mt == t {
                if let Some(f) = peers[0].frame() {
                    write_bmp(&out.join(format!("{name}.bmp")), &f);
                }
            }
        }
    }
    // Drain the wire with idle input so both peers settle.
    for t in total..total + 40 {
        for p in 0..2 {
            while let Some((_, input, adv)) = wire[1 - p].pop_front() {
                peers[p].add_remote_input(input, adv);
            }
            let adv = peers[p].advance(HostInput::keys(0)).expect("advance");
            if p == 0 {
                confirmed.extend(adv.confirmed_inputs);
            }
            wire[p].push_back((t, adv.outgoing, adv.tick_advantage));
        }
    }
    let f0 = peers[0].frame().unwrap();
    let f1 = peers[1].frame().unwrap();
    write_bmp(&out.join("final_peer0.bmp"), &f0);
    write_bmp(&out.join("final_peer1.bmp"), &f1);

    // Ground truth: the confirmed inputs straight through, no rollback.
    let mut straight = tango_backend_mgba::SharedLink::boot(
        &rom,
        Some(&save),
        Some(rtc),
        &tango_gamesupport_aw2::pvp::AW2E,
        Some(match_type),
        None,
    )
    .expect("boot");
    for row in &confirmed {
        straight.tick(*row);
    }
    let fs = straight.side(0).frame().unwrap();
    write_bmp(&out.join("final_straight.bmp"), &fs);

    println!("ticks simulated: {}", total + 40);
    println!("confirmed rows: {}", confirmed.len());
    println!("advances that rolled back: {rollbacks}, deepest rollback: {deepest} ticks");
    println!("peer 0 and peer 1 final frames identical: {}", f0 == f1);
    println!("peer frame equals straight replay frame: {}", f0 == fs);
}
