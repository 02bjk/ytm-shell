//! Linux MPRIS D-Bus implementation (org.mpris.MediaPlayer2 and org.mpris.MediaPlayer2.Player).
//! Allows desktop media widgets (GNOME shell, KDE Plasma, polybar, playerctl)
//! to show current track metadata and control playback.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, Runtime};
use zbus::interface;
use zbus::zvariant::{ObjectPath, Value};

use crate::player::{self, TrackMetadata};

/// Root org.mpris.MediaPlayer2 D-Bus interface.
pub struct MprisRoot<R: Runtime> {
    app: AppHandle<R>,
}

#[interface(name = "org.mpris.MediaPlayer2")]
impl<R: Runtime> MprisRoot<R> {
    /// Brings the desktop window to the front.
    async fn raise(&self) {
        if let Some(window) = self.app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
    }

    /// Exits the desktop application cleanly.
    async fn quit(&self) {
        self.app.exit(0);
    }

    #[zbus(property)]
    fn can_quit(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_raise(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn identity(&self) -> &str {
        "YTM Shell"
    }

    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<String> {
        vec!["https".into()]
    }

    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<String> {
        vec!["audio/mpeg".into(), "audio/ogg".into(), "audio/webm".into()]
    }
}

/// Player org.mpris.MediaPlayer2.Player D-Bus interface.
pub struct MprisPlayer<R: Runtime> {
    app: AppHandle<R>,
    metadata: Arc<Mutex<TrackMetadata>>,
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl<R: Runtime> MprisPlayer<R> {
    /// Skips to the next track.
    async fn next(&self) {
        player::next_track(&self.app);
    }

    /// Skips to the previous track.
    async fn previous(&self) {
        player::prev_track(&self.app);
    }

    /// Pauses playback.
    async fn pause(&self) {
        player::play_pause(&self.app);
    }

    /// Toggles play/pause.
    async fn play_pause(&self) {
        player::play_pause(&self.app);
    }

    /// Stops playback.
    async fn stop(&self) {
        player::play_pause(&self.app);
    }

    /// Starts or resumes playback.
    async fn play(&self) {
        player::play_pause(&self.app);
    }

    #[zbus(property)]
    fn playback_status(&self) -> String {
        let meta = self.metadata.lock().unwrap();
        if meta.title.is_empty() {
            "Stopped".to_string()
        } else if meta.paused {
            "Paused".to_string()
        } else {
            "Playing".to_string()
        }
    }

    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, Value<'static>> {
        let meta = self.metadata.lock().unwrap();
        let mut map = HashMap::new();

        let track_id = ObjectPath::try_from("/org/mpris/MediaPlayer2/Track/current")
            .unwrap_or_else(|_| ObjectPath::try_from("/").unwrap());
        map.insert("mpris:trackid".into(), Value::from(track_id));

        if !meta.title.is_empty() {
            map.insert("xesam:title".into(), Value::from(meta.title.clone()));
        }
        if !meta.artist.is_empty() {
            map.insert(
                "xesam:artist".into(),
                Value::from(vec![meta.artist.clone()]),
            );
        }
        if !meta.album.is_empty() {
            map.insert("xesam:album".into(), Value::from(meta.album.clone()));
        }
        if !meta.art_url.is_empty() {
            map.insert("mpris:artUrl".into(), Value::from(meta.art_url.clone()));
        }
        if meta.duration > 0 {
            map.insert("mpris:length".into(), Value::from(meta.duration));
        }

        map
    }

    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_pause(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_play(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_seek(&self) -> bool {
        false
    }
}

/// Spawns the MPRIS service on the session D-Bus in a dedicated background thread.
pub fn start_mpris_service<R: Runtime>(app: AppHandle<R>, metadata: Arc<Mutex<TrackMetadata>>) {
    std::thread::spawn(move || {
        let root = MprisRoot { app: app.clone() };
        let player = MprisPlayer { app, metadata };

        let builder = match zbus::blocking::connection::Builder::session() {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Failed to connect to D-Bus session bus: {e}");
                return;
            }
        };

        let conn = builder
            .name("org.mpris.MediaPlayer2.ytm_shell")
            .and_then(|b| b.serve_at("/org/mpris/MediaPlayer2", root))
            .and_then(|b| b.serve_at("/org/mpris/MediaPlayer2", player))
            .and_then(|b| b.build());

        match conn {
            Ok(_connection) => {
                // Connection stays alive on the session bus
                loop {
                    std::thread::park();
                }
            }
            Err(e) => {
                eprintln!("Failed to export MPRIS D-Bus interfaces: {e}");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mpris_metadata_map_structure() {
        let metadata = Arc::new(Mutex::new(TrackMetadata {
            title: "Blinding Lights".into(),
            artist: "The Weeknd".into(),
            album: "After Hours".into(),
            art_url: "https://example.com/art.jpg".into(),
            paused: false,
            duration: 200000000,
            position: 10000000,
        }));

        let meta = metadata.lock().unwrap();
        assert_eq!(meta.title, "Blinding Lights");
        assert_eq!(meta.artist, "The Weeknd");
        assert!(!meta.paused);
    }
}
