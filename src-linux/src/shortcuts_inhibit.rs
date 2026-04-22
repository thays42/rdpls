//! Wayland keyboard-shortcuts-inhibit integration for rdpls (GTK4 port).
//!
//! Uses the `zwp_keyboard_shortcuts_inhibit_manager_v1` unstable protocol to
//! ask the compositor (niri on the primary target, GNOME/Mutter also honored)
//! to stop eating keys while rdpls has focus. Alt+Tab, Super, etc. then reach
//! the remote RDP client.
//!
//! We share GTK's existing Wayland connection via `Backend::from_foreign_display`
//! and dispatch our protocol objects onto a dedicated queue. We never dispatch
//! incoming events on that queue (GTK owns the main loop), we only flush our
//! outgoing requests. The inhibitor & manager don't send events we need to
//! act on.
//!
//! Ported from the GTK3 version that lived in `src-tauri/src/shortcuts_inhibit.rs`.
//! The protocol plumbing is unchanged; only the surface/seat extraction moved
//! from `gdk_wayland_window_*` to `gdk_wayland_surface_*` for GTK4.

use std::os::raw::c_void;
use std::sync::Mutex;

use gtk::glib::translate::ToGlibPtr;
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
    _queue: EventQueue<Sink>,
}

// SAFETY: wayland-client Connection, QueueHandle, and Proxy types are all
// Send + Sync in 0.31 (verified via trait bounds). We bundle them here.
unsafe impl Send for InhibitState {}

static STATE: Lazy<Mutex<Option<InhibitState>>> = Lazy::new(|| Mutex::new(None));

/// Install the initial inhibitor on the given realized GTK4 window. No-op if
/// the Wayland plumbing is unavailable (running under XWayland, or the
/// compositor doesn't advertise the inhibit manager global).
pub fn install(window: &gtk::ApplicationWindow) {
    match setup(window) {
        Some(()) => log::info!("shortcuts-inhibit: installed"),
        None => log::warn!("shortcuts-inhibit: not available"),
    }
}

fn setup(window: &gtk::ApplicationWindow) -> Option<()> {
    let surface = window.surface()?;
    let display = surface.display();

    let raw_display: *mut gdk::ffi::GdkDisplay = display.to_glib_none().0;
    let wl_display_ptr: *mut c_void = unsafe {
        gdk_wayland_sys::gdk_wayland_display_get_wl_display(raw_display as *mut _)
    } as *mut c_void;
    if wl_display_ptr.is_null() {
        return None;
    }

    let raw_surface: *mut gdk::ffi::GdkSurface = surface.to_glib_none().0;
    let wl_surface_ptr: *mut c_void = unsafe {
        gdk_wayland_sys::gdk_wayland_surface_get_wl_surface(raw_surface as *mut _)
    } as *mut c_void;
    if wl_surface_ptr.is_null() {
        return None;
    }

    let gdk_seat = display.default_seat()?;
    let raw_seat: *mut gdk::ffi::GdkSeat = gdk_seat.to_glib_none().0;
    let wl_seat_ptr: *mut c_void = unsafe {
        gdk_wayland_sys::gdk_wayland_seat_get_wl_seat(raw_seat as *mut _)
    } as *mut c_void;
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
    let wl_surface = WlSurface::from_id(&conn, surface_id).ok()?;

    let seat_id =
        unsafe { ObjectId::from_ptr(WlSeat::interface(), wl_seat_ptr as *mut _) }.ok()?;
    let seat = WlSeat::from_id(&conn, seat_id).ok()?;

    let inhibitor = manager.inhibit_shortcuts(&wl_surface, &seat, &qh, ());
    conn.flush().ok()?;

    *STATE.lock().unwrap() = Some(InhibitState {
        conn,
        qh,
        manager,
        surface: wl_surface,
        seat,
        inhibitor: Some(inhibitor),
        _queue: queue,
    });

    Some(())
}

/// Toggle the inhibitor on/off. Returns the new state (true = now inhibiting).
/// Invoked by the `rdpls_toggle_inhibit` script-message handler.
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
    log::info!(
        "shortcuts-inhibit: now {}",
        if now_inhibiting { "ON" } else { "OFF" }
    );
    now_inhibiting
}
