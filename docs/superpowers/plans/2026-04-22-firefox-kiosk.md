# Firefox-kiosk Linux Pivot Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Linux host stack with Firefox Developer Edition in `--kiosk` + a sideloaded WebExtension that handles chord capture, Keyboard Lock toggling, quit, popup redirect, and the existing toggle toast. Single-user scope (maintainer only); macOS `src-tauri/` untouched.

**Architecture:** Unpacked MV3 WebExtension under `src-firefox/extension/`, symlinked into a dedicated Firefox profile at `~/.local/share/rdpls/profile/`. Fullscreen+lock calls live in a top-frame-only content script to preserve user activation; background script tracks lock state, handles quit, and routes toast messages. No launcher binary — a `.desktop` entry invokes Firefox directly. `Makefile` handles install/uninstall/dev.

**Tech Stack:** Firefox Developer Edition (installed by user at `~/.local/opt/firefox-dev/`), WebExtension API (MV3, `browser.*` namespace), `web-ext` CLI for dev loop, GNU Make, POSIX sh.

**Spec:** `docs/superpowers/specs/2026-04-22-firefox-kiosk-design.md` — read this first.

**Branch:** All work lands on `firefox` (already cut from `main`; webkit6 attempt archived on `gtk4-webkit6-linux`).

---

## Chunk 1: Verification phase

Eight assumptions in the spec must be confirmed before extension code is written. A wrong assumption here invalidates the architecture, not just a bug. All findings are recorded in `docs/firefox-keyboard-flow.md` — that doc grows through this chunk and becomes the living reference for the Firefox path.

**Abort criteria:** if any of the following are observed, stop and re-spec rather than proceeding to Chunk 2:
- §1: `lock()` rejects even with Fullscreen API active.
- §2: No usable sequencing of `requestFullscreen` + `lock` preserves user activation.
- §6: `lock()` shows a per-invocation permission prompt with no pre-grant mechanism.
- §4: The RDP iframe blocks content scripts entirely AND is where focus always lives.

### Task 1.1: Install Firefox Developer Edition and create scratch profile

**Files:**
- Create: `~/.local/opt/firefox-dev/` (user-owned, outside the repo)
- Create: `/tmp/rdpls-verify-profile/` (scratch profile, discarded after chunk)

- [ ] **Step 1: Download and extract Firefox Developer Edition**

```bash
mkdir -p ~/.local/opt
cd ~/.local/opt
curl -L -o firefox-dev.tar.xz 'https://download.mozilla.org/?product=firefox-devedition-latest-ssl&os=linux64&lang=en-US'
tar -xJf firefox-dev.tar.xz
mv firefox firefox-dev
rm firefox-dev.tar.xz
~/.local/opt/firefox-dev/firefox --version
```
Expected: prints a version string like `Mozilla Firefox 137.0b…`

- [ ] **Step 2: Create scratch profile for verification work**

```bash
mkdir -p /tmp/rdpls-verify-profile
~/.local/opt/firefox-dev/firefox -CreateProfile "rdpls-verify /tmp/rdpls-verify-profile" -no-remote
ls /tmp/rdpls-verify-profile/
```
Expected: profile directory contains `prefs.js`, `user.js` (empty), etc.

- [ ] **Step 3: Create the findings doc scaffold**

Create `docs/firefox-keyboard-flow.md`:

```markdown
# Firefox Keyboard Flow — Verification Findings

Findings from the pre-plan verification phase of the Firefox-kiosk
pivot. Each section below corresponds to a verification item in
`docs/superpowers/specs/2026-04-22-firefox-kiosk-design.md`
(Pre-plan verifications).

Date of verification: <YYYY-MM-DD>
Firefox Dev Edition version: <fill in>
Compositor: niri on Fedora 43
```

- [ ] **Step 4: Commit**

```bash
git add docs/firefox-keyboard-flow.md
git commit -m "docs: scaffold firefox-keyboard-flow.md for verification findings"
```

### Task 1.2: Verify Fullscreen API requirement for Keyboard Lock

**Files:**
- Modify: `docs/firefox-keyboard-flow.md`

- [ ] **Step 1: Launch Firefox Dev Edition with scratch profile**

```bash
~/.local/opt/firefox-dev/firefox --profile /tmp/rdpls-verify-profile --kiosk --no-remote 'about:blank' &
```

- [ ] **Step 2: Enable the Keyboard Lock pref in about:config**

Navigate to `about:config` in the URL bar, accept the warning, search `dom.keyboard-lock.enabled`, set to `true`. Close and re-launch Firefox with the same command so the pref takes effect for the session.

- [ ] **Step 3: Test lock() without fullscreen**

In the devtools console on `about:blank`:

```javascript
navigator.keyboard.lock().then(() => console.log('LOCKED')).catch(e => console.error('REJECTED', e.name, e.message))
```
Record the outcome. Expected: rejection (`InvalidStateError` or similar) because the document is not in Fullscreen API state.

- [ ] **Step 4: Test lock() with fullscreen**

In the console:

```javascript
document.documentElement.requestFullscreen().then(() => navigator.keyboard.lock()).then(() => console.log('LOCKED')).catch(e => console.error('REJECTED', e.name, e.message))
```
Note: `requestFullscreen()` requires a user-activation gesture; devtools console calls don't count. Instead, add a button to the page (`document.body.innerHTML = '<button id=b>go</button>'`) and attach the handler; click it.

- [ ] **Step 5: Record findings**

Add a section to `docs/firefox-keyboard-flow.md`:

```markdown
## §1: Fullscreen API requirement for Keyboard Lock

- `navigator.keyboard.lock()` without `document.fullscreenElement`:
  **<RESOLVED | REJECTED with <ErrorName>>**
- `navigator.keyboard.lock()` after `requestFullscreen()`:
  **<RESOLVED | REJECTED with <ErrorName>>**

**Conclusion:** <Fullscreen API is required | Fullscreen API is not required>.
Design implication: <chord handler must call requestFullscreen first | design simplifies, drop fullscreen calls>.
```

- [ ] **Step 6: Commit**

