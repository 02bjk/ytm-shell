//! Navigation allowlist validation.
//! Enforces exact host matching, strict HTTPS scheme, no userinfo credentials,
//! and standard port restrictions to prevent open redirects and SSRF/phishing.

use url::Url;

/// Default launch home URL for YouTube Music.
pub const DEFAULT_HOME_URL: &str = "https://music.youtube.com";

/// Exact host allowlist required for YouTube Music and Google authentication flows.
/// Note: `www.google.com` is intentionally NOT in this blanket allowlist to prevent
/// Google Search and web engine tracking from running inside the webview.
pub const ALLOWLISTED_HOSTS: &[&str] = &[
    "music.youtube.com",
    "accounts.google.com",
    "accounts.youtube.com",
    "consent.youtube.com",
    "recaptcha.net",
    "www.recaptcha.net",
];

/// Known ad, syndication, and telemetry domains that must be strictly blocked (never opened externally).
pub const BLOCKED_HOST_SUFFIXES: &[&str] = &[
    "googlesyndication.com",
    "doubleclick.net",
    "google-analytics.com",
    "googletagmanager.com",
    "adservice.google.com",
    "googleadservices.com",
];

/// Decision outcome for an evaluated URL.
#[derive(Debug, PartialEq, Eq)]
pub enum NavigationDecision {
    /// URL is allowlisted and may be loaded internally within the webview.
    AllowInternal,
    /// URL is valid HTTPS but not allowlisted; should be opened in system default browser.
    OpenExternal,
    /// URL is invalid, insecure (non-HTTPS), hostile, or ad/telemetry; must be rejected outright.
    Block,
}

