#![allow(unsafe_code)]
//! Linux platform module: WebKitGTK 4.1 FFI hardening, POSIX 0700 permissions,
//! and secure browser launching. Isolated module for native WebKit/GTK FFI.

use glib::prelude::ObjectExt;
use std::fs::DirBuilder;
use std::os::unix::fs::DirBuilderExt;
use std::path::Path;
use tauri::{Runtime, WebviewWindow};
use webkit2gtk::{
    CookieManagerExt, PermissionRequestExt, SettingsExt, WebContextExt, WebViewExt,
    WebsiteDataManagerExt,
};

/// Applies WebKitGTK-specific security configurations and media streaming settings to the webview instance.
pub fn apply_platform_hardening<R: Runtime>(window: &WebviewWindow<R>) {
    let _ = window.with_webview(|webview| {
        let wv = webview.inner();

        // WebKitSettings hardening and media playback enablement
        if let Some(settings) = wv.settings() {
            // Disable WebRTC to prevent local and public IP leakage
            settings.set_enable_webrtc(false);
            // Disable developer tools in release builds
            settings.set_enable_developer_extras(cfg!(debug_assertions));
            // Disable DNS prefetching to avoid untracked lookups
            settings.set_enable_dns_prefetching(false);

            // GPU and Hardware Graphics/Video Acceleration
            let disable_gpu = std::env::args().any(|arg| arg == "--disable-gpu");
            let force_gpu = std::env::args().any(|arg| arg == "--force-gpu");

            if disable_gpu {
                settings.set_hardware_acceleration_policy(webkit2gtk::HardwareAccelerationPolicy::Never);
                settings.set_enable_webgl(false);
            } else if force_gpu {
                settings.set_hardware_acceleration_policy(webkit2gtk::HardwareAccelerationPolicy::Always);
                settings.set_enable_webgl(true);
            } else {
                // Default: OnDemand acceleration (activates GPU for video/compositing without wasting power)
                settings.set_hardware_acceleration_policy(webkit2gtk::HardwareAccelerationPolicy::OnDemand);
                settings.set_enable_webgl(true);
            }

            // Essential HTML5 media and streaming capabilities
            // Disable user gesture requirement so asynchronous player fetch & transitions do not get blocked
            settings.set_media_playback_requires_user_gesture(false);
            settings.set_media_playback_allows_inline(true);
            settings.set_enable_media(true);
            settings.set_enable_mediasource(true);
            settings.set_enable_webaudio(true);
            settings.set_enable_media_capabilities(true);
            settings.set_enable_encrypted_media(true);
            settings.set_enable_media_stream(true);
        }

        // In release builds, suppress the right-click context menu
        if !cfg!(debug_assertions) {
            wv.connect_context_menu(|_, _, _, _| true);
        }

        // Hook WebKitGTK permission-request: allow MediaKeySystem access for audio DRM, reject others
        wv.connect_permission_request(|_, request| {
            if request.is::<webkit2gtk::MediaKeySystemPermissionRequest>() {
                request.allow();
                true
            } else {
                request.deny();
                true
            }
        });
    });
}

/// Installs WebKitGTK native Content Rule List for Layer B request blocking.
pub fn install_content_filter<R: Runtime>(
    window: &WebviewWindow<R>,
    filter_json: &str,
    profile_dir: &Path,
) {
    use glib::translate::ToGlibPtr;
    use std::ffi::CString;

    let filter_dir = profile_dir.join("content_filters");
    let _ = std::fs::create_dir_all(&filter_dir);

    let c_dir = match CString::new(filter_dir.to_str().unwrap_or("/tmp")) {
        Ok(s) => s,
        Err(_) => return,
    };

    let filter_id = match CString::new("ytm_shell_layer_b") {
        Ok(s) => s,
        Err(_) => return,
    };

    let filter_bytes = glib::Bytes::from(filter_json.as_bytes());

    let _ = window.with_webview(move |webview| {
        let wv = webview.inner();
        let manager = match wv.user_content_manager() {
            Some(m) => m,
            None => return,
        };

        unsafe {
            let store = webkit2gtk::ffi::webkit_user_content_filter_store_new(c_dir.as_ptr());
            if store.is_null() {
                return;
            }

            struct FilterSaveContext {
                manager: *mut webkit2gtk::ffi::WebKitUserContentManager,
                store: *mut webkit2gtk::ffi::WebKitUserContentFilterStore,
            }

            let raw_manager = manager.to_glib_none().0;
            glib::gobject_ffi::g_object_ref(raw_manager as *mut _);

            let ctx = Box::into_raw(Box::new(FilterSaveContext {
                manager: raw_manager,
                store,
            }));

            unsafe extern "C" fn on_filter_saved(
                source_object: *mut glib::gobject_ffi::GObject,
                res: *mut gtk::gio::ffi::GAsyncResult,
                user_data: glib::ffi::gpointer,
            ) {
                let ctx = Box::from_raw(user_data as *mut FilterSaveContext);
                let mut err = std::ptr::null_mut();
                let filter = webkit2gtk::ffi::webkit_user_content_filter_store_save_finish(
                    source_object as *mut webkit2gtk::ffi::WebKitUserContentFilterStore,
                    res,
                    &mut err,
                );

                if !filter.is_null() {
                    webkit2gtk::ffi::webkit_user_content_manager_add_filter(ctx.manager, filter);
                    webkit2gtk::ffi::webkit_user_content_filter_unref(filter);
                } else if !err.is_null() {
                    let msg = std::ffi::CStr::from_ptr((*err).message).to_string_lossy();
                    eprintln!("[WebKit Filter Error] Content filter compilation failed: {msg}");
                    glib::ffi::g_error_free(err);
                }

                glib::gobject_ffi::g_object_unref(ctx.manager as *mut _);
                glib::gobject_ffi::g_object_unref(ctx.store as *mut _);
            }

            webkit2gtk::ffi::webkit_user_content_filter_store_save(
                store,
                filter_id.as_ptr(),
                filter_bytes.to_glib_none().0,
                std::ptr::null_mut(),
                Some(on_filter_saved),
                ctx as glib::ffi::gpointer,
            );
        }
    });
}

