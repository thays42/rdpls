# Karabiner-Elements rules for rdpls

The Microsoft web RDP client doesn't translate Mac's `Cmd` to the Windows key,
and macOS eats several useful combos at the OS / AppKit-menu layer before the
WebView sees them. Karabiner-Elements sits below both, so we can rewrite keys
at the HID level and have them reach the WKWebView as if the user typed the
Windows equivalent directly. Rules below are scoped to rdpls' bundle
identifier (`com.rdpls.client`), so they're inert in every other app.

## Install

Karabiner accepts two shapes depending on where you're pasting. Pick one:

**Option A — drop-in file for the UI importer.** Uses the full
`{"title": …, "rules": […]}` wrapper (the first JSON block below).

1. Save the wrapped JSON as
   `~/.config/karabiner/assets/complex_modifications/rdpls.json`.
2. Karabiner-Elements → Settings → Complex Modifications → Add predefined rule.
3. Enable every group you want under the "rdpls" heading.

**Option B — direct edit of `~/.config/karabiner/karabiner.json`.** Paste
only the individual rule objects (no wrapper) inside
`profiles[i].complex_modifications.rules`, merging with anything already
there. Use the second JSON block below. Pasting the wrapped version here
triggers `manipulators is missing or empty in {"rules":[…]}` because
Karabiner treats the wrapper as a single rule.

## What's mapped

All rules fire only when rdpls is the frontmost app.

