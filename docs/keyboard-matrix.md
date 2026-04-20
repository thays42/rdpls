# Keyboard Test Matrix

**Tested on:** Fedora 43, niri (Wayland), webkit2gtk-4.1 2.52.1, rdpls debug build
**Date:** 2026-04-20

Tested during a live RDP session reached via the myapps.microsoft.com portal.

## Browser-style keys (WebView level)

Nothing on this side is intercepted by Tauri or WebKitGTK.

| Key             | Reaches remote? |
|-----------------|-----------------|
| F5              | ✅              |
| F11             | ✅              |
| Ctrl+W          | ✅              |
| Ctrl+T          | ✅              |
| Ctrl+L          | ✅              |
| Ctrl+R          | ✅              |
| Alt+Left        | ✅              |
| Alt+Right       | ✅              |
| Ctrl+Shift+T    | ✅              |
| Ctrl+N          | ✅              |
| Ctrl+Tab        | ✅              |

## Compositor-level keys (niri)

These are keys that niri normally consumes. They reach the remote only when
rdpls' `zwp_keyboard_shortcuts_inhibit_manager_v1` inhibitor is active (which
it is by default at startup).

| Key             | Reaches remote (inhibit ON)? |
|-----------------|------------------------------|
| Alt+Tab         | ✅                           |
| Super / Win     | ✅                           |

Toggle the inhibitor with **Ctrl+Alt+Shift+Escape**. When OFF, niri reclaims
these keys (useful for intentional window switching).

## rdpls-specific

| Key                      | Behavior |
|--------------------------|----------|
| Ctrl+Alt+Shift+Escape    | Toggle shortcut-inhibit on/off |
