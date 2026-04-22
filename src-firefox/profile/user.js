// src-firefox/profile/user.js
// Seeded by `make install` into ~/.local/share/rdpls/profile/.
// Once seeded, the live profile's user.js is not re-overwritten on
// subsequent `make install` runs (see Makefile). For a re-seed,
// `make uninstall-hard && make install`.

// --- Keyboard Lock API ---
user_pref("dom.keyboard-lock.enabled", true);

// --- Fullscreen UX ---
user_pref("full-screen-api.warning.timeout", 0);

// --- window.open backstop (shim in content.js is primary) ---
user_pref("browser.link.open_newwindow", 1);
user_pref("browser.link.open_newwindow.restriction", 0);

// --- Sideloaded unsigned extension (Dev Edition only) ---
user_pref("xpinstall.signatures.required", false);

// --- Update suppression. Final list may be trimmed after §7 verification. ---
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
