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
- No independently-toggled fullscreen. Fullscreen API state is
  coupled to lock state (entered on lock-on, exited on lock-off) —
  not exposed as its own chord. The `--kiosk` window itself is always
  fullscreen regardless.

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
  `browser.link.open_newwindow.restriction = 0` — redirect `window.open`
  calls without a features string into the current tab. These prefs
  are a **backstop**, not primary: Gecko treats a `window.open` call
  with a features string as an explicit request for a new window and
  bypasses the pref. The extension's `window.open` shim (below) is the
  primary mechanism.
- `xpinstall.signatures.required = false` — load the unsigned
  extension.
- `app.update.auto = false`, `app.update.service.enabled = false` —
  don't surprise the kiosk with an update prompt. The exact canonical
  set of update-suppression prefs for the targeted Firefox Dev Edition
  build is a pre-plan verification item (see Risks).
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
(`navigator.keyboard.lock()`). When lock is active it is **expected**
to do all of the following; which of these actually hold in the
targeted Firefox Dev Edition build is a pre-plan verification item
(see Risks §1 and §2):

1. Request `zwp_keyboard_shortcuts_inhibit_manager_v1` on Firefox's
   Wayland surface, so the compositor stops grabbing Super, Alt+Tab,
   and other global chords.
2. Suppress Firefox's own chrome shortcuts (`Ctrl+W`, `Ctrl+L`, `F11`,
   etc.), routing those keys to content instead. The W3C spec allows
   the UA to define the intercepted set; Firefox's actual set is not
   exhaustively documented and must be measured.
3. Route every key to the focused page, where the RDP client's JS
   forwards them to the remote session.

Keyboard Lock is only callable when the document is in the Fullscreen
API state (`document.fullscreenElement !== null`), which is distinct
from Firefox's `--kiosk` window-level fullscreen. The chord handler
therefore enters Fullscreen API before calling `.lock()`.

**User activation is the critical constraint.** Both
`requestFullscreen()` and `lock()` require a live user-activation
token. That token is consumed synchronously in the keydown handler
that saw the chord; a round-trip through `browser.runtime.sendMessage`
to the background and back to a content script loses it. The
fullscreen-and-lock sequence therefore runs **inside the top-frame
content script's keydown handler**, not in the background. The
background's role is limited to (a) tracking lock state, (b) handling
quit (no activation required), and (c) routing "show toast" messages
to the top frame along with the new state.

Chord set (both chords are `Ctrl+Alt+Shift+<key>` to stay clear of
common compositor and Firefox bindings):

| Chord | Action |
| --- | --- |
| `Ctrl+Alt+Shift+.` | Toggle keyboard lock |
| `Ctrl+Alt+Shift+Q` | Quit the window |

Chord capture strategy:

- Two content scripts:
  - `content.js` runs in **all frames** (`all_frames: true`). It owns
    chord capture inside iframes and the toast DOM. When it sees a
    chord in a non-top frame it forwards to the top frame via
    `browser.runtime.sendMessage` (for quit, which needs no
    activation) or — for lock toggling — relays the chord to the top
    frame's `content-fullscreen.js` via a `window.postMessage` into
    the top frame. This path loses activation, so the top-frame
    handler only uses the relayed message to update UI state after a
    later top-frame chord press; it does not attempt `lock()` from a
    non-top-frame origin. In practice the RDP iframe is
    where focus lives, so a chord pressed in the iframe shows a
    toast explaining "press Ctrl+Alt+Shift+. again with this window
    in focus" — acceptable because once the user is in lock-off mode
    they can click the chrome area to shift focus.
  - `content-fullscreen.js` runs **top frame only**
    (`all_frames: false`) and is the single place where
    `requestFullscreen()` and `navigator.keyboard.lock()` /
    `unlock()` are called. Its keydown handler runs synchronously
    when the top frame has focus and preserves user activation.
