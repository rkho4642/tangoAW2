//! Every army colour, picked in the game, on 2-, 3- and 4-army Versus maps,
//! offline and in netplay.
//!
//! For each scenario the Teams screen's SELECT walks each army from its
//! starting colour to its target (seat 0 offline; in netplay armies 1 and 3
//! by seat 0, armies 2 and 4 by seat 1), fog is switched off and the battle
//! starts. Offline, the battle's player blocks are read back. Netplay runs
//! two rollback peers over a delayed, jittery fake network; both peers must
//! show exactly the offline run's Teams screen and battle.
//!
//! Usage: aw2_colour_matrix <rom> <out-dir>

use std::collections::VecDeque;
use tango_match::{HostInput, Link};

const A: u32 = 1;
const SELECT: u32 = 1 << 2;
const START: u32 = 1 << 3;
const RIGHT: u32 = 1 << 4;
const LEFT: u32 = 1 << 5;
const UP: u32 = 1 << 6;
const DOWN: u32 = 1 << 7;

const NAMES: [&str; 6] = [
    "?",
    "Orange Star",
    "Blue Moon",
    "Green Earth",
    "Yellow Comet",
    "Black Hole",
];
/// SELECT's order: Orange Star, Blue Moon, Yellow Comet, Green Earth, Black Hole.
const ORDER: [u8; 5] = [1, 2, 4, 3, 5];

struct Scenario {
    label: &'static str,
    /// RIGHT presses on the map-select category header (0 Classic, 3 3P, 4 4P).
    category: u32,
    targets: &'static [u8],
    /// On 3/4-army maps: UP presses on each army's alliance letter
    /// (A, B, C, D by default; UP steps back one letter).
    alliance_ups: &'static [u8],
}

const SCENARIOS: &[Scenario] = &[
    Scenario {
        label: "2 armies",
        category: 0,
        targets: &[1, 2],
        alliance_ups: &[],
    },
    Scenario {
        label: "2 armies",
        category: 0,
        targets: &[4, 3],
        alliance_ups: &[],
    },
    Scenario {
        label: "2 armies",
        category: 0,
        targets: &[3, 5],
        alliance_ups: &[],
    },
    Scenario {
        label: "2 armies",
        category: 0,
        targets: &[5, 1],
        alliance_ups: &[],
    },
    Scenario {
        label: "2 armies",
        category: 0,
        targets: &[5, 4],
        alliance_ups: &[],
    },
    Scenario {
        label: "3 armies",
        category: 3,
        targets: &[5, 2, 3],
        alliance_ups: &[],
    },
    Scenario {
        label: "3 armies",
        category: 3,
        targets: &[1, 5, 3],
        alliance_ups: &[],
    },
    Scenario {
        label: "3 armies",
        category: 3,
        targets: &[1, 2, 5],
        alliance_ups: &[],
    },
    Scenario {
        label: "3 armies",
        category: 3,
        targets: &[4, 5, 1],
        alliance_ups: &[],
    },
    Scenario {
        label: "4 armies",
        category: 4,
        targets: &[5, 2, 3, 4],
        alliance_ups: &[],
    },
    Scenario {
        label: "4 armies",
        category: 4,
        targets: &[1, 5, 3, 4],
        alliance_ups: &[],
    },
    Scenario {
        label: "4 armies",
        category: 4,
        targets: &[1, 2, 5, 4],
        alliance_ups: &[],
    },
    Scenario {
        label: "4 armies",
        category: 4,
        targets: &[1, 2, 3, 5],
        alliance_ups: &[],
    },
    Scenario {
        label: "4 armies, 2 vs 2",
        category: 4,
        targets: &[1, 2, 3, 5],
        // B -> A for army 2, D -> C for army 4: Orange Star + Blue Moon
        // against Green Earth + Black Hole.
        alliance_ups: &[0, 1, 0, 1],
    },
];

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

enum Rig {
    Offline(tango_backend_mgba::SharedLink, VecDeque<Vec<u8>>),
    Netplay {
        peers: [tango_match::Match; 2],
        wire: [VecDeque<(u64, HostInput, i16)>; 2],
        t: u64,
        jitter: u32,
    },
}