/// Evaluates a URL against strict security rules for navigation.
pub fn evaluate_url(raw_url: &str) -> NavigationDecision {
    let parsed = match Url::parse(raw_url) {
        Ok(u) => u,
        Err(_) => return NavigationDecision::Block,
    };

    // Scheme must strictly be https (no http, file, javascript, data, or custom schemes)
    if parsed.scheme() != "https" {
        return NavigationDecision::Block;
    }

    // Disallow userinfo (e.g. user:pass@host) to mitigate credential spoofing tricks
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return NavigationDecision::Block;
    }

    // Enforce standard HTTPS port only (None or explicit 443)
    if let Some(port) = parsed.port() {
        if port != 443 {
            return NavigationDecision::Block;
        }
    }

    // Extract and validate host
    let host = match parsed.host_str() {
        Some(h) => h,
        None => return NavigationDecision::Block,
    };

    // 1. Block known ad, syndication, and tracking domains from opening anywhere
    let is_ad_or_tracker = BLOCKED_HOST_SUFFIXES.iter().any(|&suffix| {
        host.eq_ignore_ascii_case(suffix)
            || host.to_ascii_lowercase().ends_with(&format!(".{suffix}"))
    });
    if is_ad_or_tracker {
        return NavigationDecision::Block;
    }

    // 2. Block ad-specific and telemetry paths even on first-party domains
    let path = parsed.path().to_ascii_lowercase();
    if path.starts_with("/pagead/")
        || path.starts_with("/sodar/")
        || path.contains("/api/stats/ads")
    {
        return NavigationDecision::Block;
    }

    // 3. Surgical path-level exception for reCAPTCHA on www.google.com:
    // We intentionally reject general www.google.com (search, tracking, history)
    // and ONLY permit the dedicated /recaptcha/ endpoint for login bot challenges.
    if host.eq_ignore_ascii_case("www.google.com") {
        if path.starts_with("/recaptcha/") {
            return NavigationDecision::AllowInternal;
        } else {
            return NavigationDecision::OpenExternal;
        }
    }

    // 4. Exact host match against allowlist
    let is_allowlisted = ALLOWLISTED_HOSTS
        .iter()
        .any(|&allowed| host.eq_ignore_ascii_case(allowed));

    if is_allowlisted {
        return NavigationDecision::AllowInternal;
    }

    // 5. Safe external navigation: valid external https URL intended for external browser
    NavigationDecision::OpenExternal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allowlisted_hosts() {
        assert_eq!(
            evaluate_url("https://music.youtube.com/"),
            NavigationDecision::AllowInternal
        );
        assert_eq!(
            evaluate_url("https://music.youtube.com:443/explore"),
            NavigationDecision::AllowInternal
        );
        assert_eq!(
            evaluate_url("https://accounts.google.com/ServiceLogin"),
            NavigationDecision::AllowInternal
        );
        assert_eq!(
            evaluate_url("https://accounts.youtube.com/accounts/SetSID"),
            NavigationDecision::AllowInternal
        );
        assert_eq!(
            evaluate_url("https://consent.youtube.com/m?continue=https%3A%2F%2Fmusic.youtube.com"),
            NavigationDecision::AllowInternal
        );
        assert_eq!(
            evaluate_url("https://recaptcha.net/recaptcha/api.js"),
            NavigationDecision::AllowInternal
        );
    }

    #[test]
    fn test_google_search_vs_recaptcha() {
        // www.google.com/recaptcha is strictly allowed for login verification
        assert_eq!(
            evaluate_url("https://www.google.com/recaptcha/api2/anchor"),
            NavigationDecision::AllowInternal
        );
        // www.google.com search or root is NOT allowed internally; opens in external browser
        assert_eq!(
            evaluate_url("https://www.google.com/search?q=youtube+music"),
            NavigationDecision::OpenExternal
        );
        assert_eq!(
            evaluate_url("https://www.google.com/"),
            NavigationDecision::OpenExternal
        );
    }

    #[test]
    fn test_ad_and_syndication_blocking() {
        // Ads and syndication should be blocked, never opened in external browser
        assert_eq!(
            evaluate_url("https://tpc.googlesyndication.com/sodar/5k7CCto5.html"),
            NavigationDecision::Block
        );
        assert_eq!(
            evaluate_url("https://googleads.g.doubleclick.net/pagead/ads"),
            NavigationDecision::Block
        );
        assert_eq!(
            evaluate_url("https://www.google-analytics.com/analytics.js"),
            NavigationDecision::Block
        );
        assert_eq!(
            evaluate_url("https://www.googletagmanager.com/gtm.js"),
            NavigationDecision::Block
        );
        assert_eq!(
            evaluate_url("https://music.youtube.com/api/stats/ads"),
            NavigationDecision::Block
        );
    }

    #[test]
    fn test_case_insensitivity_for_allowlist() {
        assert_eq!(
            evaluate_url("https://MUSIC.YOUTUBE.COM/"),
            NavigationDecision::AllowInternal
        );
        assert_eq!(
            evaluate_url("https://Accounts.Google.Com/"),
            NavigationDecision::AllowInternal
        );
    }

    #[test]
    fn test_subdomain_spoofing_attempts() {
        // Attackers appending malicious domain suffix
        assert_eq!(
            evaluate_url("https://music.youtube.com.evil.com/"),
            NavigationDecision::OpenExternal
        );
        assert_eq!(
            evaluate_url("https://accounts.google.com.attacker.org/login"),
            NavigationDecision::OpenExternal
        );
    }

    #[test]
    fn test_userinfo_tricks() {
        // Userinfo trick to fool naive host parsers
        assert_eq!(
            evaluate_url("https://evil.com@music.youtube.com/"),
            NavigationDecision::Block
        );
        assert_eq!(
            evaluate_url("https://music.youtube.com@evil.com/"),
            NavigationDecision::Block
        );
        assert_eq!(
            evaluate_url("https://user:pass@music.youtube.com/"),
            NavigationDecision::Block
        );
    }

    #[test]
    fn test_insecure_and_non_https_schemes() {
        assert_eq!(
            evaluate_url("http://music.youtube.com/"),
            NavigationDecision::Block
        );
        assert_eq!(
            evaluate_url("file:///etc/passwd"),
            NavigationDecision::Block
        );
        assert_eq!(
            evaluate_url("javascript:alert(document.cookie)"),
            NavigationDecision::Block
        );
        assert_eq!(
            evaluate_url("data:text/html,<script>alert(1)</script>"),
            NavigationDecision::Block
        );
        assert_eq!(
            evaluate_url("blob:https://music.youtube.com/uuid"),
            NavigationDecision::Block
        );
    }

    #[test]
    fn test_port_restrictions() {
        // Non-standard HTTPS port should be blocked
        assert_eq!(
            evaluate_url("https://music.youtube.com:8443/"),
            NavigationDecision::Block
        );
        assert_eq!(
            evaluate_url("https://evil.com:8080/"),
            NavigationDecision::Block
        );
    }

    #[test]
    fn test_punycode_and_homographs() {
        // Valid punycode homograph (Cyrillic 'о' in google: xn--ggle-p50a.com)
        assert_eq!(
            evaluate_url("https://xn--ggle-p50a.com/"),
            NavigationDecision::OpenExternal
        );
        // Invalid punycode sequence fails URL parsing and is blocked
        assert_eq!(
            evaluate_url("https://xn--ccounts-97a.google.com/"),
            NavigationDecision::Block
        );
    }

    #[test]
    fn test_outbound_links() {
        // External safe links open in default browser
        assert_eq!(
            evaluate_url("https://support.google.com/youtube"),
            NavigationDecision::OpenExternal
        );
        assert_eq!(
            evaluate_url("https://github.com/tauri-apps/tauri"),
            NavigationDecision::OpenExternal
        );
    }

    #[test]
    fn test_malformed_urls() {
        assert_eq!(evaluate_url("not a url"), NavigationDecision::Block);
        assert_eq!(evaluate_url("https://"), NavigationDecision::Block);
        assert_eq!(evaluate_url(""), NavigationDecision::Block);
    }
}
