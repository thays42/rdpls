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

use std::ptr::NonNull;
use std::sync::Mutex;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags};

/// AppKit objects aren't Send in general, but the monitor handle is opaque —
/// we never dereference it off the main thread. `rdpls_toggle_passthrough`
/// hops to the main thread before touching the NSEvent API; the Mutex only
/// needs to serialize access to the Option itself.
struct MonitorHandle(Retained<AnyObject>);
unsafe impl Send for MonitorHandle {}

static MONITOR: Mutex<Option<MonitorHandle>> = Mutex::new(None);

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

fn install_locked(guard: &mut std::sync::MutexGuard<'_, Option<MonitorHandle>>) {
    if guard.is_some() {
        return;
    }
    let handler = RcBlock::new(|event: NonNull<NSEvent>| -> *mut NSEvent {
        let ev = unsafe { event.as_ref() };
        if should_swallow(ev) {
            std::ptr::null_mut()
        } else {
            event.as_ptr()
        }
    });
    let monitor = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &handler)
    };
    **guard = monitor.map(MonitorHandle);
}

fn toggle_locked() -> bool {
    let mut guard = MONITOR.lock().unwrap();
    if let Some(m) = guard.take() {
        unsafe { NSEvent::removeMonitor(&m.0) };
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