- Listeners use the **capture phase** (`addEventListener('keydown',
  handler, true)`) to fire before any page-level handler that might
  call `stopImmediatePropagation`.
- **Safety net chord** `Ctrl+Alt+Shift+/` bound in the top-frame
  `content-fullscreen.js`. It calls `navigator.keyboard.unlock()` and
  `document.exitFullscreen()` **unconditionally** — no toggle logic,
  no state read. This is what it buys over the main chord: if the
  extension's state tracking or the main-chord handler is broken, the
  safety net is a dumb, single-purpose escape. (It does not add
  coverage to sandboxed iframes the main chord can't reach; a
  sandboxed iframe that blocks our content script is out of reach for
  any JS-based chord. The mitigation for that case is focus: user
  moves focus to the top frame before pressing the safety net.) Bound
  through the same content-script mechanism rather than
  `browser.commands`, because `browser.commands` bindings are
  suppressed while Keyboard Lock is active.

Lock toggle — ON (top frame has focus):

1. `content-fullscreen.js` sees `Ctrl+Alt+Shift+.` in its keydown
   capture handler.
2. Synchronously in the same handler: `await
   document.documentElement.requestFullscreen({ navigationUI: 'hide' })`
   then `await navigator.keyboard.lock()`. User activation token from
   the keypress covers both calls.
3. On resolve: post `{ type: 'lock-state', on: true }` to the
   background (for state tracking) and render the "keys → remote"
   toast via the shared DOM pill (same visual and timing as
   `docs/superpowers/specs/2026-04-20-toggle-toast-design.md` —
   referenced spec lives on `main` and was inherited when `firefox`
   branched from it).

Lock toggle — OFF (top frame has focus; lock is active so top frame
receives the key):

1. `content-fullscreen.js` sees `Ctrl+Alt+Shift+.`.
2. `await navigator.keyboard.unlock()` then
   `await document.exitFullscreen()`.
3. Post `{ type: 'lock-state', on: false }` to background, render
   "keys → compositor" toast.

Quit:

1. Either content script sees `Ctrl+Alt+Shift+Q`, messages background.
2. Background script calls `browser.windows.remove(windowId)` on the
   current window. With Firefox in `--kiosk` + `--no-remote`, removing
   the only window terminates the Firefox process, which quits the
   app cleanly.

### Popup redirect

Microsoft launches the RDP session via `window.open`, typically with
a features string (dimensions, toolbar flags). Gecko treats a features
string as an explicit new-window request and bypasses the
`browser.link.open_newwindow` pref, so the pref cannot be relied on as
primary. The **primary** mechanism is a content-script `window.open`
shim injected into all frames that rewrites the call to same-tab
navigation (`window.location.assign(url)`). The prefs remain set as a
backstop for any code path the shim misses (e.g., an
`<a target="_blank">` anchor).

### Toggle toast

Reuses the DOM pill from the existing design (injected element,
transient, text-only). Always rendered in the **top frame's DOM** by
`content.js` (which runs in all frames but only renders the pill when
`window.top === window`). State source is explicit: the toast
message's text is determined by the `on: boolean` field included in
the `lock-state` post from `content-fullscreen.js`. `content.js`
subscribes to that message and renders "keys → remote" when `on` is
true, "keys → compositor" when false. The background script is not in
the toast critical path.

### Project layout

