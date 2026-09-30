//! The new COs' own music, with the Dual Strike pack: each new CO's turn
//! theme is Dual Strike's, converted at run time from the player's .nds to
//! a song for AW2's sound engine (MP2K, "Sappy") and played by it.
//!
//! **Which theme.** Dual Strike's CO record (arm9 `0x0215360C + 0x220*id`)
//! holds the CO's map music at `+0x14`: a sequence id of the sound archive
//! (`data/sound_data.sdat`). The pack keeps those sequences (SSEQ), their
//! instrument banks (SBNK) and sample archives (SWAR) ([`keep`]); nothing
//! else of the archive.
//!
//! **Sequence.** Every SSEQ track is walked (calls expanded, loops and
//! jumps followed) into timed notes and controls; the jump back that
//! repeats a track is its loop. An MP2K track is written for each (the
//! player plays at most [`MAX_TRACKS`]; beyond that the tracks that sound
//! least at the same time are merged). Dual Strike counts 48 ticks a beat
//! and MP2K 24; the song's TEMPO is doubled instead (the byte is the
//! beat count, not half of it), so every note and rest keeps its exact tick.
//! Volumes are Dual Strike's squared curves made linear, pan and pitch
//! bend carry over, notes longer than MP2K's longest are tied.
//!
//! **Instruments.** Every program a song plays is an MP2K rhythm voice
//! (128 sub-voices, one per key) so each key keeps its region's own root
//! note; its sample is decoded (IMA-ADPCM or PCM), filtered and resampled
//! to AW2's mixing rate (13379 Hz; loop lengths kept whole) and stored as
//! 8-bit, its envelope converted from Dual Strike's per-5.2 ms steps in
//! decibels to MP2K's per-frame factors.
//!
//! **Where.** Everything goes after the cartridge's 8 MB, from [`BASE`]
//! (mGBA grows the image when it is written there); AW2's song table
//! (`0x0824238C`, 505 songs) is copied there with the new songs after it
//! (ids from [`FIRST_SONG`]) and its six literal-pool words switched while
//! the pack is on. The new COs' rows then name their song
//! ([`crate::co_roster`]). AW2's own songs, players and mixer are untouched.

use mgba::core::Core;
use std::collections::HashMap;
use std::sync::OnceLock;

// --- Where things are ------------------------------------------------------

/// Start of the music: after the 8 MB cartridge image.
pub const BASE: u32 = 0x0880_0000;
const MAGIC: u32 = 0x4D53_4444; // "DDSM"
const TABLE: u32 = BASE + 0x100;
/// AW2's song table: 505 entries of (header, player, player).
const AW2_SONG_TABLE: u32 = 0x0824_238C;
const AW2_SONGS: u32 = 505;
/// Every ROM word holding the song table's address (a full scan finds these
/// six, in m4aSongNumStart and its siblings).
const SONG_POOL: [u32; 6] = [0x0807_04A0, 0x0807_04D4, 0x0807_0520, 0x0807_0574, 0x0807_05A8, 0x0807_2B9C];
/// The first new song's id.
pub const FIRST_SONG: u16 = AW2_SONGS as u16;
/// The music player the CO themes use (player 0 of `0x08242314`: 8 tracks).
const PLAYER: u16 = 1;
/// Tracks of that player.
pub const MAX_TRACKS: usize = 8;
/// The CO themes' reverb byte.
const REVERB: u8 = 173;
/// AW2's mixing rate (SoundInfo: 224 samples a frame).
const MIX_RATE: f64 = 13379.0;
/// Dual Strike's sequencer and envelope step: 64 * 2728 cycles at 33.5 MHz.
const DS_UPDATE: f64 = 64.0 * 2728.0 / 33_513_982.0;
/// The GBA's frame rate (MP2K's envelopes and tempo run per frame).
const GBA_FPS: f64 = 16_777_216.0 / 280_896.0;
/// Where Dual Strike's CO records are and where each one's music id is.
const DS_RECORDS: u32 = 0x0215_360C;
const DS_RECORD: u32 = 0x220;
const DS_MUSIC: u32 = 0x14;
/// Overall level: Dual Strike's squared volumes are quieter than AW2's
/// linear ones; this puts the new themes level with AW2's own.
const GAIN: f64 = 1.1;

// --- The sound archive (SDAT) ------------------------------------------------

fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?))
}
fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

/// The sequence ids of the new COs' themes (their records' `+0x14`), in
/// [`crate::co_new::NEW`]'s order.
fn theme_ids(arm9: &[u8]) -> Option<Vec<u16>> {
    crate::co_new::NEW
        .iter()
        .map(|&(ds, _)| u16_at(arm9, (DS_RECORDS - 0x0200_0000 + DS_RECORD * ds as u32 + DS_MUSIC) as usize))
        .collect()
}

/// From the sound archive, the files the new COs' themes need, as pack
/// files: `sound/seq/<id>` (INFO record, 12 bytes, then the SSEQ),
/// `sound/bank/<id>` (INFO record, then the SBNK), `sound/wave/<id>` (SWAR).
pub fn keep(sdat: &[u8], arm9: &[u8]) -> Option<Vec<(String, Vec<u8>)>> {
    let info = u32_at(sdat, 0x18)? as usize;
    let fat = u32_at(sdat, 0x20)? as usize;
    let file = |id: u16| -> Option<Vec<u8>> {
        let o = fat + 0x0C + 16 * id as usize;
        let (at, len) = (u32_at(sdat, o)? as usize, u32_at(sdat, o + 4)? as usize);
        sdat.get(at..at + len).map(|s| s.to_vec())
    };
    let record = |kind: usize, id: u16| -> Option<&[u8]> {
        let list = info + u32_at(sdat, info + 8 + 4 * kind)? as usize;
        if id as u32 >= u32_at(sdat, list)? {
            return None;
        }
        let at = u32_at(sdat, list + 4 + 4 * id as usize)? as usize;
        if at == 0 {
            return None;
        }
        sdat.get(info + at..info + at + 12)
    };
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    let mut have = std::collections::HashSet::new();
    for id in theme_ids(arm9)? {
        let seq = record(0, id)?;
        let bank = u16_at(seq, 4)?;
        if have.insert(format!("s{id}")) {
            out.push((format!("sound/seq/{id}"), [seq, &file(u16_at(seq, 0)?)?].concat()));
        }
        let b = record(2, bank)?;
        if have.insert(format!("b{bank}")) {
            out.push((format!("sound/bank/{bank}"), [b, &file(u16_at(b, 0)?)?].concat()));
        }
        for k in 0..4 {
            let w = u16_at(b, 4 + 2 * k)?;
            if w != 0xFFFF && have.insert(format!("w{w}")) {
                out.push((format!("sound/wave/{w}"), file(u16_at(record(3, w)?, 0)?)?));
            }
        }
    }
    Some(out)
}

// --- Sequences (SSEQ) ----------------------------------------------------------

