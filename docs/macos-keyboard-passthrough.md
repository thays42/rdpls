# macOS keyboard passthrough — design

## Why this is a separate doc

On Linux (niri / Wayland), keyboard passthrough is one call: request
`zwp_keyboard_shortcuts_inhibit_manager_v1` against our surface and the
compositor stops swallowing Alt+Tab, Super, etc. The protocol doesn't exist on
macOS. Apple also reserves a non-trivial set of keys at the OS level that no
application can reclaim. Before writing code we need to agree on what is
reachable and what the toggle actually does.

## What macOS reserves

These keys never reach any application, regardless of technique, unless the
user disables them in System Settings:

- `Cmd+Tab`, `Cmd+Shift+Tab` — app switcher
- `Cmd+Space`, `Cmd+Opt+Space` — Spotlight / Finder search
- `Ctrl+Up`, `Ctrl+Down`, `Ctrl+Left`, `Ctrl+Right` — Mission Control / Spaces
- `F3` / Mission Control key, `F4` / Launchpad key
- Brightness, volume, media keys (unless "Use F1, F2, etc. as standard function
  keys" is enabled)
- `Cmd+Q`, `Cmd+H`, `Cmd+M`, `Cmd+W` — only if an app menu exists; we ship with
  no menu so these are free

The RDP client cannot receive these no matter what we do. Users who need
Windows shortcuts that collide (e.g. `Cmd+Tab` → `Alt+Tab`) must remap on the
macOS side via System Settings → Keyboard → Keyboard Shortcuts, or use
`hidutil`. This is documentation, not code.

## What we can intercept

Everything else can be grabbed by the app with an `NSEvent` local monitor:

```swift
NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
    // return nil  → swallow (app handles it)
    // return event → pass on to the focused view
    return event
}
```

Local monitors fire only while rdpls is the key window, which is what we want
— we never want to eat keys while the user is in another app. A local monitor
is enough for all the in-window collisions we care about:

- macOS system beep on unhandled keys — swallow
- WKWebView's own accelerators (`Cmd+[`, `Cmd+]` for back/forward; `Cmd+R`
  reload; `Cmd+F` find) — swallow so they go to the remote session
- Ctrl/Cmd+scroll zoom — already handled by injected JS

Global monitors (`addGlobalMonitorForEvents`) observe events in other apps but
cannot consume them, so they're not useful here.

## What the toggle does on macOS

On Linux, `Ctrl+Alt+Shift+Escape` toggles the compositor inhibit. The natural
Mac analogue is toggling the local `NSEvent` monitor:

- **Inhibit ON** (default): monitor installed, WKWebView accelerators and
  unhandled keys pass through to the remote session. Toast: "Keys → Remote."
- **Inhibit OFF**: monitor removed, WKWebView gets `Cmd+F`/`Cmd+R`/etc. back.
  Toast: "Keys → Local."

The toggle does *not* affect OS-reserved keys — those are never ours to
forward. The toast wording matches Linux exactly, so muscle memory carries.

Ownership of the hotkey itself: still `Ctrl+Alt+Shift+Escape`, injected as JS
in the WebView (same as Linux). The JS calls a new Tauri command
`rdpls_toggle_passthrough` on macOS instead of `rdpls_toggle_inhibit`. Until
the monitor is implemented, the Mac build wires the same hotkey to
`rdpls_exit` so the decorationless window has a way out.

## Implementation (shipped)

Lives in `src-tauri/src/passthrough_macos.rs`. The below sketch matches the
code at the time of landing — treat the code as the source of truth.



```rust
// src-tauri/src/passthrough_macos.rs
use std::sync::Mutex;
use objc2::rc::Retained;
use objc2_app_kit::NSEvent;

static MONITOR: Mutex<Option<Retained<AnyObject>>> = Mutex::new(None);

#[tauri::command]
pub fn rdpls_toggle_passthrough() -> bool {
    let mut guard = MONITOR.lock().unwrap();
    if guard.is_some() {
        // remove monitor → keys go to WKWebView (local mode)
        let m = guard.take().unwrap();
        unsafe { NSEvent::removeMonitor(&m); }
        false
    } else {
        // install monitor that returns nil for the keys we want to swallow
        let handler = block::ConcreteBlock::new(|event: &NSEvent| -> Option<Retained<NSEvent>> {
            // decision table: which events to eat vs pass through
            None // placeholder: swallow everything
        });
        let m = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(
            NSEventMask::KeyDown, &handler.copy(),
        )};
        *guard = Some(m);
        true
    }
}
```

Key decisions still open:

1. **Default state.** Inhibit-on at launch (Linux default) or inhibit-off? On
   Linux the inhibitor is cheap and the compositor handles everything; on
   macOS, inhibit-on means the user loses `Cmd+R` / `Cmd+F` / etc. on the
   login and auth pages before they even hit the RDP session. Proposal:
   **inhibit-off until the WebView navigates to a known RDP session URL**, or
   simpler, inhibit-off and trust the user to toggle.

2. **Decision table.** Which keys does the monitor actually swallow? Probably
   all `Cmd`-modified events while the RDP app is focused, but this needs a
   real session to tune against.

3. **Right-Cmd as Windows key.** The RDP web client maps left-Cmd to left-Win
   by default. Some users prefer right-Cmd only. Out of scope here — a System
   Settings remap or `hidutil` is the right layer.

## Ship order

1. Exit hotkey + UA + popup redirect + context menu suppression. *(shipped)*
2. Keyboard passthrough monitor + toggle command + toast. *(shipped — starts
   inhibit-on, swallows `Cmd+R`/`Cmd+[`/`Cmd+]` and their Shift variants.)*
3. Any tuning of the decision table from real-session testing.
