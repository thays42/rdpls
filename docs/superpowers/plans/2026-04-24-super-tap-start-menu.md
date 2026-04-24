# Super/Cmd tap → Alt+F3 Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a bare Super tap (Linux) or Cmd tap (macOS) open the Start menu in the remote Windows session, and document the web client's Win-key limitations honestly.

**Architecture:** A pure `tap_tracker` state machine consumes platform-native key events and emits an "inject Alt+F3" action on qualifying taps. On Linux the injection goes through `zwp_virtual_keyboard_v1` for trusted events; on macOS it goes through `CGEventPost` with a re-entry guard. Gated on the existing passthrough/inhibit state so the Ctrl+Alt+Shift+. toggle still hands Super back to the compositor.

**Tech Stack:** Rust + Tauri v2, webkit2gtk 2.x + GTK3 + Wayland (Linux), objc2 + AppKit + CoreGraphics (macOS).

**Spec:** `docs/superpowers/specs/2026-04-24-super-tap-start-menu-design.md`

---

## Chunk 1: Shared tap-tracker module

### Task 1: Define tap-tracker types and step function

**Files:**
- Create: `src-tauri/src/tap_tracker.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod tap_tracker;` above the platform-gated modules)

- [ ] **Step 1: Write the failing tests**

Write the full test module first — the pure-logic nature of this makes TDD cheap. Create `src-tauri/src/tap_tracker.rs` with:

```rust
//! Platform-agnostic tap detector for a single tracked modifier key.
//!
//! Consumers feed in abstract `Event`s; the tracker returns an `Action`
//! describing what to do with that event (swallow vs pass through) and
//! whether to inject Alt+F3. See
//! `docs/superpowers/specs/2026-04-24-super-tap-start-menu-design.md` for
//! the state machine.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Tracking,
    Tainted,
}

impl Default for State {
    fn default() -> Self {
        State::Idle
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// The tracked modifier went down. `other_key_held` reflects whether any
    /// non-tracked key (modifier or not) is currently held at press time.
    /// `autorepeat` is true when the platform reports this as a repeat of
    /// a still-held press.
    ModifierDown { other_key_held: bool, autorepeat: bool },
    /// The tracked modifier went up.
    ModifierUp,
    /// Any other key went down or up while we care about it.
    OtherKey,
    /// Passthrough / inhibit was toggled OFF.
    PassthroughOff,
    /// Window focus was lost.
    FocusLost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Drop the originating event; don't let the WebView see it.
    Swallow,
    /// Let the event propagate to the WebView normally.
    PassThrough,
    /// Swallow the originating event and inject Alt+F3 into the WebView.
    SwallowAndInjectAltF3,
    /// Tracker-internal signal (passthrough-off / focus-lost); caller had no
    /// event to suppress, so this is a no-op for the event layer.
    Noop,
}

/// Apply an event to the state machine, returning the new state and the
/// action the caller should take.
pub fn step(state: State, event: Event) -> (State, Action) {
    use Action::*;
    use Event::*;
    use State::*;

    match (state, event) {
        // Toggles and focus loss reset regardless of current state.
        (_, PassthroughOff) | (_, FocusLost) => (Idle, Noop),

        // Idle: modifier goes down.
        (Idle, ModifierDown { other_key_held: true, .. }) => (Tainted, Swallow),
        (Idle, ModifierDown { other_key_held: false, .. }) => (Tracking, Swallow),

        // Tracking: autorepeat of our modifier stays in tracking.
        (Tracking, ModifierDown { autorepeat: true, .. }) => (Tracking, Swallow),
        // Tracking: non-autorepeat modifier-down shouldn't happen without a
        // keyup first; treat as taint defensively.
        (Tracking, ModifierDown { .. }) => (Tainted, Swallow),

        // Tracking: another key → tainted.
        (Tracking, OtherKey) => (Tainted, PassThrough),

        // Tracking: clean keyup → this was a tap.
        (Tracking, ModifierUp) => (Idle, SwallowAndInjectAltF3),

        // Tainted: stay tainted until keyup.
        (Tainted, OtherKey) => (Tainted, PassThrough),
        (Tainted, ModifierDown { .. }) => (Tainted, Swallow),
        (Tainted, ModifierUp) => (Idle, Swallow),

        // Idle + stray keyup or other-key: no involvement.
        (Idle, ModifierUp) => (Idle, PassThrough),
        (Idle, OtherKey) => (Idle, PassThrough),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn down_clean() -> Event {
        Event::ModifierDown { other_key_held: false, autorepeat: false }
    }

    fn down_with_other_held() -> Event {
        Event::ModifierDown { other_key_held: true, autorepeat: false }
    }

    fn down_repeat() -> Event {
        Event::ModifierDown { other_key_held: false, autorepeat: true }
    }

    #[test]
    fn clean_tap_fires_alt_f3() {
        let (s, a) = step(State::Idle, down_clean());
        assert_eq!((s, a), (State::Tracking, Action::Swallow));
        let (s, a) = step(s, Event::ModifierUp);
        assert_eq!((s, a), (State::Idle, Action::SwallowAndInjectAltF3));
    }

    #[test]
    fn other_key_while_tracking_taints() {
        let (s, _) = step(State::Idle, down_clean());
        let (s, a) = step(s, Event::OtherKey);
        assert_eq!((s, a), (State::Tainted, Action::PassThrough));
        let (s, a) = step(s, Event::ModifierUp);
        assert_eq!((s, a), (State::Idle, Action::Swallow));
    }

    #[test]
    fn other_key_already_held_at_down_taints_immediately() {
        let (s, a) = step(State::Idle, down_with_other_held());
        assert_eq!((s, a), (State::Tainted, Action::Swallow));
        let (s, a) = step(s, Event::ModifierUp);
        assert_eq!((s, a), (State::Idle, Action::Swallow));
    }

    #[test]
    fn autorepeat_modifier_down_stays_in_tracking() {
        let (s, _) = step(State::Idle, down_clean());
        let (s, a) = step(s, down_repeat());
        assert_eq!((s, a), (State::Tracking, Action::Swallow));
        let (s, a) = step(s, Event::ModifierUp);
        assert_eq!((s, a), (State::Idle, Action::SwallowAndInjectAltF3));
    }

    #[test]
    fn passthrough_off_resets_from_tracking() {
        let (s, _) = step(State::Idle, down_clean());
        let (s, a) = step(s, Event::PassthroughOff);
        assert_eq!((s, a), (State::Idle, Action::Noop));
        // Subsequent keyup is just a stray release, no injection.
        let (s, a) = step(s, Event::ModifierUp);
        assert_eq!((s, a), (State::Idle, Action::PassThrough));
    }

    #[test]
    fn focus_lost_resets_from_tainted() {
        let (s, _) = step(State::Idle, down_clean());
        let (s, _) = step(s, Event::OtherKey);
        let (s, a) = step(s, Event::FocusLost);
        assert_eq!((s, a), (State::Idle, Action::Noop));
    }

    #[test]
    fn idle_other_key_passes_through() {
        let (s, a) = step(State::Idle, Event::OtherKey);
        assert_eq!((s, a), (State::Idle, Action::PassThrough));
    }

    #[test]
    fn double_down_without_up_is_defensively_tainted() {
        // If we somehow see a second non-repeat down before a keyup, don't
        // fire Alt+F3 on the eventual release.
        let (s, _) = step(State::Idle, down_clean());
        let (s, a) = step(s, down_clean());
        assert_eq!((s, a), (State::Tainted, Action::Swallow));
        let (s, a) = step(s, Event::ModifierUp);
        assert_eq!((s, a), (State::Idle, Action::Swallow));
    }
}
```

- [ ] **Step 2: Register the module in lib.rs**

