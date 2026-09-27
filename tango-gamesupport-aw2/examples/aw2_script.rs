//! gba_probe's script language, run on the shared console with tangoAW2's
//! Advance Wars 2 patches installed (single-player: seat 0's pad), so a
//! script sees exactly what Play offline does.
//!
//! Usage: aw2_script <rom> <script> [--save <sav>]
//!
//! Script lines (blank lines and `#` comments are skipped):
//!   wait N               run N frames with nothing held
//!   press KEYS [N]       hold KEYS for N frames (default 2), then release for 6
//!   hold KEYS N          hold KEYS for N frames
//!   shot NAME            write NAME.bmp (240x160)
//!   dump NAME            write NAME.ewram (256 KiB) and NAME.iwram (32 KiB)
//!   save NAME            write the cartridge save to NAME.sav
//!   poke8 ADDR VAL       write a byte (hex ADDR/VAL)
//!   poke16 ADDR VAL      write a halfword (hex ADDR/VAL)
//!   watch8 ADDR          print the byte every time it changes
//!   sticky8 ADDR VAL     write a byte before every later frame; `unstick` stops
//!   peek ADDR LEN        print LEN bytes at ADDR (hex)
//! KEYS is `+`-joined from A B SELECT START RIGHT LEFT UP DOWN R L.

use std::io::Write;

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

/// The shared console with tangoAW2's patches, behind the calls the probe
/// script runner makes.
struct Aw2Link(tango_backend_mgba::SharedLink);

impl Aw2Link {
    fn tick(&mut self, keys: &[u32]) {
        use tango_match::Link;
        self.0.tick([
            tango_match::HostInput::keys(keys[0]),
            tango_match::HostInput::keys(keys[0]),
        ]);
    }
    fn core(&self, _: usize) -> &mgba::core::Core {
        self.0.core()
    }
    fn core_mut(&mut self, _: usize) -> &mut mgba::core::Core {
        self.0.core_mut()
    }
    fn frame(&mut self) -> Vec<u8> {
        use tango_match::Link;
        self.0.side(0).frame().expect("frame")
    }
    fn export_save(&mut self, _: usize) -> Option<Vec<u8>> {
        use tango_match::Link;
        self.0.side(0).export_save()
    }
}

fn write_rgba_bmp(path: &str, rgba: &[u8]) {
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
    let script = std::fs::read_to_string(&args[2]).expect("script");
    let mut save = None;
    let mut i = 3;
    while i < args.len() {
        if args[i] == "--save" {
            save = Some(std::fs::read(&args[i + 1]).expect("save"));
            i += 2;
        } else {
            i += 1;
        }
    }
    let mut link = Aw2Link(
        tango_backend_mgba::SharedLink::boot(
            &rom,
            save.as_deref(),
            Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000)),
            &tango_gamesupport_aw2::pvp::AW2E,
            None,
            None,
        )
        .expect("boot"),
    );
    let mut frame = 0u64;
    let mut sticky: Vec<(u32, u8)> = Vec::new();
    let mut watch: Vec<(u32, u32)> = Vec::new();
    fn run(link: &mut Aw2Link, sticky: &[(u32, u8)], watch: &mut [(u32, u32)], frame: &mut u64, keys: u32, n: u32) {
        for _ in 0..n {
            for w in watch.iter_mut() {
                let v = link.core(0).raw_read_8(w.0, -1) as u32;
                if v != w.1 {
                    println!(
                        "watch {:08x}: {:02x} -> {:02x} @{} keys={:03x}",
                        w.0, w.1, v, frame, keys
                    );
                    w.1 = v;
                }
            }
            for &(a, v) in sticky {
                link.core_mut(0).raw_write_8(a, -1, v);
            }
            link.tick(&[keys]);
            *frame += 1;
        }
    }
    for line in script.lines() {
        let line = line.split('#').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts[0] {
            "wait" => run(&mut link, &sticky, &mut watch, &mut frame, 0, parts[1].parse().unwrap()),
            "press" => {
                let n = parts.get(2).map(|n| n.parse().unwrap()).unwrap_or(2);
                run(&mut link, &sticky, &mut watch, &mut frame, key_bits(parts[1]), n);
                run(&mut link, &sticky, &mut watch, &mut frame, 0, 6);
            }
            "hold" => run(
                &mut link,
                &sticky,
                &mut watch,
                &mut frame,
                key_bits(parts[1]),
                parts[2].parse().unwrap(),
            ),
            "shot" => write_rgba_bmp(&format!("{}.bmp", parts[1]), &link.frame()),
            "dump" => {
                let core = link.core(0);
                let mut ew = vec![0u8; 0x40000];
                core.raw_read_range(0x0200_0000, -1, &mut ew);
                let mut iw = vec![0u8; 0x8000];
                core.raw_read_range(0x0300_0000, -1, &mut iw);
                std::fs::write(format!("{}.ewram", parts[1]), ew).unwrap();
                std::fs::write(format!("{}.iwram", parts[1]), iw).unwrap();
            }
            "dumpvram" => {
                let mut v = vec![0u8; 0x18000];
                link.core(0).raw_read_range(0x0600_0000, -1, &mut v);
                std::fs::write(format!("{}.vram", parts[1]), v).unwrap();
            }
            "save" => {
                if let Some(s) = link.export_save(0) {
                    std::fs::write(format!("{}.sav", parts[1]), s).unwrap();
                }
            }
            "sticky8" => sticky.push((hex(parts[1]), hex(parts[2]) as u8)),
            "watch8" => watch.push((hex(parts[1]), 0)),
            "unstick" => sticky.clear(),
            "poke16" => link.core_mut(0).raw_write_16(hex(parts[1]), -1, hex(parts[2]) as u16),
            "poke8" => link.core_mut(0).raw_write_8(hex(parts[1]), -1, hex(parts[2]) as u8),
            "peek" => {
                let mut buf = vec![0u8; parts[2].parse().unwrap()];
                link.core(0).raw_read_range(hex(parts[1]), -1, &mut buf);
                let s: Vec<String> = buf.iter().map(|b| format!("{b:02x}")).collect();
                println!("{} @{frame}: {}", parts[1], s.join(" "));
            }
            other => panic!("unknown command {other}"),
        }
        std::io::stdout().flush().ok();
    }
    println!("ran {frame} frames");
}