```
src-firefox/
  extension/
    manifest.json                # MV3; browser_specific_settings.gecko.id
                                 # = "rdpls@local" (matches profile
                                 # extensions/ subdir name)
    background.js                # lock-state tracking + quit handler
    content.js                   # all frames: chord listener for Q,
                                 # window.open shim, toast DOM (top only)
    content-fullscreen.js        # top frame only: fullscreen-API +
                                 # Keyboard Lock calls, posts lock-state
                                 # messages to content.js for the toast
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

The extension's sideload path is
`~/.local/share/rdpls/profile/extensions/rdpls@local/` (unpacked
directory — not `.xpi`), created by `make install` as a symlink to
`src-firefox/extension/`. The directory name must exactly match
`browser_specific_settings.gecko.id` for Firefox to load it.

CLAUDE.md is updated as part of this work: the Linux section describes
the Firefox-kiosk stack, not the Tauri webkit2gtk path. The gtk4
webkit6 attempt is referenced historically only through the archived
commit on `gtk4-webkit6-linux`.

### Pre-plan verifications

The design rests on several assumptions about Firefox behavior that
cannot be confirmed from the spec alone. The implementation plan must
begin with a **verification phase** that answers each item below
before code is written against it. A wrong assumption here does not
just cause a bug — it invalidates the architecture.

1. **Fullscreen API requirement for Keyboard Lock.** Does Firefox Dev
   Edition require `document.fullscreenElement !== null` before
   `navigator.keyboard.lock()` resolves? Test: call `lock()` from the
   JS console in a `--kiosk` window without calling
   `requestFullscreen()` first. If it resolves, the design simplifies
   (drop `requestFullscreen`/`exitFullscreen` from the flow). If it
   rejects, keep the current design.
2. **User activation across the fullscreen-then-lock sequence.** Does
   a single keydown's activation token cover both `await
   requestFullscreen()` and the subsequent `await lock()`? Test in the
   console with a keypress-triggered handler; if the second call
   rejects for lack of activation, we need a different sequencing
   (e.g., call `lock()` first where possible, or move to non-awaited
   chained `.then()` to stay inside the activation frame).
3. **Set of chrome shortcuts suppressed by Keyboard Lock.** Enumerate
   which Firefox chrome shortcuts are actually routed to content when
   lock is active: `Ctrl+W`, `Ctrl+L`, `Ctrl+R`, `Ctrl+T`, `Ctrl+N`,
   `Ctrl+Q`, `F11`, `Alt+F4`, etc. Any key Firefox still intercepts
   needs a per-key plan (either accept that it won't pass to the
   remote, or add a content-script `preventDefault` in the capture
   phase). Record results in `docs/firefox-keyboard-flow.md`.
4. **Iframe chord delivery under Microsoft's RDP embed.** Confirm
   whether Microsoft's RDP client loads in a same-origin, cross-
   origin, or sandboxed cross-origin iframe, and whether
   `all_frames: true` content scripts are injected into it. If the
   iframe is sandboxed with `allow-scripts` only (no `allow-same-
   origin`), WebExtension content scripts should still run, but this
   needs confirmation in the running app. Record results in
   `docs/firefox-keyboard-flow.md`. If the iframe does block content
   scripts, the "click chrome area to escape focus" recovery in the
   lock-on iframe fallback is also invalidated — update that flow
   accordingly.
5. **`browser.windows.remove` behavior on `--kiosk --no-remote`.**
   Confirm it cleanly terminates the Firefox process with no prompt.
   If a confirmation dialog appears, add
   `browser.tabs.warnOnClose=false` and
   `browser.sessionstore.resume_from_crash=false` to `user.js`.
6. **Keyboard permission prompt.** Does `navigator.keyboard.lock()`
   show a permission prompt on first call in Firefox? If so, the
   profile seed needs a pre-granted permission entry or the first
   launch needs a documented "click Allow once" step.
7. **Canonical update-suppression prefs.** The exact set of prefs
   that fully disable Firefox's update machinery in the targeted Dev
   Edition build. Some legacy prefs (`app.update.enabled`) no longer
   exist; some require policy JSON rather than prefs. Determine the
   right set and record it in `user.js` with comments.
8. **`window.open` features-string behavior.** Confirm that Microsoft
   passes a features string (so the prefs really are insufficient as
   primary) and that the content-script shim catches the call before
   Gecko opens the new window. If the shim is too late, fall back to
   a `browser.webRequest.onBeforeRequest` or the
   `browser.tabs.onCreated` + redirect approach.

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
