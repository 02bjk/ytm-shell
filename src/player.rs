//! YouTube Music player DOM selectors, control scripts, and metadata extraction.
//! Consolidated into single constant structures for rapid maintenance.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

/// Play/Pause script: evaluates click on the primary player-bar play button,
/// falling back to video.paused ? play() : pause() if the button is absent.
pub const PLAY_PAUSE_SCRIPT: &str = r#"
    (function() {
        var btn = document.querySelector('#play-pause-button, .play-pause-button, tp-yt-paper-icon-button#play-pause-button');
        if (btn) {
            btn.click();
        } else {
            var video = document.querySelector('video');
            if (video) {
                if (video.paused) { video.play(); } else { video.pause(); }
            }
        }
    })();
"#;

/// Next track script: clicks the next track button in the player bar.
pub const NEXT_TRACK_SCRIPT: &str = r#"
    (function() {
        var btn = document.querySelector('.next-button, #next-button, button[aria-label="Next song"], button[title="Next song"]');
        if (btn) { btn.click(); }
    })();
"#;

/// Previous track script: clicks the previous track button in the player bar.
pub const PREV_TRACK_SCRIPT: &str = r#"
    (function() {
        var btn = document.querySelector('.previous-button, #previous-button, button[aria-label="Previous song"], button[title="Previous song"]');
        if (btn) { btn.click(); }
    })();
"#;

/// Metadata extraction script: pulls track details from navigator.mediaSession
/// and DOM player elements without granting remote IPC capabilities.
pub const METADATA_EXTRACT_SCRIPT: &str = r#"
    (function() {
        var meta = window.navigator && window.navigator.mediaSession && window.navigator.mediaSession.metadata;
        var video = document.querySelector('video');
        var titleEl = document.querySelector('.title.ytmusic-player-bar, ytmusic-player-bar .title');
        var bylineEl = document.querySelector('.byline.ytmusic-player-bar, ytmusic-player-bar .byline');
        var imgEl = document.querySelector('ytmusic-player-bar img, .image.ytmusic-player-bar');

        var title = (meta && meta.title) || (titleEl ? titleEl.innerText : '') || '';
        var artist = (meta && meta.artist) || (bylineEl ? bylineEl.innerText : '') || '';
        var album = (meta && meta.album) || '';
        var artUrl = '';
        if (meta && meta.artwork && meta.artwork.length > 0) {
            artUrl = meta.artwork[meta.artwork.length - 1].src || '';
        } else if (imgEl && imgEl.src) {
            artUrl = imgEl.src;
        }

        var isPaused = video ? video.paused : true;
        var duration = (video && !isNaN(video.duration) && isFinite(video.duration)) ? Math.floor(video.duration * 1000000) : 0;
        var position = (video && !isNaN(video.currentTime) && isFinite(video.currentTime)) ? Math.floor(video.currentTime * 1000000) : 0;

        return JSON.stringify({
            title: title.trim(),
            artist: artist.trim(),
            album: album.trim(),
            artUrl: artUrl,
            paused: isPaused,
            duration: duration,
            position: position
        });
    })()
"#;

/// Wipes DOM-level persistent client storage (localStorage, sessionStorage, IndexedDB).
pub const WIPE_DOM_STORAGE_SCRIPT: &str = r#"
    (function() {
        try { localStorage.clear(); } catch (_) {}
        try { sessionStorage.clear(); } catch (_) {}
        try {
            if (window.indexedDB && indexedDB.databases) {
                indexedDB.databases().then(function(dbs) {
                    dbs.forEach(function(db) {
                        if (db.name) { indexedDB.deleteDatabase(db.name); }
                    });
                });
            }
        } catch (_) {}
    })();
"#;

/// Thread-safe playback metadata representation for MPRIS / SMTC integration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrackMetadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    #[serde(rename = "artUrl", default)]
    pub art_url: String,
    #[serde(default = "default_paused")]
    pub paused: bool,
    #[serde(default)]
    pub duration: i64,
    #[serde(default)]
    pub position: i64,
}

fn default_paused() -> bool {
    true
}

/// Triggers Play/Pause toggle on the main webview.
pub fn play_pause<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.eval(PLAY_PAUSE_SCRIPT);
    }
}

/// Advances to the next track on the main webview.
pub fn next_track<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.eval(NEXT_TRACK_SCRIPT);
    }
}

/// Returns to the previous track on the main webview.
pub fn prev_track<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.eval(PREV_TRACK_SCRIPT);
    }
}

/// Spawns a background worker that periodically updates track metadata via native DOM query.
pub fn start_metadata_polling<R: Runtime>(
    app: AppHandle<R>,
    metadata: std::sync::Arc<std::sync::Mutex<TrackMetadata>>,
) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(1500));
        if let Some(window) = app.get_webview_window("main") {
            let meta_arc = metadata.clone();
            crate::platform::query_track_metadata(&window, move |new_meta| {
                if let Some(m) = new_meta {
                    let mut current = meta_arc.lock().unwrap();
                    if *current != m {
                        *current = m;
                    }
                }
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_scripts_defined() {
        assert!(PLAY_PAUSE_SCRIPT.contains("#play-pause-button"));
        assert!(PLAY_PAUSE_SCRIPT.contains("video.paused"));
        assert!(NEXT_TRACK_SCRIPT.contains(".next-button"));
        assert!(PREV_TRACK_SCRIPT.contains(".previous-button"));
        assert!(METADATA_EXTRACT_SCRIPT.contains("navigator.mediaSession"));
        assert!(WIPE_DOM_STORAGE_SCRIPT.contains("localStorage.clear()"));
        assert!(WIPE_DOM_STORAGE_SCRIPT.contains("sessionStorage.clear()"));
        assert!(WIPE_DOM_STORAGE_SCRIPT.contains("indexedDB.deleteDatabase"));
    }

    #[test]
    fn test_metadata_deserialization() {
        let json_data = r#"{
            "title": "Starboy",
            "artist": "The Weeknd ft. Daft Punk",
            "album": "Starboy",
            "artUrl": "https://lh3.googleusercontent.com/sample_art",
            "paused": false,
            "duration": 230000000,
            "position": 45000000
        }"#;

        let meta: TrackMetadata = serde_json::from_str(json_data).expect("valid metadata");
        assert_eq!(meta.title, "Starboy");
        assert_eq!(meta.artist, "The Weeknd ft. Daft Punk");
        assert_eq!(meta.album, "Starboy");
        assert_eq!(meta.art_url, "https://lh3.googleusercontent.com/sample_art");
        assert!(!meta.paused);
        assert_eq!(meta.duration, 230000000);
        assert_eq!(meta.position, 45000000);
    }

    #[test]
    fn test_metadata_deserialization_empty_defaults() {
        let json_data = r#"{
            "title": "",
            "artist": "",
            "album": ""
        }"#;

        let meta: TrackMetadata = serde_json::from_str(json_data).expect("valid empty metadata");
        assert_eq!(meta.title, "");
        assert!(meta.paused);
        assert_eq!(meta.duration, 0);
    }
}