/// A control a track changes.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Ctl {
    Prog(u8),
    Vol(u8),
    Expr(u8),
    Master(u8),
    Pan(u8),
    Bend(i8),
    BendRange(u8),
    Transpose(i8),
    Tempo(u16),
    ModDepth(u8),
    ModSpeed(u8),
    ModType(u8),
    ModDelay(u16),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Ev {
    Note { key: u8, vel: u8, len: u32, prog: u8 },
    Ctl(Ctl),
}

/// One track, walked: its events by tick, and the loop (the tick the jump
/// back goes to, the tick of the jump).
#[derive(Default, Debug)]
struct Track {
    evs: Vec<(u32, Ev)>,
    looped: Option<(u32, u32)>,
    end: u32,
}

struct Reader<'a> {
    d: &'a [u8],
}

impl Reader<'_> {
    fn byte(&self, p: &mut usize) -> Option<u8> {
        let b = *self.d.get(*p)?;
        *p += 1;
        Some(b)
    }
    fn var(&self, p: &mut usize) -> Option<u32> {
        let mut v = 0u32;
        for _ in 0..4 {
            let b = self.byte(p)?;
            v = (v << 7) | (b & 0x7F) as u32;
            if b & 0x80 == 0 {
                return Some(v);
            }
        }
        Some(v)
    }
    fn u16(&self, p: &mut usize) -> Option<u16> {
        let v = u16_at(self.d, *p)?;
        *p += 2;
        Some(v)
    }
    fn u24(&self, p: &mut usize) -> Option<usize> {
        let b = self.d.get(*p..*p + 3)?;
        *p += 3;
        Some(b[0] as usize | (b[1] as usize) << 8 | (b[2] as usize) << 16)
    }
}

/// Walk track `start` of the SSEQ data `d`.
fn walk(d: &[u8], start: usize) -> Option<Track> {
    let r = Reader { d };
    let mut t = Track::default();
    let (mut p, mut tick, mut prog) = (start, 0u32, 0u8);
    let mut note_wait = true;
    let mut calls: Vec<usize> = Vec::new();
    let mut loops: Vec<(usize, u8, u32, usize)> = Vec::new(); // (after D4, count left, tick, events)
    let mut seen: HashMap<usize, (u32, usize)> = HashMap::new();
    let mut steps = 0;
    loop {
        steps += 1;
        if steps > 400_000 || tick > 1 << 24 {
            break;
        }
        if calls.is_empty() {
            seen.entry(p).or_insert((tick, t.evs.len()));
        }
        let c = r.byte(&mut p)?;
        let ctl = |t: &mut Track, c: Ctl| t.evs.push((tick, Ev::Ctl(c)));
        match c {
            0x00..=0x7F => {
                let vel = r.byte(&mut p)? & 0x7F;
                let len = r.var(&mut p)?;
                t.evs.push((tick, Ev::Note { key: c, vel, len, prog }));
                if note_wait {
                    tick += len;
                }
            }
            0x80 => tick += r.var(&mut p)?,
            0x81 => {
                prog = r.var(&mut p)? as u8;
                ctl(&mut t, Ctl::Prog(prog));
            }
            0x93 => p += 4,
            0x94 | 0x95 => {
                let to = r.u24(&mut p)?;
                if c == 0x95 {
                    calls.push(p);
                    p = to;
                } else if calls.is_empty() {
                    if let Some(&(at, _)) = seen.get(&to) {
                        t.looped = Some((at, tick));
                        break;
                    }
                    p = to;
                } else {
                    p = to;
                }
            }
            0xA0 | 0xA1 => {
                // A random (the middle of its range is taken) or variable
                // (read as 0) last argument.
                let cmd = r.byte(&mut p)?;
                let vel = if cmd < 0x80 || (0xB0..=0xBD).contains(&cmd) { r.byte(&mut p)? } else { 0 };
                let v = if c == 0xA0 {
                    let (lo, hi) = (r.u16(&mut p)? as i16 as i32, r.u16(&mut p)? as i16 as i32);
                    ((lo + hi) / 2).max(0) as u32
                } else {
                    p += 1;
                    0
                };
                match cmd {
                    0x00..=0x7F => {
                        t.evs.push((tick, Ev::Note { key: cmd, vel: vel & 0x7F, len: v, prog }));
                        if note_wait {
                            tick += v;
                        }
                    }
                    0x80 => tick += v,
                    0xC0 => t.evs.push((tick, Ev::Ctl(Ctl::Pan(v as u8)))),
                    0xC1 => t.evs.push((tick, Ev::Ctl(Ctl::Vol(v as u8)))),
                    0xC4 => t.evs.push((tick, Ev::Ctl(Ctl::Bend(v as u8 as i8)))),
                    0xD5 => t.evs.push((tick, Ev::Ctl(Ctl::Expr(v as u8)))),
                    _ => {}
                }
            }
            0xA2 => {}
            0xB0..=0xBD => p += 3,
            0xC0..=0xD7 => {
                let v = r.byte(&mut p)?;
                match c {
                    0xC0 => ctl(&mut t, Ctl::Pan(v)),
                    0xC1 => ctl(&mut t, Ctl::Vol(v)),
                    0xC2 => ctl(&mut t, Ctl::Master(v)),
                    0xC3 => ctl(&mut t, Ctl::Transpose(v as i8)),
                    0xC4 => ctl(&mut t, Ctl::Bend(v as i8)),
                    0xC5 => ctl(&mut t, Ctl::BendRange(v)),
                    0xC7 => note_wait = v != 0,
                    0xCA => ctl(&mut t, Ctl::ModDepth(v)),
                    0xCB => ctl(&mut t, Ctl::ModSpeed(v)),
                    0xCC => ctl(&mut t, Ctl::ModType(v)),
                    0xD4 => loops.push((p, v, tick, t.evs.len())),
                    0xD5 => ctl(&mut t, Ctl::Expr(v)),
                    // priority, tie, portamento, ADSR overrides, print:
                    // not converted.
                    _ => {}
                }
            }
            0xE0 => {
                let v = r.u16(&mut p)?;
                ctl(&mut t, Ctl::ModDelay(v));
            }
            0xE1 => {
                let v = r.u16(&mut p)?;
                ctl(&mut t, Ctl::Tempo(v));
            }
            0xE3 => p += 2,
            0xFC => {
                if let Some(l) = loops.last_mut() {
                    if l.1 == 0 {
                        // An endless loop: the track's loop.
                        t.looped = Some((l.2, tick));
                        break;
                    }
                    l.1 -= 1;
                    if l.1 > 0 {
                        p = l.0;
                    } else {
                        loops.pop();
                    }
                }
            }
            0xFD => match calls.pop() {
                Some(q) => p = q,
                None => break,
            },
            0xFE => p += 2,
            0xFF => break,
            _ => return None,
        }
    }
    t.end = tick;
    Some(t)
}

/// A sequence: its tracks, and its INFO volume.
struct Seq {
    tracks: Vec<Track>,
    volume: u8,
    bank: u16,
}

