//! Injected JS that captures the two intentional in-window accelerators and
//! relays them to Rust via webkit6's script-message handlers.
//!
//! Chord rationale: GNOME/Mutter eats Ctrl+Alt+Shift+Escape when keyboard-
//! shortcut-inhibit is OFF, so we can't use Escape to toggle back. Period
//! sidesteps this (not bound by any known compositor default).
//!
//! Intercepting at the WebView rather than as a global shortcut: Wayland
//! compositors do not honor X11/rdev-style global grabs. The app always has
//! focus when the user wants to trigger this, so WebView-level capture is
//! sufficient.
//!
//! Cross-frame relay: `window.webkit.messageHandlers` is only exposed in the
//! top frame. When focus lands in a cross-origin subframe (the RDP session
//! iframe), keydown still fires our listener but the direct postMessage to
//! the handler would no-op. Subframes relay via `window.top.postMessage`; the
//! top frame routes allowlisted commands through to the message handler.
//!
//! State prediction: webkit6 script-message handlers don't return a value to
//! the page, so the toast can't read the post-toggle state from Rust. Instead
//! we track the expected state in JS (initial: both ON to match startup) and
//! flip it locally each time we dispatch.

pub const HOTKEY_SCRIPT: &str = r#"
(function () {
    if (window.__rdpls_escape_installed) return;
    window.__rdpls_escape_installed = true;

    var IS_TOP = window.top === window;
    var ALLOWED = ['rdpls_toggle_inhibit', 'rdpls_toggle_fullscreen'];

    // Startup: inhibit is ON (installed in shortcuts_inhibit::install on
    // window realize), fullscreen is ON (ApplicationWindow::set_fullscreened).
    // Kept global so all injected frames share the same view.
    if (!window.__rdpls_state) {
        window.__rdpls_state = {
            rdpls_toggle_inhibit: true,
            rdpls_toggle_fullscreen: true
        };
    }

    function showToast(text) {
        if (!IS_TOP) return;
        if (!document.body) return;

        var el = window.__rdpls_toast_el;
        if (!el || !el.isConnected) {
            el = document.createElement('div');
            el.style.cssText =
                'position:fixed;top:16px;left:50%;' +
                'transform:translateX(-50%);' +
                'padding:10px 20px;border-radius:999px;' +
                'background:rgba(0,0,0,0.75);color:#fff;' +
                'font:500 14px system-ui,sans-serif;' +
                'pointer-events:none;z-index:2147483647;' +
                'opacity:0;transition:opacity 200ms ease-out;';
            document.body.appendChild(el);
            window.__rdpls_toast_el = el;
        }

        el.textContent = text;

        if (window.__rdpls_toast_timer) {
            clearTimeout(window.__rdpls_toast_timer);
        }

        el.style.transition = 'none';
        el.style.opacity = '1';
        void el.offsetHeight;
        el.style.transition = 'opacity 200ms ease-out';

        window.__rdpls_toast_timer = setTimeout(function () {
            el.style.opacity = '0';
        }, 1500);
    }

    function send(cmd) {
        try {
            if (window.webkit && window.webkit.messageHandlers &&
                window.webkit.messageHandlers[cmd]) {
                window.webkit.messageHandlers[cmd].postMessage(null);
            }
        } catch (_) {}
    }

    function dispatch(cmd, onLabel, offLabel) {
        if (IS_TOP) {
            var next = !window.__rdpls_state[cmd];
            window.__rdpls_state[cmd] = next;
            send(cmd);
            showToast(next ? onLabel : offLabel);
        } else {
            try {
                window.top.postMessage({
                    __rdpls: { cmd: cmd, onLabel: onLabel, offLabel: offLabel }
                }, '*');
            } catch (_) {}
        }
    }

    if (IS_TOP) {
        window.addEventListener('message', function (e) {
            var msg = e.data && e.data.__rdpls;
            if (!msg) return;
            if (ALLOWED.indexOf(msg.cmd) === -1) return;
            var next = !window.__rdpls_state[msg.cmd];
            window.__rdpls_state[msg.cmd] = next;
            send(msg.cmd);
            showToast(next ? msg.onLabel : msg.offLabel);
        }, false);
    }

    window.addEventListener('keydown', function (e) {
        if (!(e.ctrlKey && e.altKey && e.shiftKey)) return;

        if (e.code === 'Period') {
            e.preventDefault();
            e.stopPropagation();
            dispatch('rdpls_toggle_inhibit', 'Keys → Remote', 'Keys → Local');
        } else if (e.code === 'KeyF') {
            e.preventDefault();
            e.stopPropagation();
            dispatch('rdpls_toggle_fullscreen', 'Fullscreen ON', 'Fullscreen OFF');
        }
    }, { capture: true });
})();
"#;

