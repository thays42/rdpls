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

A "tap" is: modifier keydown → modifier keyup with no intervening key events
(including other modifiers), and passthrough ON at both press and release.

State machine, per modifier (Super on Linux, Cmd on macOS):

| Current state | Event                                      | Next state | Side effect                         |
|---------------|--------------------------------------------|------------|-------------------------------------|
| idle          | modifier keydown, passthrough ON           | tracking   | swallow event                       |
| idle          | modifier keydown, passthrough OFF          | idle       | pass through                        |
| tracking      | any other keydown or keyup                 | tainted    | pass through                        |
| tracking      | modifier keyup                             | idle       | inject Alt+F3 press+release         |
| tainted       | modifier keyup                             | idle       | no injection                        |
| tracking \| tainted | passthrough toggles OFF              | idle       | no injection                        |

Notes:
- Bare modifier keydown is **swallowed** even before we know whether it's a
  tap. The WebView has no use for a bare Super/Cmd event and firing Alt+F3
  on keydown would fire before the user could abort.
- "Any other key" includes additional modifier keys. Super+Shift (no further
  key) is not a tap.
- Autorepeat keydowns on the modifier itself are treated as a no-op in
  `tracking` (not tainting). Platform APIs differentiate; we honor that.

## Architecture

### Linux (`src-tauri/src/passthrough_linux.rs`, new)

- Installed from `configure_webview` in `lib.rs` by connecting to the WebView
  widget's `key-press-event` and `key-release-event` signals (webkit2gtk
  `WebView` inherits from `gtk::Widget`, which emits these).
- Reads passthrough state via a new `shortcuts_inhibit::is_inhibiting() -> bool`
  helper that inspects `STATE`.
- On qualifying tap, synthesizes an Alt+F3 sequence (Alt press → F3 press → F3
  release → Alt release) as `GdkEventKey` values and dispatches each via
  `gtk_widget_event` on the WebView widget.
- Tap-tracker state is per-process (only one main window) behind a `Mutex`.

**Open risk:** WebKit may mark synthesized `GdkEventKey` dispatches as
`isTrusted=false` in the resulting JS `KeyboardEvent`, and Microsoft's client
may filter non-trusted events. If empirical testing shows injection doesn't
reach the remote, fall back to *just swallowing bare Super* (no injection) —
users get silence instead of niri overview popping open, plus the existing
Ctrl+Alt+Shift+. escape hatch. The decision is made after implementing and
testing; the design up to the injection call is identical.

### macOS (extend `src-tauri/src/passthrough_macos.rs`)

- The existing NSEvent local monitor already sees every key event, gates on
  the passthrough state, and returns `nil` to swallow or passes the event
  through. Extend it with the tap-tracker state.
- On qualifying Cmd tap, post Alt+F3 via `CGEventPost(.cghidEventTap, …)` as
  two key events (Alt down with F3 down flags? or Alt down, F3 down, F3 up,
  Alt up — pick the shape that WKWebView sees as "Alt held during F3").
  Implementation will use the four-event form for safety.
- CGEventPost emits OS-level events; WKWebView sees them as trusted. This is
  the same mechanism Karabiner uses, so the `isTrusted` concern present on
  Linux does not apply on macOS.
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
- `src-tauri/src/shortcuts_inhibit.rs` — add `is_inhibiting() -> bool`.
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

## Out-of-scope / deferred

- A `docs/linux-keyboard-remap.md` analogue to the Karabiner doc, pointing at
  niri keybindings and in-guest AutoHotkey. Useful but not blocking; open a
  follow-up once this lands.
- Detecting the web client's "no longer in a session" state and disabling the
  remap outside sessions. Alt+F3 does nothing outside a session so the
  spurious firing is harmless; not worth the detection complexity today.