Add near the top of `src-tauri/src/lib.rs`, before the platform-gated mods:

```rust
mod keyboard;
mod tap_tracker;

#[cfg(target_os = "linux")]
mod shortcuts_inhibit;
```

- [ ] **Step 3: Run the tests — expect PASS on first run**

```bash
cargo test --manifest-path src-tauri/Cargo.toml tap_tracker
```

Expected: all 8 tests pass. If any fail, the state machine has a bug — fix `step` until they pass. No implementation-without-test dance here because the tests and logic are presented together; TDD at this granularity (a pure enum machine) means the whole module lands in one commit after the test pass is proven.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/tap_tracker.rs src-tauri/src/lib.rs
git commit -m "feat: add pure tap_tracker state machine"
```

---

## Chunk 2: Linux integration

### Task 2: Expose `is_inhibiting()` from shortcuts_inhibit

**Files:**
- Modify: `src-tauri/src/shortcuts_inhibit.rs`

- [ ] **Step 1: Add the helper**

Append to `shortcuts_inhibit.rs`:

```rust
/// Whether we currently hold an active shortcut-inhibitor. Used by the
/// tap-tracker gating — we only consume bare Super when we know the
/// compositor is passing it through to us.
pub fn is_inhibiting() -> bool {
    STATE
        .lock()
        .unwrap()
        .as_ref()
        .map(|s| s.inhibitor.is_some())
        .unwrap_or(false)
}
```

- [ ] **Step 2: Build**

```bash
cargo build --manifest-path src-tauri/Cargo.toml
```

Expected: clean build (no warning about unused fn — it'll be used in the next task).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/shortcuts_inhibit.rs
git commit -m "feat(linux): expose shortcuts_inhibit::is_inhibiting()"
```

### Task 3: Bind zwp_virtual_keyboard_manager_v1 and expose inject_alt_f3

