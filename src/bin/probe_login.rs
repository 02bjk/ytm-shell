// Probe script to test Google Login behavior on WebKitGTK with different User-Agents
use javascriptcore::ValueExt;
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use webkit2gtk::{LoadEvent, Settings, SettingsExt, WebView, WebViewExt};

fn test_ua(ua: Option<&str>, desc: &str) {
    println!("\n==========================================");
    println!("Testing: {}", desc);
    println!("UA: {:?}", ua.unwrap_or("Default WebKitGTK"));

    let _ = gtk::init();

    let settings = Settings::new();
    if let Some(user_agent) = ua {
        settings.set_user_agent(Some(user_agent));
    }
    settings.set_enable_javascript(true);

    let webview = WebView::with_settings(&settings);
    let done = Arc::new(AtomicBool::new(false));
    let done_clone = done.clone();
    let desc_str = desc.to_string();

    webview.connect_load_changed(move |wv, event| {
        if event == LoadEvent::Finished {
            let uri = wv.uri().map(|u| u.to_string()).unwrap_or_default();
            println!("[{}] Load Finished at URI: {}", desc_str, uri);

            let desc_eval = desc_str.clone();
            let done_inner = done_clone.clone();
            let wv_clone = wv.clone();

            // Give SPA 3 seconds to render form elements
            glib::timeout_add_seconds_local(3, move || {
                let desc_eval2 = desc_eval.clone();
                let done_inner2 = done_inner.clone();
                wv_clone.evaluate_javascript(
                    "JSON.stringify({ title: document.title, inputs: Array.from(document.querySelectorAll('input')).map(i => i.name || i.type || i.id), text: (document.body ? document.body.innerText.substring(0, 500).replace(/\\n+/g, ' ') : '') })",
                    None,
                    None,
                    None::<&gtk::gio::Cancellable>,
                    move |result: Result<javascriptcore::Value, glib::Error>| {
                        match result {
                            Ok(val) => {
                                let s = val.to_str();
                                println!("[{}] DOM Result: {}", desc_eval2, s);
                                if s.contains("Couldn't sign you in") || s.contains("browser or app may not be secure") {
                                    println!("[{}] RESULT: BLOCKED ('This browser or app may not be secure')", desc_eval2);
                                } else if s.contains("identifier") || s.contains("Email") || s.contains("Sign in") {
                                    println!("[{}] RESULT: ALLOWED (Email/Identifier input present)", desc_eval2);
                                } else {
                                    println!("[{}] RESULT: UNKNOWN ({})", desc_eval2, s);
                                }
                            }
                            Err(e) => println!("[{}] JS Eval Error: {:?}", desc_eval2, e),
                        }
                        done_inner2.store(true, Ordering::SeqCst);
                        gtk::main_quit();
                    },
                );
                glib::ControlFlow::Break
            });
        }
    });

    let target_url = "https://accounts.google.com/ServiceLogin?service=youtube&continue=https%3A%2F%2Fmusic.youtube.com%2F";
    webview.load_uri(target_url);

    // Timeout fallback after 20 seconds
    let done_timeout = done.clone();
    glib::timeout_add_seconds_local(20, move || {
        if !done_timeout.load(Ordering::SeqCst) {
            println!("Timeout reached for test");
            gtk::main_quit();
        }
        glib::ControlFlow::Break
    });

    gtk::main();
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("all");

    if mode == "all" || mode == "default" {
        test_ua(None, "1. Default WebKitGTK User-Agent");
    }
    if mode == "all" || mode == "chrome" {
        test_ua(
            Some("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36"),
            "2. Chrome on Linux User-Agent",
        );
    }
    if mode == "all" || mode == "firefox" {
        test_ua(
            Some("Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0"),
            "3. Firefox on Linux User-Agent",
        );
    }
}