fn parse_seq(file: &[u8]) -> Option<Seq> {
    let (info, sseq) = file.split_at(12);
    let base = u32_at(sseq, 0x18)? as usize;
    let d = sseq.get(base..)?;
    let r = Reader { d };
    let mut starts = vec![(0u8, 0usize)];
    let mut p = 0;
    loop {
        match *d.get(p)? {
            0xFE => p += 3,
            0x93 => {
                let mut q = p + 1;
                let n = r.byte(&mut q)?;
                let at = r.u24(&mut q)?;
                starts.push((n, at));
                p = q;
            }
            _ => break,
        }
    }
    starts[0].1 = p;
    starts.sort();
    let tracks = starts.iter().map(|&(_, at)| walk(d, at)).collect::<Option<Vec<_>>>()?;
    Some(Seq {
        tracks,
        volume: info[6],
        bank: u16_at(info, 4)?,
    })
}

// --- Instruments (SBNK) and samples (SWAR) -------------------------------------

#[derive(Clone, Copy, Debug)]
struct Region {
    kind: u8, // 1 PCM, 2 PSG square, 3 noise
    wave: u16,
    arc: u8,
    root: u8,
    attack: u8,
    decay: u8,
    sustain: u8,
    release: u8,
    pan: u8,
}

/// Program `prog`'s regions, `(lowest key, highest key, region)`.
fn regions(sbnk: &[u8], prog: u8) -> Vec<(u8, u8, Region)> {
    let get = || -> Option<Vec<(u8, u8, Region)>> {
        let n = u32_at(sbnk, 0x38)? as usize;
        if prog as usize >= n {
            return Some(Vec::new());
        }
        let e = 0x3C + 4 * prog as usize;
        let kind = *sbnk.get(e)?;
        let off = u16_at(sbnk, e + 1)? as usize;
        let region = |k: u8, o: usize| -> Option<Region> {
            let b = sbnk.get(o..o + 10)?;
            Some(Region {
                kind: k,
                wave: u16::from_le_bytes([b[0], b[1]]),
                arc: b[2],
                root: b[4],
                attack: b[5],
                decay: b[6],
                sustain: b[7],
                release: b[8],
                pan: b[9],
            })
        };
        let mut out = Vec::new();
        match kind {
            1..=3 => out.push((0, 127, region(kind, off)?)),
            16 => {
                let (lo, hi) = (*sbnk.get(off)?, *sbnk.get(off + 1)?);
                for k in lo..=hi.min(127) {
                    let o = off + 2 + 12 * (k - lo) as usize;
                    out.push((k, k, region(u16_at(sbnk, o)? as u8, o + 2)?));
                }
            }
            17 => {
                let (mut lo, mut o) = (0u8, off + 8);
                for i in 0..8 {
                    let hi = *sbnk.get(off + i)?;
                    if hi == 0 {
                        break;
                    }
                    out.push((lo, hi, region(u16_at(sbnk, o)? as u8, o + 2)?));
                    lo = hi.saturating_add(1);
                    o += 12;
                }
            }
            _ => {}
        }
        Some(out)
    };
    get().unwrap_or_default()
}

/// A decoded sample: 16-bit PCM, its rate, and the loop start (if it loops).
struct Sample {
    pcm: Vec<i16>,
    rate: u32,
    loop_start: Option<usize>,
}

const ADPCM_STEP: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97, 107, 118, 130, 143,
    157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552,
    1707, 1878, 2066, 2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493, 10442, 11487,
    12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];
const ADPCM_INDEX: [i32; 8] = [-1, -1, -1, -1, 2, 4, 6, 8];

/// Sample `i` of a SWAR.
fn sample(swar: &[u8], i: u16) -> Option<Sample> {
    if i as u32 >= u32_at(swar, 0x38)? {
        return None;
    }
    let o = u32_at(swar, 0x3C + 4 * i as usize)? as usize;
    let (kind, looped, rate) = (*swar.get(o)?, *swar.get(o + 1)? != 0, u16_at(swar, o + 2)? as u32);
    let (ls, ll) = (u16_at(swar, o + 6)? as usize * 4, u32_at(swar, o + 8)? as usize * 4);
    let d = swar.get(o + 12..o + 12 + ls + ll)?;
    let (pcm, loop_start) = match kind {
        0 => (d.iter().map(|&b| (b as i8 as i16) << 8).collect(), ls),
        1 => (d.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect(), ls / 2),
        2 => {
            let (mut s, mut idx) = (i16::from_le_bytes([d[0], d[1]]) as i32, (d[2] as i32).min(88));
            let mut out = Vec::with_capacity((d.len() - 4) * 2);
            for &b in &d[4..] {
                for n in [b & 15, b >> 4] {
                    let st = ADPCM_STEP[idx as usize];
                    let mut diff = st >> 3;
                    if n & 1 != 0 {
                        diff += st >> 2;
                    }
                    if n & 2 != 0 {
                        diff += st >> 1;
                    }
                    if n & 4 != 0 {
                        diff += st;
                    }
                    s = if n & 8 != 0 { s - diff } else { s + diff }.clamp(-0x7FFF, 0x7FFF);
                    idx = (idx + ADPCM_INDEX[(n & 7) as usize]).clamp(0, 88);
                    out.push(s as i16);
                }
            }
            (out, ls.saturating_sub(4) * 2)
        }
        _ => return None,
    };
    let loop_start = (looped && loop_start < pcm.len()).then_some(loop_start);
    Some(Sample { pcm, rate: rate.max(1), loop_start })
}

/// An MP2K sample: 8-bit, at `rate` Hz, looping from `loop_start`.
struct Wave {
    data: Vec<i8>,
    rate: f64,
    loop_start: Option<usize>,
}

/// Low-pass and resample to at most [`MIX_RATE`] (a loop keeps a whole
/// number of samples, and its rate is set to match), then 8 bits.
fn convert_sample(s: &Sample) -> Wave {
    let src = &s.pcm;
    let n = src.len();
    let r0 = s.rate as f64;
    let want = r0.min(MIX_RATE);
    let (ratio, out_len, out_loop) = match s.loop_start {
        Some(ls) => {
            let ll = (n - ls).max(1);
            let new_ll = ((ll as f64 * want / r0).round() as usize).max(1);
            let ratio = new_ll as f64 / ll as f64;
            let new_ls = (ls as f64 * ratio).round() as usize;
            (ratio, new_ls + new_ll, Some((new_ls, ls)))
        }
        None => {
            let ratio = want / r0;
            (ratio, ((n as f64 * ratio).ceil() as usize).max(1), None)
        }
    };
    // The source at any index: silence before and after, the loop repeated.
    let at = |i: i64| -> f64 {
        if i < 0 {
            return 0.0;
        }
        let i = i as usize;
        if i < n {
            return src[i] as f64;
        }
        match s.loop_start {
            Some(ls) if n > ls => src[ls + (i - ls) % (n - ls)] as f64,
            _ => 0.0,
        }
    };
    let data: Vec<i8> = if ratio >= 0.999 {
        (0..out_len).map(|j| ((at(j as i64) / 256.0).round().clamp(-128.0, 127.0)) as i8).collect()
    } else {
        // Windowed sinc, cut off at 90% of the new Nyquist.
        let cutoff = 0.9 * ratio * 0.5; // cycles per source sample
        let half = (8.0 / ratio).ceil() as i64;
        (0..out_len)
            .map(|j| {
                let pos = match out_loop {
                    Some((new_ls, ls)) if j >= new_ls => ls as f64 + (j - new_ls) as f64 / ratio,
                    _ => j as f64 / ratio,
                };
                let c = pos.floor() as i64;
                let (mut acc, mut wsum) = (0.0, 0.0);
                for i in c - half..=c + half {
                    let x = i as f64 - pos;
                    let sinc = if x.abs() < 1e-9 {
                        2.0 * cutoff
                    } else {
                        (2.0 * std::f64::consts::PI * cutoff * x).sin() / (std::f64::consts::PI * x)
                    };
                    let w = 0.5 + 0.5 * (std::f64::consts::PI * x / (half as f64 + 1.0)).cos();
                    acc += at(i) * sinc * w;
                    wsum += sinc * w;
                }
                let v = if wsum.abs() > 1e-9 { acc / wsum } else { 0.0 };
                (v / 256.0).round().clamp(-128.0, 127.0) as i8
            })
            .collect()
    };
    Wave {
        data,
        rate: r0 * ratio,
        loop_start: out_loop.map(|(l, _)| l),
    }
}