**Files:**
- Modify: `src-tauri/src/shortcuts_inhibit.rs`
- Modify: `src-tauri/Cargo.toml` (if `wayland-protocols-misc` isn't already in; the virtual-keyboard protocol lives there)

- [ ] **Step 1: Add the wayland-protocols-misc crate**

Edit `src-tauri/Cargo.toml` under the Linux target block:

```toml
wayland-protocols-misc = { version = "0.3", features = ["client"] }
```

- [ ] **Step 2: Extend InhibitState and Sink for the virtual keyboard**

In `shortcuts_inhibit.rs`:

1. Add the new imports at the top:

```rust
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};
```

2. Add `noop_dispatch!` entries:

```rust
noop_dispatch!(ZwpVirtualKeyboardManagerV1);
noop_dispatch!(ZwpVirtualKeyboardV1);
```

3. Extend `InhibitState`:

```rust
struct InhibitState {
    conn: Connection,
    qh: QueueHandle<Sink>,
    manager: ZwpKeyboardShortcutsInhibitManagerV1,
    surface: WlSurface,
    seat: WlSeat,
    inhibitor: Option<ZwpKeyboardShortcutsInhibitorV1>,
    virtual_keyboard: Option<ZwpVirtualKeyboardV1>,
    _queue: EventQueue<Sink>,
}
```

4. In `setup()`, after the inhibit manager binding, attempt to bind the virtual-keyboard manager and create a keyboard. It's optional — missing virtual-keyboard support degrades us to swallow-only mode:

```rust
let virtual_keyboard = globals
    .bind::<ZwpVirtualKeyboardManagerV1, _, _>(&qh, 1..=1, ())
    .ok()
    .map(|vkm| {
        let kb = vkm.create_virtual_keyboard(&seat, &qh, ());
        // Upload a minimal XKB keymap so the compositor knows what our
        // keycodes mean. Using the keysym-based approach would require
        // a keymap with Alt_L = keycode 64, F3 = keycode 69 (standard
        // evdev offsets + 8). See keymap_string() below.
        let (fd, size) = build_keymap_shm().expect("keymap shm creation");
        kb.keymap(1 /* xkb_v1 */, fd.as_fd(), size);
        kb
    });
```

5. Add `build_keymap_shm()`:

```rust
use std::io::Write;
use std::os::fd::{AsFd, OwnedFd};

fn build_keymap_shm() -> std::io::Result<(OwnedFd, u32)> {
    // Minimal XKB keymap: identity, US layout. Compositor maps our
    // evdev-style keycodes (linux keycode + 8) to keysyms via this.
    // We only ever emit Alt_L (keycode 64) and F3 (69), but shipping
    // a real us-layout keeps the compositor happy in case it validates.
    let keymap = include_str!("us_keymap.xkb");
    let bytes = keymap.as_bytes();
    let mut file = tempfile::tempfile()?;
    file.write_all(bytes)?;
    file.write_all(&[0])?; // null terminator required by protocol
    let len = (bytes.len() + 1) as u32;
    Ok((OwnedFd::from(file), len))
}
```

Store the keymap at `src-tauri/src/us_keymap.xkb` — download from `xkbcommon`'s test fixtures or use the output of:
```bash
xkbcomp -xkb $DISPLAY - 2>/dev/null | head -200
```
…and trim to just the keycode + keysym sections. Commit the file as-is.

Add `tempfile = "3"` to Cargo.toml's Linux dependencies.

6. Add `inject_alt_f3()`:

```rust
/// Send Alt+F3 to the focused client via the virtual keyboard. No-op when
/// the virtual-keyboard manager wasn't bound at setup.
pub fn inject_alt_f3() {
    let guard = STATE.lock().unwrap();
    let Some(state) = guard.as_ref() else { return };
    let Some(kb) = state.virtual_keyboard.as_ref() else { return };

    // evdev linux/input-event-codes: KEY_LEFTALT=56, KEY_F3=61.
    // Wayland wants keycode with a +8 offset (X11 convention).
    const KEY_LEFTALT: u32 = 56;
    const KEY_F3: u32 = 61;
    const KEY_STATE_PRESSED: u32 = 1;
    const KEY_STATE_RELEASED: u32 = 0;
    const MOD_DEPRESSED_ALT: u32 = 0x08; // Mod1 bit

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u32)
        .unwrap_or(0);

    // modifiers must be set BEFORE the F3 press so the web client sees
    // F3 arriving under an active Alt modifier.
    kb.modifiers(MOD_DEPRESSED_ALT, 0, 0, 0);
    kb.key(now, KEY_LEFTALT, KEY_STATE_PRESSED);
    kb.key(now.wrapping_add(1), KEY_F3, KEY_STATE_PRESSED);
    kb.key(now.wrapping_add(2), KEY_F3, KEY_STATE_RELEASED);
    kb.key(now.wrapping_add(3), KEY_LEFTALT, KEY_STATE_RELEASED);
    kb.modifiers(0, 0, 0, 0);
    let _ = state.conn.flush();
}
```

- [ ] **Step 3: Build and verify compilation**

```bash
cargo build --manifest-path src-tauri/Cargo.toml
```

Expected: clean build. If `wayland-protocols-misc` feature names don't match, consult `cargo doc -p wayland-protocols-misc --open` and adjust.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/shortcuts_inhibit.rs src-tauri/src/us_keymap.xkb
git commit -m "feat(linux): bind virtual-keyboard protocol and add inject_alt_f3"
```

### Task 4: Add passthrough_linux.rs with GTK signal wiring

**Files:**
- Create: `src-tauri/src/passthrough_linux.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create the module**

```rust
//! Linux-side tap detector for bare Super → Alt+F3.
//!
//! Hooks the WebView widget's key-press / key-release / focus-out signals,
//! feeds them into the shared `tap_tracker` state machine, and invokes
//! `shortcuts_inhibit::inject_alt_f3()` on qualifying taps.
//!
//! The state is guarded by a Mutex, but in practice all GTK signal handlers
//! run on the main thread so contention only arises if the Ctrl+Alt+Shift+.
//! toggle fires mid-tap — harmless, since the toggle path only sets the
//! Wayland inhibit state, not the tracker state.

use std::sync::Mutex;

use gtk::prelude::*;
use gtk::glib;

use crate::shortcuts_inhibit;
use crate::tap_tracker::{self, Action, Event, State};

const SUPER_L_KEYCODE: u16 = 133; // evdev + 8; KEY_LEFTMETA=125
const SUPER_R_KEYCODE: u16 = 134; // KEY_RIGHTMETA=126

struct Tracker {
    state: State,
    last_modifier_down_keycode: Option<u16>,
}

static TRACKER: Mutex<Tracker> = Mutex::new(Tracker {
    state: State::Idle,
    last_modifier_down_keycode: None,
});

fn is_super_keycode(code: u16) -> bool {
    code == SUPER_L_KEYCODE || code == SUPER_R_KEYCODE
}

/// Hook the tap tracker into the WebView widget. Called from
/// `configure_webview` after the UserContentManager scripts are installed.
pub fn install(webview: &webkit2gtk::WebView) {
    webview.connect_key_press_event(|widget, event| {
        let code = event.hardware_keycode();
        let state_flags = event.state();

        let ev = if is_super_keycode(code) {
            // Detect autorepeat via identical keycode still held in tracker.
            let mut guard = TRACKER.lock().unwrap();
            let autorepeat = matches!(guard.state, State::Tracking)
                && guard.last_modifier_down_keycode == Some(code);
            let other_held = !state_flags
                .difference(
                    gdk::ModifierType::MOD2_MASK | gdk::ModifierType::LOCK_MASK,
                )
                .is_empty()
                && !state_flags.contains(gdk::ModifierType::MOD4_MASK); // Mod4 is Super itself
            guard.last_modifier_down_keycode = Some(code);
            Event::ModifierDown { other_key_held: other_held, autorepeat }
        } else {
            Event::OtherKey
        };

        handle(widget, ev)
    });

    webview.connect_key_release_event(|widget, event| {
        let code = event.hardware_keycode();
        let ev = if is_super_keycode(code) {
            TRACKER.lock().unwrap().last_modifier_down_keycode = None;
            Event::ModifierUp
        } else {
            Event::OtherKey
        };
        handle(widget, ev)
    });

    if let Some(toplevel) = webview.toplevel().and_then(|t| t.downcast::<gtk::Window>().ok()) {
        toplevel.connect_focus_out_event(|_, _| {
            feed(Event::FocusLost);
            glib::Propagation::Proceed
        });
    }
}

fn handle(_widget: &webkit2gtk::WebView, ev: Event) -> glib::Propagation {
    if !shortcuts_inhibit::is_inhibiting() {
        // Passthrough off: reset any in-flight state and let GTK keep the event.
        feed(Event::PassthroughOff);
        return glib::Propagation::Proceed;
    }
    let action = feed(ev);
    match action {
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
        guard.last_modifier_down_keycode = None;
    }
    action
}
```

- [ ] **Step 2: Register the module in lib.rs**

Under the Linux-gated `mod shortcuts_inhibit;`:

```rust
#[cfg(target_os = "linux")]
mod passthrough_linux;
```

- [ ] **Step 3: Call install from configure_webview**

In `src-tauri/src/lib.rs`, inside `configure_webview`, after `manager.add_script(&escape_script);`, add (still inside the `with_webview` closure):

```rust
passthrough_linux::install(&webview);
```

- [ ] **Step 4: Build**

```bash
cargo build --manifest-path src-tauri/Cargo.toml
```

Expected: clean. If `connect_focus_out_event` return type mismatches on your gtk crate version, check with `cargo doc -p gtk --open` — we use `glib::Propagation` which became the new return type in gtk 0.18.

- [ ] **Step 5: Manual smoke test**

```bash
cargo tauri dev
```

Verify in the debug log that `shortcuts-inhibit: installed` appears. Open an RDP session. Tap Super — the remote Start menu should open. Tap Super while holding Shift — nothing should happen. Toggle Ctrl+Alt+Shift+. and press Super — niri overview should now open instead.

If the Start menu doesn't open but the log shows the virtual-keyboard was bound, the issue is likely the keymap. Try extracting a real one via `xkbcommon` tooling and replacing `us_keymap.xkb`.

If the virtual-keyboard global wasn't advertised by niri (check `WAYLAND_DEBUG=1 cargo tauri dev 2>&1 | grep virtual_keyboard`), niri may need the protocol enabled in config. Document in CLAUDE.md.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/passthrough_linux.rs src-tauri/src/lib.rs
git commit -m "feat(linux): add Super-tap → Alt+F3 tap tracker"
```

---

## Chunk 3: macOS integration

### Task 5: Wire tap tracker into NSEvent monitor

**Files:**
- Modify: `src-tauri/src/passthrough_macos.rs`
- Modify: `src-tauri/Cargo.toml` (add core-graphics deps)

- [ ] **Step 1: Add core-graphics to Cargo.toml**

In the macOS target block:

```toml
core-graphics = "0.24"
objc2-app-kit = { version = "0.3", features = ["NSEvent", "NSWindow"] }
```

- [ ] **Step 2: Extend passthrough_macos.rs**

At the top of the file add imports:

```rust
use core_graphics::event::{
    CGEvent, CGEventField, CGEventFlags, CGEventTapLocation, CGEventType, KeyCode,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

use crate::tap_tracker::{self, Action, Event, State};
```

Add a tracker state struct alongside `MONITOR`:

```rust
struct TrackerState {
    state: State,
    last_down_keycode: Option<u16>,
}

static TRACKER: Mutex<TrackerState> = Mutex::new(TrackerState {
    state: State::Idle,
    last_down_keycode: None,
});

/// Marker we stamp on CGEvents we post ourselves, so our own local monitor
/// can recognize and skip them (prevents re-entry on the synthetic Alt+F3).
const RDPLS_EVENT_MARKER: i64 = 0x7264_706c_7300_0001; // "rdpls\x00\x01"

const VK_COMMAND: u16 = 0x37;
const VK_RIGHT_COMMAND: u16 = 0x36;
const VK_F3: u16 = 0x63;
```

In the handler block, branch before calling `should_swallow`:

```rust
let handler = RcBlock::new(|event: NonNull<NSEvent>| -> *mut NSEvent {
    let ev = unsafe { event.as_ref() };

    // Skip our own synthesized events.
    if is_rdpls_synthetic(ev) {
        return event.as_ptr();
    }

    if let Some(action) = feed_tracker(ev) {
        match action {
            Action::Swallow => return std::ptr::null_mut(),
            Action::SwallowAndInjectAltF3 => {
                post_alt_f3();
                return std::ptr::null_mut();
            }
            Action::PassThrough | Action::Noop => {}
        }
    }

    if should_swallow(ev) {
        std::ptr::null_mut()
    } else {
        event.as_ptr()
    }
});
```

Add helpers at the bottom:

```rust
fn is_rdpls_synthetic(ev: &NSEvent) -> bool {
    unsafe {
        let cg = ev.CGEvent();
        if cg.is_null() {
            return false;
        }
        let user_data = core_graphics::event::CGEventRef::from_ptr(cg as *const _)
            .get_integer_value_field(CGEventField::EventSourceUserData);
        user_data == RDPLS_EVENT_MARKER
    }
}

fn feed_tracker(ev: &NSEvent) -> Option<Action> {
    let kc = ev.keyCode();
    let is_cmd = kc == VK_COMMAND || kc == VK_RIGHT_COMMAND;
    let event_type = ev.r#type();

    // FlagsChanged tells us modifier down/up; KeyDown/KeyUp for regular keys.
    use objc2_app_kit::NSEventType;
    let tracker_event = match event_type {
        NSEventType::FlagsChanged if is_cmd => {
            let cmd_down = ev
                .modifierFlags()
                .contains(NSEventModifierFlags::Command);
            if cmd_down {
                let mut guard = TRACKER.lock().unwrap();
                let autorepeat = matches!(guard.state, State::Tracking)
                    && guard.last_down_keycode == Some(kc);
                let other_held = ev
                    .modifierFlags()
                    .intersection(NSEventModifierFlags::DeviceIndependentFlagsMask)
                    .difference(NSEventModifierFlags::Command)
                    != NSEventModifierFlags::empty();
                guard.last_down_keycode = Some(kc);
                drop(guard);
                Event::ModifierDown { other_key_held: other_held, autorepeat }
            } else {
                TRACKER.lock().unwrap().last_down_keycode = None;
                Event::ModifierUp
            }
        }
        NSEventType::FlagsChanged => Event::OtherKey,
        NSEventType::KeyDown | NSEventType::KeyUp => Event::OtherKey,
        _ => return None,
    };

    let mut guard = TRACKER.lock().unwrap();
    let (new_state, action) = tap_tracker::step(guard.state, tracker_event);
    guard.state = new_state;
    if matches!(new_state, State::Idle) {
        guard.last_down_keycode = None;
    }
    Some(action)
}

fn post_alt_f3() {
    let Ok(src) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) else {
        return;
    };

    let stamp = |ev: &CGEvent| {
        ev.set_integer_value_field(
            CGEventField::EventSourceUserData,
            RDPLS_EVENT_MARKER,
        );
        ev.set_flags(CGEventFlags::CGEventFlagAlternate);
    };

    if let Ok(down) = CGEvent::new_keyboard_event(src.clone(), VK_F3, true) {
        stamp(&down);
        down.post(CGEventTapLocation::HID);
    }
    if let Ok(src2) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
        if let Ok(up) = CGEvent::new_keyboard_event(src2, VK_F3, false) {
            stamp(&up);
            up.post(CGEventTapLocation::HID);
        }
    }
}
```

- [ ] **Step 3: Change NSEventMask at addLocalMonitor to include FlagsChanged**

Locate the existing `NSEventMask::KeyDown` in `install_locked` and change to:

```rust
NSEventMask::KeyDown | NSEventMask::KeyUp | NSEventMask::FlagsChanged
```

- [ ] **Step 4: Add focus-out observer**

Inside `install_locked`, after installing the event monitor, observe `NSWindowDidResignKeyNotification` on the main window and reset the tracker:

```rust
// Pseudocode — the actual NSNotificationCenter + block ownership dance
// mirrors the existing RcBlock usage. Add a second RcBlock and a second
// static Mutex<Option<MonitorHandle>> for the observer token.
```

If the NSNotificationCenter plumbing turns out to require more than ~40 lines, defer it to a follow-up and document the gap — focus-loss is an edge case, not the main risk.

- [ ] **Step 5: Build**

```bash
cargo build --target aarch64-apple-darwin --manifest-path src-tauri/Cargo.toml
```

On a Mac or in CI. If you're on Linux only, skip this — just verify the Linux build still passes:

```bash
cargo build --manifest-path src-tauri/Cargo.toml
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/passthrough_macos.rs
git commit -m "feat(macos): add Cmd-tap → Alt+F3 via CGEventPost"
```

---

## Chunk 4: Documentation

### Task 6: Update keyboard-matrix.md

**Files:**
- Modify: `docs/keyboard-matrix.md`

- [ ] **Step 1: Rewrite the Compositor-level keys and rdpls-specific sections**

Replace the "Super / Win" row in the compositor-level table with:

```markdown
| Super / Win     | ⚠️ See "Web-client limitation" below |
```

Add a new section before "rdpls-specific":

```markdown
## Web-client limitation: the Windows key

