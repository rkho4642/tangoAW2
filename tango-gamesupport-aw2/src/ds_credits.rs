//! The DS Campaign's staff credits: after Means to an End's ending, Dual
//! Strike's staff roll in AW2's staff roll, with Dual Strike's music.
//!
//! Dual Strike keeps its staff roll in overlay 5 (the story module): a
//! list of 28 sections at `0x0236A418` (null-terminated), each a stream of
//! words: `0` a blank line, `1 <text>` a name, `2 <text>` a heading,
//! `3 <text>` a second heading line, and a last word, the section's time in
//! frames (120, or 200 for the first and the last).
//!
//! AW2's staff roll (`StartStaffRoll` `0x0806C874` starts proc script
//! `0x08581AC8`) reads its pages from a null-terminated list at
//! `0x0858265C`: a page is six (kind, text) pairs (kind 1 a heading, drawn
//! in orange between stars; 2 a name, typed in; 0 nothing) and the page's
//! time in frames. During a DS session the list's four literal-pool words
//! (`0x0806BFEC`, `0x0806C0B0`, `0x0806C108`, `0x0806C134`) point at Dual
//! Strike's pages, built here: each section's lines in order, centred in
//! the six slots; headings in AW2's style (between stars), split in two
//! lines when longer than the page is wide; a section with more lines
//! than a page holds goes on over two (its heading on each). Text is
//! Dual Strike's own, read from the pack; an apostrophe is AW2's `~`.
//!
//! The roll runs from a session copy of AW2's staff roll script: the music
//! is Dual Strike's staff roll stream ([`crate::ds_music::staff_roll_song`];
//! a pack without it keeps AW2's); AW2's epilogue before the roll (its
//! "War is over" paper and its last mission's recap) and its "Campaign
//! Clear" screen and campaign rank after the copyright screen are left
//! out (AW2's campaign's). Dual Strike's top-screen pictures during the
//! roll are not shown.

use crate::ds_campaign_data::{Built, Ds};
use mgba::core::Core;

/// Dual Strike's section list (overlay 5).
const DS_SECTIONS: u32 = 0x0236_A418;
const OV5: u32 = 0x0235_0560;
/// AW2's page list and the words that point at it.
const AW2_PAGES: u32 = 0x0858_265C;
pub const PAGE_POOLS: [u32; 4] = [0x0806_BFEC, 0x0806_C0B0, 0x0806_C108, 0x0806_C134];
/// AW2's staff roll script: its music op (0x1B, song 304: kept), its
/// setup and AW2's own epilogue (the "War is over" paper, the last
/// mission's recap: left out), then the roll from its screen setup
/// (`0x0806BB08`) to the fade after the copyright screen (kept).
pub const AW2_STAFF_ROLL: u32 = 0x0858_1AC8;
const MUSIC_OP: u32 = 0x0858_1AE0;
const ROLL_START: u32 = 0x0858_1B40;
const KEPT_END: u32 = 0x0858_1BD0;
/// Page slots and the widest line (characters, the stars included).
const SLOTS: usize = 6;
const WIDTH: usize = 21;

#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    Blank,
    Heading(String),
    Name(String),
}

/// Dual Strike's sections: lines and time.
pub fn sections(ds: &Ds) -> Option<Vec<(Vec<Line>, u32)>> {
    let ov5 = ds.ov5;
    let u32_at = |a: u32| -> Option<u32> {
        let o = a.checked_sub(OV5)? as usize;
        Some(u32::from_le_bytes(ov5.get(o..o + 4)?.try_into().ok()?))
    };
    let text = |a: u32| -> Option<String> {
        let o = a.checked_sub(OV5)? as usize;
        let s = ov5.get(o..)?;
        let end = s.iter().position(|&b| b == 0)?;
        let t: String = s[..end].iter().map(|&b| b as char).collect();
        (!t.is_empty() && t.chars().all(|c| (' '..='~').contains(&c))).then_some(t)
    };
    let mut out = Vec::new();
    let mut at = DS_SECTIONS;
    loop {
        let sec = u32_at(at)?;
        if sec == 0 {
            break;
        }
        let mut lines = Vec::new();
        let mut p = sec;
        let time = loop {
            match u32_at(p)? {
                0 => {
                    lines.push(Line::Blank);
                    p += 4;
                }
                k @ 1..=3 => {
                    let t = text(u32_at(p + 4)?)?;
                    lines.push(if k == 1 { Line::Name(t) } else { Line::Heading(t) });
                    p += 8;
                }
                t => break t,
            }
            if lines.len() > 16 {
                return None;
            }
        };
        out.push((lines, time));
        at += 4;
        if out.len() > 64 {
            return None;
        }
    }
    (!out.is_empty()).then_some(out)
}

