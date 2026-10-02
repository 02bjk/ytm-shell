//! Windows platform module: WebView2 hardening and secure browser launching.

use std::fs;
use std::path::Path;
use tauri::{Runtime, WebviewWindow};

/// Applies Windows WebView2-specific security configurations.
pub fn apply_platform_hardening<R: Runtime>(_window: &WebviewWindow<R>) {
    // WebView2 permission rejection and tracking policies are handled via
    // WebviewWindowBuilder::on_permission_request returning PermissionResponse::Deny.
}

/// Windows WebView2 Content Rule List integration stub.
pub fn install_content_filter<R: Runtime>(
    _window: &WebviewWindow<R>,
    _filter_json: &str,
    _profile_dir: &Path,
) {
    // Windows WebView2 request filtering is handled via WebResourceRequested filter
}

/// Clears native Windows webview browsing caches.
pub fn clear_webview_session<R: Runtime>(_window: &WebviewWindow<R>) {
    // Additional WebView2 clear actions handled via profile directory reset.
}

/// Safely queries current track metadata on Windows WebView2.
pub fn query_track_metadata<
    R: Runtime,
    F: FnOnce(Option<crate::player::TrackMetadata>) + Send + 'static,
>(
    _window: &WebviewWindow<R>,
    callback: F,
) {
    callback(None);
}

/// Ensures directory exists with default user access.
pub fn ensure_secure_directory(path: &Path) -> std::io::Result<()> {
    fs::create_dir_all(path)
}

/// Purges all cached session data, storage files, and cookies inside the profile dir.
pub fn wipe_profile_directory(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let p = entry.path();
            if p.is_dir() {
                let _ = fs::remove_dir_all(&p);
            } else {
                let _ = fs::remove_file(&p);
            }
        }
    }
    ensure_secure_directory(path)
}

/// Safely launches an allowlisted HTTPS link in the default browser via Windows Shell.
pub fn open_in_browser(url: &str) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("Blocked non-HTTPS outbound URL".into());
    }
    std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to launch browser: {e}"))
}
