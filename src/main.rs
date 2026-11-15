#![deny(unsafe_code)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod adblock;
pub mod allowlist;
#[cfg(target_os = "linux")]
pub mod mpris;
pub mod network_block;
pub mod platform;
pub mod player;
pub mod shortcuts;
pub mod tray;

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

/// Baseline security script injected at document-start before any remote scripts run.
const SECURITY_INIT_SCRIPT: &str = r#"
(function() {
    'use strict';
    // Suppress context menu in release builds
    window.addEventListener('contextmenu', function(e) {
        if (!window.__YTM_DEBUG__) {
            e.preventDefault();
        }
    }, true);

    // Enforce strict referrer policy at DOM level
    try {
        var meta = document.createElement('meta');
        meta.name = 'referrer';
        meta.content = 'strict-origin-when-cross-origin';
        (document.head || document.documentElement).appendChild(meta);
    } catch (_) {}

    // Disable file drop into webview window
    window.addEventListener('dragover', function(e) { e.preventDefault(); }, false);
    window.addEventListener('drop', function(e) { e.preventDefault(); }, false);
})();
"#;

fn print_help() {
    println!(
        "ytm-shell (YouTube Music Desktop) - Hardened Security Shell\n\
         Usage: ytm-shell [FLAGS]\n\n\
         Flags:\n\
           --minimized         Launch directly to system tray in background\n\
           --disable-gpu       Disable hardware GPU acceleration (force software rasterization)\n\
           --force-gpu         Force permanent GPU acceleration\n\
           --allow-unfiltered  Disable network adblocking (debugging only)\n\
           --uninstall         Completely uninstall ytm-shell and wipe all leftover data\n\
           -h, --help          Print this help message"
    );
}

