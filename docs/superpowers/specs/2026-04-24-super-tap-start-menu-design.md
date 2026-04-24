# Super/Cmd tap → Alt+F3 (Start menu in remote)

**Status:** Design, 2026-04-24.

## Problem

Microsoft's HTML5 RDP client (Windows 365 / AVD web client) does not forward
Meta/Super/Cmd keydowns to the remote RDP protocol stream. Pressing Super in a
session does nothing in the remote VM, regardless of whether the host OS keeps
the key out of compositor hands. This is a Microsoft-side limitation, not a
rdpls bug; the keyboard matrix row claiming Super worked on niri was
documenting compositor-level passthrough, not remote delivery.

The web client documents exactly four alternate keystrokes that it does
translate into remote-side actions:

| Client combo      | Remote effect              |
|-------------------|----------------------------|
| Ctrl+Alt+End      | Ctrl+Alt+Del               |
| **Alt+F3**        | **Windows key (Start menu)** |
| Alt+PageUp        | Alt+Tab                    |
| Alt+PageDown      | Alt+Shift+Tab              |

No documented or community-found alternate exists for Win+L, Win+D, Win+E,
Win+R, or Win+Arrow. Those combos cannot be delivered through the web client;
they have to be handled inside the guest (e.g. AutoHotkey).

The one affordance we *can* recover is the bare Windows key → Start menu
behavior, via Alt+F3.

## Goals

- Bare Super tap on Linux and bare Cmd tap on macOS open the remote Start menu.
- No spurious Alt+F3 injection when Super/Cmd is used as a modifier (Super+L,
  Cmd+Tab, etc.) or held without intent.
- No behavior change when the user has toggled passthrough OFF
  (Ctrl+Alt+Shift+.) — Super still reaches niri for overview, Cmd still does
  macOS system things.
- Honest documentation: the Super row in `docs/keyboard-matrix.md` no longer
  claims "reaches remote"; CLAUDE.md gains a Web-client-limitation note.

## Non-goals

- Rewriting Super+letter or Super+arrow combos into something the web client
  accepts. Nothing the web client accepts exists for those, per Microsoft's
  published shortcut table and confirmed by an unanswered techcommunity
  thread on exactly this question.
- Providing in-guest remaps. That lives in user-side AutoHotkey / PowerToys;
  we point at it in docs and stop.
- Changing the inhibit protocol, toggle chord, or fullscreen behavior.

## Tap semantics

A "tap" is: modifier keydown → modifier keyup with no other key held at
keydown, no intervening key events, and passthrough ON at both press and
release.

State machine, per modifier (Super on Linux, Cmd on macOS):

| Current state     | Event                                           | Next state | Side effect                         |
|-------------------|-------------------------------------------------|------------|-------------------------------------|
| idle              | modifier keydown, passthrough ON, nothing else held | tracking | swallow event                       |
| idle              | modifier keydown, passthrough ON, other key held    | tainted  | swallow event                       |
| idle              | modifier keydown, passthrough OFF               | idle       | pass through                        |
| tracking          | modifier autorepeat keydown                     | tracking   | swallow event                       |
| tracking          | any other keydown or keyup                      | tainted    | pass through                        |
| tracking          | modifier keyup                                  | idle       | inject Alt+F3 press+release         |
| tainted           | any key event except modifier keyup             | tainted    | pass through                        |
| tainted           | modifier keyup                                  | idle       | swallow event (symmetric to keydown) |
| any non-idle      | passthrough toggles OFF                         | idle       | no injection                        |
| any non-idle      | window focus-out                                | idle       | no injection                        |

Notes:
- Bare modifier keydown is **swallowed** even before we know whether it's a
  tap. The WebView has no use for a bare Super/Cmd event and firing Alt+F3
  on keydown would fire before the user could abort.
- Modifier keyup is **also** swallowed in both `tracking` and `tainted` paths,
  so the WebView never sees lone Super/Cmd press/release pairs at all. This
  keeps the JS side from observing a bare modifier event that it couldn't
  have acted on anyway.