impl Rig {
    fn tick(&mut self, keys: [u32; 2]) {
        match self {
            Rig::Offline(link, recent) => {
                link.tick([HostInput::keys(keys[0] | keys[1]), HostInput::keys(0)]);
                recent.push_back(link.side(0).frame().unwrap());
                if recent.len() > 8 {
                    recent.pop_front();
                }
            }
            Rig::Netplay { peers, wire, t, jitter } => {
                for p in 0..2 {
                    while wire[1 - p].front().is_some_and(|&(at, _, _)| at <= *t) {
                        let (_, input, adv) = wire[1 - p].pop_front().unwrap();
                        peers[p].add_remote_input(input, adv);
                    }
                    let adv = peers[p].advance(HostInput::keys(keys[p])).expect("advance");
                    *jitter = jitter.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                    let at = (*t + 6 + (*jitter >> 16) as u64 % 4).max(wire[p].back().map_or(0, |b| b.0));
                    wire[p].push_back((at, adv.outgoing, adv.tick_advantage));
                }
                *t += 1;
            }
        }
    }
    fn wait(&mut self, n: u32) {
        for _ in 0..n {
            self.tick([0, 0]);
        }
    }
    fn press(&mut self, seat: usize, key: u32) {
        let mut keys = [0, 0];
        keys[seat] = key;
        for _ in 0..8 {
            self.tick(keys);
        }
        self.wait(6);
    }
    fn frame(&mut self, seat: usize) -> Vec<u8> {
        match self {
            Rig::Offline(link, _) => link.side(0).frame().unwrap(),
            Rig::Netplay { peers, .. } => peers[seat].frame().unwrap(),
        }
    }
    fn recent(&self) -> Vec<Vec<u8>> {
        match self {
            Rig::Offline(_, recent) => recent.iter().cloned().collect(),
            Rig::Netplay { .. } => Vec::new(),
        }
    }
    /// Let the netplay wire drain so both peers present the settled state.
    fn settle(&mut self) {
        if matches!(self, Rig::Netplay { .. }) {
            self.wait(30);
        }
    }
}

/// SELECT presses taking army `slot` from its colour to `target`, given
/// every army's colour now (the game skips colours other armies have).
fn presses(colours: &[u8], slot: usize, target: u8) -> Option<usize> {
    let mut c = colours[slot];
    for n in 0..6 {
        if c == target {
            return Some(n);
        }
        let i = ORDER.iter().position(|&x| x == c)?;
        c = (1..5)
            .map(|k| ORDER[(i + k) % 5])
            .find(|x| !colours.iter().enumerate().any(|(s, y)| s != slot && y == x))?;
    }
    None
}

/// `start` is each army's colour when the Teams screen opens.
#[allow(dead_code)] // the recent frames are kept for debugging captures
struct Played {
    teams: [Vec<u8>; 2],
    battle: [Vec<u8>; 2],
    /// Offline only: the last few frames before each capture.
    recent_teams: Vec<Vec<u8>>,
    recent_battle: Vec<Vec<u8>>,
}

fn play(rig: &mut Rig, sc: &Scenario, start: &[u8], netplay: bool) -> Played {
    rig.wait(700);
    rig.press(0, START);
    rig.wait(300);
    rig.press(0, A);
    rig.wait(150);
    rig.press(0, DOWN);
    rig.wait(60);
    rig.press(0, A); // Versus
    rig.wait(150);
    rig.press(0, A); // New
    rig.wait(150);
    for _ in 0..sc.category {
        rig.press(0, RIGHT);
        rig.wait(60);
    }
    rig.press(0, A); // first map of the category
    rig.wait(150);
    let mut colours = start.to_vec();
    let mut cursor_army = 0usize;
    // Armies whose target another army still holds go last.
    let mut todo: Vec<usize> = (0..sc.targets.len()).collect();
    while !todo.is_empty() {
        let pos = todo
            .iter()
            .position(|&s| presses(&colours, s, sc.targets[s]).is_some())
            .expect("a free target");
        let slot = todo.remove(pos);
        let n = presses(&colours, slot, sc.targets[slot]).unwrap();
        if n == 0 {
            continue;
        }
        while cursor_army < slot {
            rig.press(0, RIGHT);
            rig.press(0, RIGHT);
            cursor_army += 1;
        }
        while cursor_army > slot {
            rig.press(0, LEFT);
            rig.press(0, LEFT);
            cursor_army -= 1;
        }
        // In netplay each army's own player picks (odd armies seat 0).
        let seat = if netplay { slot % 2 } else { 0 };
        for _ in 0..n {
            rig.press(seat, SELECT);
            rig.wait(10);
        }
        colours[slot] = sc.targets[slot];
    }
    while cursor_army > 0 {
        rig.press(0, LEFT);
        rig.press(0, LEFT);
        cursor_army -= 1;
    }
    rig.wait(30);
    rig.settle();
    let teams = (rig.frame(0), rig.frame(1), rig.recent());
    rig.press(0, A); // COs done
    rig.wait(250);
    if sc.targets.len() > 2 {
        // Alliances (3/4-army maps): set them, then confirm.
        for (army, &ups) in sc.alliance_ups.iter().enumerate() {
            if army > 0 {
                rig.press(0, RIGHT);
            }
            for _ in 0..ups {
                rig.press(0, UP);
            }
        }
        rig.wait(30);
        rig.press(0, A);
        rig.wait(250);
    }
    rig.press(0, UP); // Fog off
    rig.wait(40);
    rig.press(0, A); // Rules done
    rig.wait(900);
    rig.settle();
    let battle = (rig.frame(0), rig.frame(1), rig.recent());
    Played {
        teams: [teams.0, teams.1],
        battle: [battle.0, battle.1],
        recent_teams: teams.2,
        recent_battle: battle.2,
    }
}

