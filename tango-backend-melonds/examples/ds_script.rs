//! A scripted DS, for looking at what a cart shows (reference screenshots,
//! memory). Reads commands from stdin, one per line; prints `@ok FRAME`
//! after each.
//!
//!   ds_script ROM [SAVE]
//!   wait N               run N frames, nothing held
//!   press KEYS N         hold KEYS (A+B+SELECT+START+RIGHT+LEFT+UP+DOWN+R+L+X+Y) N
//!                        frames, then 6 frames released
//!   hold KEYS N          hold KEYS N frames
//!   touch X Y N          touch the bottom screen at (X, Y) N frames, then 6 released
//!   shot NAME            write NAME.bmp (256x384: the top screen over the bottom)
//!   peek ADDR LEN        print LEN bytes at ADDR (hex) on the ARM9's bus
//!   poke8 ADDR VAL / poke16 / poke32
//!   dumprange ADDR LEN FILE   write LEN bytes at ADDR to FILE
//!   state NAME / load NAME    save / restore the console's state
//!   save NAME            write the cart save to NAME
//!   quit

use std::io::{BufRead, Write};

fn key_bits(s: &str) -> u32 {
    s.split('+')
        .map(|k| match k {
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
            "X" => 1 << 10,
            "Y" => 1 << 11,
            _ => 0,
        })
        .fold(0, |a, b| a | b)
}

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or(0)
}

