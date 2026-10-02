//! System tray implementation: menu construction, event routing, and data wiping.

use crate::player;
use std::path::PathBuf;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Runtime};

/// Embedded 32x32 PNG icon for the system tray (baked into binary at compile time).
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../icons/32x32.png");

/// Initializes the system tray with navigation, playback controls, and lifecycle events.
pub fn setup_tray<R: Runtime>(app: &AppHandle<R>, profile_dir: PathBuf) -> tauri::Result<()> {
    // Build menu items
    let open_item = MenuItem::with_id(app, "open", "Open YouTube Music", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let play_pause_item = MenuItem::with_id(app, "play_pause", "Play / Pause", true, None::<&str>)?;
    let next_track_item = MenuItem::with_id(app, "next_track", "Next Track", true, None::<&str>)?;
    let prev_track_item =
        MenuItem::with_id(app, "prev_track", "Previous Track", true, None::<&str>)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let wipe_item =
        MenuItem::with_id(app, "wipe_data", "Sign out & Wipe Data", true, None::<&str>)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &open_item,
            &sep1,
            &play_pause_item,
            &next_track_item,
            &prev_track_item,
            &sep2,
            &wipe_item,
            &sep3,
            &quit_item,
        ],
    )?;

    let icon = Image::from_bytes(TRAY_ICON_BYTES)?;

    let _tray = TrayIconBuilder::with_id("ytm-tray")
        .icon(icon)
        .tooltip("YouTube Music")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| {
            handle_tray_event(app, event.id.as_ref(), &profile_dir);
        })
        .build(app)?;

    Ok(())
}

/// Dispatches tray menu clicks.
fn handle_tray_event<R: Runtime>(app: &AppHandle<R>, item_id: &str, profile_dir: &std::path::Path) {
    match item_id {
        "open" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }
        "play_pause" => {
            player::play_pause(app);
        }
        "next_track" => {
            player::next_track(app);
        }
        "prev_track" => {
            player::prev_track(app);
        }
        "wipe_data" => {
            wipe_all_data(app, profile_dir);
        }
        "quit" => {
            app.exit(0);
        }
        _ => {}
    }
}

/// Fully signs out and wipes all browsing data, cookies, and stored files.
pub fn wipe_all_data<R: Runtime>(app: &AppHandle<R>, profile_dir: &std::path::Path) {
    if let Some(window) = app.get_webview_window("main") {
        // 1. Wipe DOM-level storage (localStorage, sessionStorage, IndexedDB)
        let _ = window.eval(player::WIPE_DOM_STORAGE_SCRIPT);

        // 2. Native webview cookie and cache purge
        crate::platform::clear_webview_session(&window);

        // 3. Filesystem profile directory purge
        let _ = crate::platform::wipe_profile_directory(profile_dir);

        // 4. Reload clean sign-in screen and refocus window
        let url = "https://music.youtube.com".parse().expect("valid URL");
        let _ = window.navigate(url);
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
