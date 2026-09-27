//! In-game army picking on Versus' Teams screen.
//!
//! Boots the console as Play offline does, goes to Versus → Bean Island →
//! Teams, presses SELECT on army 1 until it reaches Black Hole, moves to
//! army 2 and presses L, then starts the battle and prints each army's
//! colour. With `--preset T,S` the launcher's Armies preset is applied
//! first (as the lobby or the offline picker would).
//!
//! Usage: aw2_teams_check <rom> <out-dir> [--preset T,S]

use tango_match::{HostInput, Link};

const A: u32 = 1;
const SELECT: u32 = 1 << 2;
const START: u32 = 1 << 3;
const RIGHT: u32 = 1 << 4;
const DOWN: u32 = 1 << 7;
const L: u32 = 1 << 9;

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
    let out = std::path::PathBuf::from(&args[2]);
    let preset = args.iter().position(|a| a == "--preset").map(|i| {
        let v: Vec<u8> = args[i + 1].split(',').map(|x| x.parse().unwrap()).collect();
        (v[0], v[1])
    });
    let save = vec![0xffu8; tango_gamesupport_aw2::SAVE_SIZE];
    let mut link =
        tango_backend_mgba::SharedLink::boot(&rom, Some(&save), None, &tango_gamesupport_aw2::pvp::AW2E, preset, None)
            .expect("boot");
    let wait = |link: &mut tango_backend_mgba::SharedLink, n: u32| {
        for _ in 0..n {
            link.tick([HostInput::keys(0), HostInput::keys(0)]);
        }
    };
    let press = |link: &mut tango_backend_mgba::SharedLink, keys: u32| {
        for _ in 0..8 {
            link.tick([HostInput::keys(keys), HostInput::keys(0)]);
        }
        for _ in 0..6 {
            link.tick([HostInput::keys(0), HostInput::keys(0)]);
        }
    };
    wait(&mut link, 700);
    press(&mut link, START);
    wait(&mut link, 300);
    press(&mut link, A);
    wait(&mut link, 150);
    press(&mut link, DOWN);
    wait(&mut link, 60);
    for _ in 0..3 {
        press(&mut link, A); // Versus, New, Bean Island
        wait(&mut link, 150);
    }
    let shot = |link: &mut tango_backend_mgba::SharedLink, name: &str| {
        write_bmp(&out.join(format!("{name}.bmp")), &link.side(0).frame().unwrap());
    };
    shot(&mut link, "teams_0");
    for i in 1..=4 {
        press(&mut link, SELECT);
        wait(&mut link, 20);
        shot(&mut link, &format!("teams_{i}"));
    }
    press(&mut link, RIGHT);
    wait(&mut link, 20);
    press(&mut link, RIGHT);
    wait(&mut link, 20);
    press(&mut link, L);
    wait(&mut link, 20);
    shot(&mut link, "teams_army2_L");
    press(&mut link, A);
    wait(&mut link, 100);
    press(&mut link, A);
    wait(&mut link, 700);
    shot(&mut link, "battle");
    let (mode, colours) = tango_gamesupport_aw2::pvp::army_state(link.core());
    println!("preset {preset:?}: mode {mode}, battle army colours {colours:?}");
}
