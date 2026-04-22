// background.js — state owner.
//
// Responsibilities:
//   - Track lock-state (informational) so other components can query it.
//   - Handle quit by closing the Firefox window. In --kiosk --no-remote
//     with a single window, removing the window terminates the Firefox
//     process, which quits the app.
//
// Lock / unlock / fullscreen calls live in content-fullscreen.js to
// preserve user activation.

let lockState = false;

browser.runtime.onMessage.addListener((msg, sender) => {
  switch (msg?.type) {
    case 'quit': {
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
      return undefined;
  }
});

console.log('[rdpls] background.js loaded');
