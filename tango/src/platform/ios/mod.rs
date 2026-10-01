//! iOS glue: the app's folders, logging, the document picker, the
//! GameController pads and hardware keyboard, safe areas and links.
//! The UIKit side is `bridge.m`, compiled by build.rs.

pub mod touch_pad;

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::PathBuf;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RawPad {
    id: u32,
    buttons: u32,
    axes: [f32; 6],
}

type PickCb = extern "C" fn(ctx: *mut c_void, paths: *const *const c_char, n: usize);

extern "C" {
    fn tango_ios_init();
    fn tango_ios_safe_area(out: *mut f64);
    fn tango_ios_keyboard_height() -> f64;
    fn tango_ios_open_url(url: *const c_char);
    fn tango_ios_set_clipboard_text(text: *const c_char);
    fn tango_ios_set_clipboard_png(bytes: *const u8, len: usize);
    fn tango_ios_pick_files(kind: c_int, multiple: c_int, cb: PickCb, ctx: *mut c_void);
    fn tango_ios_pads(out: *mut RawPad, max: c_int) -> c_int;
    fn tango_ios_keys(codes: *const u16, held: *mut u8, n: c_int);
}

/// Audio session, screen lock. Called once from `run_app`.
pub fn init() {
    unsafe { tango_ios_init() };
}

/// The app's Documents folder, which the Files app shows as
/// "On My iPhone/iPad > tangoAW2". The data folder lives here.
pub fn documents_dir() -> PathBuf {
    home().join("Documents")
}

/// Library/Application Support: settings, out of the user's way.
pub fn config_dir() -> PathBuf {
    home().join("Library").join("Application Support").join("tangoaw2")
}

/// Library/Caches: derived data the system may purge.
pub fn cache_dir() -> PathBuf {
    home().join("Library").join("Caches").join("tangoaw2")
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// env_logger to both the console (Xcode / Console.app) and a file in
/// Documents/logs, so a player can send a log from the Files app.
pub fn init_logging() {
    struct Tee(Option<std::fs::File>);
    impl std::io::Write for Tee {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let _ = std::io::stderr().write_all(buf);
            if let Some(f) = self.0.as_mut() {
                let _ = f.write_all(buf);
            }
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            if let Some(f) = self.0.as_mut() {
                f.flush()?;
            }
            Ok(())
        }
    }
    let dir = documents_dir().join("logs");
    let _ = std::fs::create_dir_all(&dir);
    let name = format!("tangoaw2-{}.log", chrono::Local::now().format("%Y%m%d-%H%M%S"));
    let file = std::fs::File::create(dir.join(name)).ok();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Pipe(Box::new(Tee(file))))
        .init();
}

/// Safe-area insets in points: top, left, bottom, right.
pub fn safe_area() -> [f32; 4] {
    let mut out = [0f64; 4];
    unsafe { tango_ios_safe_area(out.as_mut_ptr()) };
    out.map(|v| v as f32)
}

/// Points of the screen's bottom the on-screen keyboard covers.
pub fn keyboard_height() -> f32 {
    unsafe { tango_ios_keyboard_height() as f32 }
}

pub fn open_url(url: &str) {
    if let Ok(c) = CString::new(url) {
        unsafe { tango_ios_open_url(c.as_ptr()) };
    }
}

/// Show a folder of the app's data in the Files app.
pub fn open_folder(path: &std::path::Path) {
    let dir = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent().map(|p| p.to_path_buf()).unwrap_or_else(documents_dir)
    };
    let mut url = String::from("shareddocuments://");
    for b in dir.to_string_lossy().bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => url.push(b as char),
            _ => url.push_str(&format!("%{b:02X}")),
        }
    }
    open_url(&url);
}

pub fn set_clipboard_text(text: &str) {
    if let Ok(c) = CString::new(text) {
        unsafe { tango_ios_set_clipboard_text(c.as_ptr()) };
    }
}

pub fn set_clipboard_image(img: &image::RgbaImage) {
    let mut png = Vec::new();
    if img
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .is_ok()
    {
        unsafe { tango_ios_set_clipboard_png(png.as_ptr(), png.len()) };
    }
}