/// Keys of the made-up PSG samples in the wave cache.
const PSG_SQUARE: u16 = 0xFFFE;
const PSG_NOISE: u16 = 0xFFFD;
/// Middle C (MP2K's key 60 at a sample's own rate).
const MIDDLE_C: f64 = 261.625_565_3;

/// Dual Strike's PSG square, duty `(d + 1) / 8`: one 64-sample period that
/// sounds middle C at its rate (quieter than full scale, as the DS's PSG).
fn psg_square(d: u16) -> Wave {
    let high = 8 * (d as usize + 1);
    Wave {
        data: (0..64).map(|i| if i < high { 48 } else { -48 }).collect(),
        rate: MIDDLE_C * 64.0,
        loop_start: Some(0),
    }
}

/// Dual Strike's PSG noise: a looped stretch of a 15-bit LFSR's output.
fn psg_noise() -> Wave {
    let mut lfsr: u16 = 0x7FFF;
    let data = (0..8192)
        .map(|_| {
            let bit = (lfsr ^ (lfsr >> 1)) & 1;
            lfsr = (lfsr >> 1) | (bit << 14);
            if lfsr & 1 != 0 { 40 } else { -40 }
        })
        .collect();
    Wave { data, rate: MIX_RATE, loop_start: Some(0) }
}

/// An MP2K sample: header (type, loop flag, rate * 1024, loop start,
/// length), the 8-bit data and one more sample for the mixer's
/// interpolation (the loop's first, or silence).
fn wave_bytes(w: &Wave) -> Vec<u8> {
    let mut h = Vec::with_capacity(16 + w.data.len() + 1);
    h.extend_from_slice(&0u16.to_le_bytes());
    h.extend_from_slice(&(if w.loop_start.is_some() { 0x4000u16 } else { 0 }).to_le_bytes());
    h.extend_from_slice(&((w.rate * 1024.0).round() as u32).to_le_bytes());
    h.extend_from_slice(&(w.loop_start.unwrap_or(0) as u32).to_le_bytes());
    h.extend_from_slice(&(w.data.len() as u32).to_le_bytes());
    h.extend(w.data.iter().map(|&x| x as u8));
    h.push(match w.loop_start {
        Some(l) => w.data[l] as u8,
        None => 0,
    });
    h
}

// --- Envelopes and volumes --------------------------------------------------------

/// Dual Strike's volume curve: `(v / 127)^2`, linear.
fn square(v: u8) -> f64 {
    let v = v.min(127) as f64 / 127.0;
    v * v
}

/// Dual Strike's attack: per step, attenuation * rate / 256.
fn ds_attack_rate(a: u8) -> u32 {
    const TOP: [u32; 19] = [0x00, 0x01, 0x05, 0x0E, 0x1A, 0x26, 0x33, 0x3F, 0x49, 0x54, 0x5C, 0x64, 0x6D, 0x74, 0x7B, 0x7F, 0x84, 0x89, 0x8F];
    let a = a.min(127);
    if a < 109 {
        255 - a as u32
    } else {
        TOP[(127 - a) as usize]
    }
}

/// Dual Strike's decay and release: attenuation falls by this many 1/1280 dB per step.
fn ds_fall_rate(r: u8) -> u32 {
    match r.min(127) {
        127 => 0xFFFF,
        126 => 0x3C00,
        r if r < 50 => r as u32 * 2 + 1,
        r => 0x1E00 / (126 - r as u32),
    }
}

/// MP2K's per-frame factor for a Dual Strike fall rate.
fn mp2k_fall(r: u8) -> u8 {
    let rate = ds_fall_rate(r);
    if rate >= 0xFFFF {
        return 0;
    }
    let db_per_frame = rate as f64 / 1280.0 / (DS_UPDATE * GBA_FPS);
    (256.0 * 10f64.powf(-db_per_frame / 20.0)).round().clamp(0.0, 255.0) as u8
}

/// MP2K's attack step for a Dual Strike attack: the frames Dual Strike takes
/// to come within 1 dB of full.
fn mp2k_attack(a: u8) -> u8 {
    let rate = ds_attack_rate(a) as i64;
    let mut amp: i64 = -92544;
    let mut steps = 0;
    while amp < -1280 && steps < 10_000 {
        amp = amp * rate / 256;
        steps += 1;
    }
    let frames = steps as f64 * DS_UPDATE * GBA_FPS;
    if frames <= 1.0 {
        255
    } else {
        (255.0 / frames).ceil().clamp(1.0, 255.0) as u8
    }
}

/// `[attack, decay, sustain, release]` for MP2K.
fn adsr(r: &Region) -> [u8; 4] {
    [
        mp2k_attack(r.attack),
        mp2k_fall(r.decay),
        (255.0 * square(r.sustain)).round() as u8,
        mp2k_fall(r.release),
    ]
}

// --- MP2K ------------------------------------------------------------------------

/// Note and wait lengths MP2K has commands for (`N01..N96`, `W01..W96`).
const LENGTHS: [u32; 48] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 28, 30, 32, 36, 40, 42, 44, 48, 52, 54, 56, 60,
    64, 66, 68, 72, 76, 78, 80, 84, 88, 90, 92, 96,
];
const WAIT: u8 = 0x80; // W00; W(LENGTHS[i]) is 0x81 + i
const NOTE: u8 = 0xD0; // N(LENGTHS[i]) is 0xD0 + i
const FINE: u8 = 0xB1;
const GOTO: u8 = 0xB2;
const TEMPO: u8 = 0xBB;
const KEYSH: u8 = 0xBC;
const VOICE: u8 = 0xBD;
const VOL: u8 = 0xBE;
const PAN: u8 = 0xBF;
const BEND: u8 = 0xC0;
const BENDR: u8 = 0xC1;
const LFOS: u8 = 0xC2;
const LFODL: u8 = 0xC3;
const MOD: u8 = 0xC4;
const MODT: u8 = 0xC5;
const EOT: u8 = 0xCE;
const TIE: u8 = 0xCF;

