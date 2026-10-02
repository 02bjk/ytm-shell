//! Global shortcuts for hardware media keys (Play/Pause, Next, Prev)
//! and fallback combinations (Control+Alt+Space, Control+Alt+Right, Control+Alt+Left).

use crate::player;
use tauri::{AppHandle, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Registers global media shortcuts with non-fatal fallbacks for Wayland/X11/Windows.
pub fn setup_shortcuts<R: Runtime>(app: &AppHandle<R>) {
    let play_pause_hotkey: Option<Shortcut> = "MediaPlayPause".parse().ok();
    let next_hotkey: Option<Shortcut> = "MediaTrackNext".parse().ok();
    let prev_hotkey: Option<Shortcut> = "MediaTrackPrevious".parse().ok();
    let alt_space_hotkey: Option<Shortcut> = "Control+Alt+Space".parse().ok();
    let alt_right_hotkey: Option<Shortcut> = "Control+Alt+Right".parse().ok();
    let alt_left_hotkey: Option<Shortcut> = "Control+Alt+Left".parse().ok();

    let plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(move |app_handle, hotkey, event| {
            if event.state() == ShortcutState::Pressed {
                let id = hotkey.id();
                if play_pause_hotkey.as_ref().map(|k| k.id()) == Some(id)
                    || alt_space_hotkey.as_ref().map(|k| k.id()) == Some(id)
                {
                    player::play_pause(app_handle);
                } else if next_hotkey.as_ref().map(|k| k.id()) == Some(id)
                    || alt_right_hotkey.as_ref().map(|k| k.id()) == Some(id)
                {
                    player::next_track(app_handle);
                } else if prev_hotkey.as_ref().map(|k| k.id()) == Some(id)
                    || alt_left_hotkey.as_ref().map(|k| k.id()) == Some(id)
                {
                    player::prev_track(app_handle);
                }
            }
        })
        .build();

    let _ = app.plugin(plugin);

    let global_shortcut = app.global_shortcut();
    for key_str in &[
        "MediaPlayPause",
        "MediaTrackNext",
        "MediaTrackPrevious",
        "Control+Alt+Space",
        "Control+Alt+Right",
        "Control+Alt+Left",
    ] {
        if let Ok(shortcut) = key_str.parse::<Shortcut>() {
            if let Err(e) = global_shortcut.register(shortcut) {
                // Non-fatal: hardware media keys are managed natively by MPRIS D-Bus on Linux
                // and may be claimed exclusively by the desktop environment/Wayland compositor.
                if cfg!(debug_assertions) && !key_str.starts_with("Media") {
                    eprintln!("Optional global shortcut '{key_str}' not bound: {e}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shortcut_strings_parseable() {
        assert!("MediaPlayPause".parse::<Shortcut>().is_ok());
        assert!("MediaTrackNext".parse::<Shortcut>().is_ok());
        assert!("MediaTrackPrevious".parse::<Shortcut>().is_ok());
        assert!("Control+Alt+Space".parse::<Shortcut>().is_ok());
        assert!("Control+Alt+Right".parse::<Shortcut>().is_ok());
        assert!("Control+Alt+Left".parse::<Shortcut>().is_ok());
    }
}
