//! macOS keyboard-passthrough toggle via `NSEvent` local monitor.
//!
//! This is the Mac analog of the Wayland `zwp_keyboard_shortcuts_inhibit`
//! dance in `shortcuts_inhibit.rs`. There is no compositor-level inhibit on
//! macOS, so we install a process-local key-down monitor that swallows a
//! small set of browser-like shortcuts that would otherwise fire before the
//! remote RDP page sees them. Anything not in the swallow list passes
//! through untouched.
//!
//! See `docs/macos-keyboard-passthrough.md` for the rationale, the list of
//! OS-reserved keys we can never intercept, and open design questions.
//!
//! TODO: on window focus-out (NSWindowDidResignKey), reset TRACKER to Idle.
//! Requires NSNotificationCenter observer wiring — deferred; in practice
//! focus-out is rare during a single tap because macOS doesn't swap focus
//! under a held Cmd.

use std::ptr::NonNull;
use std::sync::Mutex;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags, NSEventType};

use crate::tap_tracker::{self, Action, Event as TrackerEvent, State};

/// AppKit objects aren't Send in general, but the monitor handle is opaque —
/// we never dereference it off the main thread. `rdpls_toggle_passthrough`
/// hops to the main thread before touching the NSEvent API; the Mutex only
/// needs to serialize access to the Option itself.
struct MonitorHandle(Retained<AnyObject>);
unsafe impl Send for MonitorHandle {}

static MONITOR: Mutex<Option<MonitorHandle>> = Mutex::new(None);

struct TrackerState {
    state: State,
    last_down_keycode: Option<u16>,
}

static TRACKER: Mutex<TrackerState> = Mutex::new(TrackerState {
    state: State::Idle,
    last_down_keycode: None,
});

// macOS virtual keycodes (see <HIToolbox/Events.h>):
const VK_COMMAND: u16 = 0x37; // kVK_Command
const VK_RIGHT_COMMAND: u16 = 0x36; // kVK_RightCommand
const VK_F3: u16 = 0x63; // kVK_F3

/// Keys we swallow while inhibiting. They fire app-level reload / history
/// navigation that the user almost never wants inside the RDP session.
/// Kept intentionally small — see the design doc before extending.
fn should_swallow(event: &NSEvent) -> bool {
    let flags = event
        .modifierFlags()
        .intersection(NSEventModifierFlags::DeviceIndependentFlagsMask);

    let command_only = flags == NSEventModifierFlags::Command
        || flags == (NSEventModifierFlags::Command | NSEventModifierFlags::Shift);

    if !command_only {
        return false;
    }

    let Some(chars) = event.charactersIgnoringModifiers() else {
        return false;
    };
    let s = chars.to_string();
    matches!(s.as_str(), "r" | "R" | "[" | "]" | "{" | "}")
}

/// Feed an NSEvent into the tap tracker. Returns the action to take, or
/// `None` if the event isn't one the tracker cares about.
fn feed_tracker(ev: &NSEvent) -> Option<Action> {
    let event_type = ev.r#type();
    let keycode = ev.keyCode();
    let is_cmd_key = keycode == VK_COMMAND || keycode == VK_RIGHT_COMMAND;

    let tracker_event = if event_type == NSEventType::FlagsChanged && is_cmd_key {
        let flags = ev
            .modifierFlags()
            .intersection(NSEventModifierFlags::DeviceIndependentFlagsMask);
        let cmd_now_held = flags.contains(NSEventModifierFlags::Command);
        if cmd_now_held {
            // FlagsChanged doesn't carry an autorepeat bit on its own; infer
            // it: if we're already Tracking on the same physical key, this
            // is the OS reasserting the held state.
            let autorepeat = {
                let guard = TRACKER.lock().unwrap();
                matches!(guard.state, State::Tracking) && guard.last_down_keycode == Some(keycode)
            };
            let other_held =
                flags.difference(NSEventModifierFlags::Command) != NSEventModifierFlags::empty();
            TRACKER.lock().unwrap().last_down_keycode = Some(keycode);
            TrackerEvent::ModifierDown {
                other_key_held: other_held,
                autorepeat,
            }
        } else {
            TRACKER.lock().unwrap().last_down_keycode = None;
            TrackerEvent::ModifierUp
        }
    } else if event_type == NSEventType::FlagsChanged
        || event_type == NSEventType::KeyDown
        || event_type == NSEventType::KeyUp
    {
        TrackerEvent::OtherKey
    } else {
        return None;
    };

    let mut guard = TRACKER.lock().unwrap();
    let (new_state, action) = tap_tracker::step(guard.state, tracker_event);
    guard.state = new_state;
    if matches!(new_state, State::Idle) {
        guard.last_down_keycode = None;
    }
    Some(action)
}

