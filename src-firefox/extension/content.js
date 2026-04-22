// content.js — runs in all frames (`all_frames: true`).
//
// Owns:
//   - Quit chord (Ctrl+Alt+Shift+Q) -> sendMessage to background.
//   - window.open shim -> rewrite to same-tab navigation.
//   - Toast DOM: rendered only in the top frame; subscribes to
//     `{ source: 'rdpls', type: 'toast' | 'toast-hint' }` postMessages
//     from content-fullscreen.js.
//
// Lock/unlock themselves live in content-fullscreen.js (top-frame only)
// so user activation is preserved.

(() => {
  'use strict';

  const isTop = window.top === window;

  // --- Quit chord (any frame) ---
  window.addEventListener('keydown', (e) => {
    if (!(e.ctrlKey && e.altKey && e.shiftKey && (e.key === 'q' || e.key === 'Q'))) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    browser.runtime.sendMessage({ type: 'quit' }).catch((err) => {
      console.error('[rdpls] quit failed', err);
    });
  }, true);

  // --- Non-top-frame lock chord: hint the user to press it from the top frame ---
  if (!isTop) {
    window.addEventListener('keydown', (e) => {
      if (!(e.ctrlKey && e.altKey && e.shiftKey && e.key === '.')) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      try {
        window.top.postMessage({
          source: 'rdpls',
          type: 'toast-hint',
          text: 'Press Ctrl+Alt+Shift+. with the main page in focus'
        }, '*');
      } catch (_) {
        // Cross-origin sandboxing may block postMessage to top; fall through silently.
      }
    }, true);
  }

  // --- window.open shim (page world) ---
  // Content scripts run in an isolated world; overriding window.open from here
  // wouldn't affect page code. Inject a <script> so the override lands in the
  // main world before any page script runs (run_at: document_start).
  const injectOpenShim = () => {
    const s = document.createElement('script');
    s.textContent = `
      (function() {
        const origOpen = window.open;
        window.open = function(url, target, features) {
          if (url) {
            try { window.location.assign(url); }
            catch (e) { console.error('[rdpls] open shim nav failed', e); }
            return window;
          }
          return origOpen.apply(this, arguments);
        };
      })();
    `;
    (document.head || document.documentElement).appendChild(s);
    s.remove();
  };
  injectOpenShim();

  // --- Toast (top frame only) ---
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
          'position: fixed',
          'top: 1rem',
          'left: 50%',
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

  console.log('[rdpls] content.js loaded', { isTop, url: location.href });
})();
