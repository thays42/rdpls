# rdpls — Dedicated RDP Browser Wrapper (Tauri)

## Context

Work systems are accessible only via `apps.microsoft.com`, which redirects through Microsoft auth and eventually lands in a web-based RDP client (Windows 365 / AVD). No `.rdp` file, no native client permitted — browser only.

The primary friction with this arrangement is **keyboard shortcut hijacking**. Browsers consume keys that belong to the remote desktop: Ctrl+W closes a tab, Ctrl+T opens one, F5 reloads the page, F11 fullscreens the wrong layer, Alt+arrows navigate browser history. Firefox works end-to-end for the auth + session flow, but the keyboard problem is inherent to running inside a browser.

The goal is a minimal native desktop wrapper that hosts the same web session in a WebView but registers almost no accelerators itself, letting keystrokes pass through to the remote client's JS handlers.

Primary target is a Fedora Framework 13 laptop running niri (Wayland). Mac Studio (M4 Max) is a future secondary target — same codebase, separate build.

Urgency: a conference is happening soon where this would be used in anger. For that trip, `chromium --app=<url>` is the fallback. This project is the durable solution afterward.

## Key Decisions

- **Stack: Tauri (Rust + system WebView).** Chosen over Electron for small binary, native feel, and alignment with Tyler's Rust direction. WebView is webkit2gtk on Linux, WKWebView on macOS.
- **Single WebView handles the entire flow.** Auth (Microsoft login + MFA), tenant picker, app launch, RDP session — all one browser session. Cookies persist via a stable WebKit data directory.
- **Entry point is `https://myapps.microsoft.com`.** No clever URL persistence to jump directly to the last session — Microsoft rotates session URLs and it's not worth the fragility.
- **UA spoofing required.** WebKitGTK's default UA trips Microsoft's "unsupported browser" warnings on W365/AVD. Spoof current Firefox or Edge per-window.
- **Wayland-native on Linux.** No `GDK_BACKEND=x11` workaround. niri implements the standard protocols (xdg-shell, xdg-desktop-portal for clipboard/IME) that GTK4 needs.
- **Keyboard strategy is subtractive, not additive.** Tauri registers almost no accelerators by default — the design is to keep it that way. No application menu (menus bind accelerators). No `tauri-plugin-global-shortcut` registrations except one deliberate escape hotkey. Disable WebKitGTK's own bindings (Ctrl+scroll zoom, context menu).
- **Escape hotkey: `Ctrl+Alt+Shift+Escape`.** The one intentional local accelerator. Purpose is a safety valve — see Unresolved for exact behavior.
- **Conditional Access is not a blocker.** Firefox on Fedora already works end-to-end, so Cornerstone's CA policy does not require a managed browser. Tauri should pass the same gate.
- **Name: `rdpls`.** Lowercase everywhere — binary, docs, window title, desktop file. Read aloud as "R-D-please." Fits the Unix-utility aesthetic (`ls`, `rdpls`).
- **Cross-platform is the same code.** macOS target adds a `cargo tauri build` on the Mac and some keyboard-remapping polish (Cmd vs Ctrl is a web-client concern, not a Tauri one). No structural changes to the project.

## Proposed Actions

Phase 1 — Scaffold
- `cargo create-tauri-app` with Rust backend, minimal HTML frontend (frontend is just a loader; the real content is the external WebView navigation).
- Single window loads `https://myapps.microsoft.com`.
- Persistent WebKit data directory for cookie survival across launches.
- Per-window UA override (current Firefox stable is a safe default).
- No application menu. No global shortcut registrations.
- Disable WebKitGTK context menu and Ctrl+scroll zoom via WebView settings.
- Acceptance: launch app, complete MS auth including MFA, reach a working RDP session, close and relaunch without re-authenticating.

Phase 2 — Keyboard validation and escape hotkey
- Systematically verify the following reach the remote client and are not swallowed locally: F5, F11, Ctrl+W, Ctrl+T, Ctrl+L, Ctrl+R, Alt+Left, Alt+Right, Ctrl+Shift+T.
- Document any keys that still leak, with the fix or a known-limitation note.
- Implement the `Ctrl+Alt+Shift+Escape` hotkey (see Unresolved for behavior).
- Acceptance: a documented keyboard test matrix with pass/fail per key, and the escape hotkey working from inside an active RDP session.

Phase 3 — Quality-of-life
- Session drop detection with a prompt or auto-reload to `myapps.microsoft.com`.
- Thin status strip (connection state, maybe latency) — toggleable, off by default.
- Clipboard permission wired through xdg-desktop-portal.
- Evaluate whether multi-window / multi-session is worth supporting.

## Unresolved Decisions

- **Escape hotkey behavior.** Agreed on the keybinding. The behavior needs to be decided: show a small local controls overlay, force-quit the app, toggle a "local shortcut mode" that temporarily re-enables browser-style keys, or something else.
- **Repo hosting and layout.** Where the repo lives (GitLab CE on the home stack, GitHub, elsewhere) and whether this is a standalone repo or slotted into an existing monorepo.
- **Icon and branding.** Cosmetic, but required at build time.
- **Phase 3 scope.** Multi-session support is a "maybe" — decide after Phases 1–2 surface whether single-window is actually limiting.