/// Synthesize Alt+F3 via Quartz (`CGEventPost`). The event flows back through
/// the local NSEvent monitor as a normal KeyDown/KeyUp on F3 with the Alt
/// flag set; the tap tracker is in `Idle` by the time it arrives (we only
/// fire this from the `ModifierUp` transition, which itself returns the
/// state machine to `Idle`), so it's classified as `OtherKey` against
/// `Idle` and passes through cleanly. No re-entry is possible.
fn post_alt_f3() {
    use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    let Ok(src_down) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) else {
        return;
    };
    let Ok(down) = CGEvent::new_keyboard_event(src_down, VK_F3, true) else {
        return;
    };
    down.set_flags(CGEventFlags::CGEventFlagAlternate);
    down.post(CGEventTapLocation::HID);

    let Ok(src_up) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) else {
        return;
    };
    let Ok(up) = CGEvent::new_keyboard_event(src_up, VK_F3, false) else {
        return;
    };
    up.set_flags(CGEventFlags::CGEventFlagAlternate);
    up.post(CGEventTapLocation::HID);
}

fn install_locked(guard: &mut std::sync::MutexGuard<'_, Option<MonitorHandle>>) {
    if guard.is_some() {
        return;
    }
    let handler = RcBlock::new(|event: NonNull<NSEvent>| -> *mut NSEvent {
        let ev = unsafe { event.as_ref() };

        // Route through the tap tracker first. It owns the Cmd-tap detector
        // and decides whether this event must be eaten (and whether to
        // synthesize Alt+F3 to launch the Windows Start menu inside RDP).
        match feed_tracker(ev) {
            Some(Action::Swallow) => return std::ptr::null_mut(),
            Some(Action::SwallowAndInjectAltF3) => {
                post_alt_f3();
                return std::ptr::null_mut();
            }
            Some(Action::PassThrough) | Some(Action::Noop) | None => {}
        }

        if should_swallow(ev) {
            std::ptr::null_mut()
        } else {
            event.as_ptr()
        }
    });
    let mask = NSEventMask::KeyDown | NSEventMask::KeyUp | NSEventMask::FlagsChanged;
    let monitor = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &handler) };
    **guard = monitor.map(MonitorHandle);
}

fn toggle_locked() -> bool {
    let mut guard = MONITOR.lock().unwrap();
    if let Some(m) = guard.take() {
        unsafe { NSEvent::removeMonitor(&m.0) };
        // Reset the tracker — the spec says any passthrough toggle resets to
        // Idle so a Cmd held across the toggle doesn't latch us in Tracking.
        let mut t = TRACKER.lock().unwrap();
        let (new_state, _) = tap_tracker::step(t.state, TrackerEvent::PassthroughOff);
        t.state = new_state;
        t.last_down_keycode = None;
        false
    } else {
        install_locked(&mut guard);
        guard.is_some()
    }
}

/// Called at startup from `lib.rs` on the main thread. Inhibit starts ON —
/// matches Linux and reflects that the app exists to host a remote session,
/// not to browse myapps.microsoft.com.
pub fn install() {
    let mut guard = MONITOR.lock().unwrap();
    install_locked(&mut guard);
}

/// Invoked from the injected JS on `Ctrl+Alt+Shift+Escape`. Returns `true`
/// when keys are now being routed to the remote (inhibit ON), `false` when
/// they've been released back to the local app. The JS uses this to pick
/// the toast label, matching the Linux wording.
///
/// The NSEvent APIs must run on the main thread, but Tauri commands dispatch
/// on a worker; hop via `run_on_main_thread` and wait for the result.
#[tauri::command]
pub fn rdpls_toggle_passthrough(app: tauri::AppHandle) -> bool {
    let (tx, rx) = std::sync::mpsc::channel();
    if app
        .run_on_main_thread(move || {
            let _ = tx.send(toggle_locked());
        })
        .is_err()
    {
        return false;
    }
    rx.recv().unwrap_or(false)
}
