//! Wayland keyboard-shortcuts-inhibit integration for rdpls.
//!
//! Uses the `zwp_keyboard_shortcuts_inhibit_manager_v1` unstable protocol to
//! ask the compositor (niri on the primary target) to stop eating keys while
//! rdpls has focus. Alt+Tab, Super, etc. then reach the remote RDP client.
//!
//! niri supports this protocol automatically — we just ask and it honors.
//! Users have a standard escape hatch via niri's `toggle-keyboard-shortcuts-inhibit`
//! action if they bind it (default unbound).
//!
//! We share GTK's existing Wayland connection via `Backend::from_foreign_display`
//! and dispatch our protocol objects onto a dedicated queue. We never dispatch
//! incoming events on that queue (GTK owns the main loop), we only flush our
//! outgoing requests. The inhibitor & manager don't send events we need to
//! act on for our purposes.

use std::os::raw::c_void;
use std::sync::Mutex;

use glib::translate::ToGlibPtr;
use gtk::prelude::*;

use once_cell::sync::Lazy;

use wayland_backend::client::ObjectId;
use wayland_backend::sys::client::Backend;
use wayland_client::globals::{registry_queue_init, GlobalList, GlobalListContents};
use wayland_client::protocol::{wl_registry::WlRegistry, wl_seat::WlSeat, wl_surface::WlSurface};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::wp::keyboard_shortcuts_inhibit::zv1::client::{
    zwp_keyboard_shortcuts_inhibit_manager_v1::ZwpKeyboardShortcutsInhibitManagerV1,
    zwp_keyboard_shortcuts_inhibitor_v1::ZwpKeyboardShortcutsInhibitorV1,
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};

struct Sink;

macro_rules! noop_dispatch {
    ($ty:ty) => {
        impl Dispatch<$ty, ()> for Sink {
            fn event(
                _: &mut Self,
                _: &$ty,
                _: <$ty as Proxy>::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
            }
        }
    };
}

noop_dispatch!(WlSurface);
noop_dispatch!(WlSeat);
noop_dispatch!(ZwpKeyboardShortcutsInhibitManagerV1);
noop_dispatch!(ZwpVirtualKeyboardManagerV1);
noop_dispatch!(ZwpVirtualKeyboardV1);