- "Any other key" includes additional modifier keys. Super+Shift (no further
  key) is not a tap.
- **Initial-state check:** if any other key (modifier or otherwise) is
  already held when the tracked modifier goes down, enter `tainted` directly.
  This prevents Alt-held-then-Super-tap from firing Alt+F3 (which would
  combine with the held Alt and mis-read intent).
- **Autorepeat detection:** GDK delivers autorepeat as repeated
  `key-press-event`s with no intervening release. Detect by comparing the
  incoming event's `hardware_keycode` against the last-down keycode; identical
  + still in tracking = repeat, swallow and stay in `tracking`. On macOS,
  `NSEvent.isARepeat` gives the same signal.
- **Focus loss:** force state → idle on window focus-out
  (GTK `focus-out-event` on Linux, `NSWindowDidResignKeyNotification` on
  macOS). Prevents a Super-down-then-Alt+Tab-away-and-back from leaving the
  tracker stuck.
- Passthrough toggle OFF while non-idle: reset state, do not inject.

## Architecture

### Linux (`src-tauri/src/passthrough_linux.rs`, new)

- Installed from `configure_webview` in `lib.rs`. Hooks the WebView widget's
  `key-press-event` and `key-release-event` signals, plus `focus-out-event`
  on the containing GTK window.
- Reads passthrough state via a new `shortcuts_inhibit::is_inhibiting() -> bool`
  helper that inspects `STATE`.
- All state (tap tracker + last-hardware-keycode for autorepeat) lives behind
  a `Mutex`. GTK key and focus signals run on the main thread, so contention
  is limited to the Ctrl+Alt+Shift+. toggle path; documented in the module
  header.

**`key-press-event` caveat.** webkit2gtk handles input via its own
`GtkIMContext` wiring and some keys consumed by the web content may not
surface a widget-level signal in the way pure-GTK apps expect. Before
merging, a smoke test must confirm the signal fires for Super press/release
with `return Propagation::Stop` actually suppressing delivery to the
WebView. If it does not, fall back to connecting on the containing GTK
window (`gtk::Window::connect_key_press_event`), which runs earlier in
dispatch.

**Event injection approach.** Synthesizing via `gtk_widget_event` is the
obvious path but produces JS `KeyboardEvent`s with `isTrusted=false`.
Microsoft's web client is expected to filter these. Plan:

1. **Primary path: `zwp_virtual_keyboard_manager_v1`.** niri supports the
   virtual-keyboard protocol. Binding it alongside the existing
   `zwp_keyboard_shortcuts_inhibit_manager_v1` in `shortcuts_inhibit.rs` lets
   us emit real keysym events at the compositor layer. These arrive at the
   WebView as if typed on the physical keyboard, so `isTrusted=true`. This
   is the Wayland-native analog of what Karabiner does on macOS.
2. **Fallback if virtual-keyboard isn't advertised:** swallow bare Super
   with no injection. Users get silence instead of niri overview, plus the
   existing Ctrl+Alt+Shift+. escape hatch. The state machine is identical;
   the `inject_alt_f3` side effect becomes a no-op.
3. **`gtk_widget_event` is not used**, to avoid shipping code whose
   effectiveness depends on webkit2gtk's internal trust-bit handling for
   synthesized GdkEventKey dispatch.

The four-event sequence (Alt down, F3 down, F3 up, Alt up) is emitted via
the virtual keyboard with standard XKB keysyms for Alt_L and F3 plus
modifier-state tracking as the protocol requires.

### macOS (extend `src-tauri/src/passthrough_macos.rs`)

- The existing NSEvent local monitor already sees every key event, gates on
  the passthrough state, and returns `nil` to swallow or passes the event
  through. Extend it with the tap-tracker state and a
  `NSWindowDidResignKeyNotification` observer that resets the tracker on
  focus loss.
- On qualifying Cmd tap, post **two** CGEvents to `.cghidEventTap`: an F3
  keydown and F3 keyup, each with `.flags = .maskAlternate` to carry the
  Alt-held modifier state. This is the shape WKWebView expects; separate
  Alt keydown/keyup events are unnecessary and would be re-observed by our
  own NSEvent monitor, causing spurious state transitions.
