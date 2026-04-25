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
| Super / Win     | ⚠️ See "Web-client limitation" below |

Toggle the inhibitor with **Ctrl+Alt+Shift+.** When OFF, the compositor reclaims
these keys (useful for intentional window switching). Period rather than Escape
because GNOME/Mutter eats modifier+Escape when inhibit is OFF, making it
impossible to toggle back.

## Web-client limitation: the Windows key

The Microsoft HTML5 RDP client (Windows 365 / AVD web client) does not
forward Meta / Super / Cmd keydowns to the remote session. This is a
Microsoft-side limitation — no amount of keyboard passthrough on the rdpls
side can deliver `Win+L`, `Win+D`, `Win+E`, `Win+R`, or `Win+Arrow` to the
guest VM through the web client.

The web client does recognize four alternate keystrokes:

| Combo        | Remote effect                |
|--------------|------------------------------|
| Ctrl+Alt+End | Ctrl+Alt+Del                 |
| **Alt+F3**   | **Windows key (Start menu)** |
| Alt+PageUp   | Alt+Tab                      |
| Alt+PageDown | Alt+Shift+Tab                |

rdpls remaps a **bare Super tap** (Linux) or **bare Cmd tap** (macOS) to
Alt+F3 so the muscle-memory "tap Super → Start menu" behavior works. It
does NOT synthesize Win+X combos — there is no combo the web client
accepts. Users who need in-guest shortcuts like Win+L should map them
inside the VM (e.g. AutoHotkey rebinding `Ctrl+Shift+L` → `LockWorkstation`).

Sources: Microsoft AVD web client docs
(`learn.microsoft.com/azure/virtual-desktop/users/client-features-web`)
and an unanswered Tech Community thread on the same question
(`techcommunity.microsoft.com/t5/azure-virtual-desktop/avd-webclient-key-mapping/td-p/3821461`).

**Known regression:** while rdpls is focused on the Microsoft auth /
launcher page (no active session), Super/Cmd is still swallowed by the
tap tracker, so the host OS's Super/Cmd handling (niri overview, etc.)
won't fire until you switch focus away or toggle passthrough off with
`Ctrl+Alt+Shift+.`. Affects Linux and macOS symmetrically.

If the compositor doesn't advertise `zwp_virtual_keyboard_manager_v1`,
rdpls cannot inject the Alt+F3 substitute and falls back to letting Super
pass through untouched — no swallow, no Start menu.

## rdpls-specific

| Key                      | Behavior |
|--------------------------|----------|
| Ctrl+Alt+Shift+.         | Toggle shortcut-inhibit on/off |
| Ctrl+Alt+Shift+F         | Toggle fullscreen              |