```bash
git add docs/firefox-keyboard-flow.md
git commit -m "docs(verify): §1 Fullscreen API requirement for Keyboard Lock"
```

### Task 1.3: Verify user activation across fullscreen+lock sequence

**Files:**
- Modify: `docs/firefox-keyboard-flow.md`

- [ ] **Step 1: Set up a test page with a keydown handler**

In the Firefox devtools console on `about:blank`:

```javascript
document.body.innerHTML = '<p>Press Ctrl+Alt+Shift+.</p>'
document.addEventListener('keydown', async (e) => {
  if (!(e.ctrlKey && e.altKey && e.shiftKey && e.key === '.')) return
  e.preventDefault()
  try {
    await document.documentElement.requestFullscreen({ navigationUI: 'hide' })
    console.log('FS OK')
    await navigator.keyboard.lock()
    console.log('LOCK OK')
  } catch (err) {
    console.error('FAILED', err.name, err.message)
  }
}, true)
```

- [ ] **Step 2: Focus the page body (click it) then press `Ctrl+Alt+Shift+.`**

Observe console. Record whether `LOCK OK` appears or whether `lock()` rejects for `NotAllowedError` (activation exhausted).

- [ ] **Step 3: Record findings**

Append to `docs/firefox-keyboard-flow.md`:

```markdown
## §2: User activation across fullscreen+lock

- Sequential `await requestFullscreen()` then `await lock()` from a
  single keydown: **<BOTH RESOLVE | lock() REJECTED>**

**Conclusion:** <sequential awaits preserve activation | need to use .then() chain inside the activation frame | need a different sequencing>.
```

- [ ] **Step 4: Commit**

```bash
git add docs/firefox-keyboard-flow.md
git commit -m "docs(verify): §2 user activation across fullscreen+lock"
```

### Task 1.4: Enumerate chrome shortcuts suppressed by Keyboard Lock

**Files:**
- Modify: `docs/firefox-keyboard-flow.md`

- [ ] **Step 1: Install a test page that logs every keydown**

With lock already active from Task 1.3 (or re-establish it), paste into the console:

```javascript
document.addEventListener('keydown', (e) => {
  console.log('KEYDOWN', e.ctrlKey?'C':'-', e.altKey?'A':'-', e.shiftKey?'S':'-', e.key, 'default-prevented?', e.defaultPrevented)
}, true)
```

- [ ] **Step 2: Press each shortcut and record whether the handler sees it**

Shortcuts to try, one at a time:

- `Ctrl+W`, `Ctrl+T`, `Ctrl+N`, `Ctrl+L`, `Ctrl+R`, `Ctrl+Shift+R`
- `Ctrl+Q`, `Ctrl+Shift+W`, `Ctrl+Shift+N`
- `Ctrl+F`, `Ctrl+G`, `Ctrl+H`, `Ctrl+J`, `Ctrl+P`
- `F5`, `F11`, `F12`, `Alt+F4`, `Alt+Home`
- `Alt+Left`, `Alt+Right`

For each: note whether the `KEYDOWN` log fires (= reached content) or whether Firefox acted on it instead (tab closed, URL bar focused, etc.).

- [ ] **Step 3: Record findings**

Append a table to `docs/firefox-keyboard-flow.md`:

```markdown
## §3: Chrome shortcuts under Keyboard Lock

| Shortcut | Reaches content? | Firefox action (if any) |
| --- | --- | --- |
| Ctrl+W | yes/no | — |
| ...   |        |   |

**Conclusion:** <list of shortcuts that still intercept and will need a content-script preventDefault, or a note that remote session handles them>.
```

- [ ] **Step 4: Commit**

```bash
git add docs/firefox-keyboard-flow.md
git commit -m "docs(verify): §3 chrome shortcuts under Keyboard Lock"
```

### Task 1.5: Verify Microsoft RDP iframe delivers chord events

**Files:**
- Modify: `docs/firefox-keyboard-flow.md`

- [ ] **Step 1: Navigate to the live RDP entry point**

```bash
# In the already-open Firefox window, navigate to:
# https://myapps.microsoft.com
# Sign in, launch an RDP app, reach the RDP session.
```

- [ ] **Step 2: Inspect the iframe structure**

In devtools, Inspector tab → select the RDP canvas → note the iframe ancestry:
- same-origin with myapps.microsoft.com?
- cross-origin to a different Microsoft domain?
- sandboxed? What flags?

- [ ] **Step 3: Check whether devtools-injected keydown listener fires in the iframe**

Switch devtools context to the RDP iframe (the context dropdown in the top-left of console), then:

```javascript
document.addEventListener('keydown', (e) => console.log('IFRAME KEYDOWN', e.key), true)
```

Click into the RDP session so focus lands there, press some keys. Note whether the iframe console logs them.

- [ ] **Step 4: Record findings**

```markdown
## §4: Microsoft RDP iframe delivery

- iframe origin: <same-origin | cross-origin to <domain>>
- iframe sandbox flags: <none | allow-scripts allow-same-origin | ...>
- keydown listener in iframe fires for session keys: <yes | no>

**Conclusion:** <all_frames: true content scripts will cover it | iframe blocks content scripts — need workaround>.
```

- [ ] **Step 5: Commit**

```bash
git add docs/firefox-keyboard-flow.md
git commit -m "docs(verify): §4 Microsoft RDP iframe delivery"
```

### Task 1.6: Verify windows.remove, permission prompt, update prefs, window.open

**Files:**
- Modify: `docs/firefox-keyboard-flow.md`

These four items are short; bundled for throughput.

- [ ] **Step 1: §5 — browser.windows.remove termination behavior**

`browser.windows.remove` is only available to extension code, not page scripts, so this check is deferred to Task 3.4 after the extension is installed. Append to findings:

```markdown
## §5: browser.windows.remove termination

DEFERRED — requires extension context. Verified in Chunk 3 Task 3.4.
```

- [ ] **Step 2: §6 — Keyboard Lock permission prompt**

Review whether the prior tests (1.2–1.3) produced any permission prompt UI (notification bar, doorhanger). Record.