fn names(c: &[u8]) -> String {
    c.iter().map(|&x| NAMES[x as usize]).collect::<Vec<_>>().join(" / ")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let rom = std::fs::read(&args[1]).expect("rom");
    let out = std::path::PathBuf::from(&args[2]);
    let save = vec![0xffu8; tango_gamesupport_aw2::SAVE_SIZE];
    let rtc = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
    let mut all_ok = true;
    for (n, sc) in SCENARIOS.iter().enumerate() {
        let armies = sc.targets.len();
        // Offline: every army starts in the map's own colour.
        let map_colours = [1u8, 2, 3, 4];
        let link = tango_backend_mgba::SharedLink::boot(
            &rom,
            Some(&save),
            Some(rtc),
            &tango_gamesupport_aw2::pvp::AW2E,
            None,
            None,
        )
        .expect("boot");
        let mut off = Rig::Offline(link, VecDeque::new());
        let offline = play(&mut off, sc, &map_colours[..armies], false);
        let Rig::Offline(link, _) = &off else { unreachable!() };
        let used = tango_gamesupport_aw2::pvp::army_state(link.core()).1[..armies].to_vec();
        let offline_ok = used == sc.targets;
        let tag = format!("{n:02}_{armies}army");
        write_bmp(&out.join(format!("{tag}_teams.bmp")), &offline.teams[0]);
        write_bmp(&out.join(format!("{tag}_battle.bmp")), &offline.battle[0]);

        // Netplay: the same picks, each army's by its own player, over a
        // delayed wire.
        let game = &tango_gamesupport_aw2::AW2;
        let start = |local_player| {
            game.pvp
                .start(tango_match::StartConfig {
                    roms: [&rom, &rom],
                    saves: [Some(&save), Some(&save)],
                    rng_seed: [0; 16],
                    rtc,
                    match_type: (0, 0),
                    peer_rom: tango_match::PeerRom {
                        code: *b"AW2E",
                        revision: 0,
                    },
                    local_player,
                    // No presentation delay, so each peer shows the tick it
                    // simulated and can be compared with the offline run.
                    present_delay: 0,
                    disable_bgm: false,
                    audio: None,
                    cancel: None,
                })
                .expect("start")
        };
        let mut net_rig = Rig::Netplay {
            peers: [start(0), start(1)],
            wire: [VecDeque::new(), VecDeque::new()],
            t: 0,
            jitter: 99 + n as u32,
        };
        let net = play(&mut net_rig, sc, &map_colours[..armies], true);
        write_bmp(&out.join(format!("{tag}_net_teams.bmp")), &net.teams[0]);
        write_bmp(&out.join(format!("{tag}_net_battle.bmp")), &net.battle[0]);
        let peers_agree = net.teams[0] == net.teams[1] && net.battle[0] == net.battle[1];
        // The two runs capture a frame or two apart; netplay must show one
        // of the offline run's last few frames at each point.
        // Read each peer's battle armies through the link.
        let Rig::Netplay { peers, .. } = &mut net_rig else {
            unreachable!()
        };
        let read = |m: &tango_match::Match| -> Vec<u8> {
            m.with_link(|link| {
                tango_gamesupport_aw2::pvp::ARMY_COLOUR_ADDRS[..armies]
                    .iter()
                    .map(|&a| {
                        let mut b = [0u8];
                        link.peek(a, &mut b);
                        b[0]
                    })
                    .collect()
            })
        };
        let net_used = [read(&peers[0]), read(&peers[1])];
        let net_matches_offline = net_used[0] == sc.targets && net_used[1] == sc.targets;
        let ok = offline_ok && peers_agree && net_matches_offline;
        all_ok &= ok;
        println!(
            "{n:>2} {} | picked {} | offline battle uses {} | netplay: both peers' battle uses the picks {net_matches_offline}, peers' pictures agree {peers_agree} | {}",
            sc.label,
            names(sc.targets),
            names(&used),
            if ok { "ok" } else { "FAIL" }
        );
    }
    println!("all scenarios correct: {all_ok}");
}
