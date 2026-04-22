# Firefox-kiosk pivot for the Linux target

## Problem

Hosting Microsoft's HTML5 RDP client in WebKitGTK (either the Tauri
webkit2gtk-4.1 path or the direct gtk4 + webkit6 rewrite on
`gtk4-webkit6-linux`) yields materially worse session rendering than
running the same URL in Firefox. The limiting factor is the GStreamer
media pipeline WebKitGTK relies on for H.264 decode: on Fedora 43 with
RPM Fusion's full ffmpeg (`avdec_h264` at primary rank) there is still
noticeable ghosting on scroll and reduced motion fidelity compared to
Firefox's in-tree decoder stack. That is the feature that matters most
for daily use, so we pivot the host.

The new challenge: Firefox is a general-purpose browser with its own
chrome shortcuts (`Ctrl+W`, `Ctrl+L`, `Ctrl+R`, `F11`, `Alt+F4`, etc.)
and the compositor continues to grab global keys (`Alt+Tab`, `Super`,
etc.). Both classes must reach the remote RDP session while the user is
working, with a chord to toggle back out.

## Goals

- Replace the Linux host stack with Firefox running in `--kiosk`, so
  RDP video renders at parity with Firefox used as a normal browser.
- All keys — compositor global shortcuts and Firefox's own chrome
  shortcuts — reach the remote session while "lock" is active.
- A single chord toggles lock on/off; a second chord quits the app.
- Single-user scope (the maintainer), so no packaging, signing, or
  cross-distro distribution work.
- macOS target is untouched: `src-tauri/` keeps working exactly as it
  does today.

## Non-goals

- No distribution to other users. No `.deb`/`.rpm`, no AMO extension
  listing, no Firefox ESR repo sourcing.
- No fullscreen toggle chord. Kiosk mode is always fullscreen and
  `Ctrl+Alt+Shift+F` no longer has a use case.
- No Rust launcher, no Tauri, no Wayland protocol code of our own.
  Firefox owns the compositor-level inhibit via the Keyboard Lock API.
- No persistent status strip. Existing toggle toast behavior (the
  transient pill) carries over into the extension; longer-lived
  status is still a future item.
- No bundling of Firefox itself. The user installs Firefox Developer
  Edition once from Mozilla's tarball.

## Design

### Stack

- **Firefox Developer Edition**, installed by the user at
  `~/.local/opt/firefox-dev/` from Mozilla's tarball. Dev Edition is
  the channel because it honors `xpinstall.signatures.required=false`
  in the profile, which means our extension can ship unsigned. Release
  Firefox refuses unsigned extensions unconditionally; ESR allows them
  but Fedora 43 has no first-class ESR package. Dev Edition is a clean
  fit for single-user.
- **Dedicated profile** at `~/.local/share/rdpls/profile/`, seeded from
  a template under `src-firefox/profile/` in the repo. The template
  carries `user.js` and the extension. The live profile retains cookies
  and cache across launches; the Makefile's seed step is idempotent
  and never overwrites the data it should not.
- **WebExtension (MV3)** at `src-firefox/extension/`. Installed into
  the profile by symlinking `src-firefox/extension/` into
  `<profile>/extensions/<addon-id>/`. The extension owns chord capture,
  keyboard-lock toggling, quit, popup redirect (as a backstop), and
  the toggle toast.
- **No launcher binary.** The desktop entry's `Exec=` line is the
  Firefox command directly. A shell script would add no value.
- **Makefile** provides `install`, `uninstall`, and `dev` targets.
  `install` seeds the profile if absent, symlinks the extension, and
  writes the `.desktop` file into `~/.local/share/applications/`.
  `uninstall` reverses it (leaving the profile data in place unless
  `make uninstall-hard` is invoked — the latter also deletes the
  profile directory). `dev` runs `web-ext run --keep-profile-changes`
  pointed at a copy of the profile, so extension edits hot-reload.

### Profile prefs (`user.js`)

- `dom.keyboard-lock.enabled = true` — required for the Keyboard Lock
  API to be callable from page context.
- `full-screen-api.warning.timeout = 0` — suppress the "document is now
  fullscreen" overlay; kiosk is always fullscreen so the banner is
  noise.