/// AW2's text for a line: an apostrophe as AW2's `~`.
fn aw2_text(s: &str) -> String {
    s.replace('\'', "~")
}

/// A heading's lines in AW2's style: between stars, in two lines when too
/// wide (split at the space nearest the middle).
fn heading_lines(t: &str) -> Vec<String> {
    let t = aw2_text(t);
    if t.len() + 2 <= WIDTH {
        return vec![format!("*{t}*")];
    }
    let mid = t.len() / 2;
    let split = t.char_indices().filter(|&(_, c)| c == ' ').map(|(i, _)| i).min_by_key(|&i| (i as i32 - mid as i32).abs());
    match split {
        Some(i) => vec![format!("*{}*", &t[..i]), format!("*{}*", &t[i + 1..])],
        None => vec![format!("*{t}*")],
    }
}

/// A section as AW2 pages: (slots, time). Lines keep their order with the
/// section's leading and trailing blanks dropped, centred in the slots.
pub fn pages(lines: &[Line], time: u32) -> Vec<(Vec<(u32, String)>, u32)> {
    let mut rows: Vec<(u32, String)> = Vec::new();
    for l in lines {
        match l {
            Line::Blank => rows.push((0, String::new())),
            Line::Heading(t) => rows.extend(heading_lines(t).into_iter().map(|h| (1, h))),
            Line::Name(t) => rows.push((2, aw2_text(t))),
        }
    }
    while rows.first().is_some_and(|r| r.0 == 0) {
        rows.remove(0);
    }
    while rows.last().is_some_and(|r| r.0 == 0) {
        rows.pop();
    }
    let mut chunks: Vec<Vec<(u32, String)>> = Vec::new();
    if rows.len() <= SLOTS {
        chunks.push(rows);
    } else {
        // Too many: blank lines go, then the names over pages, each page
        // with the section's headings first.
        rows.retain(|r| r.0 != 0);
        let heads: Vec<(u32, String)> = rows.iter().take_while(|r| r.0 == 1).cloned().collect();
        let names: Vec<(u32, String)> = rows[heads.len()..].to_vec();
        let room = SLOTS.saturating_sub(heads.len()).max(1);
        if names.is_empty() {
            chunks.push(rows.into_iter().take(SLOTS).collect());
        }
        for part in names.chunks(room) {
            let mut c = heads.clone();
            c.extend_from_slice(part);
            c.truncate(SLOTS);
            chunks.push(c);
        }
    }
    chunks
        .into_iter()
        .map(|c| {
            let pad = (SLOTS - c.len()) / 2;
            let mut slots = vec![(0u32, String::new()); pad];
            slots.extend(c);
            slots.resize(SLOTS, (0, String::new()));
            (slots, time)
        })
        .collect()
}

/// What the session needs: the page list and the staff roll's copy.
pub struct Credits {
    pub page_list: u32,
    pub staff_roll: u32,
}