/// What a picker offers.
#[derive(Clone, Copy)]
pub enum PickKind {
    AnyFile,
    Image,
}

extern "C" fn picked(ctx: *mut c_void, paths: *const *const c_char, n: usize) {
    let tx = unsafe { Box::from_raw(ctx as *mut futures::channel::oneshot::Sender<Vec<PathBuf>>) };
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let p = unsafe { CStr::from_ptr(*paths.add(i)) };
        out.push(PathBuf::from(p.to_string_lossy().into_owned()));
    }
    let _ = tx.send(out);
}

/// Present the Files document picker; resolves to temporary copies of
/// the picked files (empty when cancelled).
pub async fn pick_files(kind: PickKind, multiple: bool) -> Vec<PathBuf> {
    let (tx, rx) = futures::channel::oneshot::channel();
    let ctx = Box::into_raw(Box::new(tx)) as *mut c_void;
    let kind = match kind {
        PickKind::AnyFile => 0,
        PickKind::Image => 1,
    };
    unsafe { tango_ios_pick_files(kind, multiple as c_int, picked, ctx) };
    rx.await.unwrap_or_default()
}

/// Copy picked files into `dir`, keeping their names. Returns how many
/// were copied.
pub fn import_into(files: &[PathBuf], dir: &std::path::Path) -> usize {
    let _ = std::fs::create_dir_all(dir);
    let mut n = 0;
    for f in files {
        let Some(name) = f.file_name() else { continue };
        let dst = dir.join(name);
        // The picker's copy is ours to move; fall back to copying when it
        // sits on another volume.
        let ok = std::fs::rename(f, &dst).is_ok() || std::fs::copy(f, &dst).is_ok();
        if ok {
            n += 1;
        } else {
            log::warn!("import {}: failed", f.display());
        }
    }
    n
}

// ---------------------------------------------------------------------------
// Controllers and keyboard

/// Bit order of `RawPad::buttons` (see bridge.m).
const PAD_BUTTONS: [gamepad_facade::Button; 15] = {
    use gamepad_facade::Button as B;
    [
        B::South,
        B::East,
        B::West,
        B::North,
        B::Back,
        B::Start,
        B::Guide,
        B::LeftStick,
        B::RightStick,
        B::LeftShoulder,
        B::RightShoulder,
        B::DPadUp,
        B::DPadDown,
        B::DPadLeft,
        B::DPadRight,
    ]
};

const PAD_AXES: [gamepad_facade::Axis; 6] = {
    use gamepad_facade::Axis as A;
    [
        A::LeftX,
        A::LeftY,
        A::RightX,
        A::RightY,
        A::TriggerLeft,
        A::TriggerRight,
    ]
};

/// Keys polled from a hardware keyboard: HID usage code and the physical
/// key iced would name it.
const KEYS: &[(u16, iced::keyboard::key::Code)] = {
    use iced::keyboard::key::Code as C;
    &[
        (0x04, C::KeyA),
        (0x16, C::KeyS),
        (0x1D, C::KeyZ),
        (0x1B, C::KeyX),
        (0x14, C::KeyQ),
        (0x1A, C::KeyW),
        (0x08, C::KeyE),
        (0x28, C::Enter),
        (0x2C, C::Space),
        (0x29, C::Escape),
        (0x4F, C::ArrowRight),
        (0x50, C::ArrowLeft),
        (0x51, C::ArrowDown),
        (0x52, C::ArrowUp),
        (0xE1, C::ShiftLeft),
        (0xE5, C::ShiftRight),
    ]
};

#[derive(Default)]
struct PollState {
    pads: std::collections::HashMap<u32, RawPad>,
    keys: Vec<u8>,
}

static CONTROLLERS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static PHYSICAL_INPUT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether the on-screen controller should step aside: a game
/// controller or keyboard button was pressed since the screen was last
/// touched. (A connected controller alone does not count: one paired
/// for something else would hide the controls for good.)
pub fn touch_controls_hidden() -> bool {
    PHYSICAL_INPUT.load(std::sync::atomic::Ordering::Relaxed)
}