- `browser.link.open_newwindow = 1` and
  `browser.link.open_newwindow.restriction = 0` — force `window.open`
  into the current tab. Handles Microsoft's RDP launch popup without
  any JS on our side.
- `xpinstall.signatures.required = false` — load the unsigned
  extension.
- `app.update.auto = false`, `app.update.enabled = false`,
  `app.update.service.enabled = false` — don't surprise the kiosk with
  an update prompt.
- `browser.startup.homepage_override.mstone = "ignore"`,
  `browser.aboutwelcome.enabled = false`,
  `browser.shell.checkDefaultBrowser = false`,
  `datareporting.healthreport.uploadEnabled = false`,
  `toolkit.telemetry.enabled = false` — suppress first-run, default-
  browser prompt, and telemetry paths.
- No User-Agent override. Firefox's default UA already passes Microsoft
  M365's browser checks per the existing Linux notes.

### Keyboard handling flow

The flow hinges on the browser Keyboard Lock API
(`navigator.keyboard.lock()`), which does three things at once when
active:

1. Requests `zwp_keyboard_shortcuts_inhibit_manager_v1` on Firefox's
   Wayland surface, so the compositor stops grabbing Super, Alt+Tab,
   and other global chords.
2. Suppresses Firefox's own chrome shortcuts (`Ctrl+W`, `Ctrl+L`,
   `F11`, etc.), routing those keys to content instead.
3. Routes every key to the focused page, where the RDP client's JS
   forwards them to the remote session.

Keyboard Lock is only callable when the document is in the Fullscreen
API state (`document.fullscreenElement !== null`), which is distinct
from Firefox's `--kiosk` window-level fullscreen. The chord handler
therefore enters Fullscreen API before calling `.lock()`, using the
chord keypress as the user-activation gesture the spec requires.

Chord set (both chords are `Ctrl+Alt+Shift+<key>` to stay clear of
common compositor and Firefox bindings):

| Chord | Action |
| --- | --- |
| `Ctrl+Alt+Shift+.` | Toggle keyboard lock |
| `Ctrl+Alt+Shift+Q` | Quit the window |

Chord capture strategy:

- Content script injected into **all frames** (`all_frames: true`).
  Microsoft's RDP client runs inside an iframe, and key events dispatch
  to the frame that has focus, so a top-only listener misses them.
- Listeners use the **capture phase** (`addEventListener('keydown',
  handler, true)`) to fire before any page-level handler that might
  call `stopImmediatePropagation`.
- Each listener posts to the background script via
  `browser.runtime.sendMessage`; the background script is the single
  owner of lock/fullscreen state and decides what to do.
- **Safety net chord**: if the main lock-toggle chord ever stops
  working (page takes the key in a way we can't reach, Firefox changes
  keydown dispatch semantics), a secondary chord — `Ctrl+Alt+Shift+/`
  — registered via the same content-script path forces lock off
  unconditionally and shows an explanatory toast. Registered in the
  same mechanism as the main chord, not `browser.commands`, because
  `browser.commands` bindings are suppressed when Keyboard Lock is
  active (the whole point of lock).

Lock toggle — ON:

1. Content script sees the chord, messages background.
2. Background script posts back to the top frame to
   `document.documentElement.requestFullscreen({ navigationUI: 'hide' })`
   then `await navigator.keyboard.lock()`.
3. On resolve, content script shows the "keys → remote" toast (reusing
   the existing toast design from the gtk4-webkit6-linux branch — see
   `docs/superpowers/specs/2026-04-20-toggle-toast-design.md` for the
   visual and timing; the behavior is unchanged, only the host).

Lock toggle — OFF:

1. Content script sees the chord (still fires while lock is active —
   that's exactly why Keyboard Lock exists, to hand keys to content).
2. Background script asks the top frame to `navigator.keyboard.unlock()`
   then `document.exitFullscreen()`.
3. Content script shows the "keys → compositor" toast.

Quit:

1. Content script sees `Ctrl+Alt+Shift+Q`, messages background.
2. Background script calls `browser.windows.remove(windowId)` on the
   current window. With Firefox in `--kiosk` + `--no-remote`, removing
   the only window terminates the Firefox process, which quits the
   app cleanly.

### Popup redirect

Microsoft launches the RDP session via `window.open`. The
`browser.link.open_newwindow` prefs above redirect such calls into the
current tab at the Gecko level — no extension code required. The
extension keeps a content-script `window.open` shim as a **backstop
only**, in case Microsoft ever switches to a mechanism the prefs do
not cover (e.g., an `<a target="_blank">` route that slips through).
The shim rewrites to same-tab navigation.

### Toggle toast

Reuses the DOM pill from the existing design (injected element,
transient, text-only). The content script owns it. One change: the
toast is the extension's responsibility in every frame where the chord
fires, but only the top frame's toast is visible to the user — the
background script routes the "show toast" message to the top frame
only, so an iframe chord press still produces one toast in the expected
place.

### Project layout

```
src-firefox/
  extension/
    manifest.json                # MV3
    background.js                # state owner: lock/fullscreen/quit
    content.js                   # chord listener + toast DOM
    content-fullscreen.js        # top-frame-only: fullscreen + lock API
  profile/
    user.js                      # prefs above
    .gitignore                   # ignore runtime-only subdirs if we ever
                                 # need to commit parts of the profile
Makefile                          # install, uninstall, dev
docs/
  firefox-keyboard-flow.md       # how Keyboard Lock + Fullscreen API
                                 # interact in kiosk; successor to
                                 # keyboard-matrix.md on the Linux side
src-tauri/                        # unchanged (macOS)
src/                              # unchanged (macOS loader)
```

CLAUDE.md is updated as part of this work: the Linux section describes
the Firefox-kiosk stack, not the Tauri webkit2gtk path. The gtk4
webkit6 attempt is referenced historically only through the archived
commit on `gtk4-webkit6-linux`.

### Risks and open verifications

These are testable only by running Firefox Developer Edition with the
extension in place; flagged here so the implementation plan schedules
them before depending on them:

1. **Keyboard Lock's Fullscreen API requirement on kiosk.** Firefox
   may or may not treat `--kiosk` as satisfying the Fullscreen API
   predicate. Expected: no — kiosk is window-level, the API expects an
   explicit `requestFullscreen()` call. Mitigation baked in: the chord
   handler calls `requestFullscreen()` before `lock()`. If this turns
   out to be wrong, the fallback is simpler (just `lock()`).
2. **User-activation on chord.** `requestFullscreen()` and `lock()`
   both require user activation. The chord keypress should satisfy
   it; if Firefox's activation tracking treats the keydown as consumed
   by the time the promise resolves, we'll switch to a two-step
   handler that completes synchronously in the keydown handler.
3. **Iframe focus and chord delivery.** RDP content runs in an iframe
   or nested iframe. `all_frames: true` content scripts should cover
   every same-origin and cross-origin frame, but Microsoft uses a
   sandboxed cross-origin iframe in some flows. If a frame's sandbox
   flags prevent our content script from running, the chord won't be
   seen there. Mitigation: the safety-net chord is bound to the top
   frame too, so the user can always force-unlock.
4. **`browser.windows.remove` behavior with `--kiosk --no-remote`.**
   Expected to cleanly terminate the process. If Firefox shows a
   "really close?" prompt or any lingering tab-close confirmation,
   disable it via `browser.tabs.warnOnClose=false` and
   `browser.tabs.warnOnCloseOtherTabs=false` in `user.js`.

### Non-migration items (intentional)

- `src-tauri/` stays untouched. macOS Tauri + WKWebView already works
  and its WebKit is Apple's, not WebKitGTK, so the H.264 problem does
  not apply.
- The existing Linux passthrough docs (`docs/keyboard-matrix.md`) are
  replaced, not migrated. The Firefox mechanism is sufficiently
  different that a fresh doc (`docs/firefox-keyboard-flow.md`) is
  clearer than a diff.

## Rollout

Single-user, single-branch (`firefox`). Work lands directly on
`firefox`; no staged releases. The old Linux binary is not
uninstalled automatically; the user removes it manually if they had
ever installed it.