The Microsoft HTML5 RDP client (Windows 365 / AVD web client) does not
forward Meta / Super / Cmd keydowns to the remote session. This is a
Microsoft-side limitation — no amount of keyboard passthrough on the rdpls
side can deliver `Win+L`, `Win+D`, `Win+E`, `Win+R`, or `Win+Arrow` to the
guest VM through the web client.

The web client does recognize four alternate keystrokes:

| Combo        | Remote effect              |
|--------------|----------------------------|
| Ctrl+Alt+End | Ctrl+Alt+Del               |
| **Alt+F3**   | **Windows key (Start menu)** |
| Alt+PageUp   | Alt+Tab                    |
| Alt+PageDown | Alt+Shift+Tab              |

rdpls remaps a **bare Super tap** (Linux) or **bare Cmd tap** (macOS) to
Alt+F3 so the muscle-memory "tap Super → Start menu" behavior works. It
does NOT synthesize Win+X combos — there is no combo the web client
accepts. Users who need in-guest shortcuts like Win+L should map them
inside the VM (e.g. AutoHotkey rebinding `Ctrl+Shift+L` → `LockWorkstation`).

Sources: Microsoft AVD web client docs
(learn.microsoft.com/azure/virtual-desktop/users/client-features-web) and
an unanswered Tech Community thread on the same question
(techcommunity.microsoft.com/t5/azure-virtual-desktop/avd-webclient-key-mapping/td-p/3821461).

