//! Platform-specific security hooks and webview integrations.
//! Isolated module housing any native OS/FFI bindings.

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "windows")]
pub use windows::*;

// Fallback for other platforms during cross-compilation checks
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub mod fallback {
    use std::fs;
    use std::path::Path;
    use tauri::{Runtime, WebviewWindow};

    pub fn apply_platform_hardening<R: Runtime>(_window: &WebviewWindow<R>) {}

    pub fn install_content_filter<R: Runtime>(
        _window: &WebviewWindow<R>,
        _filter_json: &str,
        _profile_dir: &Path,
    ) {
    }

    pub fn ensure_secure_directory(path: &Path) -> std::io::Result<()> {
        fs::create_dir_all(path)
    }

    pub fn open_in_browser(_url: &str) -> Result<(), String> {
        Ok(())
    }

    pub fn query_track_metadata<
        R: Runtime,
        F: FnOnce(Option<crate::player::TrackMetadata>) + Send + 'static,
    >(
        _window: &WebviewWindow<R>,
        callback: F,
    ) {
        callback(None);
    }
}
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub use fallback::*;
