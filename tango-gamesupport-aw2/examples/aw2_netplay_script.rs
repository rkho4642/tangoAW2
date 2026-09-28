//! Two rollback peers in one process, joined by a fake network with delay
//! and jitter, driven by a script, from a given save. For netplay tests of
//! particular maps (a design map with Black Hole's inventions, say): both
//! peers must end on the same picture and the same RAM, and re-running the
//! confirmed inputs without any rollback must land there too.
//!
//! Usage: aw2_netplay_script <rom> <save> <script> <out-dir>
//!
//! Script lines (blank lines and `#` comments are skipped):
//!   seat N              later presses come from seat N's pad (0 or 1)
//!   wait N              run N frames with nothing held
//!   press KEYS [N]      hold KEYS for N frames (default 2), then release for 6
//!   hold KEYS N         hold KEYS for N frames
//!   goto X Y            walk the map cursor to tile (X, Y) with the arrows,
//!                       reading where it is from the seat's own peer
//!   noise SEAT N        SEAT mashes random buttons for N frames (the other
//!                       seat idle), for checking the turn lock
//!   shot NAME           write NAME_peer0.bmp and NAME_peer1.bmp (each
//!                       peer's own view)
//!   peek ADDR LEN       print LEN bytes at ADDR on both peers
//! KEYS is `+`-joined from A B SELECT START RIGHT LEFT UP DOWN R L.
//!
//! `TANGOAW2_DS_ROM=<file>` gives the peers the Dual Strike art, and
//! `AW2_SHARED_ART=1` plays the match as the lobby makes it when both
//! players have it (the Black Crystal and Black Obelisk maps then show).
//!
//! `AW2_TIMELINE=<file>` writes the resolved script: one line per tick with
//! both seats' keys (hex), and `# shot TICK NAME` lines, for replaying the
//! same match over a real connection (tango-session's aw2_direct_netplay).

use std::collections::VecDeque;
use tango_match::{HostInput, Link};

const CURSOR_X: u32 = 0x0300_33E4;
const CURSOR_Y: u32 = 0x0300_33E6;

fn key_bits(s: &str) -> u32 {
    s.split('+')
        .map(|k| match k.to_ascii_uppercase().as_str() {
            "A" => 1 << 0,
            "B" => 1 << 1,
            "SELECT" => 1 << 2,
            "START" => 1 << 3,
            "RIGHT" => 1 << 4,
            "LEFT" => 1 << 5,
            "UP" => 1 << 6,
            "DOWN" => 1 << 7,
            "R" => 1 << 8,
            "L" => 1 << 9,
            "NONE" | "" => 0,
            other => panic!("unknown key {other}"),
        })
        .fold(0, |a, b| a | b)
}

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s.trim_start_matches("0x"), 16).expect("hex number")
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

fn peek(m: &tango_match::Match, addr: u32, len: usize) -> Vec<u8> {
    m.with_link(|link| {
        let mut b = vec![0u8; len];
        link.peek(addr, &mut b);
        b
    })
}

/// What a script step asks for at one tick.
enum Row {
    Keys([u32; 2]),
    Shot(String),
    Peek(u32, usize),
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let rom = std::fs::read(&args[1]).expect("rom");
    let save = std::fs::read(&args[2]).expect("save");
    let script = std::fs::read_to_string(&args[3]).expect("script");
    let out = std::path::PathBuf::from(&args[4]);
    std::fs::create_dir_all(&out).ok();
    let rtc = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
    // `AW2_SHARED_ART=1`: the match as the lobby makes it when both players
    // have imported the Dual Strike art (`ds_art::SHARED_ART`).
    let shared_art = std::env::var_os("AW2_SHARED_ART").is_some();
    let match_type = (
        0u8,
        if shared_art {
            tango_gamesupport_aw2::ds_art::SHARED_ART
        } else {
            0
        },
    );

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

    let mut lines: VecDeque<String> = script
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect();
    let mut rows: VecDeque<Row> = VecDeque::new();
    let mut seat = 0usize;
    let mut seed = 7u32;

    let mut wire: [VecDeque<(usize, HostInput, i16)>; 2] = [VecDeque::new(), VecDeque::new()];
    let mut confirmed: Vec<[HostInput; 2]> = Vec::new();
    let mut rollbacks = 0u32;
    let mut deepest = 0u32;
    let mut jitter = 99u32;
    let mut t = 0usize;
    let mut timeline: Vec<String> = Vec::new();

    fn keys_for(seat: usize, bits: u32) -> [u32; 2] {
        let mut k = [0, 0];
        k[seat] = bits;
        k
    }
    fn press(rows: &mut VecDeque<Row>, seat: usize, bits: u32, n: usize) {
        for _ in 0..n {
            rows.push_back(Row::Keys(keys_for(seat, bits)));
        }
        for _ in 0..6 {
            rows.push_back(Row::Keys([0, 0]));
        }
    }