/// Document-start hook that instruments what Microsoft's player actually does
/// at runtime — distinct from CODEC_PROBE_SCRIPT which only runs capability
/// queries. This catches:
///   - Every real VideoDecoder.configure() call (the codec actually picked).
///   - Every VideoDecoder.isConfigSupported() query by the page.
///   - Every mediaCapabilities.decodingInfo() query.
///   - Every RTCRtpReceiver.getCapabilities('video') call.
///   - Worker construction + any postMessage in/out whose JSON payload
///     contains a codec-like token (avc1/vp09/av01/H264/etc.).
///
/// Output goes to stderr via the rdpls_log message handler. Must be injected
/// at document-start so hooks land before page scripts grab references.
pub const HOOK_SCRIPT: &str = r#"
(function () {
    if (window.__rdpls_hooks_installed) return;
    window.__rdpls_hooks_installed = true;

    function log(text) {
        try {
            if (window.webkit && window.webkit.messageHandlers &&
                window.webkit.messageHandlers.rdpls_log) {
                window.webkit.messageHandlers.rdpls_log.postMessage('[hook] ' + text);
            }
        } catch (_) {}
    }

    // --- Firefox fingerprint spoofing ---
    // UA alone isn't enough: Microsoft's RDP client checks Gecko-only globals
    // (InstallTrigger), Gecko-only navigator fields (buildID, oscpu), and
    // -moz-* CSS.supports queries. Without these we drop into their degraded
    // "unknown browser" video pipeline tier (lower bitrate, WASM H.264
    // fallback with no VP9/AV1 upgrade path).
    try {
        if (!('InstallTrigger' in window)) {
            Object.defineProperty(window, 'InstallTrigger', {
                value: { install: function () {} },
                writable: true, configurable: true, enumerable: false
            });
        }
    } catch (_) {}
    try {
        Object.defineProperty(navigator, 'buildID', {
            get: function () { return '20181001000000'; },
            configurable: true
        });
    } catch (_) {}
    try {
        Object.defineProperty(navigator, 'oscpu', {
            get: function () { return 'Linux x86_64'; },
            configurable: true
        });
    } catch (_) {}
    try {
        window.mozInnerScreenX = 0;
        window.mozInnerScreenY = 0;
    } catch (_) {}
    try {
        var origSupports = CSS.supports.bind(CSS);
        CSS.supports = function () {
            try {
                var joined = Array.prototype.join.call(arguments, ' ');
                if (/-moz-/.test(joined)) return true;
            } catch (_) {}
            return origSupports.apply(null, arguments);
        };
    } catch (_) {}

    // --- Clear stale "no-webworkers" flag ---
    // Once Microsoft's page stashes a "skip webworkers" flag in localStorage,
    // subsequent sessions stay on the degraded path even after we fix whatever
    // caused the original worker failure. Best-effort nuke on RDWeb origins.
    try {
        if (/msappproxy\.net|wvd\.microsoft\.com|cloudpc\.microsoft\.com/i.test(location.hostname) ||
            /\/RDWeb\//i.test(location.pathname)) {
            var removed = [];
            for (var i = localStorage.length - 1; i >= 0; i--) {
                var k = localStorage.key(i);
                if (/worker|fallback|disable|skip|degraded/i.test(k)) {
                    removed.push(k);
                    localStorage.removeItem(k);
                }
            }
            for (var j = sessionStorage.length - 1; j >= 0; j--) {
                var sk = sessionStorage.key(j);
                if (/worker|fallback|disable|skip|degraded/i.test(sk)) {
                    removed.push('sess:' + sk);
                    sessionStorage.removeItem(sk);
                }
            }
            if (removed.length) log('cleared storage: ' + removed.join(','));
        }
    } catch (_) {}

    function brief(obj) {
        try {
            var s = JSON.stringify(obj);
            if (s.length > 400) s = s.slice(0, 400) + '...';
            return s;
        } catch (_) {
            return String(obj);
        }
    }

    // --- VideoDecoder ---
    if (typeof VideoDecoder === 'function') {
        try {
            var origConfigure = VideoDecoder.prototype.configure;
            VideoDecoder.prototype.configure = function (config) {
                log('VideoDecoder.configure: ' + brief(config));
                return origConfigure.call(this, config);
            };
        } catch (_) {}

        try {
            var origIsSupported = VideoDecoder.isConfigSupported;
            VideoDecoder.isConfigSupported = function (config) {
                var p = origIsSupported.call(this, config);
                Promise.resolve(p).then(function (r) {
                    log('VideoDecoder.isConfigSupported(' + (config && config.codec) +
                        ', hw=' + (config && config.hardwareAcceleration || 'no-pref') +
                        ') -> ' + (r && r.supported ? 'yes' : 'no'));
                }).catch(function () {});
                return p;
            };
        } catch (_) {}
    }

    // --- MediaCapabilities ---
    if (navigator.mediaCapabilities && navigator.mediaCapabilities.decodingInfo) {
        try {
            var mc = navigator.mediaCapabilities;
            var origDecoding = mc.decodingInfo.bind(mc);
            mc.decodingInfo = function (cfg) {
                var p = origDecoding(cfg);
                Promise.resolve(p).then(function (r) {
                    log('mediaCapabilities.decodingInfo(' +
                        (cfg && cfg.video && cfg.video.contentType) +
                        ') -> sup=' + !!r.supported + ' smooth=' + !!r.smooth +
                        ' powerEff=' + !!r.powerEfficient);
                }).catch(function () {});
                return p;
            };
        } catch (_) {}
    }

    // --- RTCRtpReceiver.getCapabilities ---
    if (window.RTCRtpReceiver && RTCRtpReceiver.getCapabilities) {
        try {
            var origGetCaps = RTCRtpReceiver.getCapabilities;
            RTCRtpReceiver.getCapabilities = function (kind) {
                var r = origGetCaps.call(this, kind);
                if (kind === 'video' && r && r.codecs) {
                    var mimes = r.codecs.map(function (c) { return c.mimeType; }).join(',');
                    log('RTCRtpReceiver.getCapabilities(video) -> ' + mimes);
                }
                return r;
            };
        } catch (_) {}
    }

    // --- Worker logging (pass-through, no shim) ---
    // Earlier we tried a blob-URL shim to inject hooks inside the worker, but
    // the blob origin breaks whatever URL-relative calls Microsoft's librdp
    // worker makes ("We could not start the connection due to a webworker
    // error"). Drop the shim and just log that a worker was created — if
    // Microsoft falls back to main-thread decoding (its "Reconnect without
    // webworkers" path), the real VideoDecoder.configure hook above catches it.
    if (typeof window.Worker === 'function') {
        try {
            var OrigWorker = window.Worker;
            function LoggedWorker(url, opts) {
                try {
                    var abs = new URL(url, location.href).href;
                    log('new Worker(' + abs + (opts && opts.type ? ', type=' + opts.type : '') + ')');
                } catch (_) {}
                return new OrigWorker(url, opts);
            }
            LoggedWorker.prototype = OrigWorker.prototype;
            window.Worker = LoggedWorker;
        } catch (_) {}
    }

    log('hooks installed on ' + location.href);
})();
"#;