/// Builds the pages, their texts and the staff roll's copy into the
/// campaign's ROM blob.
pub fn build(core: &Core, ds: &Ds, built: &mut Built) -> Option<Credits> {
    let mut texts: Vec<(String, u32)> = Vec::new();
    let mut page_at = Vec::new();
    for (lines, time) in sections(ds)? {
        for (slots, t) in pages(&lines, time) {
            let mut b = Vec::with_capacity(4 * (2 * SLOTS + 1));
            for (kind, s) in slots {
                let ptr = if kind == 0 {
                    0
                } else if let Some(&(_, a)) = texts.iter().find(|(x, _)| *x == s) {
                    a
                } else {
                    let mut z = s.clone().into_bytes();
                    z.push(0);
                    let a = built.add(&z);
                    texts.push((s, a));
                    a
                };
                b.extend_from_slice(&kind.to_le_bytes());
                b.extend_from_slice(&ptr.to_le_bytes());
            }
            b.extend_from_slice(&t.to_le_bytes());
            page_at.push(built.add(&b));
        }
    }
    let mut list: Vec<u8> = page_at.iter().flat_map(|a| a.to_le_bytes()).collect();
    list.extend_from_slice(&0u32.to_le_bytes());
    let page_list = built.add(&list);
    // The staff roll's copy: AW2's setup and music, the roll to the
    // copyright screen's fade, then the end; Dual Strike's music.
    // (the map has faded out: the roll's own screen setup and fade-in come
    // next; AW2's setup and fade before it show AW2's paper)
    let mut music = vec![0u8; 8];
    core.raw_read_range(MUSIC_OP, -1, &mut music);
    let mut roll = vec![0u8; (KEPT_END - ROLL_START) as usize];
    core.raw_read_range(ROLL_START, -1, &mut roll);
    let mut script = [music, roll].concat();
    let op = 0;
    if u16::from_le_bytes([script[op], script[op + 1]]) != 0x1B {
        return None;
    }
    if let Some(song) = crate::ds_music::staff_roll_song() {
        script[op + 2..op + 4].copy_from_slice(&(song as i16).to_le_bytes());
    }
    script.extend_from_slice(&[0u8; 8]);
    let staff_roll = built.add(&script);
    Some(Credits { page_list, staff_roll })
}

/// The roll's setup (`0x0806BB08`) starts its music, AW2's song 416, here
/// (`PlaySong`'s call, r0 the song): in a session, Dual Strike's.
pub const ROLL_SONG_CALL: u32 = 0x0806_BC84;
const AW2_ROLL_SONG: u32 = 416;
pub fn roll_song(core: &mut Core, session: bool) {
    if !session || core.gba().cpu().gpr(0) as u32 != AW2_ROLL_SONG {
        return;
    }
    if let Some(song) = crate::ds_music::staff_roll_song() {
        core.gba_mut().cpu_mut().set_gpr(0, song as i32);
    }
}

/// Every frame: the page list words point at Dual Strike's pages during a
/// session, at AW2's otherwise.
pub fn tick(core: &mut Core, session: bool, credits: Option<&Credits>) {
    let want = match (session, credits) {
        (true, Some(c)) => c.page_list,
        _ => AW2_PAGES,
    };
    for at in PAGE_POOLS {
        let now = core.raw_read_32(at, -1);
        if now != want && (now == AW2_PAGES || credits.is_some_and(|c| now == c.page_list)) {
            core.raw_write_32(at, -1, want);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_split_when_wide() {
        assert_eq!(heading_lines("PRODUCER"), vec!["*PRODUCER*"]);
        assert_eq!(heading_lines("NORTH AMERICAN LOCALIZATION"), vec!["*NORTH AMERICAN*", "*LOCALIZATION*"]);
        assert_eq!(aw2_text("TIM O'LEARY"), "TIM O~LEARY");
    }

    #[test]
    fn pages_centre_and_overflow() {
        let l = |n: &str| Line::Name(n.to_string());
        let p = pages(&[Line::Blank, Line::Heading("DIRECTOR".into()), l("A"), Line::Blank], 120);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].0.iter().map(|s| s.0).collect::<Vec<_>>(), vec![0, 0, 1, 2, 0, 0]);
        let many = [Line::Heading("NORTH AMERICAN LOCALIZATION".into()), l("A"), l("B"), l("C"), l("D"), l("E")];
        let p = pages(&many, 120);
        assert_eq!(p.len(), 2);
        assert!(p.iter().all(|(s, _)| s.iter().filter(|r| r.0 == 1).count() == 2));
        assert_eq!(p.iter().map(|(s, _)| s.iter().filter(|r| r.0 == 2).count()).sum::<usize>(), 5);
    }
}