fn write_bmp(path: &str, w: usize, h: usize, rgb: &[[u8; 3]]) {
    let row = (w * 3 + 3) & !3;
    let size = 54 + row * h;
    let mut b = Vec::with_capacity(size);
    b.extend_from_slice(b"BM");
    b.extend_from_slice(&(size as u32).to_le_bytes());
    b.extend_from_slice(&[0; 4]);
    b.extend_from_slice(&54u32.to_le_bytes());
    b.extend_from_slice(&40u32.to_le_bytes());
    b.extend_from_slice(&(w as i32).to_le_bytes());
    b.extend_from_slice(&(h as i32).to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&24u16.to_le_bytes());
    b.extend_from_slice(&[0; 24]);
    for y in (0..h).rev() {
        for x in 0..w {
            let p = rgb[y * w + x];
            b.extend_from_slice(&[p[2], p[1], p[0]]);
        }
        b.extend(std::iter::repeat(0).take(row - w * 3));
    }
    std::fs::write(path, b).unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let rom = std::fs::read(&args[1]).expect("rom");
    let save = args.get(2).and_then(|p| std::fs::read(p).ok());
    let mut solo = melonds_rollback::Solo::new(&rom, save.as_deref(), (2026, 1, 1, 12, 0, 0)).expect("boot");
    let mut frame: u64 = 0;
    let mut states: std::collections::HashMap<String, Vec<u8>> = Default::default();
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    let mut run = |solo: &mut melonds_rollback::Solo, frame: &mut u64, keys: u32, touch: Option<(u16, u16)>, n: u32| {
        for _ in 0..n {
            solo.tick(melonds_rollback::Input { keys, touch, mic: false });
            *frame += 1;
        }
    };
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let p: Vec<&str> = line.split_whitespace().collect();
        match p.first().copied().unwrap_or("") {
            "wait" => run(&mut solo, &mut frame, 0, None, p[1].parse().unwrap_or(1)),
            "press" => {
                run(&mut solo, &mut frame, key_bits(p[1]), None, p.get(2).and_then(|n| n.parse().ok()).unwrap_or(2));
                run(&mut solo, &mut frame, 0, None, 6);
            }
            "hold" => run(&mut solo, &mut frame, key_bits(p[1]), None, p[2].parse().unwrap_or(1)),
            "touch" => {
                let (x, y) = (p[1].parse().unwrap_or(0), p[2].parse().unwrap_or(0));
                run(&mut solo, &mut frame, 0, Some((x, y)), p.get(3).and_then(|n| n.parse().ok()).unwrap_or(4));
                run(&mut solo, &mut frame, 0, None, 6);
            }
            "shot" => {
                let mut side = solo.side();
                let nds = side.console();
                if let Some((top, bottom)) = nds.framebuffers() {
                    let mut rgb = Vec::with_capacity(256 * 384);
                    for px in top.iter().chain(bottom.iter()) {
                        let c = tango_backend_melonds::unpacked_bgr666_to_rgba8(*px);
                        rgb.push([c[0], c[1], c[2]]);
                    }
                    write_bmp(&format!("{}.bmp", p[1]), 256, 384, &rgb);
                }
            }
            "peek" => {
                let (a, n) = (hex(p[1]), p[2].parse::<u32>().unwrap_or(1));
                let mut side = solo.side();
                let nds = side.console();
                let bytes: Vec<String> = (0..n).map(|k| format!("{:02x}", nds.read8(a + k))).collect();
                writeln!(out, "{:08x}: {}", a, bytes.join(" ")).ok();
            }
            "poke8" | "poke16" | "poke32" => {
                let (a, v) = (hex(p[1]), hex(p[2]));
                let mut side = solo.side();
                let nds = side.console();
                match p[0] {
                    "poke8" => nds.write8(a, v as u8),
                    "poke16" => nds.write16(a, v as u16),
                    _ => nds.write32(a, v),
                }
            }
            "dumprange" => {
                let (a, n) = (hex(p[1]), p[2].parse::<u32>().unwrap_or(0));
                let mut side = solo.side();
                let nds = side.console();
                let bytes: Vec<u8> = (0..n).map(|k| nds.read8(a + k)).collect();
                std::fs::write(p[3], bytes).ok();
            }
            "state" => {
                let mut buf = Vec::new();
                solo.side().console().save_state(&mut buf).ok();
                states.insert(p[1].to_string(), buf);
            }
            "load" => {
                if let Some(buf) = states.get(p[1]) {
                    solo.side().console().load_state(buf).ok();
                }
            }
            "trapjump" => {
                // Once the ARM9 reaches ADDR, it goes to TARGET instead (an
                // interworking address): a call replaced by another.
                let (at, to) = (hex(p[1]), hex(p[2]));
                let done = std::rc::Rc::new(std::cell::Cell::new(false));
                let d2 = done.clone();
                solo.side().console().set_traps(vec![(
                    at,
                    Box::new(move |nds: &mut melonds::Nds| {
                        if !d2.get() {
                            d2.set(true);
                            eprintln!("trapjump {at:08x} -> {to:08x} lr {:08x} r0 {:08x} r1 {:08x} r2 {:08x}", nds.reg(14), nds.reg(0), nds.reg(1), nds.reg(2));
                            nds.jump(to);
                        }
                    }),
                )]);
            }
            "trapprint" => {
                // Print r0..r3, LR and the text at r0 for the first N times
                // the ARM9 reaches ADDR.
                let at = hex(p[1]);
                let left = std::rc::Rc::new(std::cell::Cell::new(p[2].parse::<u32>().unwrap_or(10)));
                solo.side().console().set_traps(vec![(
                    at,
                    Box::new(move |nds: &mut melonds::Nds| {
                        if left.get() > 0 {
                            left.set(left.get() - 1);
                            let r: Vec<u32> = (0..4).map(|i| nds.reg(i)).collect();
                            let text = |nds: &mut melonds::Nds, a: u32| -> String {
                                (0..24)
                                    .map(|k| nds.read8(a.wrapping_add(k)))
                                    .take_while(|&c| c != 0)
                                    .map(|c| if (32..127).contains(&c) { c as char } else { '.' })
                                    .collect()
                            };
                            let (s0, s1) = (text(nds, r[0]), text(nds, r[1]));
                            eprintln!(
                                "trap {at:08x} r0 {:08x} r1 {:08x} r2 {:08x} r3 {:08x} lr {:08x} {s0:?} {s1:?} r4 {:08x} r5 {:08x} r6 {:08x} r7 {:08x}",
                                r[0], r[1], r[2], r[3], nds.reg(14), nds.reg(4), nds.reg(5), nds.reg(6), nds.reg(7)
                            );
                        }
                    }),
                )]);
            }
            "trapprints" => {
                // `trapprints N ADDR...`: print r0..r7 and LR the first N times
                // the ARM9 reaches each ADDR (several traps at once).
                let n = p[1].parse::<u32>().unwrap_or(10);
                let mut traps: Vec<(u32, Box<dyn FnMut(&mut melonds::Nds)>)> = Vec::new();
                for a in &p[2..] {
                    let at = hex(a);
                    let left = std::cell::Cell::new(n);
                    traps.push((
                        at,
                        Box::new(move |nds: &mut melonds::Nds| {
                            if left.get() > 0 {
                                left.set(left.get() - 1);
                                let r: Vec<u32> = (0..8).map(|i| nds.reg(i)).collect();
                                eprintln!(
                                    "trap {at:08x} r0 {:08x} r1 {:08x} r2 {:08x} r3 {:08x} r4 {:08x} r5 {:08x} r6 {:08x} r7 {:08x} lr {:08x}",
                                    r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7], nds.reg(14)
                                );
                            }
                        }),
                    ));
                }
                solo.side().console().set_traps(traps);
            }
            "trappoke" => {
                // `trappoke ADDR WHERE VAL8 [N]`: the first N times (1) the
                // ARM9 reaches ADDR, write the byte VAL8 at WHERE (a decision forced).
                let (at, wh, v) = (hex(p[1]), hex(p[2]), hex(p[3]));
                let left = std::cell::Cell::new(p.get(4).and_then(|n| n.parse::<u32>().ok()).unwrap_or(1));
                solo.side().console().set_traps(vec![(
                    at,
                    Box::new(move |nds: &mut melonds::Nds| {
                        if left.get() > 0 {
                            left.set(left.get() - 1);
                            let old = nds.read8(wh) as u32 | (nds.read8(wh + 1) as u32) << 8;
                            nds.write8(wh, v as u8);
                            eprintln!("trappoke {at:08x}: [{wh:08x}] {old:04x} -> {v:04x} lr {:08x}", nds.reg(14));
                        }
                    }),
                )]);
            }
            "readwatch" => {
                // Print the PC and LR of the first N reads of ADDR (`readwatch
                // off` removes the watch).
                if p[1] == "off" {
                    solo.side().console().set_watches(Vec::new());
                } else {
                    let at = hex(p[1]);
                    let left = std::rc::Rc::new(std::cell::Cell::new(p[2].parse::<u32>().unwrap_or(10)));
                    solo.side().console().set_watches(vec![(
                        at,
                        Box::new(move |nds: &mut melonds::Nds| {
                            if left.get() > 0 {
                                left.set(left.get() - 1);
                                let (pc, lr) = (nds.pc(), nds.reg(14));
                                eprintln!("read {at:08x} pc {pc:08x} lr {lr:08x}");
                            }
                        }),
                    )]);
                }
            }
            "statefile" => {
                let mut buf = Vec::new();
                solo.side().console().save_state(&mut buf).ok();
                std::fs::write(p[1], buf).ok();
            }
            "loadfile" => {
                if let Ok(buf) = std::fs::read(p[1]) {
                    solo.side().console().load_state(&buf).ok();
                }
            }
            "save" => {
                let m = solo.side().console().save_memory();
                std::fs::write(p[1], m).ok();
            }
            "quit" => break,
            _ => {}
        }
        writeln!(out, "@ok {frame}").ok();
        out.flush().ok();
    }
}
