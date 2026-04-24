//! Linux-side tap detector for bare Super → Alt+F3.
//!
//! Hooks the WebView widget's key-press / key-release / focus-out signals,
//! feeds them into the shared `tap_tracker` state machine, and invokes
//! `shortcuts_inhibit::inject_alt_f3()` on qualifying taps.
//!
//! All GTK signal handlers run on the main thread, so the Mutex here only
//! serializes against the Ctrl+Alt+Shift+. toggle path (also main-thread in
//! practice, via the Tauri command dispatch — the Mutex is defense-in-depth).

use std::sync::Mutex;

use gtk::glib;
use gtk::prelude::*;

use crate::shortcuts_inhibit;
use crate::tap_tracker::{self, Action, Event, State};

// evdev keycodes + 8 (X11 convention used by GDK hardware_keycode).
// KEY_LEFTMETA=125 → 133. KEY_RIGHTMETA=126 → 134.
const SUPER_L_HW_KEYCODE: u16 = 133;
const SUPER_R_HW_KEYCODE: u16 = 134;

struct TrackerState {
    state: State,
    last_down_keycode: Option<u16>,
}

static TRACKER: Mutex<TrackerState> = Mutex::new(TrackerState {
    state: State::Idle,
    last_down_keycode: None,
});

fn is_super(code: u16) -> bool {
    code == SUPER_L_HW_KEYCODE || code == SUPER_R_HW_KEYCODE
}

/// Hook the tap tracker onto the WebView's GTK widget signals. Call from
/// `configure_webview` after UserContentManager scripts are installed.
pub fn install(webview: &webkit2gtk::WebView) {
    webview.connect_key_press_event(|_, event| {
        let code = event.hardware_keycode();
        let ev = if is_super(code) {
            let mut guard = TRACKER.lock().unwrap();
            let autorepeat = matches!(guard.state, State::Tracking)
                && guard.last_down_keycode == Some(code);
            // Other non-Super modifier held at press time. gdk::ModifierType
            // bits: SHIFT_MASK, CONTROL_MASK, MOD1_MASK (Alt), MOD4_MASK
            // (Super itself). MOD2_MASK is NumLock; LOCK_MASK is CapsLock.
            // If any of Shift/Ctrl/Alt is held, taint immediately.
            let flags = event.state();
            let other_held = flags.contains(gdk::ModifierType::SHIFT_MASK)
                || flags.contains(gdk::ModifierType::CONTROL_MASK)
                || flags.contains(gdk::ModifierType::MOD1_MASK);
            guard.last_down_keycode = Some(code);
            drop(guard);
            Event::ModifierDown { other_key_held: other_held, autorepeat }
        } else {
            Event::OtherKey
        };
        handle(ev)
    });

    webview.connect_key_release_event(|_, event| {
        let code = event.hardware_keycode();
        let ev = if is_super(code) {
            TRACKER.lock().unwrap().last_down_keycode = None;
            Event::ModifierUp
        } else {
            Event::OtherKey
        };
        handle(ev)
    });

    if let Some(toplevel) = webview
        .toplevel()
        .and_then(|t| t.downcast::<gtk::Window>().ok())
    {
        toplevel.connect_focus_out_event(|_, _| {
            feed(Event::FocusLost);
            glib::Propagation::Proceed
        });
    }
}

fn handle(ev: Event) -> glib::Propagation {
    if !shortcuts_inhibit::is_inhibiting() {
        // Passthrough off: reset any tracked state; let GTK keep the event.
        feed(Event::PassthroughOff);
        return glib::Propagation::Proceed;
    }
    match feed(ev) {
        Action::Swallow => glib::Propagation::Stop,
        Action::PassThrough | Action::Noop => glib::Propagation::Proceed,
        Action::SwallowAndInjectAltF3 => {
            shortcuts_inhibit::inject_alt_f3();
            glib::Propagation::Stop
        }
    }
}

fn feed(ev: Event) -> Action {
    let mut guard = TRACKER.lock().unwrap();
    let (new_state, action) = tap_tracker::step(guard.state, ev);
    guard.state = new_state;
    if matches!(new_state, State::Idle) {
        guard.last_down_keycode = None;
    }
    action
}