/// Waits for `ticks`.
fn waits(out: &mut Vec<u8>, mut ticks: u32) {
    while ticks > 0 {
        let i = LENGTHS.iter().rposition(|&l| l <= ticks).unwrap();
        out.push(WAIT + 1 + i as u8);
        ticks -= LENGTHS[i];
    }
}

/// A note of `len` ticks: `Nxx key vel [extra]`, or `None` if longer than
/// MP2K's longest (then it is tied).
fn note(key: u8, vel: u8, len: u32) -> Option<Vec<u8>> {
    let len = len.max(1);
    let i = LENGTHS.iter().rposition(|&l| l <= len)?;
    let extra = len - LENGTHS[i];
    if extra > 3 {
        return None;
    }
    let mut v = vec![NOTE + i as u8, key, vel.max(1)];
    if extra > 0 {
        v.push(extra as u8);
    }
    Some(v)
}

/// A track's state as MP2K sees it.
#[derive(Clone, Copy, PartialEq, Debug)]
struct State {
    prog: u8,
    vol: u8,
    expr: u8,
    master: u8,
    pan: u8,
    bend: i8,
    bend_range: u8,
    transpose: i8,
    mod_depth: u8,
    mod_speed: u8,
    mod_type: u8,
    mod_delay: u16,
}

impl Default for State {
    fn default() -> Self {
        State {
            prog: 0,
            vol: 127,
            expr: 127,
            master: 127,
            pan: 64,
            bend: 0,
            bend_range: 2,
            transpose: 0,
            mod_depth: 0,
            mod_speed: 16,
            mod_type: 0,
            mod_delay: 0,
        }
    }
}

impl State {
    fn apply(&mut self, c: Ctl) {
        match c {
            Ctl::Prog(v) => self.prog = v,
            Ctl::Vol(v) => self.vol = v,
            Ctl::Expr(v) => self.expr = v,
            Ctl::Master(v) => self.master = v,
            Ctl::Pan(v) => self.pan = v,
            Ctl::Bend(v) => self.bend = v,
            Ctl::BendRange(v) => self.bend_range = v,
            Ctl::Transpose(v) => self.transpose = v,
            Ctl::ModDepth(v) => self.mod_depth = v,
            Ctl::ModSpeed(v) => self.mod_speed = v,
            Ctl::ModType(v) => self.mod_type = v,
            Ctl::ModDelay(v) => self.mod_delay = v,
            Ctl::Tempo(_) => {}
        }
    }
}

/// The MP2K commands that take `from` to `to` (`None`: nothing is known).
fn state_commands(out: &mut Vec<u8>, from: Option<&State>, to: &State, song_volume: u8, tick_rate: f64) {
    let vol = |s: &State| -> u8 {
        (127.0 * GAIN * square(song_volume) * square(s.master) * square(s.vol) * square(s.expr))
            .round()
            .clamp(0.0, 127.0) as u8
    };
    let lfo_speed = |s: &State| -> u8 {
        // Dual Strike: phase += speed << 6 per step, 0x10000 a cycle; MP2K:
        // 256 a cycle, stepped per tick.
        let hz = s.mod_speed as f64 * 64.0 / 65536.0 / DS_UPDATE;
        (hz * 256.0 / tick_rate).round().clamp(1.0, 255.0) as u8
    };
    let changed = |f: fn(&State) -> u32| from.is_none_or(|a| f(a) != f(to));
    if changed(|s| s.prog as u32) {
        out.extend([VOICE, to.prog & 0x7F]);
    }
    if from.is_none_or(|a| vol(a) != vol(to)) {
        out.extend([VOL, vol(to)]);
    }
    if changed(|s| s.pan as u32) {
        out.extend([PAN, to.pan.min(127)]);
    }
    if changed(|s| s.bend_range as u32) {
        out.extend([BENDR, to.bend_range.min(127)]);
    }
    if changed(|s| s.bend as u8 as u32) {
        out.extend([BEND, (64 + (to.bend as i32 >> 1)).clamp(0, 127) as u8]);
    }
    if changed(|s| s.transpose as u8 as u32) {
        out.extend([KEYSH, to.transpose as u8]);
    }
    if from.is_none_or(|a| lfo_speed(a) != lfo_speed(to)) && (to.mod_depth > 0 || from.is_some()) {
        out.extend([LFOS, lfo_speed(to)]);
    }
    if changed(|s| s.mod_delay as u32) {
        let ticks = (to.mod_delay as f64 * DS_UPDATE * tick_rate).round().clamp(0.0, 127.0) as u8;
        out.extend([LFODL, ticks]);
    }
    if changed(|s| s.mod_type as u32) {
        out.extend([MODT, to.mod_type.min(2)]);
    }
    if changed(|s| s.mod_depth as u32) {
        out.extend([MOD, to.mod_depth.min(127)]);
    }
}

/// One MP2K track's events: the Dual Strike tracks it plays (`src`), each
/// event with its source.
#[derive(Clone, Copy)]
enum Item {
    Ev(usize, Ev),
    Eot(u8),
}

/// A track written: its bytes (with the loop's GOTO target as an offset).
struct TrackOut {
    bytes: Vec<u8>,
    goto_at: Option<(usize, usize)>, // (offset of the GOTO's address, offset it goes to)
}