| Mac input              | Sent to remote        | Why                              |
| ---------------------- | --------------------- | -------------------------------- |
| `Cmd + A`              | `Ctrl + A`            | select-all (C / V / X left alone; see note below) |
| `Cmd + Z`              | `Ctrl + Z`            | undo (`Cmd+Shift+Z` → `Ctrl+Shift+Z` passes through for redo) |
| `Cmd + Y`              | `Ctrl + Y`            | redo (alternate)                 |
| `Cmd + S`              | `Ctrl + S`            | save                             |
| `Cmd + F`              | `Ctrl + F`            | find                             |
| `Cmd + P`              | `Ctrl + P`            | print                            |
| `Cmd + N / T / W`      | `Ctrl + N / T / W`    | new / new-tab / close-tab        |
| `Cmd + B / I / U`      | `Ctrl + B / I / U`    | bold / italic / underline        |
| `Cmd + R`              | `Ctrl + R`            | reload (also unblocks the key; the NSEvent monitor in rdpls swallows `Cmd+R` by default to prevent WebView reload — this rule fires first and the monitor sees `Ctrl+R` instead, which it doesn't swallow) |
| `Cmd + L`              | `Ctrl + L`            | focus address bar in remote browsers |
| `Cmd + Left / Right`   | `Home / End`          | Mac line-home/end → Windows      |
| `Cmd + Up / Down`      | `Ctrl + Home / End`   | Mac doc-home/end → Windows       |
| `Option + Left / Right`| `Ctrl + Left / Right` | word-motion                      |
| `Option + Delete`      | `Ctrl + Backspace`    | delete word backward             |

Deliberately *not* remapped:

- `Cmd + C / V / X` — kept as macOS system cut / copy / paste. Microsoft's web client syncs the remote clipboard through the browser clipboard API, so `Cmd+C` on selected remote content still ends up on your Mac clipboard via the sync — no Karabiner rule needed, and this preserves normal Mac muscle-memory on local UI elements (login page inputs, etc.).
- `Cmd + Q`, `Cmd + H`, `Cmd + M` — leave macOS quit / hide / minimize alone so you can always get out of rdpls.
- `Cmd + Space` — Spotlight is more useful than any in-session substitute.
- `Cmd + ArrowLeft / Right` is also a macOS 26 window-snap binding. If you want the Windows `Home`/`End` mapping above to win, disable "Tile window to left/right of screen" under System Settings → Desktop & Dock → Windows, or use a separate macOS shortcut there. Karabiner sits below the window manager so the rule *will* win, but the macOS setting may flash the snap intent.
- `Alt+F4` — MS's web client treats Alt+F4 as disconnect, so we don't route `Cmd + W` there.
- `Option + Tab` — leave unmapped. Blacklist rdpls in AltTab.app (if installed) and plain `Opt+Tab` flows through as `Alt+Tab`, which the web RDP client already treats as the in-session app cycler. An earlier version of this doc remapped to `Alt+Insert`, but Mac keyboards don't have a physical Insert key and WKWebView doesn't translate Karabiner's synthetic Insert into a JS `keydown` the MS client recognizes — the remap broke the cycler instead of fixing it.

For anything missing, Microsoft documents the web-client-specific combos at
`learn.microsoft.com/en-us/windows-365/enterprise/shortcut-keys` and
`learn.microsoft.com/en-us/azure/virtual-desktop/client-features-web`. The
Start-menu substitute is `Alt + Home`.

## The JSON

### Wrapped form (Option A — drop-in file for the UI importer)

```json
{
  "title": "rdpls — Mac keys for Windows RDP sessions",
  "rules": [
    {
      "description": "rdpls: Cmd+letter → Ctrl+letter (edit + browser shortcuts). Cmd+C/V/X intentionally omitted so macOS system cut/copy/paste keeps working; MS web RDP syncs the remote clipboard through the browser clipboard API regardless.",
      "manipulators": [
        { "type": "basic",
          "from": { "key_code": "a", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
          "to":   [{ "key_code": "a", "modifiers": ["left_control"] }],
          "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
        { "type": "basic",
          "from": { "key_code": "z", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
          "to":   [{ "key_code": "z", "modifiers": ["left_control"] }],
          "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
        { "type": "basic",
          "from": { "key_code": "y", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
          "to":   [{ "key_code": "y", "modifiers": ["left_control"] }],
          "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
        { "type": "basic",
          "from": { "key_code": "s", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
          "to":   [{ "key_code": "s", "modifiers": ["left_control"] }],
          "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
        { "type": "basic",
          "from": { "key_code": "f", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
          "to":   [{ "key_code": "f", "modifiers": ["left_control"] }],
          "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
        { "type": "basic",
          "from": { "key_code": "p", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
          "to":   [{ "key_code": "p", "modifiers": ["left_control"] }],
          "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
        { "type": "basic",
          "from": { "key_code": "n", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
          "to":   [{ "key_code": "n", "modifiers": ["left_control"] }],
          "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
        { "type": "basic",
          "from": { "key_code": "t", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
          "to":   [{ "key_code": "t", "modifiers": ["left_control"] }],
          "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
        { "type": "basic",
          "from": { "key_code": "w", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
          "to":   [{ "key_code": "w", "modifiers": ["left_control"] }],
          "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
        { "type": "basic",
          "from": { "key_code": "b", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
          "to":   [{ "key_code": "b", "modifiers": ["left_control"] }],
          "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
        { "type": "basic",
          "from": { "key_code": "i", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
          "to":   [{ "key_code": "i", "modifiers": ["left_control"] }],
          "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
        { "type": "basic",
          "from": { "key_code": "u", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
          "to":   [{ "key_code": "u", "modifiers": ["left_control"] }],
          "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
        { "type": "basic",
          "from": { "key_code": "r", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
          "to":   [{ "key_code": "r", "modifiers": ["left_control"] }],
          "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
        { "type": "basic",
          "from": { "key_code": "l", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
          "to":   [{ "key_code": "l", "modifiers": ["left_control"] }],
          "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] }
      ]
    },
    {
      "description": "rdpls: Cmd/Option text navigation → Windows equivalents",
      "manipulators": [
        { "type": "basic",
          "from": { "key_code": "left_arrow", "modifiers": { "mandatory": ["command"], "optional": ["shift", "caps_lock"] } },
          "to":   [{ "key_code": "home" }],
          "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
        { "type": "basic",
          "from": { "key_code": "right_arrow", "modifiers": { "mandatory": ["command"], "optional": ["shift", "caps_lock"] } },
          "to":   [{ "key_code": "end" }],
          "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
        { "type": "basic",
          "from": { "key_code": "up_arrow", "modifiers": { "mandatory": ["command"], "optional": ["shift", "caps_lock"] } },
          "to":   [{ "key_code": "home", "modifiers": ["left_control"] }],
          "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
        { "type": "basic",
          "from": { "key_code": "down_arrow", "modifiers": { "mandatory": ["command"], "optional": ["shift", "caps_lock"] } },
          "to":   [{ "key_code": "end", "modifiers": ["left_control"] }],
          "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
        { "type": "basic",
          "from": { "key_code": "left_arrow", "modifiers": { "mandatory": ["option"], "optional": ["shift", "caps_lock"] } },
          "to":   [{ "key_code": "left_arrow", "modifiers": ["left_control"] }],
          "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
        { "type": "basic",
          "from": { "key_code": "right_arrow", "modifiers": { "mandatory": ["option"], "optional": ["shift", "caps_lock"] } },
          "to":   [{ "key_code": "right_arrow", "modifiers": ["left_control"] }],
          "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
        { "type": "basic",
          "from": { "key_code": "delete_or_backspace", "modifiers": { "mandatory": ["option"], "optional": ["caps_lock"] } },
          "to":   [{ "key_code": "delete_or_backspace", "modifiers": ["left_control"] }],
          "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] }
      ]
    }
  ]
}
```

### Unwrapped form (Option B — paste into `karabiner.json` `rules` array)

Merge the array elements below with whatever already lives in
`profiles[i].complex_modifications.rules`. The array here is two rule
objects — paste them as siblings of your existing rules, not inside one.

```json
[
  {
    "description": "rdpls: Cmd+letter → Ctrl+letter (edit + browser shortcuts). Cmd+C/V/X intentionally omitted so macOS system cut/copy/paste keeps working; MS web RDP syncs the remote clipboard through the browser clipboard API regardless.",
    "manipulators": [
      { "type": "basic",
        "from": { "key_code": "a", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
        "to":   [{ "key_code": "a", "modifiers": ["left_control"] }],
        "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
      { "type": "basic",
        "from": { "key_code": "z", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
        "to":   [{ "key_code": "z", "modifiers": ["left_control"] }],
        "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
      { "type": "basic",
        "from": { "key_code": "y", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
        "to":   [{ "key_code": "y", "modifiers": ["left_control"] }],
        "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
      { "type": "basic",
        "from": { "key_code": "s", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
        "to":   [{ "key_code": "s", "modifiers": ["left_control"] }],
        "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
      { "type": "basic",
        "from": { "key_code": "f", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
        "to":   [{ "key_code": "f", "modifiers": ["left_control"] }],
        "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
      { "type": "basic",
        "from": { "key_code": "p", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
        "to":   [{ "key_code": "p", "modifiers": ["left_control"] }],
        "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
      { "type": "basic",
        "from": { "key_code": "n", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
        "to":   [{ "key_code": "n", "modifiers": ["left_control"] }],
        "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
      { "type": "basic",
        "from": { "key_code": "t", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
        "to":   [{ "key_code": "t", "modifiers": ["left_control"] }],
        "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
      { "type": "basic",
        "from": { "key_code": "w", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
        "to":   [{ "key_code": "w", "modifiers": ["left_control"] }],
        "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
      { "type": "basic",
        "from": { "key_code": "b", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
        "to":   [{ "key_code": "b", "modifiers": ["left_control"] }],
        "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
      { "type": "basic",
        "from": { "key_code": "i", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
        "to":   [{ "key_code": "i", "modifiers": ["left_control"] }],
        "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
      { "type": "basic",
        "from": { "key_code": "u", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
        "to":   [{ "key_code": "u", "modifiers": ["left_control"] }],
        "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
      { "type": "basic",
        "from": { "key_code": "r", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
        "to":   [{ "key_code": "r", "modifiers": ["left_control"] }],
        "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
      { "type": "basic",
        "from": { "key_code": "l", "modifiers": { "mandatory": ["command"], "optional": ["any"] } },
        "to":   [{ "key_code": "l", "modifiers": ["left_control"] }],
        "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] }
    ]
  },
  {
    "description": "rdpls: Cmd/Option text navigation → Windows equivalents",
    "manipulators": [
      { "type": "basic",
        "from": { "key_code": "left_arrow", "modifiers": { "mandatory": ["command"], "optional": ["shift", "caps_lock"] } },
        "to":   [{ "key_code": "home" }],
        "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
      { "type": "basic",
        "from": { "key_code": "right_arrow", "modifiers": { "mandatory": ["command"], "optional": ["shift", "caps_lock"] } },
        "to":   [{ "key_code": "end" }],
        "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
      { "type": "basic",
        "from": { "key_code": "up_arrow", "modifiers": { "mandatory": ["command"], "optional": ["shift", "caps_lock"] } },
        "to":   [{ "key_code": "home", "modifiers": ["left_control"] }],
        "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
      { "type": "basic",
        "from": { "key_code": "down_arrow", "modifiers": { "mandatory": ["command"], "optional": ["shift", "caps_lock"] } },
        "to":   [{ "key_code": "end", "modifiers": ["left_control"] }],
        "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
      { "type": "basic",
        "from": { "key_code": "left_arrow", "modifiers": { "mandatory": ["option"], "optional": ["shift", "caps_lock"] } },
        "to":   [{ "key_code": "left_arrow", "modifiers": ["left_control"] }],
        "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
      { "type": "basic",
        "from": { "key_code": "right_arrow", "modifiers": { "mandatory": ["option"], "optional": ["shift", "caps_lock"] } },
        "to":   [{ "key_code": "right_arrow", "modifiers": ["left_control"] }],
        "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] },
      { "type": "basic",
        "from": { "key_code": "delete_or_backspace", "modifiers": { "mandatory": ["option"], "optional": ["caps_lock"] } },
        "to":   [{ "key_code": "delete_or_backspace", "modifiers": ["left_control"] }],
        "conditions": [{ "type": "frontmost_application_if", "bundle_identifiers": ["^com\\.rdpls\\.client$"] }] }
    ]
  }
]
```

## A note on the rdpls passthrough toggle

The `Ctrl+Alt+Shift+.` toggle (see `docs/macos-keyboard-passthrough.md`)
is independent of Karabiner. When inhibit is **off**, the `NSEvent` monitor
is not installed — all keys reach WKWebView unchanged, including the Ctrl-ified
output of these Karabiner rules. When inhibit is **on**, the monitor swallows
`Cmd+R` / `Cmd+[` / `Cmd+]` to keep the WebView from reloading or navigating.
Karabiner's rule fires before the monitor sees the event, so `Cmd+R` is
already `Ctrl+R` by the time the monitor runs; the monitor's swallow list
only checks for Cmd-modified keys, so it passes the remapped `Ctrl+R` through.
