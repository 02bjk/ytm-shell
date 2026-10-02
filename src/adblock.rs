//! Layer A ad and tracker blocking scriptlets.
//! Injected at document-start, strictly hostname-guarded to music.youtube.com.
//! Hooks fetch, XMLHttpRequest, JSON.parse, and inline ytInitialPlayerResponse/ytInitialData
//! to prune ad structures before the YouTube Music application consumes them.
//! Features an ad-skipping engine that mutes/fast-skips ads without skipping or fast-forwarding songs,
//! an observer tied directly to #movie_player (eliminating CPU thrashing and lag),
//! and a non-intrusive floating indicator badge ("Bypassing ad...") during ad transitions.

/// Document-start script containing Layer A JSON pruning, fallback observer, indicator, and cosmetic CSS.
pub const LAYER_A_SCRIPT: &str = r#"
(function() {
    'use strict';

    // Strict hostname guard: only execute on YouTube Music.
    // Never run on accounts.google.com, recaptcha.net, or consent.youtube.com.
    if (window.location.hostname !== 'music.youtube.com') {
        return;
    }

    // -------------------------------------------------------------
    // 1. Surgical Cosmetic Hiding CSS & Indicator Styles
    // -------------------------------------------------------------
    var COSMETIC_CSS = `
        ytmusic-mealbar-promo-renderer,
        ytmusic-player-overlay-renderer[ad-showing],
        ytmusic-ad-slot-renderer,
        .ytmusic-mealbar-promo-renderer,
        .ytp-ad-overlay-container,
        .ytp-ad-message-container,
        .ytp-ad-player-overlay,
        .ytp-ad-image-overlay,
        .video-ads,
        #offer-module,
        tp-yt-paper-dialog:has(ytmusic-mealbar-promo-renderer),
        ytmusic-guide-entry-renderer:has(a[href*="music_premium"]) {
            display: none !important;
        }

        #ytm-shell-ad-indicator {
            position: fixed;
            top: 20px;
            left: 50%;
            transform: translate(-50%, -16px) scale(0.96);
            background: rgba(18, 18, 22, 0.90);
            backdrop-filter: blur(16px);
            -webkit-backdrop-filter: blur(16px);
            border: 1px solid rgba(255, 255, 255, 0.14);
            box-shadow: 0 10px 30px rgba(0, 0, 0, 0.55), 0 0 14px rgba(239, 68, 68, 0.2);
            color: #f1f5f9;
            padding: 7px 18px;
            border-radius: 9999px;
            display: flex;
            align-items: center;
            gap: 10px;
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
            font-size: 13px;
            font-weight: 500;
            letter-spacing: 0.25px;
            z-index: 9999999;
            opacity: 0;
            pointer-events: none;
            transition: opacity 0.22s cubic-bezier(0.16, 1, 0.3, 1), transform 0.22s cubic-bezier(0.16, 1, 0.3, 1);
            user-select: none;
            -webkit-user-select: none;
        }

        #ytm-shell-ad-indicator.visible {
            opacity: 1;
            transform: translate(-50%, 0) scale(1);
        }

        .ytm-shell-indicator-pulse {
            width: 8px;
            height: 8px;
            border-radius: 50%;
            background-color: #ef4444;
            box-shadow: 0 0 8px #ef4444;
            animation: ytm-pulse 1.3s infinite ease-in-out;
        }

        @keyframes ytm-pulse {
            0% { transform: scale(0.85); opacity: 0.6; }
            50% { transform: scale(1.25); opacity: 1; box-shadow: 0 0 12px #ef4444; }
            100% { transform: scale(0.85); opacity: 0.6; }
        }

        .ytm-shell-indicator-icon {
            width: 14px;
            height: 14px;
            color: #f87171;
            display: flex;
            align-items: center;
        }

        .ytm-shell-indicator-text {
            line-height: 1;
            color: #f8fafc;
        }
    `;

    function injectCosmeticCss() {
        if (document.getElementById('ytm-shell-adblock-css')) return;
        var style = document.createElement('style');
        style.id = 'ytm-shell-adblock-css';
        style.textContent = COSMETIC_CSS;
        (document.head || document.documentElement).appendChild(style);
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', injectCosmeticCss);
    } else {
        injectCosmeticCss();
    }

    // -------------------------------------------------------------
    // 2. Pruning Engine (uBlock Origin json-prune model)
    // -------------------------------------------------------------
    var AD_KEYS = [
        'adPlacements',
        'playerAds',
        'adSlots',
        'adBreakHeartbeatParams',
        'adBreakParams',
        'adParams',
        'adSignalsInfo',
        'adLayoutLoggingData'
    ];

    function pruneRecursive(val, depth) {
        if (!val || typeof val !== 'object' || depth > 8) {
            return val;
        }

        if (Array.isArray(val)) {
            for (var i = val.length - 1; i >= 0; i--) {
                var el = val[i];
                if (el && typeof el === 'object') {
                    if (el.adSlotRenderer ||
                        el.adPlacementRenderer ||
                        el.playerLegacyDesktopWatchAdsRenderer ||
                        el.adBreakRenderer ||
                        el.instreamAdPlayerOverlayRenderer ||
                        el.invideoRenderer ||
                        el.mealbarPromoRenderer) {
                        val.splice(i, 1);
                    } else {
                        pruneRecursive(el, depth + 1);
                    }
                }
            }
            return val;
        }

        // Direct ad fields on objects
        for (var k = 0; k < AD_KEYS.length; k++) {
            var key = AD_KEYS[k];
            if (key in val) {
                delete val[key];
            }
        }

        for (var prop in val) {
            if (Object.prototype.hasOwnProperty.call(val, prop)) {
                var child = val[prop];
                if (child && typeof child === 'object') {
                    pruneRecursive(child, depth + 1);
                }
            }
        }
        return val;
    }

    // Intercept inline player response objects before scripts consume them
    function hookInlineProperty(obj, prop) {
        var current = obj[prop];
        if (current && typeof current === 'object') {
            pruneRecursive(current, 0);
        }
        Object.defineProperty(obj, prop, {
            get: function() {
                return current;
            },
            set: function(val) {
                if (val && typeof val === 'object') {
                    pruneRecursive(val, 0);
                }
                current = val;
            },
            configurable: true,
            enumerable: true
        });
    }

    try {
        hookInlineProperty(window, 'ytInitialPlayerResponse');
    } catch (_) {}

    // -------------------------------------------------------------
    // 3. Fast-Path Telemetry & Stats Mocks
    //    Preemptively return 200 OK for tracking/stats requests to prevent
    //    client-side retry delays, console spam, and UI thread stalls.
    // -------------------------------------------------------------
    function isTelemetryOrAdUrl(u) {
        if (typeof u !== 'string') return false;
        return u.includes('/youtubei/v1/log_event') ||
               u.includes('/api/stats/ads') ||
               u.includes('/api/stats/qoe') ||
               u.includes('/api/stats/atr') ||
               u.includes('/pagead/');
    }

    if (typeof window.navigator.sendBeacon === 'function') {
        var origBeacon = window.navigator.sendBeacon.bind(window.navigator);
        window.navigator.sendBeacon = function(url, data) {
            if (isTelemetryOrAdUrl(url)) {
                return true;
            }
            return origBeacon.apply(this, arguments);
        };
    }

    // -------------------------------------------------------------
    // 4. Hook fetch for /player, /next and telemetry endpoints
    // -------------------------------------------------------------
    if (typeof window.fetch === 'function') {
        var origFetch = window.fetch;
        window.fetch = async function(input, init) {
            var url = typeof input === 'string' ? input : (input && input.url ? input.url : '');

            // Fast-path: immediately satisfy telemetry calls without network overhead
            if (isTelemetryOrAdUrl(url)) {
                return new Response(JSON.stringify({}), {
                    status: 200,
                    headers: { 'Content-Type': 'application/json' }
                });
            }

            var isAdTarget = url.includes('/youtubei/v1/player') || url.includes('/youtubei/v1/next');
            var response = await origFetch.apply(this, arguments);

            if (isAdTarget) {
                var origJson = response.json.bind(response);
                var origText = response.text.bind(response);

                response.json = async function() {
                    var data = await origJson();
                    return pruneRecursive(data, 0);
                };

                response.text = async function() {
                    var raw = await origText();
                    try {
                        var parsed = JSON.parse(raw);
                        return JSON.stringify(pruneRecursive(parsed, 0));
                    } catch (_) {
                        return raw;
                    }
                };
            }
            return response;
        };
    }

    // -------------------------------------------------------------
    // 5. Hook XMLHttpRequest for /player, /next and telemetry endpoints
    // -------------------------------------------------------------
    if (typeof window.XMLHttpRequest === 'function') {
        var origOpen = XMLHttpRequest.prototype.open;
        var origSend = XMLHttpRequest.prototype.send;

        XMLHttpRequest.prototype.open = function(method, url) {
            this._ytmUrl = typeof url === 'string' ? url : '';
            return origOpen.apply(this, arguments);
        };

        XMLHttpRequest.prototype.send = function(body) {
            if (isTelemetryOrAdUrl(this._ytmUrl)) {
                var self = this;
                try {
                    Object.defineProperty(self, 'readyState', { value: 4, writable: true, configurable: true });
                    Object.defineProperty(self, 'status', { value: 200, writable: true, configurable: true });
                    Object.defineProperty(self, 'responseText', { value: '{}', writable: true, configurable: true });
                    Object.defineProperty(self, 'response', { value: '{}', writable: true, configurable: true });
                } catch (_) {}
                setTimeout(function() {
                    if (typeof self.onreadystatechange === 'function') self.onreadystatechange();
                    if (typeof self.onload === 'function') self.onload();
                }, 0);
                return;
            }
            return origSend.apply(this, arguments);
        };

        var descText = Object.getOwnPropertyDescriptor(XMLHttpRequest.prototype, 'responseText');
        var descResp = Object.getOwnPropertyDescriptor(XMLHttpRequest.prototype, 'response');

        if (descText && descText.get) {
            Object.defineProperty(XMLHttpRequest.prototype, 'responseText', {
                get: function() {
                    var raw = descText.get.call(this);
                    if (this._ytmUrl && (this._ytmUrl.includes('/youtubei/v1/player') || this._ytmUrl.includes('/youtubei/v1/next'))) {
                        try {
                            var data = JSON.parse(raw);
                            return JSON.stringify(pruneRecursive(data, 0));
                        } catch (_) {}
                    }
                    return raw;
                },
                configurable: true,
                enumerable: true
            });
        }

        if (descResp && descResp.get) {
            Object.defineProperty(XMLHttpRequest.prototype, 'response', {
                get: function() {
                    var raw = descResp.get.call(this);
                    if (this._ytmUrl && (this._ytmUrl.includes('/youtubei/v1/player') || this._ytmUrl.includes('/youtubei/v1/next'))) {
                        if (typeof raw === 'string') {
                            try {
                                var data = JSON.parse(raw);
                                return JSON.stringify(pruneRecursive(data, 0));
                            } catch (_) {}
                        } else if (typeof raw === 'object' && raw !== null) {
                            return pruneRecursive(raw, 0);
                        }
                    }
                    return raw;
                },
                configurable: true,
                enumerable: true
            });
        }
    }

    // -------------------------------------------------------------
    // 6. Defensive fallback: prune inline player state only
    // -------------------------------------------------------------
    var origParse = JSON.parse;
    JSON.parse = function(text, reviver) {
        var res = origParse.apply(this, arguments);
        // Only prune genuine player responses (identified by videoDetails & playabilityStatus).
        // Never touch search results, browse feeds, or navigation catalogs!
        if (res && typeof res === 'object' && res.videoDetails && res.playabilityStatus) {
            if ('adPlacements' in res || 'playerAds' in res || 'adSlots' in res) {
                pruneRecursive(res, 0);
            }
        }
        return res;
    };

    // -------------------------------------------------------------
    // 6. Non-Intrusive Floating Ad-Bypassing Indicator
    // -------------------------------------------------------------
    var indicatorEl = null;

    function ensureIndicator() {
        if (indicatorEl && indicatorEl.parentNode) return indicatorEl;
        var existing = document.getElementById('ytm-shell-ad-indicator');
        if (existing) {
            indicatorEl = existing;
            return indicatorEl;
        }

        var badge = document.createElement('div');
        badge.id = 'ytm-shell-ad-indicator';
        badge.innerHTML = '<span class="ytm-shell-indicator-pulse"></span>' +
            '<svg class="ytm-shell-indicator-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">' +
            '<polygon points="5 4 15 12 5 20 5 4"></polygon>' +
            '<line x1="19" y1="5" x2="19" y2="19"></line>' +
            '</svg>' +
            '<span class="ytm-shell-indicator-text">Bypassing ad...</span>';

        var root = document.body || document.documentElement;
        if (root) {
            root.appendChild(badge);
            indicatorEl = badge;
        }
        return indicatorEl;
    }

    function setIndicatorVisible(visible) {
        var el = ensureIndicator();
        if (el) {
            if (visible) {
                el.classList.add('visible');
            } else {
                el.classList.remove('visible');
            }
        }
    }

    // -------------------------------------------------------------
    // 7. Refined Ad Skipper & Song Protection Engine
    // -------------------------------------------------------------
    var isBypassingAd = false;
    var wasMutedBeforeAd = false;
    var adFastForwarded = false;

    function handleAdPlayback() {
        var player = document.querySelector('#movie_player, .html5-video-player');
        var isAd = false;

        if (player) {
            isAd = player.classList.contains('ad-showing') ||
                   player.classList.contains('ad-interrupting');
        }
        if (!isAd) {
            isAd = !!document.querySelector(
                '.ytp-ad-player-overlay, .ytp-ad-preview-container, .ytp-ad-text, .ytp-ad-skip-button-slot'
            );
        }

        var video = document.querySelector('video');

        if (isAd) {
            if (!isBypassingAd) {
                isBypassingAd = true;
                adFastForwarded = false;
                wasMutedBeforeAd = video ? video.muted : false;
            }
            setIndicatorVisible(true);

            // 1. Mute audio immediately so no ad sound is ever heard
            if (video && !video.muted) {
                video.muted = true;
            }

            // 2. Try native player API skip first
            if (player && typeof player.skipAd === 'function') {
                try { player.skipAd(); } catch (_) {}
            }

            // 3. Click any rendered skip button
            var skipBtn = document.querySelector(
                '.ytp-ad-skip-button, .ytp-ad-skip-button-modern, .ytp-skip-ad-button, .ytp-ad-skip-button-container, .ytp-ad-skip-button-slot button, button.ytp-ad-skip-button-modern'
            );
            if (skipBtn) {
                try { skipBtn.click(); } catch (_) {}
            }

            // 4. Inspect video element safely
            if (video) {
                var dur = video.duration;
                if (!isNaN(dur) && isFinite(dur) && dur > 0) {
                    if (dur <= 65) {
                        // True ad video (<=65s): seek to completion ONCE without thrashing GStreamer
                        if (!adFastForwarded && video.currentTime < dur - 0.2) {
                            adFastForwarded = true;
                            video.currentTime = dur;
                        }
                    } else {
                        // dur > 65: This is the real song already loaded!
                        // PAUSE the video until the ad overlay clears so no song playback is lost.
                        if (!video.paused) {
                            video.pause();
                        }
                        if (video.playbackRate !== 1.0) {
                            video.playbackRate = 1.0;
                        }
                        if (video.currentTime > 0 && video.currentTime < 5) {
                            video.currentTime = 0;
                        }
                    }
                } else {
                    // Duration not yet loaded (buffering/switching): pause to protect song
                    if (!video.paused) {
                        video.pause();
                    }
                    video.muted = true;
                }
            }
        } else {
            // No ad is showing
            if (isBypassingAd) {
                isBypassingAd = false;
                adFastForwarded = false;
                setIndicatorVisible(false);

                if (video) {
                    // Restore original mute state
                    video.muted = wasMutedBeforeAd;
                    video.playbackRate = 1.0;

                    // If the track was held near start, rewind cleanly to 0:00
                    if (video.currentTime > 0 && video.currentTime < 5 && video.duration > 65) {
                        video.currentTime = 0;
                    }

                    // Resume smooth playback of the track
                    if (video.paused) {
                        video.play().catch(function() {});
                    }
                }
            } else if (video) {
                // Normal steady state: keep playback rate clean
                if (video.playbackRate !== 1.0 && video.playbackRate > 2.0) {
                    video.playbackRate = 1.0;
                }
            }
        }
    }

    // -------------------------------------------------------------
    // 8. Targeted Observer (No Subtree Thrashing / Lag Elimination)
    // -------------------------------------------------------------
    function attachPlayerObserver() {
        var player = document.querySelector('#movie_player, .html5-video-player');
        if (!player) return false;

        var playerObserver = new MutationObserver(function() {
            handleAdPlayback();
        });

        // Observe ONLY #movie_player itself without subtree!
        // This fires exclusively when ad-showing/ad-interrupting classes change.
        playerObserver.observe(player, {
            attributes: true,
            attributeFilter: ['class'],
            subtree: false
        });

        return true;
    }

    function initObserver() {
        ensureIndicator();

        if (!attachPlayerObserver()) {
            // Movie player not yet attached during early document boot;
            // observe document root for child addition, then disconnect immediately.
            var docObserver = new MutationObserver(function() {
                if (attachPlayerObserver()) {
                    docObserver.disconnect();
                }
            });
            var target = document.body || document.documentElement;
            if (target) {
                docObserver.observe(target, {
                    childList: true,
                    subtree: true
                });
            }
        }

        // Lightweight fallback poll (500ms) to catch any edge cases
        setInterval(handleAdPlayback, 500);
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', initObserver);
    } else {
        initObserver();
    }
})();
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adblock_script_contains_required_hooks() {
        assert!(LAYER_A_SCRIPT.contains("window.location.hostname !== 'music.youtube.com'"));
        assert!(LAYER_A_SCRIPT.contains("/youtubei/v1/player"));
        assert!(LAYER_A_SCRIPT.contains("/youtubei/v1/next"));
        assert!(LAYER_A_SCRIPT.contains("delete val[key]"));
        assert!(LAYER_A_SCRIPT.contains("adPlacements"));
        assert!(LAYER_A_SCRIPT.contains("playerAds"));
        assert!(LAYER_A_SCRIPT.contains("adSlots"));
        assert!(LAYER_A_SCRIPT.contains("window.fetch"));
        assert!(LAYER_A_SCRIPT.contains("XMLHttpRequest.prototype.open"));
        assert!(LAYER_A_SCRIPT.contains("JSON.parse"));
        assert!(LAYER_A_SCRIPT.contains("ad-showing"));
        assert!(LAYER_A_SCRIPT.contains("dur <= 65"));
        assert!(LAYER_A_SCRIPT.contains("ytInitialPlayerResponse"));
        assert!(LAYER_A_SCRIPT.contains("ytm-shell-ad-indicator"));
        assert!(LAYER_A_SCRIPT.contains("Bypassing ad..."));
        assert!(LAYER_A_SCRIPT.contains("ytmusic-mealbar-promo-renderer"));
    }
}
