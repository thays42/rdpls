# Keyboard Test Matrix

**Tested on:** Fedora 43, niri (Wayland), webkit2gtk-4.1 2.52.1, rdpls debug build
**Date:** 2026-04-20

All keys listed below were tested during a live RDP session reached via the
myapps.microsoft.com portal.

| Key             | Reaches remote? | Notes |
|-----------------|-----------------|-------|
| F5              | ✅              |       |
| F11             | ✅              |       |
| Ctrl+W          | ✅              |       |
| Ctrl+T          | ✅              |       |
| Ctrl+L          | ✅              |       |
| Ctrl+R          | ✅              |       |
| Alt+Left        | ✅              |       |
| Alt+Right       | ✅              |       |
| Ctrl+Shift+T    | ✅              |       |
| Ctrl+N          | ✅              |       |
| Ctrl+Tab        | ✅              |       |

## Known limitations

- **Alt+Tab** is intercepted by niri (compositor-level window switching). This is
  true of every application on niri; there is no in-app fix. Users who want
  Alt+Tab inside the remote session should remap their niri binding or use a
  different combo for niri's switcher.