**Known regression:** while rdpls is focused on the Microsoft auth /
launcher page (no active session), Super is still swallowed, so niri's
overview won't open on Super. Use Ctrl+Alt+Shift+. to hand Super back to
niri when needed.
```

- [ ] **Step 2: Commit**

```bash
git add docs/keyboard-matrix.md
git commit -m "docs: document web-client Win-key limitation and Super-tap remap"
```

### Task 7: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Add a bullet under Key Constraints**

After the existing "Wayland-native" bullet, add:

```markdown
- **Web-client cannot receive Win-modifier keys.** Microsoft's HTML5 RDP
  client drops Meta/Super/Cmd keydowns — `Win+L`, `Win+D`, `Win+Arrow`
  etc. are not deliverable through the web client, period. The only
  Win-proxy it accepts is `Alt+F3` (Start menu). rdpls remaps a bare
  Super/Cmd *tap* to Alt+F3; Win-combos must be handled in-guest (e.g.
  AutoHotkey). See `docs/keyboard-matrix.md` for the full matrix.
```

Update the "Status" section to note the Super-tap remap shipped in Phase 3.

- [ ] **Step 2: Update the Project Layout section**

Add to the file list:
```
  src/passthrough_linux.rs     # Linux only: WebView key-event hook + virtual-keyboard injection for Super-tap → Alt+F3
  src/tap_tracker.rs           # Pure state machine for Super/Cmd tap detection (platform-agnostic)
```

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: note web-client Win-key limitation in CLAUDE.md"
```

---

## Post-implementation verification

Before considering the plan complete:

- [ ] `cargo test --manifest-path src-tauri/Cargo.toml` — all tests pass.
- [ ] `cargo tauri dev` on niri — launch, open RDP session, verify:
  - Tap Super → remote Start menu opens.
  - Shift+Super → nothing (not a tap).
  - Super+L → nothing in remote (web client drops it; expected).
  - Ctrl+Alt+Shift+. off → Super opens niri overview.
  - Ctrl+Alt+Shift+. on → Super-tap again opens remote Start menu.
- [ ] If tested on macOS: analogous checks with Cmd.
- [ ] If virtual-keyboard isn't advertised: confirm the code degrades gracefully (no panic, Super is swallowed but no Alt+F3 fires). Document niri config change if needed.