```markdown
## §6: Keyboard Lock permission prompt

- Prompt observed during §1/§2 tests: <yes with UI "<text>" | no>

**Conclusion:** <none needed | one-time accept on first launch documented in install notes>.
```

- [ ] **Step 3: §7 — canonical update-suppression prefs**

Query Firefox for which of the candidate update prefs actually exist in this build. In `about:config`, search:

- `app.update.auto` — expected: exists, user-set
- `app.update.enabled` — expected: does not exist in modern Firefox
- `app.update.service.enabled` — expected: exists
- `app.update.background.scheduling.enabled` — check if exists

Record exactly which prefs exist and are writable via `user.js`:

```markdown
## §7: Update-suppression prefs

- app.update.auto: <exists | missing>
- app.update.service.enabled: <exists | missing>
- app.update.background.scheduling.enabled: <exists | missing>
- <other>: <...>

**Conclusion:** `user.js` should set: <final list>.
```

- [ ] **Step 4: §8 — window.open features-string behavior**

With the RDP session open (from Task 1.5), attempt to trigger the popup-to-session path once. Observe whether:
- A new window opens (prefs insufficient)
- Or the same tab navigates (prefs sufficient)

Also, in the devtools console, test:

```javascript
window.open('https://example.com', '_blank', 'width=800,height=600')
```

Note whether a new window appears (prefs bypassed by features string) or the same tab navigates.

```markdown
## §8: window.open features-string

- `window.open(url, '_blank', 'width=...,height=...')` with the
  open_newwindow prefs set: **<new window | same tab>**

**Conclusion:** <prefs insufficient as primary; shim required | prefs are sufficient>.
```

- [ ] **Step 5: Commit**

```bash
git add docs/firefox-keyboard-flow.md
git commit -m "docs(verify): §§5-8 remaining verifications (§5 deferred)"
```

### Task 1.7: Update spec if any finding invalidates an assumption

**Files:**
- Modify (maybe): `docs/superpowers/specs/2026-04-22-firefox-kiosk-design.md`

- [ ] **Step 1: Cross-check findings against spec assumptions**

For each finding §1–§8, compare to what the spec assumed. If a finding contradicts the spec (e.g., lock works without fullscreen, or a chrome shortcut is not suppressed), update the spec to match reality. If all findings confirm the spec, no-op.

- [ ] **Step 2: Commit if spec was modified**

```bash
git add docs/superpowers/specs/2026-04-22-firefox-kiosk-design.md
git commit -m "docs(spec): reconcile with verification findings"
```

If nothing changed, skip the commit.

---

## Chunk 2: Scaffolding, Makefile, and root-file updates

Set up the `src-firefox/` tree and rewrite root-level files (`CLAUDE.md`, `README.md`, `Makefile`, `.gitignore`) to describe the new architecture. No extension code yet.

### Task 2.1: Clean up stale directories and create src-firefox skeleton

**Files:**
- Delete: `src-linux/` (stale empty directory from prior branch checkout)
- Create: `src-firefox/extension/`
- Create: `src-firefox/profile/`

- [ ] **Step 1: Remove stale src-linux directory**

```bash
rm -rf src-linux
```

- [ ] **Step 2: Create src-firefox structure with placeholder files so git tracks the dirs**

```bash
mkdir -p src-firefox/extension
mkdir -p src-firefox/profile
```

- [ ] **Step 3: Verify no stale files remain**