/// Write the MP2K track for Dual Strike tracks `srcs` of `seq`.
fn write_track(seq: &Seq, srcs: &[usize], song_loop: Option<(u32, u32)>, tick_rate: f64) -> TrackOut {
    let song_end = seq.tracks.iter().map(|t| t.end).max().unwrap_or(0);
    // Every event in order: at a tick, each source's events in their order.
    let mut items: Vec<(u32, usize, Item)> = Vec::new();
    for (si, &s) in srcs.iter().enumerate() {
        for &(tick, ev) in seq.tracks[s].evs.iter() {
            items.push((tick, si, Item::Ev(si, ev)));
        }
    }
    items.sort_by_key(|&(t, si, _)| (t, si));
    let (loop_start, loop_end) = match song_loop {
        Some((a, b)) => (a, b),
        None => (u32::MAX, song_end),
    };
    // Notes too long for one command are tied; their EOT goes in at the
    // tick they end (inside the loop, after the loop's start again).
    let mut eots: Vec<(u32, u8)> = Vec::new();
    let mut expanded: Vec<(u32, u8, Item)> = Vec::new(); // (tick, order, item); EOT first at a tick
    for &(tick, _, it) in &items {
        if tick >= loop_end && song_loop.is_some() {
            continue;
        }
        if let Item::Ev(_, Ev::Note { key, len, .. }) = it {
            if note(key, 1, len).is_none() {
                let mut end = tick + len;
                if song_loop.is_some() && end >= loop_end {
                    end = loop_start + (end - loop_end) % (loop_end - loop_start).max(1);
                }
                eots.push((end, key));
            }
        }
        expanded.push((tick, 1, it));
    }
    for (t, k) in eots {
        expanded.push((t, 0, Item::Eot(k)));
    }
    expanded.sort_by_key(|&(t, o, _)| (t, o));

    let n = srcs.len();
    let mut out = Vec::new();
    // Emission, with the sources' states and the state MP2K has.
    let emit = |out: &mut Vec<u8>,
                list: &[(u32, u8, Item)],
                from_tick: u32,
                srcst: &mut Vec<State>,
                cur: &mut Option<State>,
                owner: &mut usize| {
        let mut t = from_tick;
        for &(tick, _, it) in list {
            match it {
                Item::Ev(s, Ev::Ctl(c)) => {
                    srcst[s].apply(c);
                    if let Ctl::Tempo(bpm) = c {
                        waits(out, tick - t);
                        t = tick;
                        // Dual Strike: 0.8 ticks per beat-per-minute a second;
                        // MP2K: 2 * byte * fps / 150 ticks a second.
                        let byte = (bpm as f64 * (1.0 / (240.0 * DS_UPDATE)) * 150.0 / (2.0 * GBA_FPS)).round().clamp(1.0, 255.0);
                        out.extend([TEMPO, byte as u8]);
                        continue;
                    }
                    if s == *owner {
                        if let Some(c0) = cur.as_ref() {
                            if *c0 != srcst[s] {
                                waits(out, tick - t);
                                t = tick;
                                state_commands(out, Some(c0), &srcst[s], seq.volume, tick_rate);
                                *cur = Some(srcst[s]);
                            }
                        }
                    }
                }
                Item::Ev(s, Ev::Note { key, vel, len, .. }) => {
                    waits(out, tick - t);
                    t = tick;
                    if s != *owner || cur.is_none() || cur.as_ref() != Some(&srcst[s]) {
                        *owner = s;
                        state_commands(out, cur.as_ref(), &srcst[s], seq.volume, tick_rate);
                        *cur = Some(srcst[s]);
                    }
                    let v = (127.0 * square(vel)).round().clamp(1.0, 127.0) as u8;
                    match note(key, v, len) {
                        Some(b) => out.extend(b),
                        None => out.extend([TIE, key, v]),
                    }
                }
                Item::Eot(key) => {
                    waits(out, tick - t);
                    t = tick;
                    out.extend([EOT, key]);
                }
            }
        }
        t
    };
    let mut srcst = vec![State::default(); n];
    let mut cur: Option<State> = None;
    let mut owner = 0usize;
    let intro: Vec<_> = expanded.iter().copied().filter(|e| e.0 < loop_start).collect();
    let body: Vec<_> = expanded.iter().copied().filter(|e| e.0 >= loop_start).collect();
    let t = emit(&mut out, &intro, 0, &mut srcst, &mut cur, &mut owner);
    let Some(_) = song_loop else {
        emit(&mut out, &body, t, &mut srcst, &mut cur, &mut owner);
        out.push(FINE);
        return TrackOut { bytes: out, goto_at: None };
    };
    waits(&mut out, loop_start.saturating_sub(t));
    // The loop's later passes start from the state the loop ends with: a
    // dry pass gives it; the loop sets it in full at its start (so every
    // pass, the first too, starts from it) and is written from there.
    let (mut s2, mut c2, mut o2) = (srcst.clone(), cur, owner);
    emit(&mut Vec::new(), &body, loop_start, &mut s2, &mut c2, &mut o2);
    let label = out.len();
    if let Some(c) = c2.as_ref() {
        state_commands(&mut out, None, c, seq.volume, tick_rate);
    }
    let (mut s3, mut c3, mut o3) = (s2, c2, o2);
    let t = emit(&mut out, &body, loop_start, &mut s3, &mut c3, &mut o3);
    waits(&mut out, loop_end - t);
    out.push(GOTO);
    let at = out.len();
    out.extend([0; 4]);
    TrackOut { bytes: out, goto_at: Some((at, label)) }
}

/// Sounding time two sets of tracks share (ticks), for choosing merges.
fn overlap(seq: &Seq, a: &[usize], b: &[usize], until: u32) -> u64 {
    let busy = |srcs: &[usize]| -> Vec<(u32, u32)> {
        let mut v: Vec<(u32, u32)> = srcs
            .iter()
            .flat_map(|&s| seq.tracks[s].evs.iter())
            .filter_map(|&(t, e)| match e {
                Ev::Note { len, .. } if t < until => Some((t, t + len.max(1) + 24)),
                _ => None,
            })
            .collect();
        v.sort();
        let mut m: Vec<(u32, u32)> = Vec::new();
        for (s, e) in v {
            match m.last_mut() {
                Some(l) if s <= l.1 => l.1 = l.1.max(e),
                _ => m.push((s, e)),
            }
        }
        m
    };
    let (x, y) = (busy(a), busy(b));
    let (mut i, mut j, mut tot) = (0, 0, 0u64);
    while i < x.len() && j < y.len() {
        let (s, e) = (x[i].0.max(y[j].0), x[i].1.min(y[j].1));
        if s < e {
            tot += (e - s) as u64;
        }
        if x[i].1 < y[j].1 {
            i += 1;
        } else {
            j += 1;
        }
    }
    tot
}

/// Which Dual Strike tracks each MP2K track plays: one each, or merged
/// (the pairs that sound together least) down to [`MAX_TRACKS`].
fn plan_tracks(seq: &Seq, until: u32) -> Vec<Vec<usize>> {
    let mut groups: Vec<Vec<usize>> = (0..seq.tracks.len())
        .filter(|&t| seq.tracks[t].evs.iter().any(|e| matches!(e.1, Ev::Note { .. })))
        .map(|t| vec![t])
        .collect();
    // Tempo lives on a track without notes sometimes: keep its events.
    for t in 0..seq.tracks.len() {
        if !groups.iter().any(|g| g.contains(&t)) && seq.tracks[t].evs.iter().any(|e| matches!(e.1, Ev::Ctl(Ctl::Tempo(_)))) {
            groups.insert(0, vec![t]);
        }
    }
    while groups.len() > MAX_TRACKS {
        let mut best = (u64::MAX, 0, 1);
        for i in 0..groups.len() {
            for j in i + 1..groups.len() {
                let o = overlap(seq, &groups[i], &groups[j], until);
                if o < best.0 {
                    best = (o, i, j);
                }
            }
        }
        let g = groups.remove(best.2);
        groups[best.1].extend(g);
        groups[best.1].sort();
    }
    groups
}

/// Everything the music adds, laid out from [`BASE`].
pub struct Music {
    pub blob: Vec<u8>,
    /// Song id per new CO, in [`crate::co_new::NEW`]'s order.
    pub songs: Vec<u16>,
    /// Each song's header address (in [`Music::songs`]' id order from [`FIRST_SONG`]).
    pub headers: Vec<u32>,
}

struct Blob {
    bytes: Vec<u8>,
}

impl Blob {
    fn at(&self) -> u32 {
        BASE + self.bytes.len() as u32
    }
    fn align(&mut self) {
        while self.bytes.len() % 4 != 0 {
            self.bytes.push(0);
        }
    }
    fn put(&mut self, b: &[u8]) -> u32 {
        self.align();
        let a = self.at();
        self.bytes.extend_from_slice(b);
        a
    }
}