fn perform_cli_uninstall() {
    println!("==> Performing complete uninstallation of YTM Shell...");

    // 1. Data directories to purge
    let mut dirs_to_remove = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        let home_p = std::path::PathBuf::from(home);
        dirs_to_remove.push(home_p.join(".local/share/com.ytm.shell"));
        dirs_to_remove.push(home_p.join(".config/com.ytm.shell"));
        dirs_to_remove.push(home_p.join(".cache/com.ytm.shell"));
        dirs_to_remove.push(home_p.join(".local/share/ytm-shell"));
        dirs_to_remove.push(home_p.join(".config/ytm-shell"));
        dirs_to_remove.push(home_p.join(".cache/ytm-shell"));
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        let appdata_p = std::path::PathBuf::from(appdata);
        dirs_to_remove.push(appdata_p.join("com.ytm.shell"));
        dirs_to_remove.push(appdata_p.join("ytm-shell"));
    }
    if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
        let localappdata_p = std::path::PathBuf::from(localappdata);
        dirs_to_remove.push(localappdata_p.join("com.ytm.shell"));
        dirs_to_remove.push(localappdata_p.join("ytm-shell"));
        dirs_to_remove.push(localappdata_p.join("YouTube Music"));
    }

    for dir in &dirs_to_remove {
        if dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(dir) {
                eprintln!("    [!] Could not remove {}: {e}", dir.display());
            } else {
                println!("    [-] Purged data: {}", dir.display());
            }
        }
    }

    // 2. Desktop entries & icons (Linux)
    #[cfg(target_os = "linux")]
    {
        if let Ok(home) = std::env::var("HOME") {
            let home_p = std::path::PathBuf::from(home);
            let desktop_file = home_p.join(".local/share/applications/ytm-shell.desktop");
            let icon_file = home_p.join(".local/share/icons/hicolor/128x128/apps/ytm-shell.png");
            let uninstaller_file = home_p.join(".local/bin/ytm-shell-uninstall");

            for file in &[desktop_file, icon_file, uninstaller_file] {
                if file.exists() {
                    let _ = std::fs::remove_file(file);
                    println!("    [-] Removed: {}", file.display());
                }
            }

            let _ = std::process::Command::new("update-desktop-database")
                .arg(home_p.join(".local/share/applications"))
                .status();
        }
    }

    // 3. Remove own binary
    if let Ok(exe_path) = std::env::current_exe() {
        if exe_path.exists() {
            println!("    [-] Removing binary: {}", exe_path.display());
            #[cfg(target_os = "windows")]
            {
                let _ = std::process::Command::new("cmd")
                    .args([
                        "/c", "timeout", "/t", "1", "/nobreak", ">nul", "&", "del", "/f", "/q",
                    ])
                    .arg(&exe_path)
                    .spawn();
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = std::fs::remove_file(&exe_path);
            }
        }
    }

    println!("==> YTM Shell and all leftover files have been completely uninstalled.");
    std::process::exit(0);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_help();
        return;
    }

    if args.iter().any(|arg| arg == "--uninstall") {
        perform_cli_uninstall();
        return;
    }

    let is_minimized = args.iter().any(|arg| arg == "--minimized");
    let allow_unfiltered = args.iter().any(|arg| arg == "--allow-unfiltered");

    tauri::Builder::default()
        // Single instance plugin: refocus existing window unless launched with --minimized
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let has_minimized = args.iter().any(|arg| arg == "--minimized");
            if !has_minimized {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }
        }))
        // Close-to-tray: intercept window close event to hide window while audio keeps playing
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(move |app| {
            // Persistent isolated profile directory under app data
            let app_data_dir = app.path().app_data_dir().expect("valid app data dir");
            let profile_dir = app_data_dir.join("profile");
            platform::ensure_secure_directory(&profile_dir)
                .expect("failed to secure profile directory");

            // Initialize Layer B & C network blocking engine
            let blocker = match network_block::NetworkBlocker::init(&profile_dir) {
                Ok(b) => Some(b),
                Err(err) => {
                    if allow_unfiltered {
                        eprintln!("[SECURITY WARNING] Adblock engine initialization failed ({err}), proceeding due to --allow-unfiltered");
                        None
                    } else {
                        eprintln!("[SECURITY ERROR] Adblock engine initialization failed ({err}). Halting external navigation (fail-closed).");
                        None
                    }
                }
            };

            let initial_url = match &blocker {
                Some(_) => WebviewUrl::External(
                    allowlist::DEFAULT_HOME_URL
                        .parse()
                        .expect("valid initial URL"),
                ),
                None if allow_unfiltered => WebviewUrl::External(
                    allowlist::DEFAULT_HOME_URL
                        .parse()
                        .expect("valid initial URL"),
                ),
                None => WebviewUrl::App("error.html".into()),
            };

            let nav_blocker = blocker.clone();

            // Construct main window in Rust with strict security flags
            let window = WebviewWindowBuilder::new(app, "main", initial_url)
                .title("YouTube Music")
                .inner_size(1200.0, 800.0)
                .min_inner_size(800.0, 500.0)
                .center()
                .visible(!is_minimized)
                .devtools(cfg!(debug_assertions))
                .disable_drag_drop_handler()
                .data_directory(profile_dir.clone())
                .initialization_script(SECURITY_INIT_SCRIPT)
                .initialization_script(adblock::LAYER_A_SCRIPT)
                // Strict navigation allowlist & Layer B check
                .on_navigation(move |url| {
                    if let Some(ref b) = nav_blocker {
                        if b.check_network_request(
                            url.as_str(),
                            "https://music.youtube.com/",
                            "main_frame",
                            "GET",
                        ) {
                            return false;
                        }
                    }
                    match allowlist::evaluate_url(url.as_str()) {
                        allowlist::NavigationDecision::AllowInternal => true,
                        allowlist::NavigationDecision::OpenExternal => {
                            let _ = platform::open_in_browser(url.as_str());
                            false
                        }
                        allowlist::NavigationDecision::Block => false,
                    }
                })
                // Deny unvetted popups; allow only allowlisted login navigation
                .on_new_window(
                    move |url, _features| match allowlist::evaluate_url(url.as_str()) {
                        allowlist::NavigationDecision::AllowInternal => {
                            tauri::webview::NewWindowResponse::Allow
                        }
                        allowlist::NavigationDecision::OpenExternal => {
                            let _ = platform::open_in_browser(url.as_str());
                            tauri::webview::NewWindowResponse::Deny
                        }
                        allowlist::NavigationDecision::Block => {
                            tauri::webview::NewWindowResponse::Deny
                        }
                    },
                )
                // Block all downloads
                .on_download(|_webview, _event| false)
                // Deny unvetted web permissions; allow MediaKeySystemAccess for audio DRM streams
                .on_permission_request(|_webview, kind| match kind {
                    tauri::webview::PermissionKind::MediaKeySystemAccess => {
                        tauri::webview::PermissionResponse::Allow
                    }
                    _ => tauri::webview::PermissionResponse::Deny,
                })
                .build()?;

            // Apply native OS/webview security hardening
            platform::apply_platform_hardening(&window);

            // Install WebKitGTK native Content Rule List filter for Layer B request blocking
            if let Some(ref b) = blocker {
                if let Ok(filter_json) = network_block::NetworkBlocker::to_webkit_content_rule_list(
                    network_block::CURATED_RULES,
                ) {
                    platform::install_content_filter(&window, &filter_json, &profile_dir);
                }

                // Spawn Layer C background updater
                network_block::ListUpdater::spawn_if_due(profile_dir.clone(), None);

                // Register blocker in app state for metrics and debugging
                app.manage(b.clone());
            }

            // Initialize system tray icon and menu
            tray::setup_tray(app.handle(), profile_dir)?;

            // Shared playback metadata for MPRIS, tray, and desktop integration
            let metadata =
                std::sync::Arc::new(std::sync::Mutex::new(player::TrackMetadata::default()));

            // Register global media keys and keyboard fallbacks
            shortcuts::setup_shortcuts(app.handle());

            // Linux MPRIS D-Bus interface (org.mpris.MediaPlayer2.ytm_shell)
            #[cfg(target_os = "linux")]
            mpris::start_mpris_service(app.handle().clone(), metadata.clone());

            // Start background DOM metadata polling
            player::start_metadata_polling(app.handle().clone(), metadata);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running ytm-shell application");
}