/// Safely queries current track metadata and playback state directly from WebKitGTK DOM.
pub fn query_track_metadata<
    R: Runtime,
    F: FnOnce(Option<crate::player::TrackMetadata>) + Send + 'static,
>(
    window: &WebviewWindow<R>,
    callback: F,
) {
    use javascriptcore::ValueExt;
    let _ = window.with_webview(|webview| {
        let wv = webview.inner();
        wv.evaluate_javascript(
            crate::player::METADATA_EXTRACT_SCRIPT,
            None,
            None,
            None::<&gtk::gio::Cancellable>,
            move |result| {
                let meta = match result {
                    Ok(val) => {
                        let json_str = val.to_str();
                        serde_json::from_str::<crate::player::TrackMetadata>(&json_str).ok()
                    }
                    Err(_) => None,
                };
                callback(meta);
            },
        );
    });
}

/// Clears native WebKitGTK cookies, memory cache, and network caches.
pub fn clear_webview_session<R: Runtime>(window: &WebviewWindow<R>) {
    let _ = window.with_webview(|webview| {
        let wv = webview.inner();
        if let Some(dm) = wv.website_data_manager() {
            if let Some(cm) = dm.cookie_manager() {
                #[allow(deprecated)]
                cm.delete_all_cookies();
            }
        }
        if let Some(ctx) = wv.web_context() {
            ctx.clear_cache();
        }
    });
}

/// Creates a directory with POSIX 0700 permissions (user read/write/execute only).
pub fn ensure_secure_directory(path: &Path) -> std::io::Result<()> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    builder.mode(0o700);
    builder.create(path)
}

/// Purges all cached session data, storage files, and cookies inside the profile dir.
pub fn wipe_profile_directory(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let p = entry.path();
            if p.is_dir() {
                let _ = std::fs::remove_dir_all(&p);
            } else {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
    ensure_secure_directory(path)
}

/// Safely launches an allowlisted HTTPS link in the user's default browser via GIO AppInfo.
pub fn open_in_browser(url: &str) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("Blocked non-HTTPS outbound URL".into());
    }
    gtk::gio::AppInfo::launch_default_for_uri(url, None::<&gtk::gio::AppLaunchContext>)
        .map_err(|e| format!("Failed to launch browser: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_secure_directory_and_wipe() {
        let temp_dir = std::env::temp_dir().join("ytm_shell_test_profile");
        let _ = fs::remove_dir_all(&temp_dir);

        // Ensure creation with 0700
        ensure_secure_directory(&temp_dir).expect("must create secure dir");
        assert!(temp_dir.exists());

        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::MetadataExt;
            let meta = fs::metadata(&temp_dir).unwrap();
            let mode = meta.mode() & 0o777;
            assert_eq!(mode, 0o700, "directory must be 0700");
        }

        // Create sample files and subdirs to test wipe
        let cookie_file = temp_dir.join("cookies.sqlite");
        fs::write(&cookie_file, b"fake_session_cookie").unwrap();
        let cache_subdir = temp_dir.join("cache");
        fs::create_dir(&cache_subdir).unwrap();
        fs::write(cache_subdir.join("cached_data.bin"), b"cached_bytes").unwrap();

        assert!(cookie_file.exists());
        assert!(cache_subdir.exists());

        // Perform wipe
        wipe_profile_directory(&temp_dir).expect("must wipe cleanly");

        // Target directory must still exist and be empty
        assert!(temp_dir.exists());
        assert!(!cookie_file.exists());
        assert!(!cache_subdir.exists());
        assert_eq!(fs::read_dir(&temp_dir).unwrap().count(), 0);

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_open_in_browser_scheme_check() {
        assert!(open_in_browser("http://evil.com").is_err());
        assert!(open_in_browser("file:///etc/passwd").is_err());
        assert!(open_in_browser("javascript:alert(1)").is_err());
    }
}