fn pack_file(path: &str) -> Option<&'static [u8]> {
    crate::ds_pack::pack()?.file(path)
}

/// Convert every new CO's theme.
fn build() -> Option<Music> {
    let pack = crate::ds_pack::pack()?;
    let ids = theme_ids(&pack.arm9)?;
    let mut blob = Blob { bytes: vec![0; (TABLE - BASE) as usize] };
    // The song table: AW2's, then ours (filled in below).
    let mut distinct: Vec<u16> = Vec::new();
    for &id in &ids {
        if !distinct.contains(&id) {
            distinct.push(id);
        }
    }
    let table_at = TABLE;
    blob.bytes.resize((TABLE - BASE) as usize + 8 * (AW2_SONGS as usize + distinct.len()), 0);
    let mut headers = Vec::new();
    let mut waves: HashMap<(u16, u16), u32> = HashMap::new();
    let silent = {
        let mut w = Vec::new();
        w.extend_from_slice(&0u16.to_le_bytes());
        w.extend_from_slice(&0u16.to_le_bytes());
        w.extend_from_slice(&((MIX_RATE * 1024.0) as u32).to_le_bytes());
        w.extend_from_slice(&0u32.to_le_bytes());
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&[0; 17]);
        blob.put(&w)
    };
    let voice_silent = {
        let mut v = [0u8; 12];
        v[0] = 0;
        v[1] = 60;
        v[4..8].copy_from_slice(&silent.to_le_bytes());
        v[8..12].copy_from_slice(&[255, 0, 255, 0]);
        v
    };
    for (n, &sid) in distinct.iter().enumerate() {
        let seq = parse_seq(pack_file(&format!("sound/seq/{sid}"))?)?;
        let bank = pack_file(&format!("sound/bank/{}", seq.bank))?;
        let (binfo, sbnk) = bank.split_at(12);
        let arcs: Vec<u16> = (0..4).map(|k| u16_at(binfo, 4 + 2 * k).unwrap_or(0xFFFF)).collect();
        // The loop: the latest loop start of the tracks, and their end.
        let loops: Vec<(u32, u32)> = seq.tracks.iter().filter_map(|t| t.looped).collect();
        let song_loop = loops.iter().copied().max_by_key(|l| l.1);
        let until = song_loop.map_or(seq.tracks.iter().map(|t| t.end).max().unwrap_or(0), |l| l.1);
        // Ticks a second: the song's main tempo (the loop's).
        let bpm = seq
            .tracks
            .iter()
            .flat_map(|t| t.evs.iter())
            .filter_map(|&(_, e)| match e {
                Ev::Ctl(Ctl::Tempo(b)) => Some(b),
                _ => None,
            })
            .last()
            .unwrap_or(120);
        let tick_rate = bpm as f64 / (240.0 * DS_UPDATE);
        // Instruments: every program played.
        let mut progs: Vec<u8> = seq
            .tracks
            .iter()
            .flat_map(|t| t.evs.iter())
            .filter_map(|&(_, e)| match e {
                Ev::Note { prog, .. } => Some(prog),
                _ => None,
            })
            .collect();
        progs.sort();
        progs.dedup();
        let mut group = Vec::with_capacity(128 * 12);
        for _ in 0..128 {
            group.extend_from_slice(&voice_silent);
        }
        for &p in &progs {
            let mut sub = Vec::with_capacity(128 * 12);
            let regs = regions(sbnk, p);
            for k in 0..128u8 {
                let Some(&(_, _, r)) = regs.iter().find(|(lo, hi, _)| (*lo..=*hi).contains(&k)) else {
                    sub.extend_from_slice(&voice_silent);
                    continue;
                };
                let mut v = [0u8; 12];
                let pan = if r.pan != 64 { 0x80 | (0x40 + r.pan as i32 - 64).clamp(0, 0x7F) as u8 } else { 0 };
                match r.kind {
                    1..=3 => {
                        // PCM regions play their sample; Dual Strike's PSG
                        // square (duty 0..6 of 8) and noise play a sample
                        // made for them, so everything goes through MP2K's
                        // DirectSound channels (no GB channels).
                        let arc = arcs.get(r.arc as usize).copied().unwrap_or(0xFFFF);
                        let key = match r.kind {
                            1 => (arc, r.wave),
                            2 => (PSG_SQUARE, r.wave.min(6)),
                            _ => (PSG_NOISE, 0),
                        };
                        let at = match waves.get(&key) {
                            Some(&w) => w,
                            None => {
                                let w = match r.kind {
                                    1 => pack_file(&format!("sound/wave/{arc}")).and_then(|f| sample(f, r.wave)).map(|s| convert_sample(&s)),
                                    2 => Some(psg_square(key.1)),
                                    _ => Some(psg_noise()),
                                };
                                let Some(w) = w else {
                                    sub.extend_from_slice(&voice_silent);
                                    continue;
                                };
                                let at = blob.put(&wave_bytes(&w));
                                waves.insert(key, at);
                                at
                            }
                        };
                        v[0] = 0x00;
                        v[1] = (60 + k as i32 - r.root as i32).clamp(0, 127) as u8;
                        v[3] = pan;
                        v[4..8].copy_from_slice(&at.to_le_bytes());
                        v[8..12].copy_from_slice(&adsr(&r));
                    }
                    _ => {
                        sub.extend_from_slice(&voice_silent);
                        continue;
                    }
                }
                sub.extend_from_slice(&v);
            }
            let sub_at = blob.put(&sub);
            let e = 12 * p as usize;
            group[e..e + 12].copy_from_slice(&[0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            group[e + 4..e + 8].copy_from_slice(&sub_at.to_le_bytes());
        }
        let group_at = blob.put(&group);
        // Tracks.
        let plan = plan_tracks(&seq, until);
        let mut track_at = Vec::new();
        for srcs in &plan {
            let t = write_track(&seq, srcs, song_loop, tick_rate);
            let mut bytes = t.bytes;
            blob.align();
            let at = blob.at();
            if let Some((goto, label)) = t.goto_at {
                bytes[goto..goto + 4].copy_from_slice(&(at + label as u32).to_le_bytes());
            }
            blob.put(&bytes);
            track_at.push(at);
        }
        let mut header = vec![track_at.len() as u8, 0, 0, REVERB];
        header.extend_from_slice(&group_at.to_le_bytes());
        for a in &track_at {
            header.extend_from_slice(&a.to_le_bytes());
        }
        let header_at = blob.put(&header);
        headers.push(header_at);
        let e = (table_at - BASE) as usize + 8 * (AW2_SONGS as usize + n);
        blob.bytes[e..e + 4].copy_from_slice(&header_at.to_le_bytes());
        blob.bytes[e + 4..e + 6].copy_from_slice(&PLAYER.to_le_bytes());
        blob.bytes[e + 6..e + 8].copy_from_slice(&PLAYER.to_le_bytes());
    }
    blob.bytes[0..4].copy_from_slice(&MAGIC.to_le_bytes());
    let songs = ids
        .iter()
        .map(|id| FIRST_SONG + distinct.iter().position(|d| d == id).unwrap() as u16)
        .collect();
    Some(Music { blob: blob.bytes, songs, headers })
}

static BUILT: OnceLock<Option<Music>> = OnceLock::new();

/// The converted music (built on first use from the pack).
pub fn music() -> Option<&'static Music> {
    BUILT.get_or_init(build).as_ref()
}