/// Tiny user script injected at document start to suppress Ctrl+scroll zoom.
/// WebKit's default wheel handling lets it slip through even with developer
/// extras off, and zooming the Microsoft RDP client's viewport is never what
/// the user wants.
pub const ZOOM_SUPPRESS_SCRIPT: &str =
    "window.addEventListener('wheel', function(e) { if (e.ctrlKey) { e.preventDefault(); } }, { passive: false, capture: true });";

/// One-shot WebCodecs capability probe. Answers the question: does this
/// WebKit6 build advertise HW decode for AV1/VP9/H.264/HEVC? Microsoft's
/// HTML5 RDP client picks a codec based on VideoDecoder.isConfigSupported,
/// so whatever we see here is what the remote will be told it can send.
///
/// Output lands in two places: the rdpls_log message handler (stderr) and
/// a persistent green overlay in the bottom-right of the page (click to
/// dismiss). Devtools is intentionally unavailable in rdpls.
pub const CODEC_PROBE_SCRIPT: &str = r#"
(async function () {
    if (window.__rdpls_codec_probe_ran) return;
    window.__rdpls_codec_probe_ran = true;
    if (window.top !== window) return;

    var codecs = [
        { label: 'AV1 Main L4.0 8b', codec: 'av01.0.04M.08' },
        { label: 'AV1 Main L5.0 8b', codec: 'av01.0.12M.08' },
        { label: 'VP9 P0 L1.0 8b',   codec: 'vp09.00.10.08' },
        { label: 'VP9 P0 L5.0 8b',   codec: 'vp09.00.50.08' },
        { label: 'H.264 Baseline',   codec: 'avc1.42E01E' },
        { label: 'H.264 Main 3.1',   codec: 'avc1.4D401F' },
        { label: 'H.264 High 4.0',   codec: 'avc1.640028' },
        { label: 'HEVC Main L3.1',   codec: 'hev1.1.6.L93.B0' }
    ];

    var lines = [];
    lines.push('UA: ' + navigator.userAgent);
    lines.push('VideoDecoder: ' + (('VideoDecoder' in window) ? 'yes' : 'NO'));
    lines.push('MediaSource:  ' + (('MediaSource' in window) ? 'yes' : 'no'));
    lines.push('');
    lines.push('codec                 any   hw    sw');

    async function probe(base, accel) {
        try {
            var cfg = Object.assign({}, base);
            if (accel) cfg.hardwareAcceleration = accel;
            var r = await VideoDecoder.isConfigSupported(cfg);
            return r && r.supported ? 'yes' : 'no';
        } catch (e) {
            return 'err';
        }
    }

    if ('VideoDecoder' in window) {
        for (var i = 0; i < codecs.length; i++) {
            var c = codecs[i];
            var base = { codec: c.codec, codedWidth: 1920, codedHeight: 1080 };
            var any = await probe(base, null);
            var hw  = await probe(base, 'prefer-hardware');
            var sw  = await probe(base, 'prefer-software');
            var row = (c.label + '                      ').slice(0, 22) +
                      (any + '    ').slice(0, 6) +
                      (hw  + '    ').slice(0, 6) +
                      sw;
            lines.push(row);
        }
    }

    // MediaCapabilities.decodingInfo — more honest than WebCodecs because it
    // exposes `smooth` and `powerEfficient`. Microsoft likely consults this.
    if (navigator.mediaCapabilities && navigator.mediaCapabilities.decodingInfo) {
        lines.push('');
        lines.push('mediaCapabilities.decodingInfo (media-source, 1920x1080@30, 4Mbps):');
        lines.push('codec                 sup smooth powerEff');
        var mcCodecs = [
            { label: 'AV1',           ct: 'video/mp4; codecs="av01.0.04M.08"' },
            { label: 'VP9',           ct: 'video/webm; codecs="vp09.00.10.08"' },
            { label: 'H.264 High 4.0', ct: 'video/mp4; codecs="avc1.640028"' },
            { label: 'HEVC Main L3.1', ct: 'video/mp4; codecs="hev1.1.6.L93.B0"' }
        ];
        for (var m = 0; m < mcCodecs.length; m++) {
            var mc = mcCodecs[m];
            try {
                var info = await navigator.mediaCapabilities.decodingInfo({
                    type: 'media-source',
                    video: { contentType: mc.ct, width: 1920, height: 1080, bitrate: 4000000, framerate: 30 }
                });
                lines.push(
                    (mc.label + '                      ').slice(0, 22) +
                    (info.supported ? 'yes' : 'no ') + ' ' +
                    (info.smooth ? 'yes   ' : 'no    ') + ' ' +
                    (info.powerEfficient ? 'yes' : 'no')
                );
            } catch (e) {
                lines.push((mc.label + '                      ').slice(0, 22) + 'err ' + (e.message || e));
            }
        }
    }

    // RTCRtpReceiver.getCapabilities — WebRTC codec list. AVD isn't WebRTC but
    // Microsoft's Teams/RDP media stack shares code and may sniff this list.
    if (window.RTCRtpReceiver && RTCRtpReceiver.getCapabilities) {
        lines.push('');
        lines.push('RTCRtpReceiver.getCapabilities("video").codecs:');
        try {
            var caps = RTCRtpReceiver.getCapabilities('video');
            if (caps && caps.codecs) {
                for (var k = 0; k < caps.codecs.length; k++) {
                    var cc = caps.codecs[k];
                    lines.push('  ' + cc.mimeType + (cc.sdpFmtpLine ? ' ; ' + cc.sdpFmtpLine : ''));
                }
            } else {
                lines.push('  (none)');
            }
        } catch (e) {
            lines.push('  err: ' + (e.message || e));
        }
    } else {
        lines.push('');
        lines.push('RTCRtpReceiver.getCapabilities: NOT EXPOSED');
    }

    // MSE side-check: what does MediaSource.isTypeSupported say? (Mostly
    // relevant if Microsoft falls back to <video>+MSE rather than canvas.)
    if ('MediaSource' in window) {
        lines.push('');
        lines.push('MSE isTypeSupported:');
        var mime = [
            'video/mp4; codecs="avc1.42E01E"',
            'video/mp4; codecs="avc1.640028"',
            'video/mp4; codecs="hev1.1.6.L93.B0"',
            'video/webm; codecs="vp09.00.10.08"',
            'video/mp4; codecs="av01.0.04M.08"'
        ];
        for (var j = 0; j < mime.length; j++) {
            lines.push('  ' + (MediaSource.isTypeSupported(mime[j]) ? 'yes' : 'no ') + '  ' + mime[j]);
        }
    }

    var text = lines.join('\n');

    try {
        if (window.webkit && window.webkit.messageHandlers &&
            window.webkit.messageHandlers.rdpls_log) {
            window.webkit.messageHandlers.rdpls_log.postMessage('[codec-probe]\n' + text);
        }
    } catch (_) {}

    function showPanel() {
        if (!document.body) { setTimeout(showPanel, 100); return; }
        var panel = document.getElementById('__rdpls_codec_panel');
        if (!panel) {
            panel = document.createElement('pre');
            panel.id = '__rdpls_codec_panel';
            panel.style.cssText =
                'position:fixed;bottom:16px;right:16px;margin:0;' +
                'padding:12px 16px;border-radius:8px;' +
                'background:rgba(0,0,0,0.85);color:#0f0;' +
                'font:11px ui-monospace,Menlo,monospace;' +
                'white-space:pre;z-index:2147483647;' +
                'max-width:60vw;cursor:pointer;';
            panel.title = 'click to dismiss';
            panel.addEventListener('click', function () { panel.remove(); });
            document.body.appendChild(panel);
        }
        panel.textContent = text + '\n\n(click to dismiss)';
    }
    showPanel();
})();
"#;
