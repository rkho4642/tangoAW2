//! iOS only (added to iced_winit by ios/deps.py): turns touches into the
//! mouse events the app's widgets are written for.
//!
//! The first finger down becomes the mouse: it moves the cursor, presses
//! and releases the left button. Dragging it past a few points scrolls
//! instead (the press is withdrawn first, so a list row the drag started
//! on is not clicked) — unless the widget under it is itself a drag
//! target (a slider, a resize handle), which keeps the mouse drag. Every
//! other finger down at the same time stays a touch, for the on-screen
//! game controller, which reads several fingers at once.

use crate::core::mouse::Interaction;
use winit::dpi::PhysicalPosition;
use winit::event::{DeviceId, ElementState, MouseButton, MouseScrollDelta, Touch, TouchPhase, WindowEvent};

/// Physical pixels a finger travels before a press turns into a scroll.
const SCROLL_SLOP: f64 = 24.0;

struct Primary {
    id: u64,
    start: PhysicalPosition<f64>,
    last: PhysicalPosition<f64>,
    scrolling: bool,
}

thread_local! {
    static PRIMARY: std::cell::RefCell<Option<Primary>> = const { std::cell::RefCell::new(None) };
}

fn device() -> DeviceId {
    // Synthesized events; nothing downstream compares device ids.
    DeviceId::dummy()
}

fn moved(position: PhysicalPosition<f64>) -> WindowEvent {
    WindowEvent::CursorMoved {
        device_id: device(),
        position,
    }
}

fn button(state: ElementState) -> WindowEvent {
    WindowEvent::MouseInput {
        device_id: device(),
        state,
        button: MouseButton::Left,
    }
}

fn is_drag_target(i: Interaction) -> bool {
    matches!(
        i,
        Interaction::Grab
            | Interaction::Grabbing
            | Interaction::ResizingHorizontally
            | Interaction::ResizingVertically
            | Interaction::ResizingDiagonallyUp
            | Interaction::ResizingDiagonallyDown
            | Interaction::ResizingColumn
            | Interaction::ResizingRow
            | Interaction::Text
    )
}

/// The events to deliver in place of `event`. Anything but a touch passes
/// through unchanged.
pub fn convert(event: WindowEvent, interaction: Interaction) -> Vec<WindowEvent> {
    let &WindowEvent::Touch(Touch { id, location, phase, .. }) = &event else {
        return vec![event];
    };
    PRIMARY.with(|p| {
        let mut p = p.borrow_mut();
        let is_primary = p.as_ref().map(|p| p.id == id);
        match (phase, is_primary) {
            (TouchPhase::Started, None) => {
                *p = Some(Primary {
                    id,
                    start: location,
                    last: location,
                    scrolling: false,
                });
                vec![moved(location), button(ElementState::Pressed)]
            }
            (TouchPhase::Moved, Some(true)) => {
                let prim = p.as_mut().unwrap();
                let (dx, dy) = (location.x - prim.last.x, location.y - prim.last.y);
                prim.last = location;
                if prim.scrolling {
                    return vec![moved(prim.start), WindowEvent::MouseWheel {
                        device_id: device(),
                        delta: MouseScrollDelta::PixelDelta(PhysicalPosition::new(dx, dy)),
                        phase: TouchPhase::Moved,
                    }];
                }
                let travelled = (location.x - prim.start.x).hypot(location.y - prim.start.y);
                if travelled > SCROLL_SLOP && !is_drag_target(interaction) {
                    // Withdraw the press away from everything, so the
                    // widget it started on sees a release that is not a
                    // click, then scroll by the whole travel so far.
                    //
                    // iced reads the cursor once per batch of events, so
                    // the cursor stays away for this batch and comes back
                    // with the first scroll, on the next move.
                    prim.scrolling = true;
                    prim.last = prim.start;
                    return vec![
                        moved(PhysicalPosition::new(-10_000.0, -10_000.0)),
                        button(ElementState::Released),
                    ];
                }
                vec![moved(location)]
            }
            (TouchPhase::Ended | TouchPhase::Cancelled, Some(true)) => {
                let prim = p.take().unwrap();
                if prim.scrolling {
                    return Vec::new();
                }
                // No CursorLeft after the release: in the same batch it
                // would take the cursor away from the click.
                vec![moved(location), button(ElementState::Released)]
            }
            // A second finger while the first is down, or a stray
            // event for a finger we never saw start: a plain touch.
            _ => vec![event],
        }
    })
}

// ---------------------------------------------------------------------------
// Scripted touches, for testing in the Simulator (which has no way to
// inject touches from the command line). Off unless the app is launched
// with TANGOAW2_TOUCH_SCRIPT=<file>; on a device nothing can set it.
//
// The app polls the file; each batch of lines written to it is run and
// the file emptied. Coordinates are in points:
//   tap X Y                                 one finger down and up
//   down N X Y / move N X Y / up N X Y      finger N (0 is the first)
//   wait MS

type Scripted = (u64, f64, f64, TouchPhase);

static SCRIPTED: std::sync::Mutex<Vec<Scripted>> = std::sync::Mutex::new(Vec::new());

/// An idle app sleeps in its run loop; wake it so the event loop's
/// AboutToWait (where scripted touches are delivered) comes round.
#[allow(unsafe_code)]
fn wake_main_loop() {
    unsafe extern "C" {
        fn CFRunLoopGetMain() -> *mut std::ffi::c_void;
        fn CFRunLoopWakeUp(rl: *mut std::ffi::c_void);
    }
    // SAFETY: both are thread-safe CoreFoundation calls.
    unsafe { CFRunLoopWakeUp(CFRunLoopGetMain()) };
}

fn script_thread(path: std::path::PathBuf) {
    let push = |id: u64, x: f64, y: f64, phase: TouchPhase| {
        SCRIPTED.lock().unwrap().push((id, x, y, phase));
        wake_main_loop();
    };
    let pause = |ms: u64| std::thread::sleep(std::time::Duration::from_millis(ms));
    loop {
        pause(100);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        if text.trim().is_empty() {
            continue;
        }
        let _ = std::fs::write(&path, "");
        for line in text.lines() {
            let w: Vec<&str> = line.split_whitespace().collect();
            let num = |i: usize| w.get(i).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
            match w.first().copied() {
                Some("tap") => {
                    push(0, num(1), num(2), TouchPhase::Started);
                    pause(80);
                    push(0, num(1), num(2), TouchPhase::Ended);
                    pause(80);
                }
                Some("down") => push(num(1) as u64, num(2), num(3), TouchPhase::Started),
                Some("move") => push(num(1) as u64, num(2), num(3), TouchPhase::Moved),
                Some("up") => push(num(1) as u64, num(2), num(3), TouchPhase::Ended),
                Some("wait") => pause(num(1) as u64),
                _ => {}
            }
        }
    }
}

/// Scripted touches due since the last call, as winit touch events.
pub fn scripted(scale_factor: f64) -> Vec<WindowEvent> {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        if let Some(path) = std::env::var_os("TANGOAW2_TOUCH_SCRIPT") {
            let path = std::path::PathBuf::from(path);
            let _ = std::thread::Builder::new()
                .name("touch-script".into())
                .spawn(move || script_thread(path));
        }
    });
    let mut q = SCRIPTED.lock().unwrap();
    q.drain(..)
        .map(|(id, x, y, phase)| {
            WindowEvent::Touch(Touch {
                device_id: device(),
                phase,
                location: PhysicalPosition::new(x * scale_factor, y * scale_factor),
                force: None,
                id: 0xA000 + id,
            })
        })
        .collect()
}
