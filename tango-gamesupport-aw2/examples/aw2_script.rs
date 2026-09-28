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
//!   regs                 print the CPU registers
//!   stepwatch32 ADDR N   single-step N instructions, printing each change
//!                        of the word at ADDR
//!   goto AX AY X Y       walk a cursor whose position bytes are at AX/AY to
//!                        (X, Y) with the arrows
//!   stepuntil8 ADDR [N]  single-step until the byte changes (at most N
//!                        instructions), then print the last 400 PCs
//!   steplog N            single-step N instructions, printing every
//!                        function entry (Thumb `push {.., lr}`)
//!   stepreads ADDR LEN N single-step N instructions, printing every Thumb
//!                        immediate-offset load from [ADDR, ADDR+LEN)
//! KEYS is `+`-joined from A B SELECT START RIGHT LEFT UP DOWN R L.
//!
//! `AW2_TRACE=<file>` (hex ROM addresses, one per line) traps each address
//! and prints `trap ADDR @FRAME lr r0 r1 r2 r3` whenever the CPU reaches
//! it, for finding the game code behind a behaviour.

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

static FRAME: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// tangoAW2's Advance Wars 2 support plus tracing traps.
struct Traced(Vec<u32>);

impl tango_backend_mgba::SharedGame for Traced {
    fn sim_version(&self) -> u16 {
        tango_gamesupport_aw2::pvp::AW2E.sim_version()
    }
    fn merge(&self, core: &mgba::core::Core, inputs: [u32; 2]) -> u32 {
        tango_gamesupport_aw2::pvp::AW2E.merge(core, inputs)
    }
    fn before_tick(&self, core: &mut mgba::core::Core, mode: Option<(u8, u8)>, keys: u32) -> u32 {
        tango_gamesupport_aw2::pvp::AW2E.before_tick(core, mode, keys)
    }
    fn conceal(&self, core: &mgba::core::Core, seat: usize) -> bool {
        tango_gamesupport_aw2::pvp::AW2E.conceal(core, seat)
    }
    fn overlay(&self, core: &mgba::core::Core, mode: Option<(u8, u8)>, seat: usize, rgba: &mut [u8]) {
        tango_gamesupport_aw2::pvp::AW2E.overlay(core, mode, seat, rgba)
    }
    fn traps(&self) -> Vec<(u32, Box<dyn Fn(&mut mgba::core::Core)>)> {
        let mut traps = tango_gamesupport_aw2::pvp::AW2E.traps();
        for &addr in &self.0 {
            traps.push((
                addr,
                Box::new(move |core: &mut mgba::core::Core| {
                    let cpu = core.gba().cpu();
                    println!(
                        "trap {addr:08x} @{} lr={:08x} r0={:08x} r1={:08x} r2={:08x} r3={:08x} r12={:08x} sp={:08x}",
                        FRAME.load(std::sync::atomic::Ordering::Relaxed),
                        cpu.gpr(14),
                        cpu.gpr(0),
                        cpu.gpr(1),
                        cpu.gpr(2),
                        cpu.gpr(3),
                        cpu.gpr(12),
                        cpu.gpr(13)
                    );
                }),
            ));
        }
        traps
    }
    fn boot_ticks(&self) -> u32 {
        tango_gamesupport_aw2::pvp::AW2E.boot_ticks()
    }
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
            match std::env::var("AW2_TRACE") {
                Ok(path) => Box::leak(Box::new(Traced(
                    std::fs::read_to_string(path)
                        .expect("trace list")
                        .lines()
                        .filter(|l| !l.trim().is_empty())
                        .map(|l| hex(l.trim()))
                        .collect(),
                ))) as &'static (dyn tango_backend_mgba::SharedGame + Send + Sync),
                Err(_) => &tango_gamesupport_aw2::pvp::AW2E,
            },
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
            FRAME.store(*frame, std::sync::atomic::Ordering::Relaxed);
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
            "goto" => {
                // Walk a cursor to (X, Y) with the arrows: ADDR_X/ADDR_Y hold
                // its position as bytes (e.g. the Design Room's 0200B008 and
                // 0200B00A, the battle map's 030033E4 and 030033E6).
                let (ax, ay) = (hex(parts[1]), hex(parts[2]));
                let (x, y): (i32, i32) = (parts[3].parse().unwrap(), parts[4].parse().unwrap());
                for _ in 0..200 {
                    let cx = link.core(0).raw_read_8(ax, -1) as i32;
                    let cy = link.core(0).raw_read_8(ay, -1) as i32;
                    let k = if cx < x {
                        "RIGHT"
                    } else if cx > x {
                        "LEFT"
                    } else if cy < y {
                        "DOWN"
                    } else if cy > y {
                        "UP"
                    } else {
                        break;
                    };
                    run(&mut link, &sticky, &mut watch, &mut frame, key_bits(k), 6);
                    run(&mut link, &sticky, &mut watch, &mut frame, 0, 10);
                }
            }
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
            "regs" => {
                let cpu = link.core(0).gba().cpu();
                let r: Vec<String> = (0..16).map(|i| format!("r{i}={:08x}", cpu.gpr(i) as u32)).collect();
                println!("regs @{frame}: {}", r.join(" "));
            }
            "stepuntil8" => {
                // Single-step the CPU until the byte changes, then print
                // the last instructions run (for finding the code that
                // writes it). Leaves the console mid-frame.
                let addr = hex(parts[1]);
                let max: u64 = parts.get(2).map(|n| n.parse().unwrap()).unwrap_or(20_000_000);
                let core = link.core_mut(0);
                let before = core.raw_read_8(addr, -1);
                let mut ring = std::collections::VecDeque::new();
                let mut n = 0u64;
                while core.raw_read_8(addr, -1) == before && n < max {
                    let cpu = core.gba().cpu();
                    let thumb = matches!(cpu.execution_mode(), mgba::arm_core::ExecutionMode::Thumb);
                    let pc = if thumb { cpu.thumb_pc() } else { cpu.arm_pc() };
                    ring.push_back((pc, cpu.gpr(14) as u32));
                    if ring.len() > 400 {
                        ring.pop_front();
                    }
                    core.step();
                    n += 1;
                }
                println!(
                    "stepuntil8 {addr:08x}: {before:02x} -> {:02x} after {n} steps",
                    core.raw_read_8(addr, -1)
                );
                for (pc, lr) in ring {
                    println!("  pc={pc:08x} lr={lr:08x}");
                }
            }
            "steplog" => {
                // Single-step N instructions, printing every function entry
                // (a Thumb `push {.., lr}`) with its caller.
                let n: u64 = parts[1].parse().unwrap();
                let core = link.core_mut(0);
                for _ in 0..n {
                    let cpu = core.gba().cpu();
                    if matches!(cpu.execution_mode(), mgba::arm_core::ExecutionMode::Thumb) {
                        let pc = cpu.thumb_pc();
                        if core.raw_read_16(pc, -1) & 0xff00 == 0xb500 {
                            println!(
                                "  fn {pc:08x} lr={:08x} r0={:08x} sp={:08x}",
                                core.gba().cpu().gpr(14),
                                core.gba().cpu().gpr(0),
                                core.gba().cpu().gpr(13)
                            );
                        }
                    }
                    core.step();
                }
            }
            "stepwatch32" => {
                // Single-step N instructions, printing each change of the
                // word at ADDR with the instruction that made it.
                let addr = hex(parts[1]);
                let n: u64 = parts[2].parse().unwrap();
                let core = link.core_mut(0);
                let mut last = core.raw_read_32(addr, -1);
                for _ in 0..n {
                    let pc = core.gba().cpu().gpr(15) as u32;
                    core.step();
                    let v = core.raw_read_32(addr, -1);
                    if v != last {
                        println!("  {addr:08x}: {last:08x} -> {v:08x} near pc={pc:08x} lr={:08x}", core.gba().cpu().gpr(14));
                        last = v;
                    }
                }
            }
            "stepreads" => {
                // Single-step N instructions, printing every Thumb
                // `ldrb/ldrh/ldr rd, [rn, #imm]` that reads [ADDR, ADDR+LEN).
                let (lo, len, n): (u32, u32, u64) =
                    (hex(parts[1]), parts[2].parse().unwrap(), parts[3].parse().unwrap());
                let core = link.core_mut(0);
                for _ in 0..n {
                    let cpu = core.gba().cpu();
                    if matches!(cpu.execution_mode(), mgba::arm_core::ExecutionMode::Thumb) {
                        let pc = cpu.thumb_pc();
                        let op = core.raw_read_16(pc, -1) as u32;
                        let scale = match op >> 11 {
                            0b01111 => Some(1), // ldrb
                            0b10001 => Some(2), // ldrh
                            0b01101 => Some(4), // ldr
                            _ => None,
                        };
                        if let Some(scale) = scale {
                            let rn = ((op >> 3) & 7) as usize;
                            let addr = (cpu.gpr(rn) as u32).wrapping_add(((op >> 6) & 31) * scale);
                            if addr >= lo && addr < lo + len {
                                println!("  read {addr:08x} at pc={pc:08x} lr={:08x}", cpu.gpr(14));
                            }
                        }
                    }
                    core.step();
                }
            }
            other => panic!("unknown command {other}"),
        }
        std::io::stdout().flush().ok();
    }
    println!("ran {frame} frames");
}