/// The song a new CO's turn plays, with the pack.
pub fn song(co: u8) -> Option<u16> {
    let i = crate::co_new::NEW.iter().position(|&(ds, _)| Some(ds) == crate::co_new::ds_id(co))?;
    music()?.songs.get(i).copied()
}

fn install(core: &mut Core) -> bool {
    if core.raw_read_32(BASE, -1) == MAGIC {
        return true;
    }
    let Some(m) = music() else {
        return false;
    };
    // AW2's songs first, as the game has them.
    let mut aw2 = vec![0u8; 8 * AW2_SONGS as usize];
    core.raw_read_range(AW2_SONG_TABLE, -1, &mut aw2);
    core.raw_write_range(BASE + 4, -1, &m.blob[4..(TABLE - BASE) as usize]);
    core.raw_write_range(TABLE, -1, &aw2);
    let rest = (TABLE - BASE) as usize + aw2.len();
    core.raw_write_range(BASE + rest as u32, -1, &m.blob[rest..]);
    core.raw_write_32(BASE, -1, MAGIC);
    true
}

/// The sound engine's state (`SoundInfo`, the address at `0x03007FF0`):
/// `+6` the DirectSound channels mixed, `+0x50` the 12 channels (0x40 bytes
/// each; AW2 mixes 8, the other 4 sit unused before the PCM buffer).
const SOUND_INFO_PTR: u32 = 0x0300_7FF0;
const SOUND_INFO_IDENT: u32 = 0x6873_6D53; // "Smsh"
const MAX_CHANS: u32 = 6;
const CHANNELS: u32 = 0x50;
const CHANNEL: u32 = 0x40;
const AW2_CHANNELS: u8 = 8;
const ALL_CHANNELS: u8 = 12;
/// The music player the CO themes play on (`MusicPlayerInfo`): its song.
const BGM_PLAYER: u32 = 0x0300_5AE0;

/// Dual Strike's themes play up to 16 notes at once; while one of them is
/// the music, the engine mixes all 12 of its channels, AW2's 8 otherwise
/// (the 4 extra ones stopped when it goes back).
fn channels(core: &mut Core, on: bool) {
    let info = core.raw_read_32(SOUND_INFO_PTR, -1);
    if !(0x0300_0000..0x0300_8000).contains(&info) || core.raw_read_32(info, -1) != SOUND_INFO_IDENT {
        return;
    }
    let song = core.raw_read_32(BGM_PLAYER, -1);
    let ours = on && music().is_some_and(|m| m.headers.contains(&song));
    let want = if ours { ALL_CHANNELS } else { AW2_CHANNELS };
    let have = core.raw_read_8(info + MAX_CHANS, -1);
    if have == want || !(have == AW2_CHANNELS || have == ALL_CHANNELS) {
        return;
    }
    if want == AW2_CHANNELS {
        for c in AW2_CHANNELS as u32..ALL_CHANNELS as u32 {
            core.raw_write_8(info + CHANNELS + CHANNEL * c, -1, 0);
        }
    }
    core.raw_write_8(info + MAX_CHANS, -1, want);
}

/// Every frame: the game reads tangoAW2's song table with the pack on,
/// AW2's without (idempotent; the same on both netplay peers).
pub fn tick(core: &mut Core, on: bool) {
    let on = on && install(core);
    if !on && core.raw_read_32(BASE, -1) != MAGIC {
        return;
    }
    let table = if on { TABLE } else { AW2_SONG_TABLE };
    for at in SONG_POOL {
        if core.raw_read_32(at, -1) != table {
            core.raw_write_32(at, -1, table);
        }
    }
    channels(core, on);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_add_up() {
        for t in 0..400 {
            let mut v = Vec::new();
            waits(&mut v, t);
            let sum: u32 = v.iter().map(|&b| LENGTHS[(b - WAIT - 1) as usize]).sum();
            assert_eq!(sum, t);
        }
    }

    #[test]
    fn notes_keep_their_length() {
        for len in 1..=99 {
            let b = note(60, 100, len).unwrap();
            let l = LENGTHS[(b[0] - NOTE) as usize] + b.get(3).copied().unwrap_or(0) as u32;
            assert_eq!(l, len);
        }
        assert!(note(60, 100, 100).is_none());
    }

    #[test]
    fn envelopes() {
        assert_eq!(mp2k_attack(127), 255);
        assert!(mp2k_attack(100) < 255);
        assert_eq!(mp2k_fall(127), 0);
        // Slower releases keep more each frame.
        assert!(mp2k_fall(60) > mp2k_fall(100) && mp2k_fall(100) > mp2k_fall(120));
        assert_eq!(adsr(&Region { kind: 1, wave: 0, arc: 0, root: 60, attack: 127, decay: 127, sustain: 127, release: 127, pan: 64 })[2], 255);
    }

    #[test]
    fn resampling_keeps_loops_whole() {
        let pcm: Vec<i16> = (0..3000).map(|i| ((i as f64 * 0.05).sin() * 20000.0) as i16).collect();
        let w = convert_sample(&Sample { pcm, rate: 22050, loop_start: Some(1000) });
        let ls = w.loop_start.unwrap();
        let new_ll = w.data.len() - ls;
        assert!((w.rate * 2000.0 / 22050.0 - new_ll as f64).abs() < 1e-6);
        assert!(w.rate <= MIX_RATE + 10.0);
    }

    #[test]
    fn a_track_loops() {
        // prog 1; vol 100; note 60 len 10; rest 10; jump back to the note.
        let d = [0x81, 1, 0xC1, 100, 60, 100, 10, 0x80, 10, 0x94, 4, 0, 0];
        let t = walk(&d, 0).unwrap();
        assert_eq!(t.looped, Some((0, 20))); // a note waits its length by default
        assert_eq!(t.evs.len(), 3);
    }
}

#[cfg(test)]
mod pack_tests {
    use super::*;

    /// The nine themes convert (needs `TANGOAW2_DS_ROM`):
    /// `cargo test -p tango-gamesupport-aw2 ds_music -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn themes_convert() {
        let m = music().expect("music");
        eprintln!("music: {} KB, songs {:?}", m.blob.len() / 1024, m.songs);
        let pack = crate::ds_pack::pack().unwrap();
        for id in theme_ids(&pack.arm9).unwrap() {
            let seq = parse_seq(pack_file(&format!("sound/seq/{id}")).unwrap()).unwrap();
            let loops: Vec<_> = seq.tracks.iter().map(|t| t.looped).collect();
            let loops2: Vec<_> = loops.iter().flatten().collect();
            let until = loops2.iter().map(|l| l.1).max().unwrap_or(0);
            let plan = plan_tracks(&seq, until);
            eprintln!("seq {id}: {} tracks -> {:?}, loops {:?}", seq.tracks.len(), plan, loops);
            assert!(plan.len() <= MAX_TRACKS);
        }
    }
}