    loop {
        // Expand script lines until a tick's worth of input is queued.
        while !rows.iter().any(|r| matches!(r, Row::Keys(_))) {
            let Some(line) = lines.pop_front() else { break };
            let p: Vec<&str> = line.split_whitespace().collect();
            match p[0] {
                "seat" => seat = p[1].parse().unwrap(),
                "wait" => (0..p[1].parse::<usize>().unwrap()).for_each(|_| rows.push_back(Row::Keys([0, 0]))),
                "press" => press(
                    &mut rows,
                    seat,
                    key_bits(p[1]),
                    p.get(2).map_or(2, |n| n.parse().unwrap()),
                ),
                "hold" => (0..p[2].parse::<usize>().unwrap())
                    .for_each(|_| rows.push_back(Row::Keys(keys_for(seat, key_bits(p[1]))))),
                "goto" => {
                    let (x, y): (i32, i32) = (p[1].parse().unwrap(), p[2].parse().unwrap());
                    let cx = peek(&peers[seat], CURSOR_X, 1)[0] as i32;
                    let cy = peek(&peers[seat], CURSOR_Y, 1)[0] as i32;
                    let (h, v) = (x - cx, y - cy);
                    for _ in 0..h.abs() {
                        press(&mut rows, seat, key_bits(if h > 0 { "RIGHT" } else { "LEFT" }), 6);
                        (0..6).for_each(|_| rows.push_back(Row::Keys([0, 0])));
                    }
                    for _ in 0..v.abs() {
                        press(&mut rows, seat, key_bits(if v > 0 { "DOWN" } else { "UP" }), 6);
                        (0..6).for_each(|_| rows.push_back(Row::Keys([0, 0])));
                    }
                    (0..30).for_each(|_| rows.push_back(Row::Keys([0, 0])));
                }
                "noise" => {
                    let who: usize = p[1].parse().unwrap();
                    let n: usize = p[2].parse().unwrap();
                    let buttons = ["A", "B", "START", "SELECT", "RIGHT", "LEFT", "UP", "DOWN", "R", "L"];
                    let mut i = 0;
                    while i < n {
                        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                        let b = key_bits(buttons[(seed >> 16) as usize % buttons.len()]);
                        for _ in 0..4 {
                            rows.push_back(Row::Keys(keys_for(who, b)));
                        }
                        for _ in 0..7 {
                            rows.push_back(Row::Keys([0, 0]));
                        }
                        i += 11;
                    }
                }
                "shot" => rows.push_back(Row::Shot(p[1].to_string())),
                "peek" => rows.push_back(Row::Peek(hex(p[1]), p[2].parse().unwrap())),
                other => panic!("unknown command {other}"),
            }
        }
        let Some(row) = rows.pop_front() else { break };
        let keys = match row {
            Row::Shot(name) => {
                timeline.push(format!("# shot {t} {name}"));
                for (p, peer) in peers.iter_mut().enumerate() {
                    if let Some(f) = peer.frame() {
                        write_bmp(&out.join(format!("{name}_peer{p}.bmp")), &f);
                    }
                }
                continue;
            }
            Row::Peek(addr, len) => {
                let a = peek(&peers[0], addr, len);
                let b = peek(&peers[1], addr, len);
                let s = |v: &[u8]| v.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ");
                println!("peek {addr:08x} @{t} peer0: {}", s(&a));
                if a != b {
                    println!("peek {addr:08x} @{t} peer1: {}  (differs)", s(&b));
                }
                continue;
            }
            Row::Keys(k) => k,
        };
        timeline.push(format!("{:x} {:x}", keys[0], keys[1]));
        for p in 0..2 {
            while wire[1 - p].front().is_some_and(|&(at, _, _)| at <= t) {
                let (_, input, adv) = wire[1 - p].pop_front().unwrap();
                peers[p].add_remote_input(input, adv);
            }
            let adv = peers[p].advance(HostInput::keys(keys[p])).expect("advance");
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
        t += 1;
    }
    // Drain the wire with idle input so both peers settle.
    for t in t..t + 40 {
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

    if let Ok(path) = std::env::var("AW2_TIMELINE") {
        std::fs::write(path, timeline.join("\n") + "\n").expect("timeline");
    }

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

    println!("ticks: {t}, confirmed rows: {}", confirmed.len());
    println!("advances that rolled back: {rollbacks}, deepest rollback: {deepest} ticks");
    let mut all = true;
    for seat in 0..2 {
        let a = peers[0].seat_frame(seat).unwrap();
        let b = peers[1].seat_frame(seat).unwrap();
        let s = straight.side(seat).frame().unwrap();
        write_bmp(&out.join(format!("final_seat{seat}.bmp")), &a);
        println!(
            "seat {seat}'s view: peers agree {}, matches straight replay {}",
            a == b,
            a == s
        );
        all &= a == b && a == s;
    }
    // All of EWRAM and IWRAM, both peers and the straight replay.
    for (name, base, len) in [("ewram", 0x0200_0000u32, 0x4_0000usize), ("iwram", 0x0300_0000, 0x8000)] {
        let a = peek(&peers[0], base, len);
        let b = peek(&peers[1], base, len);
        let mut s = vec![0u8; len];
        straight.peek(base, &mut s);
        let diff = |x: &[u8], y: &[u8]| x.iter().zip(y).filter(|(p, q)| p != q).count();
        println!(
            "{name}: bytes differing peer0/peer1 {}, peer0/straight {}",
            diff(&a, &b),
            diff(&a, &s)
        );
        all &= a == b && a == s;
    }
    println!("all identical: {all}");
}