- **Re-entry guard.** The NSEvent local monitor sees events posted by our
  own `CGEventPost`. Without a guard, the synthesized F3+maskAlternate
  would feed back into the tracker. Use a suppress counter: increment
  before posting, decrement when our monitor sees a matching synthesized
  event, ignore any events while the counter is non-zero.
  `CGEventSetIntegerValueField(event, .eventSourceUserData, MARKER)` before
  posting lets the monitor recognize our own events via
  `CGEventGetIntegerValueField` and skip them entirely — cleaner than a
  counter.
- CGEvents posted to `.cghidEventTap` are OS-level trusted events; WKWebView
  sees them as `isTrusted=true`. Same mechanism Karabiner uses.
- Bare Cmd tap does nothing in macOS today, so repurposing it does not break
  any existing shortcut.

### Shared logic

Tap-tracker state machine is a small state enum and a pure `fn step(state,
event) -> (state, Action)` function. Lives in a shared module
(`src-tauri/src/tap_tracker.rs`) with table-driven unit tests. Both platform
modules own an instance and feed it events.

## Files touched

- `src-tauri/src/lib.rs` — register new Linux module, wire install.
- `src-tauri/src/tap_tracker.rs` — **new**, pure tap state machine + tests.
- `src-tauri/src/passthrough_linux.rs` — **new**, GTK signal hookup +
  GdkEventKey synthesis.
- `src-tauri/src/passthrough_macos.rs` — extend with tap-tracker usage and
  CGEventPost injection.
- `src-tauri/src/shortcuts_inhibit.rs` — add `is_inhibiting() -> bool` and
  bind `zwp_virtual_keyboard_manager_v1` alongside the existing inhibit
  manager, exposing an `inject_alt_f3()` helper.
- `src-tauri/Cargo.toml` — macOS target gains `core-graphics` dep for
  CGEventPost if not already transitively available.
- `docs/keyboard-matrix.md` — Super row → ⚠️, new section listing the four
  Microsoft-accepted alternates and the rdpls Super→Alt+F3 remap.
- `CLAUDE.md` — add a "Web client key limitation" bullet under Key
  Constraints, cross-referencing the matrix.

## Testing

- **Unit:** `tap_tracker` has table-driven tests covering every transition in
  the state-machine table above, plus autorepeat handling and
  passthrough-toggled-mid-hold.
- **Integration:** manual, two environments:
  - niri (Fedora 43): launch rdpls, reach an RDP session, verify bare Super
    opens the remote Start menu; verify Super+L does nothing unexpected (no
    spurious Alt+F3); verify toggling passthrough OFF restores niri overview
    on Super.
  - macOS 14+: same matrix with Cmd.
- If Linux integration test fails (synthesized events not reaching the web
  client's handlers), pivot to the documented fallback and re-test that bare
  Super at least doesn't leak to the WebView or the compositor.

## Known regression (accepted)

On Linux, while rdpls is showing the Microsoft auth or launcher page (i.e.
before a session is active), Super is still swallowed by the tap tracker.
That means niri's overview won't open on Super while rdpls is focused even
outside a session. Alt+F3 is injected but has no useful effect outside a
session (it's a no-op in the launcher UI).

This is an accepted regression because:

- Detecting "in a session" reliably would require heuristics over URL or
  DOM state that would break the moment Microsoft restructures the launcher.
- The existing Ctrl+Alt+Shift+. toggle gives users a deliberate way to
  hand Super back to niri when they want it.
- Users who spend meaningful time on the launcher page are unusual; the
  session is the whole point.

Document the tradeoff in `docs/keyboard-matrix.md` alongside the matrix
update.

## Out-of-scope / deferred

- A `docs/linux-keyboard-remap.md` analogue to the Karabiner doc, pointing at
  niri keybindings and in-guest AutoHotkey. Useful but not blocking; open a
  follow-up once this lands.
- Session-aware gating of the remap (see regression above).
