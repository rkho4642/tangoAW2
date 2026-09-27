//! Offline play with an Armies pick, on the console single-player uses.
//!
//! Boots the shared console the way Play offline does (seat 0's input
//! only, the Armies pick as its mode) and checks two things:
//!   1. Versus against the computer comes out in the picked armies.
//!   2. Campaign keeps its story armies (Orange Star vs Black Hole's
//!      opponents), because the pick only applies to Versus.
//!
//! Usage: aw2_offline_check <rom> [out-dir]

use tango_match::{HostInput, Link};

const A: u32 = 1;
const START: u32 = 1 << 3;
const DOWN: u32 = 1 << 7;

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

struct Run {
    link: tango_backend_mgba::SharedLink,
}

impl Run {
    fn new(rom: &[u8], armies: Option<(u8, u8)>) -> Self {
        let save = vec![0xffu8; tango_gamesupport_aw2::SAVE_SIZE];
        let link = tango_backend_mgba::SharedLink::boot(
            rom,
            Some(&save),
            None,
            &tango_gamesupport_aw2::pvp::AW2E,
            armies,
            None,
        )
        .expect("boot");
        Run { link }
    }
    fn wait(&mut self, n: u32) {
        for _ in 0..n {
            self.link.tick([HostInput::keys(0), HostInput::keys(0)]);
        }
    }
    fn press(&mut self, keys: u32) {
        for _ in 0..8 {
            self.link.tick([HostInput::keys(keys), HostInput::keys(0)]);
        }
        self.wait(6);
    }
    fn shot(&mut self, path: std::path::PathBuf) {
        write_bmp(&path, &self.link.side(0).frame().unwrap());
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let rom = std::fs::read(&args[1]).expect("rom");
    let out = std::path::PathBuf::from(args.get(2).cloned().unwrap_or_else(|| ".".into()));

    // 1. Versus vs the computer, Armies = Black Hole vs Orange Star.
    let mut vs = Run::new(&rom, Some((4, 0)));
    vs.wait(700);
    vs.press(START);
    vs.wait(300);
    vs.press(A);
    vs.wait(150);
    vs.press(DOWN);
    vs.wait(60);
    vs.press(A); // Versus
    vs.wait(150);
    vs.press(A); // New
    vs.wait(150);
    vs.press(A); // Bean Island
    vs.wait(150);
    vs.press(A); // Teams (army 2 stays the computer)
    vs.wait(100);
    vs.press(A); // Rules
    vs.wait(700);
    vs.shot(out.join("offline_versus.bmp"));
    let colours = tango_gamesupport_aw2::pvp::army_state(vs.link.core());
    println!(
        "versus: mode {} army colours {:?} (want [5, 1, ..])",
        colours.0, colours.1
    );

    // 2. Campaign with the same pick: story colours must survive.
    let mut camp = Run::new(&rom, Some((4, 0)));
    camp.wait(700);
    camp.press(START);
    camp.wait(300);
    camp.press(A);
    camp.wait(150);
    camp.press(A); // Campaign
    camp.wait(150);
    camp.press(A); // New
    camp.wait(150);
    for _ in 0..310 {
        camp.press(A);
        camp.wait(50);
    }
    camp.shot(out.join("offline_campaign.bmp"));
    let colours = tango_gamesupport_aw2::pvp::army_state(camp.link.core());
    println!(
        "campaign: mode {} army colours {:?} (want story colours, not [5, 1, ..])",
        colours.0, colours.1
    );
}