```bash
git status --short
```
Expected: clean. `src-linux` and `src-firefox` should not appear (empty directories aren't tracked; commits happen as files land in 2.2).

### Task 2.2: Write profile/user.js with verified prefs

**Files:**
- Create: `src-firefox/profile/user.js`

- [ ] **Step 1: Write user.js based on verification findings**

Use findings from Tasks 1.2 (keyboard-lock enabled), 1.6 §7 (update prefs), 1.6 §6 (permission), and the spec. Exact pref set below; if §7 findings diverged, substitute.

```javascript
// src-firefox/profile/user.js
// Seeded by `make install` into ~/.local/share/rdpls/profile/.
// Once seeded, the live profile's user.js is not re-overwritten on
// subsequent `make install` runs (see Makefile target).

// --- Keyboard Lock API ---
user_pref("dom.keyboard-lock.enabled", true);

// --- Fullscreen UX ---
user_pref("full-screen-api.warning.timeout", 0);

// --- window.open backstop (shim in content.js is primary) ---
user_pref("browser.link.open_newwindow", 1);
user_pref("browser.link.open_newwindow.restriction", 0);

// --- Sideloaded unsigned extension (Dev Edition only) ---
user_pref("xpinstall.signatures.required", false);

// --- Update suppression (per §7 findings) ---
user_pref("app.update.auto", false);
user_pref("app.update.service.enabled", false);
user_pref("app.update.background.scheduling.enabled", false);

// --- First-run / welcome / default-browser ---
user_pref("browser.aboutwelcome.enabled", false);
user_pref("browser.shell.checkDefaultBrowser", false);
user_pref("browser.startup.homepage_override.mstone", "ignore");

// --- Close-confirmation dialogs that could block quit ---
user_pref("browser.tabs.warnOnClose", false);
user_pref("browser.sessionstore.resume_from_crash", false);

// --- Telemetry ---
user_pref("datareporting.healthreport.uploadEnabled", false);
user_pref("toolkit.telemetry.enabled", false);
user_pref("toolkit.telemetry.archive.enabled", false);
```

- [ ] **Step 2: Commit**

```bash
git add src-firefox/profile/user.js
git commit -m "feat(firefox): profile user.js with verified prefs"
```

### Task 2.3: Update .gitignore

**Files:**
- Modify: `.gitignore`

- [ ] **Step 1: Add dev-loop artifacts**

Append to `.gitignore`:

```
# Firefox dev loop
/.web-ext-storage
```

- [ ] **Step 2: Commit**

```bash
git add .gitignore
git commit -m "chore: ignore web-ext storage"
```

### Task 2.4: Rewrite Makefile

**Files:**
- Modify: `Makefile`

- [ ] **Step 1: Read the current Makefile to understand what to replace**

```bash
cat Makefile
```

The current Makefile describes `cargo tauri build` + cargo-deb/cargo-generate-rpm. That entire Linux path is replaced. macOS-related targets (if present) stay.

- [ ] **Step 2: Write the new Makefile**

Replace the Linux portion of `Makefile` with:

```makefile
# rdpls — Firefox-kiosk Linux install / uninstall / dev targets.
# macOS still uses `cargo tauri build` directly from src-tauri/.

PREFIX       ?= $(HOME)/.local
FIREFOX      ?= $(HOME)/.local/opt/firefox-dev/firefox
PROFILE_DIR  ?= $(HOME)/.local/share/rdpls/profile
DESKTOP_DIR  ?= $(PREFIX)/share/applications
ADDON_ID     ?= rdpls@local
REPO_ROOT    := $(shell pwd)
EXT_SRC      := $(REPO_ROOT)/src-firefox/extension
PROFILE_SRC  := $(REPO_ROOT)/src-firefox/profile
ENTRY_URL    ?= https://myapps.microsoft.com

.PHONY: install uninstall uninstall-hard dev check-firefox

check-firefox:
	@test -x "$(FIREFOX)" || { \
	  echo "error: Firefox not found at $(FIREFOX)"; \
	  echo "       install Dev Edition and/or set FIREFOX=..."; \
	  exit 1; }

install: check-firefox
	@mkdir -p "$(PROFILE_DIR)/extensions"
	@mkdir -p "$(DESKTOP_DIR)"
	# Seed user.js only if the profile doesn't already have one — don't
	# clobber runtime state. For re-seed, `make uninstall-hard && make install`.
	@if [ ! -f "$(PROFILE_DIR)/user.js" ]; then \
	  cp "$(PROFILE_SRC)/user.js" "$(PROFILE_DIR)/user.js"; \
	  echo "seeded $(PROFILE_DIR)/user.js"; \
	else \
	  echo "preserved existing $(PROFILE_DIR)/user.js"; \
	fi
	# Symlink extension (overwrite an existing symlink; refuse otherwise)
	@if [ -L "$(PROFILE_DIR)/extensions/$(ADDON_ID)" ] || [ ! -e "$(PROFILE_DIR)/extensions/$(ADDON_ID)" ]; then \
	  ln -sfn "$(EXT_SRC)" "$(PROFILE_DIR)/extensions/$(ADDON_ID)"; \
	  echo "linked extension → $(PROFILE_DIR)/extensions/$(ADDON_ID)"; \
	else \
	  echo "error: $(PROFILE_DIR)/extensions/$(ADDON_ID) exists and is not a symlink; aborting"; \
	  exit 1; \
	fi
	# Desktop entry
	@printf '%s\n' \
	  '[Desktop Entry]' \
	  'Type=Application' \
	  'Name=rdpls' \
	  'Comment=Windows 365 / AVD session' \
	  'Exec=$(FIREFOX) --profile $(PROFILE_DIR) --kiosk --no-remote --new-instance $(ENTRY_URL)' \
	  'Icon=rdpls' \
	  'Terminal=false' \
	  'Categories=Network;RemoteAccess;' \
	  'StartupWMClass=firefox-developer-edition' \
	  > "$(DESKTOP_DIR)/rdpls.desktop"
	@echo "wrote $(DESKTOP_DIR)/rdpls.desktop"
	@echo "install complete."

uninstall:
	rm -f "$(DESKTOP_DIR)/rdpls.desktop"
	rm -f "$(PROFILE_DIR)/extensions/$(ADDON_ID)"
	@echo "uninstalled extension link and desktop entry."
	@echo "profile data preserved at $(PROFILE_DIR). Use 'make uninstall-hard' to delete it."

uninstall-hard: uninstall
	rm -rf "$(PROFILE_DIR)"
	@echo "profile $(PROFILE_DIR) removed."

dev: check-firefox
	# Live-reload extension via web-ext against a dev copy of the profile.
	# Requires: npm install -g web-ext
	cd src-firefox/extension && \
	  web-ext run \
	    --firefox="$(FIREFOX)" \
	    --firefox-profile="$(PROFILE_DIR)" \
	    --keep-profile-changes \
	    --start-url="$(ENTRY_URL)"
```

- [ ] **Step 3: Smoke-test the Makefile (without committing install output)**

```bash
make check-firefox
```
Expected: either confirms firefox-dev exists or prints the error.

```bash
make -n install
```
Expected: prints commands without executing.

- [ ] **Step 4: Commit**

```bash
git add Makefile
git commit -m "feat(firefox): Makefile install/uninstall/dev targets"
```

### Task 2.5: Rewrite CLAUDE.md Linux sections

**Files:**
- Modify: `CLAUDE.md`

The current CLAUDE.md (inherited from `main`) describes the Tauri webkit2gtk-4.1 Linux path. Precise edit list:

**Edits (each a standalone change):**

1. **Top paragraph** (line 3): replace "built with Tauri" with "with platform-split hosts: Firefox Developer Edition on Linux, Tauri + WKWebView on macOS".

2. **## Status section**: replace current contents with:
   > Linux pivoted from WebKitGTK to Firefox Developer Edition in `--kiosk` + sideloaded WebExtension (single-user, no packaging). macOS port (Tauri v2 + WKWebView) is unchanged from Phase 1/2. Phase 3 items (session-drop detection, clipboard portal, status strip) are still outstanding on both platforms.

3. **## Architecture section**: replace bullet list with:
   - **Linux:** Firefox Developer Edition in `--kiosk` + sideloaded MV3 WebExtension. No Rust/Tauri on Linux. Extension owns chord capture and `navigator.keyboard.lock()` toggling. Installed via `make install`.
   - **macOS:** Tauri v2 + WKWebView. Unchanged.
   - **Single WebView per platform** handles the entire flow: auth, MFA, tenant picker, app launch, RDP session.
   - **Entry point:** `https://myapps.microsoft.com` — no session URL persistence.
   - **WebView engine:** Gecko (Firefox Dev Edition) on Linux, WKWebView on macOS.

4. **## Key Constraints section**: replace every bullet with the following set:
   - **Keyboard passthrough via Keyboard Lock API on Linux.** Extension calls `navigator.keyboard.lock()` from a top-frame content script; Firefox requests `zwp_keyboard_shortcuts_inhibit_manager_v1` and suppresses its own chrome shortcuts. Toggle with `Ctrl+Alt+Shift+.`. Quit with `Ctrl+Alt+Shift+Q`. Safety-net unconditional-unlock with `Ctrl+Alt+Shift+/`.
   - **Keyboard passthrough on macOS** uses the existing `NSEvent` local monitor — unchanged.
   - **No UA spoofing needed** on Linux (Firefox's default UA passes M365).
   - **Persistent cookies** via the dedicated profile at `~/.local/share/rdpls/profile/`.
   - **Popup redirect**: page-world `window.open` shim injected by the extension (primary), plus `browser.link.open_newwindow` prefs as backstop. macOS still uses its injected script.
   - **Always fullscreen**: Firefox `--kiosk` on Linux; Tauri `.fullscreen(true)` on macOS.

5. **Delete entirely**: the "Fedora H.264 decoder" subsection under "Build Prerequisites" (Firefox handles its own media pipeline).

6. **Build Prerequisites / Linux subsection**: replace the webkit2gtk dnf/apt blocks with:
   ```bash
   # Linux: install Firefox Developer Edition once from Mozilla
   mkdir -p ~/.local/opt && cd ~/.local/opt
   curl -L -o firefox-dev.tar.xz 'https://download.mozilla.org/?product=firefox-devedition-latest-ssl&os=linux64&lang=en-US'
   tar -xJf firefox-dev.tar.xz && mv firefox firefox-dev && rm firefox-dev.tar.xz
   # Optional: for live extension reload during development
   npm install -g web-ext
   ```

7. **Development Commands section**: replace with:
   ```bash
   # Linux
   make install          # seeds profile, symlinks extension, writes desktop entry
   make dev              # web-ext run with live reload
   make uninstall        # remove install (preserves profile data)
   make uninstall-hard   # remove install + profile data

   # macOS (unchanged)
   cargo tauri dev
   cargo tauri build
   ```

8. **Known Follow-ups**: replace the Tauri-specific bundle-identifier note with:
   > Extension addon-id is `rdpls@local`; profile sideload path is `~/.local/share/rdpls/profile/extensions/rdpls@local/` (symlink to `src-firefox/extension/`). macOS bundle identifier remains `com.rdpls.client`.
   >
   > Session-drop detection, clipboard portal wiring, and status strip are Phase 3.
   >
   > macOS passthrough notes (NSEvent monitor, swallow list) unchanged — see `docs/macos-keyboard-passthrough.md`.

9. **Project Layout section**: replace the tree with:
   ```
   src-firefox/
     extension/
       manifest.json               # MV3
       background.js               # quit + lock-state tracking
       content.js                  # all frames: quit chord, window.open shim, toast DOM
       content-fullscreen.js       # top frame only: Fullscreen API + Keyboard Lock toggle
     profile/
       user.js                     # seeded prefs
   Makefile                         # Linux install/uninstall/dev (macOS still uses cargo tauri)
   src-tauri/                       # macOS — unchanged
     src/...
     tauri.conf.json
     capabilities/default.json
   src/
     index.html                    # macOS loader
   docs/
     firefox-keyboard-flow.md      # Linux Keyboard Lock findings + ongoing reference
     macos-keyboard-passthrough.md
     macos-karabiner.md
   ```

- [ ] **Step 1: Apply the nine edits above in order**

Make the edits described in this task. Each edit corresponds to one numbered item above.

- [ ] **Step 2: Verify macOS sections are unchanged**

```bash
git diff CLAUDE.md
```
Read the diff. Any macOS-specific line (NSEvent, WKWebView, passthrough_macos.rs, Karabiner, etc.) should NOT appear in the diff except where it's part of a block that was legitimately restructured (architecture, layout). If the macOS-specific substance changed, back out those changes.

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: CLAUDE.md describes Firefox-kiosk Linux stack"
```

### Task 2.6: Rewrite README.md Linux sections

**Files:**
- Modify: `README.md`

Precise edit list (mirrors 2.5 for the public-facing doc):

1. **Delete** any "Fedora H.264 / RPM Fusion" section.
2. **Delete** the webkit2gtk / libgtk-4-dev / libsoup `dnf`/`apt` blocks for Linux.
3. **Delete** the "Tauri CLI (`cargo install tauri-cli`)" line under Linux prereqs; keep it under macOS.
4. **Insert** the Firefox Dev Edition install block (below) in place of the deleted Linux prereqs.
5. **Replace** any `cargo tauri dev` / `cargo tauri build` examples under a "Linux" header with `make dev` / `make install`. Keep them under macOS headers.
6. **Keep unchanged**: every macOS-specific block (Karabiner, `.app`/`.dmg` bundling, WKWebView notes).

- [ ] **Step 1: Apply the edits**

Perform edits 1–6 above. The Firefox Dev Edition install block:

```bash
# One-time: install Firefox Developer Edition
mkdir -p ~/.local/opt && cd ~/.local/opt
curl -L -o firefox-dev.tar.xz 'https://download.mozilla.org/?product=firefox-devedition-latest-ssl&os=linux64&lang=en-US'
tar -xJf firefox-dev.tar.xz && mv firefox firefox-dev && rm firefox-dev.tar.xz

# One-time: install web-ext for the dev loop (optional)
npm install -g web-ext

# Install rdpls
cd path/to/rdpls
make install
```

Keep macOS sections unchanged.

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: README Linux section for Firefox-kiosk"
```

---

## Chunk 3: Extension — minimal load, manifest, quit

Build the extension from the outside in: first get it loading in Firefox and handling quit. Lock and toast come later.

### Task 3.1: Write the manifest

**Files:**
- Create: `src-firefox/extension/manifest.json`

- [ ] **Step 1: Write manifest.json (MV3)**

```json
{
  "manifest_version": 3,
  "name": "rdpls",
  "version": "0.1.0",
  "description": "Chord capture and keyboard-lock toggling for the rdpls Firefox-kiosk host.",
  "browser_specific_settings": {
    "gecko": {
      "id": "rdpls@local",
      "strict_min_version": "128.0"
    }
  },
  "background": {
    "scripts": ["background.js"]
  },
  "content_scripts": [
    {
      "matches": ["<all_urls>"],
      "all_frames": true,
      "match_about_blank": true,
      "run_at": "document_start",
      "js": ["content.js"]
    },
    {
      "matches": ["<all_urls>"],
      "all_frames": false,
      "match_about_blank": true,
      "run_at": "document_start",
      "js": ["content-fullscreen.js"]
    }
  ],
  "permissions": [
    "tabs"
  ]
}
```

- [ ] **Step 2: Commit**

```bash
git add src-firefox/extension/manifest.json
git commit -m "feat(ext): manifest.json"
```

### Task 3.2: Write empty content.js and content-fullscreen.js stubs

**Files:**
- Create: `src-firefox/extension/content.js`
- Create: `src-firefox/extension/content-fullscreen.js`

- [ ] **Step 1: Create content.js stub**

```javascript
// content.js — runs in all frames. Owns chord capture (non-lock
// actions), window.open shim, and the toast DOM (rendered only in
// the top frame).
//
// Responsibilities:
//   - Quit chord (Ctrl+Alt+Shift+Q) → sendMessage to background
//   - window.open shim → rewrite to same-tab navigation
//   - Listen for lock-state messages from content-fullscreen.js and
//     render the toast in the top frame
//
// Lock/unlock themselves run in content-fullscreen.js (top-frame
// only) so user activation is preserved.

(() => {
  'use strict';
  const isTop = window.top === window;
  // TODO: chord capture, shim, toast — Tasks 3.3, 4.x, 5.x
  console.log('[rdpls] content.js loaded', { isTop, url: location.href });
})();
```

- [ ] **Step 2: Create content-fullscreen.js stub**

```javascript
// content-fullscreen.js — runs in the TOP frame only.
// Owns: document.fullscreenElement + navigator.keyboard.lock()/unlock().
// These calls require user activation, which is only preserved when
// they run synchronously in the keydown handler that observed the
// chord — i.e., here, not routed through the background script.

(() => {
  'use strict';
  // TODO: lock toggle + safety net — Tasks 4.x
  console.log('[rdpls] content-fullscreen.js loaded', { url: location.href });
})();
```

- [ ] **Step 3: Commit**

```bash
git add src-firefox/extension/content.js src-firefox/extension/content-fullscreen.js
git commit -m "feat(ext): content.js and content-fullscreen.js stubs"
```

### Task 3.3: Write background.js with quit handler

**Files:**
- Create: `src-firefox/extension/background.js`

- [ ] **Step 1: Write background.js**

```javascript
// background.js — state owner.
// Responsibilities:
//   - Track lock-state (on/off) so other components can query it
//   - Handle quit messages by closing the Firefox window
//   - Route toast messages to the top frame (later task)

let lockState = false; // informational only; lock/unlock happens in content-fullscreen.js

browser.runtime.onMessage.addListener((msg, sender) => {
  switch (msg?.type) {
    case 'quit': {
      // In --kiosk --no-remote with a single window, removing the
      // window terminates the Firefox process.
      if (sender.tab?.windowId != null) {
        return browser.windows.remove(sender.tab.windowId);
      }
      return Promise.reject(new Error('quit: no sender windowId'));
    }
    case 'lock-state': {
      lockState = !!msg.on;
      return Promise.resolve({ ok: true, lockState });
    }
    case 'get-lock-state': {
      return Promise.resolve({ lockState });
    }
    default:
      return undefined; // not handled
  }
});

console.log('[rdpls] background.js loaded');
```

- [ ] **Step 2: Commit**

```bash
git add src-firefox/extension/background.js
git commit -m "feat(ext): background.js with quit handler"
```

### Task 3.4: Wire quit chord in content.js; run end-to-end smoke test

**Files:**
- Modify: `src-firefox/extension/content.js`

- [ ] **Step 1: Add the Ctrl+Alt+Shift+Q handler**

Replace the `TODO` in `content.js` with the quit handler (add after the `isTop` line):

```javascript
  window.addEventListener('keydown', (e) => {
    if (!(e.ctrlKey && e.altKey && e.shiftKey && (e.key === 'q' || e.key === 'Q'))) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    browser.runtime.sendMessage({ type: 'quit' }).catch((err) => {
      console.error('[rdpls] quit failed', err);
    });
  }, true);
```

- [ ] **Step 2: Install and launch**

```bash
make install
~/.local/opt/firefox-dev/firefox --profile $HOME/.local/share/rdpls/profile --kiosk --no-remote --new-instance 'about:blank'
```

- [ ] **Step 3: Verify extension loaded**

Open devtools (F12). In the Console, verify both logs appear:
- `[rdpls] content.js loaded { isTop: true, url: "about:blank" }`
- `[rdpls] content-fullscreen.js loaded { url: "about:blank" }`

If they don't appear, check `about:debugging#/runtime/this-firefox` for sideload errors.

- [ ] **Step 4: Test quit**

Press `Ctrl+Alt+Shift+Q`. Firefox window should close. Process should exit — verify with:

```bash
pgrep -a firefox || echo "no firefox running"
```

- [ ] **Step 5: Fill in §5 in findings doc**

Replace the "DEFERRED" §5 in `docs/firefox-keyboard-flow.md` with the verified result:

```markdown
## §5: browser.windows.remove termination

- `Ctrl+Alt+Shift+Q` closes the --kiosk --no-remote window: **<yes | no>**
- Firefox process exits cleanly: **<yes | no>**
- Any confirmation dialog: **<none | "...">**
```

If a dialog appears, add `browser.tabs.warnOnClose=false` was already set; investigate further and extend `user.js`.

- [ ] **Step 6: Commit**

```bash
git add src-firefox/extension/content.js docs/firefox-keyboard-flow.md
git commit -m "feat(ext): quit chord + §5 verification"
```

---

## Chunk 4: Extension — lock toggle and safety net

Implement `Ctrl+Alt+Shift+.` lock toggle in the top-frame content script, plus the `Ctrl+Alt+Shift+/` unconditional safety net.

### Task 4.1: Implement lock toggle in content-fullscreen.js

**Files:**
- Modify: `src-firefox/extension/content-fullscreen.js`

- [ ] **Step 1: Write the lock-toggle handler**

Replace the TODO in `content-fullscreen.js`:

```javascript
  let locked = false;

  const postState = (on) => {
    // Tell background for state tracking + tell content.js to render the toast.
    browser.runtime.sendMessage({ type: 'lock-state', on }).catch(() => {});
    window.postMessage({ source: 'rdpls', type: 'toast', on }, '*');
  };

  window.addEventListener('keydown', async (e) => {
    // Safety-net unlock — unconditional, runs even if state is wrong
    if (e.ctrlKey && e.altKey && e.shiftKey && e.key === '/') {
      e.preventDefault();
      e.stopImmediatePropagation();
      try { await navigator.keyboard.unlock(); } catch (_) {}
      try { await document.exitFullscreen(); } catch (_) {}
      locked = false;
      postState(false);
      return;
    }

    // Lock toggle
    if (!(e.ctrlKey && e.altKey && e.shiftKey && e.key === '.')) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    try {
      if (!locked) {
        // ORDER MATTERS: fullscreen-then-lock preserves activation per §2 findings.
        await document.documentElement.requestFullscreen({ navigationUI: 'hide' });
        await navigator.keyboard.lock();
        locked = true;
        postState(true);
      } else {
        await navigator.keyboard.unlock();
        await document.exitFullscreen();
        locked = false;
        postState(false);
      }
    } catch (err) {
      console.error('[rdpls] lock toggle failed', err);
    }
  }, true);
```

If §2 findings showed the `await requestFullscreen()` then `await lock()` sequence loses activation, switch to a `.then()` chain inside the handler (no awaits):

```javascript
// Alternate form if §2 requires it:
document.documentElement.requestFullscreen({ navigationUI: 'hide' })
  .then(() => navigator.keyboard.lock())
  .then(() => { locked = true; postState(true); })
  .catch((err) => console.error('[rdpls] lock-on failed', err));
```

Pick the form that matches the verification outcome.

- [ ] **Step 2: Reload extension and test**

If `make dev` session is running, it auto-reloads. Otherwise:

```bash
# In Firefox: about:debugging#/runtime/this-firefox → find rdpls → Reload
```

- [ ] **Step 3: Test lock on**

Open `about:blank`. Press `Ctrl+Alt+Shift+.` on the page. In console, look for no errors. Verify lock is active: try `Alt+Tab` — focus should not leave Firefox (compositor inhibit active).

- [ ] **Step 4: Test lock off**

Press `Ctrl+Alt+Shift+.` again. `Alt+Tab` should now escape Firefox.

- [ ] **Step 5: Test safety net**

Press `Ctrl+Alt+Shift+.` to re-enable lock. Then press `Ctrl+Alt+Shift+/`. Lock should go off (Alt+Tab escapes again).

- [ ] **Step 6: Commit**

```bash
git add src-firefox/extension/content-fullscreen.js
git commit -m "feat(ext): lock toggle + safety net"
```

### Task 4.2: Handle the non-top-frame lock-chord case

**Files:**
- Modify: `src-firefox/extension/content.js`

- [ ] **Step 1: Add non-top-frame handling**

When `Ctrl+Alt+Shift+.` fires in an iframe, we can't directly call `lock()` there. Default behavior: show a hint toast in the top frame via `window.postMessage`.

**Conditional upgrade**: if §4 findings in `docs/firefox-keyboard-flow.md` show that content scripts DO run in the RDP iframe AND the iframe can `postMessage` to `window.top` (not blocked by sandboxing), extend the iframe handler to also post a `{ source: 'rdpls', type: 'trigger-lock-toggle' }` message, and add a `window.addEventListener('message', ...)` in `content-fullscreen.js` that dispatches a synthetic keydown or directly calls the toggle. (This relay still loses user activation, so on lock-ON it may fail — document whichever outcome occurs.)

In `content.js`, after the quit handler:

```javascript
  if (!isTop) {
    window.addEventListener('keydown', (e) => {
      if (!(e.ctrlKey && e.altKey && e.shiftKey && e.key === '.')) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      // Message parent frames up to top; top-frame content.js will pick it up.
      window.top.postMessage({ source: 'rdpls', type: 'toast-hint', text: 'Press Ctrl+Alt+Shift+. with the main page in focus' }, '*');
    }, true);
  }
```

Note: if cross-origin sandboxing prevents `window.top.postMessage` from the iframe (found in §4), this handler is a no-op and the user is stuck pressing the chord until focus shifts. The safety-net chord is the recovery.

- [ ] **Step 2: Verify**

Load a page with an iframe (e.g., an `<iframe>` on a test page). Press the chord with iframe focused. Check top-frame console for the message.

- [ ] **Step 3: Commit**

```bash
git add src-firefox/extension/content.js
git commit -m "feat(ext): iframe lock-chord hint"
```

---

## Chunk 5: Window.open shim, toggle toast, docs

### Task 5.1: Implement window.open shim in content.js

**Files:**
- Modify: `src-firefox/extension/content.js`

- [ ] **Step 1: Inject the shim via a page-world script**

Content scripts run in an isolated world; overriding `window.open` from a content script doesn't affect page code. Inject a `<script>` into the page to run in the main world.

Add to `content.js`:

```javascript
  // window.open shim — rewrite same-tab navigation. Injected into page world.
  const injectShim = () => {
    const s = document.createElement('script');
    s.textContent = `
      (function() {
        const origOpen = window.open;
        window.open = function(url, target, features) {
          if (url) {
            try { window.location.assign(url); } catch (e) { console.error('[rdpls] open shim nav failed', e); }
            return window; // non-null so callers that check don't break
          }
          return origOpen.apply(this, arguments);
        };
      })();
    `;
    (document.head || document.documentElement).appendChild(s);
    s.remove();
  };
  injectShim();
```

- [ ] **Step 2: Verify**

In the kiosk Firefox, devtools console:

```javascript
window.open('about:blank', '_blank', 'width=400,height=300')
```
Expected: navigates in the same tab, no new window.

- [ ] **Step 3: Commit**

```bash
git add src-firefox/extension/content.js
git commit -m "feat(ext): window.open shim for same-tab popup redirect"
```

### Task 5.2: Implement the toast DOM

**Files:**
- Modify: `src-firefox/extension/content.js`

- [ ] **Step 1: Add toast listener and renderer (top frame only)**

Append to `content.js` (inside the IIFE, gated on `isTop`):

```javascript
  if (isTop) {
    const TOAST_ID = '__rdpls_toast__';
    const TOAST_MS = 1800;
    let toastTimer = null;

    const showToast = (text) => {
      let el = document.getElementById(TOAST_ID);
      if (!el) {
        el = document.createElement('div');
        el.id = TOAST_ID;
        el.setAttribute('style', [
          'position: fixed', 'top: 1rem', 'left: 50%',
          'transform: translateX(-50%)',
          'padding: 0.5rem 1rem',
          'background: rgba(20,20,20,0.88)',
          'color: #f5f5f5',
          'font: 600 0.95rem/1.2 system-ui, sans-serif',
          'border-radius: 999px',
          'pointer-events: none',
          'user-select: none',
          'z-index: 2147483647',
          'opacity: 0',
          'transition: opacity 120ms ease',
        ].join(';'));
        document.documentElement.appendChild(el);
      }
      el.textContent = text;
      requestAnimationFrame(() => { el.style.opacity = '1'; });
      clearTimeout(toastTimer);
      toastTimer = setTimeout(() => { el.style.opacity = '0'; }, TOAST_MS);
    };

    window.addEventListener('message', (ev) => {
      const d = ev.data;
      if (!d || d.source !== 'rdpls') return;
      if (d.type === 'toast') {
        showToast(d.on ? 'keys → remote' : 'keys → compositor');
      } else if (d.type === 'toast-hint' && typeof d.text === 'string') {
        showToast(d.text);
      }
    });
  }
```

- [ ] **Step 2: Test**

Reload extension, navigate to `about:blank`, press `Ctrl+Alt+Shift+.`. Verify the pill appears with "keys → remote", fades after ~1.8s. Press again, pill shows "keys → compositor".

- [ ] **Step 3: Commit**

```bash
git add src-firefox/extension/content.js
git commit -m "feat(ext): toggle toast DOM + message handler"
```

### Task 5.3: End-to-end smoke test against the live RDP entry URL

**Files:** (none modified — this is a verification step)

- [ ] **Step 1: Launch via the installed desktop entry or Exec line**

```bash
# Preferred: via desktop entry (niri users: use a launcher like fuzzel/anyrun)
gtk-launch rdpls 2>/dev/null || \
  ~/.local/opt/firefox-dev/firefox --profile ~/.local/share/rdpls/profile --kiosk --no-remote --new-instance https://myapps.microsoft.com
```

`gtk-launch` is a GNOME tool and may not be present on niri; the fallback directly runs the same Exec line the desktop entry uses.

- [ ] **Step 2: Sign in and reach an RDP session**

Walk through myapps.microsoft.com → tenant picker → RDP app launch. Confirm the popup is redirected to the same tab (§8 shim).

- [ ] **Step 3: Verify chord behavior in-session**

- `Ctrl+Alt+Shift+.` → toast "keys → remote", Super/Alt+Tab pass to remote.
- `Ctrl+Alt+Shift+.` again → toast "keys → compositor", Super/Alt+Tab escape.
- `Ctrl+Alt+Shift+/` → same as off-toggle.
- `Ctrl+Alt+Shift+Q` → window closes, process exits.

- [ ] **Step 4: Record session-quality observation**

Append to `docs/firefox-keyboard-flow.md`:

```markdown
## Session smoke test (post-install)

Date: <YYYY-MM-DD>

- Popup redirect works: <yes | no>
- Chord set all functional: <yes | describe issues>
- Video quality vs. prior webkit2gtk/webkit6 attempts: <improved | same | worse>
- Any unexpected behavior: <list>
```

- [ ] **Step 5: Commit**

```bash
git add docs/firefox-keyboard-flow.md
git commit -m "docs: session smoke test findings"
```

### Task 5.4: Final cleanup and branch state

**Files:**
- Delete (if still present): `docs/keyboard-matrix.md` (replaced by `firefox-keyboard-flow.md` on Linux)

- [ ] **Step 1: Decide on docs/keyboard-matrix.md**

That doc is Linux webkit2gtk-specific. The spec says it is replaced, not migrated. Delete it:

```bash
git rm docs/keyboard-matrix.md
```

- [ ] **Step 2: Review full diff against main**

```bash
git log --oneline main..firefox
git diff --stat main..firefox
```

Confirm: `src-firefox/` added, `Makefile` + `CLAUDE.md` + `README.md` + `.gitignore` updated, `docs/firefox-keyboard-flow.md` added, `docs/keyboard-matrix.md` deleted, `src-tauri/` untouched.

- [ ] **Step 3: Commit final cleanup**

```bash
git commit -m "docs: retire keyboard-matrix.md (replaced by firefox-keyboard-flow.md)"
```

- [ ] **Step 4: Push branch if desired**

Do not merge to main without the user's explicit go-ahead.

```bash
git push -u origin firefox  # user decision
```

---

## Done

The `firefox` branch now carries a working single-user Firefox-kiosk host for the Linux side of rdpls. macOS `src-tauri/` is unchanged. Session-drop detection, clipboard portal, and a persistent status strip remain as Phase 3 items per the spec and are not part of this plan.