impl Dispatch<WlRegistry, GlobalListContents> for Sink {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpKeyboardShortcutsInhibitorV1, ()> for Sink {
    fn event(
        _: &mut Self,
        _: &ZwpKeyboardShortcutsInhibitorV1,
        event: <ZwpKeyboardShortcutsInhibitorV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        log::info!("shortcuts-inhibit: {:?}", event);
    }
}

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

// SAFETY: wayland-client Connection, QueueHandle, and Proxy types are all
// Send + Sync in 0.31 (verified via trait bounds). We bundle them here.
unsafe impl Send for InhibitState {}

static STATE: Lazy<Mutex<Option<InhibitState>>> = Lazy::new(|| Mutex::new(None));

/// Install the initial inhibitor on the given Tauri WebviewWindow's underlying
/// GTK window. No-op on non-Linux or if the Wayland plumbing is unavailable
/// (e.g. running under XWayland, or the compositor doesn't advertise the
/// inhibit manager global).
pub fn install(webview_window: &tauri::WebviewWindow) -> tauri::Result<()> {
    webview_window.with_webview(|wv| match setup(&wv) {
        Some(()) => log::info!("shortcuts-inhibit: installed"),
        None => log::warn!("shortcuts-inhibit: not available"),
    })
}

fn setup(wv: &tauri::webview::PlatformWebview) -> Option<()> {
    use glib::object::Cast;

    let webview = wv.inner();
    let widget = webview.upcast_ref::<gtk::Widget>();

    let toplevel = widget.toplevel()?;
    let gtk_window = toplevel.downcast_ref::<gtk::Window>()?;

    if !gtk_window.is_realized() {
        gtk_window.realize();
    }
    let gdk_window = gtk_window.window()?;
    let gdk_display = gdk_window.display();

    let raw_display: *mut gdk::ffi::GdkDisplay = gdk_display.to_glib_none().0;
    let wl_display_ptr: *mut c_void = unsafe {
        gdk_wayland_sys::gdk_wayland_display_get_wl_display(raw_display as *mut _)
    } as *mut c_void;
    if wl_display_ptr.is_null() {
        return None;
    }

    let raw_window: *mut gdk::ffi::GdkWindow = gdk_window.to_glib_none().0;
    let wl_surface_ptr: *mut c_void = unsafe {
        gdk_wayland_sys::gdk_wayland_window_get_wl_surface(raw_window as *mut _)
    } as *mut c_void;
    if wl_surface_ptr.is_null() {
        return None;
    }

    let gdk_seat = gdk_display.default_seat()?;
    let raw_seat: *mut gdk::ffi::GdkSeat = gdk_seat.to_glib_none().0;
    let wl_seat_ptr: *mut c_void =
        unsafe { gdk_wayland_sys::gdk_wayland_seat_get_wl_seat(raw_seat as *mut _) }
            as *mut c_void;
    if wl_seat_ptr.is_null() {
        return None;
    }

    let backend = unsafe { Backend::from_foreign_display(wl_display_ptr as *mut _) };
    let conn = Connection::from_backend(backend);

    let (globals, queue): (GlobalList, EventQueue<Sink>) = registry_queue_init(&conn).ok()?;
    let qh = queue.handle();

    let manager: ZwpKeyboardShortcutsInhibitManagerV1 = globals
        .bind(&qh, 1..=1, ())
        .map_err(|e| log::warn!("inhibit manager not available: {e}"))
        .ok()?;

    let surface_id =
        unsafe { ObjectId::from_ptr(WlSurface::interface(), wl_surface_ptr as *mut _) }.ok()?;
    let surface = WlSurface::from_id(&conn, surface_id).ok()?;

    let seat_id =
        unsafe { ObjectId::from_ptr(WlSeat::interface(), wl_seat_ptr as *mut _) }.ok()?;
    let seat = WlSeat::from_id(&conn, seat_id).ok()?;

    let inhibitor = manager.inhibit_shortcuts(&surface, &seat, &qh, ());

    let virtual_keyboard: Option<ZwpVirtualKeyboardV1> = globals
        .bind::<ZwpVirtualKeyboardManagerV1, _, _>(&qh, 1..=1, ())
        .ok()
        .and_then(|vkm| {
            let kb = vkm.create_virtual_keyboard(&seat, &qh, ());
            match upload_keymap(&kb) {
                Ok(()) => Some(kb),
                Err(e) => {
                    log::warn!("virtual-keyboard keymap upload failed: {e}");
                    None
                }
            }
        });

    conn.flush().ok()?;

    *STATE.lock().unwrap() = Some(InhibitState {
        conn,
        qh,
        manager,
        surface,
        seat,
        inhibitor: Some(inhibitor),
        virtual_keyboard,
        _queue: queue,
    });

    Some(())
}

/// Toggle the inhibitor on/off. Invoked by the Ctrl+Alt+Shift+Escape hotkey.
pub fn toggle() -> bool {
    let mut guard = STATE.lock().unwrap();
    let Some(state) = guard.as_mut() else {
        return false;
    };

    let now_inhibiting = if let Some(inhibitor) = state.inhibitor.take() {
        inhibitor.destroy();
        false
    } else {
        state.inhibitor = Some(
            state
                .manager
                .inhibit_shortcuts(&state.surface, &state.seat, &state.qh, ()),
        );
        true
    };

    let _ = state.conn.flush();
    log::info!("shortcuts-inhibit: now {}", if now_inhibiting { "ON" } else { "OFF" });
    now_inhibiting
}

use std::io::{Seek, SeekFrom, Write};
use std::os::fd::AsFd;

/// Upload a minimal us-layout XKB keymap to a newly created virtual keyboard.
/// The compositor uses this to translate our evdev-style keycodes back into
/// keysyms for the focused client. We only ever emit Alt_L (evdev 56, kc 64)
/// and F3 (evdev 61, kc 69), but shipping a real us keymap keeps strict
/// compositors happy.
fn upload_keymap(kb: &ZwpVirtualKeyboardV1) -> std::io::Result<()> {
    const KEYMAP: &str = include_str!("us_keymap.xkb");
    let mut tmp = tempfile::tempfile()?;
    tmp.write_all(KEYMAP.as_bytes())?;
    tmp.write_all(b"\0")?;
    tmp.seek(SeekFrom::Start(0))?;
    let size = (KEYMAP.len() + 1) as u32;
    kb.keymap(1 /* xkb_v1 */, tmp.as_fd(), size);
    Ok(())
}

/// Send Alt+F3 to the focused client via the virtual keyboard. No-op when
/// the virtual-keyboard manager wasn't bound at setup.
pub fn inject_alt_f3() {
    // Clone what we need and release the STATE lock before issuing Wayland
    // requests + flush, so any future re-entrant code paths can't deadlock.
    // Connection and virtual-keyboard proxies are Arc-backed internally.
    let (conn, kb) = {
        let guard = STATE.lock().unwrap();
        let Some(state) = guard.as_ref() else { return };
        let Some(kb) = state.virtual_keyboard.as_ref() else {
            log::warn!("inject_alt_f3: virtual keyboard unavailable");
            return;
        };
        (state.conn.clone(), kb.clone())
    };

    // evdev keycode for F3; the virtual-keyboard protocol uses raw evdev
    // codes (no X11 +8 offset). Alt is applied via modifiers() bits, not a
    // separate Alt_L press — that's the canonical wlroots pattern.
    const KEY_F3: u32 = 61;
    const PRESSED: u32 = 1;
    const RELEASED: u32 = 0;
    const MOD_ALT: u32 = 0x08; // Mod1

    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u32)
        .unwrap_or(0);

    kb.modifiers(MOD_ALT, 0, 0, 0);
    kb.key(t, KEY_F3, PRESSED);
    kb.key(t.wrapping_add(1), KEY_F3, RELEASED);
    kb.modifiers(0, 0, 0, 0);
    let _ = conn.flush();
}

/// Whether the tap tracker should intercept bare Super. True only when we
/// both hold an active shortcut-inhibitor (so the compositor is handing
/// Super to us) AND have a virtual keyboard bound (so we can inject Alt+F3
/// back out). If either is missing, bare Super passes through untouched so
/// the user still gets the compositor's Super handling instead of a silent
/// swallow with no Start-menu substitute.
pub fn should_intercept_super() -> bool {
    STATE
        .lock()
        .unwrap()
        .as_ref()
        .map(|s| s.inhibitor.is_some() && s.virtual_keyboard.is_some())
        .unwrap_or(false)
}

#[tauri::command]
pub fn rdpls_toggle_inhibit() -> bool {
    toggle()
}
