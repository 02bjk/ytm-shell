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

fn main() {
    // Parse CLI flags with std::env::args (no external CLI parser)
    let is_minimized = std::env::args().any(|arg| arg == "--minimized");
    let allow_unfiltered = std::env::args().any(|arg| arg == "--allow-unfiltered");

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