/// A touch on the session brings the on-screen controller back.
pub fn show_touch_controls() {
    PHYSICAL_INPUT.store(false, std::sync::atomic::Ordering::Relaxed);
}

thread_local! {
    static POLL: std::cell::RefCell<PollState> = std::cell::RefCell::new(PollState::default());
}

/// Input from a controller or keyboard since the last poll.
pub enum Polled {
    Pad(gamepad_facade::Event),
    Key(iced::keyboard::Event),
}

/// Poll the controllers and the hardware keyboard (main thread), and
/// report what changed as gamepad / keyboard events.
pub fn poll() -> Vec<Polled> {
    use gamepad_facade::{Event, EventKind, Id};
    let mut raw = [RawPad::default(); 8];
    let n = unsafe { tango_ios_pads(raw.as_mut_ptr(), raw.len() as c_int) }.max(0) as usize;
    if CONTROLLERS.swap(n, std::sync::atomic::Ordering::Relaxed) != n {
        log::info!("game controllers connected: {n}");
    }
    let mut held = vec![0u8; KEYS.len()];
    let codes: Vec<u16> = KEYS.iter().map(|(c, _)| *c).collect();
    unsafe { tango_ios_keys(codes.as_ptr(), held.as_mut_ptr(), KEYS.len() as c_int) };

    POLL.with(|s| {
        let mut s = s.borrow_mut();
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for pad in &raw[..n] {
            seen.insert(pad.id);
            let id = Id(pad.id);
            let prev = s.pads.get(&pad.id).copied();
            if prev.is_none() {
                out.push(Polled::Pad(Event {
                    id,
                    kind: EventKind::Connected,
                }));
            }
            let prev = prev.unwrap_or_default();
            for (i, b) in PAD_BUTTONS.iter().enumerate() {
                let (was, is) = (prev.buttons >> i & 1 != 0, pad.buttons >> i & 1 != 0);
                if was != is {
                    if is {
                        PHYSICAL_INPUT.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    out.push(Polled::Pad(Event {
                        id,
                        kind: if is {
                            EventKind::ButtonDown(*b)
                        } else {
                            EventKind::ButtonUp(*b)
                        },
                    }));
                }
            }
            for (i, a) in PAD_AXES.iter().enumerate() {
                if (prev.axes[i] - pad.axes[i]).abs() > 0.01 {
                    out.push(Polled::Pad(Event {
                        id,
                        kind: EventKind::AxisMotion {
                            axis: *a,
                            value: pad.axes[i],
                        },
                    }));
                }
            }
            s.pads.insert(pad.id, *pad);
        }
        let gone: Vec<u32> = s.pads.keys().copied().filter(|k| !seen.contains(k)).collect();
        for k in gone {
            s.pads.remove(&k);
            out.push(Polled::Pad(Event {
                id: Id(k),
                kind: EventKind::Disconnected,
            }));
        }

        if s.keys.len() != held.len() {
            s.keys = vec![0; held.len()];
        }
        for (i, (_, code)) in KEYS.iter().enumerate() {
            if s.keys[i] != held[i] {
                let physical_key = iced::keyboard::key::Physical::Code(*code);
                let key = iced::keyboard::Key::Unidentified;
                if held[i] != 0 {
                    PHYSICAL_INPUT.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                out.push(Polled::Key(if held[i] != 0 {
                    iced::keyboard::Event::KeyPressed {
                        key: key.clone(),
                        modified_key: key,
                        physical_key,
                        location: iced::keyboard::Location::Standard,
                        modifiers: iced::keyboard::Modifiers::empty(),
                        text: None,
                        repeat: false,
                    }
                } else {
                    iced::keyboard::Event::KeyReleased {
                        key: key.clone(),
                        modified_key: key,
                        physical_key,
                        location: iced::keyboard::Location::Standard,
                        modifiers: iced::keyboard::Modifiers::empty(),
                    }
                }));
            }
        }
        s.keys = held;
        out
    })
}
