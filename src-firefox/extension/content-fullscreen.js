// content-fullscreen.js — runs in the TOP frame only.
//
// Owns: document.fullscreenElement + navigator.keyboard.lock() / unlock().
// These calls require user activation, which is only preserved when they run
// synchronously in the keydown handler that observed the chord — i.e., here,
// not routed through the background script.
//
// Chords handled:
//   Ctrl+Alt+Shift+.   toggle keyboard lock (requestFullscreen + lock)
//   Ctrl+Alt+Shift+/   safety-net: unconditional unlock + exitFullscreen
//
// If verification §2 (docs/firefox-keyboard-flow.md) shows that the
// `await requestFullscreen()` then `await lock()` sequence loses user
// activation, switch to a .then() chain inside the handler — see the
// ALTERNATE block in the source below.

(() => {
  'use strict';

  let locked = false;

  const postState = (on) => {
    browser.runtime.sendMessage({ type: 'lock-state', on }).catch(() => {});
    window.postMessage({ source: 'rdpls', type: 'toast', on }, '*');
  };

  const lockOn = async () => {
    // ORDER MATTERS: fullscreen-then-lock — Keyboard Lock requires the
    // document to be in Fullscreen API state.
    await document.documentElement.requestFullscreen({ navigationUI: 'hide' });
    await navigator.keyboard.lock();
    locked = true;
    postState(true);
  };

  const lockOff = async () => {
    try { await navigator.keyboard.unlock(); } catch (_) {}
    try { await document.exitFullscreen(); } catch (_) {}
    locked = false;
    postState(false);
  };

  window.addEventListener('keydown', (e) => {
    if (!(e.ctrlKey && e.altKey && e.shiftKey)) return;

    // Safety-net unconditional unlock.
    if (e.key === '/') {
      e.preventDefault();
      e.stopImmediatePropagation();
      lockOff().catch((err) => console.error('[rdpls] safety-net off failed', err));
      return;
    }

    // Main toggle.
    if (e.key !== '.') return;
    e.preventDefault();
    e.stopImmediatePropagation();
    const p = locked ? lockOff() : lockOn();
    p.catch((err) => console.error('[rdpls] lock toggle failed', err));
  }, true);

  // --- ALTERNATE (activation-sensitive) sequencing ---
  // If `await` between requestFullscreen and lock drops activation on this
  // Firefox build, replace lockOn() above with:
  //
  //   const lockOn = () => {
  //     return document.documentElement.requestFullscreen({ navigationUI: 'hide' })
  //       .then(() => navigator.keyboard.lock())
  //       .then(() => { locked = true; postState(true); });
  //   };
  //
  // Same idea, no intermediate `await` inside the handler.

  console.log('[rdpls] content-fullscreen.js loaded', { url: location.href });
})();
