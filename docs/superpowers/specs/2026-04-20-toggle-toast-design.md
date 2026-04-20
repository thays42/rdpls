# Toggle toast for Ctrl+Alt+Shift+Escape

## Problem

`Ctrl+Alt+Shift+Escape` toggles the Wayland keyboard-shortcuts-inhibit
state. There is currently no visible feedback when the user presses the
hotkey, so the only way to confirm the toggle landed is to try a
compositor shortcut and see what happens. The user wants a brief,
on-screen acknowledgement.

## Goals

- Show a transient on-screen indicator each time the hotkey fires.
- Communicate the resulting state in terms the user thinks in:
  keys going to the remote vs. keys staying with the compositor.
- Never interfere with keyboard input or with the RDP canvas.
- Stay well under 5 seconds total (both states combined) so it cannot
  linger or obscure anything meaningful.

## Non-goals

- No permanent status strip. That is a separate Phase 3 item.
- No native OS notification / D-Bus / libnotify dependency.
- No separate Tauri window or overlay surface.
- No visual distinction beyond the text itself (no color swap, no icon).

## Design

### Surface

The toast is a DOM element injected into the same WebView that hosts
Microsoft's RDP client. The existing `ESCAPE_HOTKEY_SCRIPT` user script
in `src-tauri/src/keyboard.rs` already runs in every frame via
WebKitGTK's `UserContentManager`, so the DOM is the natural place for
the UI.

No Rust changes are required. `shortcuts_inhibit::toggle()` already
returns `bool` (`true` when the inhibitor is newly installed, `false`
when it was just destroyed), and that value flows back through Tauri's
`invoke` as the resolved promise value.

### Behavior

On keydown of `Ctrl+Alt+Shift+Escape`:

1. `invoke('rdpls_toggle_inhibit')` is called (existing behavior).
2. The returned promise resolves with a bool: `true` ⇒ inhibit ON
   (keys reach the remote), `false` ⇒ inhibit OFF (compositor keeps
   keys).
3. The injected script shows a pill that reads:
   - `true`  → `Keys → Remote`
   - `false` → `Keys → Local`
4. The pill is visible for 1.5 seconds, then fades out over 200 ms.
5. If the user re-toggles while the pill is still showing, the same
   DOM node is reused: text updates, pending timers reset, fade
   restarts. No stacking.
6. If the `invoke` promise rejects, no toast is shown.

### Appearance

- Position: `position: fixed; top: 16px; left: 50%;
  transform: translateX(-50%)`.
- Shape: rounded pill, ~20 px horizontal padding, ~10 px vertical.
- Colors: `background: rgba(0,0,0,0.75)`, `color: #fff`.
- Font: `system-ui, sans-serif`, 14 px, medium weight.
- Layering: `z-index: 2147483647` and `pointer-events: none` so the
  pill sits above the RDP canvas and never intercepts input.
- Transition: `opacity` only, 200 ms ease-out on fade-out, instant on
  fade-in.
- All styles are applied inline on the element. No stylesheet
  injection, no classes, no CSS custom properties.

### Safety and isolation

- The toast only paints from the top frame (`window.top === window`).
  The existing hotkey handler runs in every frame via
  `UserContentInjectedFrames::AllFrames`, which is correct for
  capturing the key wherever focus lives, but `position: fixed`
  inside a nested (possibly cross-origin) iframe would anchor to the
  iframe's viewport, not the window. Iframe handlers still invoke
  the toggle; they just don't render the pill.
- A single guarded singleton is cached at `window.__rdpls_toast_el`.
  Repeated injections of the user script (e.g. on navigation) detect
  the existing node and reuse it.
- The element is appended to `document.body` lazily on first toggle.
  If `document.body` is not yet available, the script bails out
  silently for that invocation — the next toggle will succeed.
- A single module-scoped timer handle (`window.__rdpls_toast_timer`)
  is used so rapid re-toggles cleanly cancel any pending fade.
- Nothing on the pill uses CSS classes, global selectors, or anything
  else that could collide with Microsoft's page styles.

## Testing

Manual only — there is no frontend test harness in the project and
this change is behavioral in a cross-origin WebView.

Verify:
1. Fresh app launch, press `Ctrl+Alt+Shift+Escape` once: `Keys →
   Local` appears centered at the top, fades out inside ~2 s.
2. Press again: `Keys → Remote` appears, fades out.
3. Mash the hotkey several times within 2 s: text updates each time,
   fade resets each time, exactly one pill on screen throughout.
4. During an active RDP session, the pill is visible on top of the
   remote canvas and does not capture mouse clicks (clicks pass
   through to the RDP canvas).
5. During the Microsoft auth flow (no RDP canvas yet), the pill
   still shows.

## Rollout

Single commit on `main`. No migration, no flag, no user-facing
documentation update beyond this spec.
