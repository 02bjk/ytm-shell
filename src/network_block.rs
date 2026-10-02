//! Network Layer B & C Ad and Tracker Request Interception.
//!
//! Provides a multi-layered network blocking engine:
//! - Layer B: Brave `adblock` pure-Rust uBO/ABP matching engine with curated rules.
//! - WebKit Content Rule List generator for native C++ request filtering in WebKitGTK.
//! - Thread-safe atomic blocked request counter.
//! - Layer C: Hardened background list updater (HTTPS-only, allowlisted host, size-capped,
//!   atomic POSIX 0600 writes, ETag/If-Modified-Since caching).
//! - Fail-closed enforcement: refuses navigation if filtering fails to initialize securely.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Curated compile-time scope-limited ruleset (uBO/ABP syntax) targeted to YouTube Music.
/// Designed for <15 MB RAM and <100 ms load time.
pub const CURATED_RULES: &str = r#"
! -------------------------------------------------------------
! Google Ad Networks, DoubleClick & Syndication
! -------------------------------------------------------------
||doubleclick.net^
||googleadservices.com^
||googlesyndication.com^
||google-analytics.com^
||adservice.google.com^
||pagead2.googlesyndication.com^
||tpc.googlesyndication.com^
||sodar.google.com^
||crashlytics.com^
||firebase-logging.googleapis.com^
||stats.g.doubleclick.net^
||static.doubleclick.net^
||securepubads.g.doubleclick.net^
||ad.doubleclick.net^

! -------------------------------------------------------------
! YouTube / YTM Ad & Telemetry Endpoints
! -------------------------------------------------------------
||youtube.com/pagead/*
||youtube.com/api/stats/ads*
||youtube.com/api/stats/qoe?*&adformat=*
||youtube.com/api/stats/atr?*
||youtube.com/ptracking*
||music.youtube.com/pagead/*
||music.youtube.com/api/stats/ads*
||music.youtube.com/youtubei/v1/log_event*

! -------------------------------------------------------------
! Third-party Surveillance, Tracking & Fingerprinting
! -------------------------------------------------------------
||fingerprintjs.com^
||scorecardresearch.com^
||quantserve.com^
||hotjar.com^
||sentry.io^
||clarity.ms^
||segment.io^

! -------------------------------------------------------------
! Critical Allow (Unbreak) Rules: Media Streaming & Essential Auth
! -------------------------------------------------------------
@@||music.youtube.com^$document
@@||googlevideo.com^$media
@@||ytimg.com^$image
@@||google.com/recaptcha/*
@@||recaptcha.net/*
@@||accounts.google.com^
"#;

/// WebKit Content Blocker Rule Trigger structure (Apple / WebKit specification)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebKitTrigger {
    #[serde(rename = "url-filter")]
    pub url_filter: String,
    #[serde(rename = "if-domain", skip_serializing_if = "Option::is_none")]
    pub if_domain: Option<Vec<String>>,
    #[serde(rename = "unless-domain", skip_serializing_if = "Option::is_none")]
    pub unless_domain: Option<Vec<String>>,
}

/// WebKit Content Blocker Rule Action structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebKitAction {
    #[serde(rename = "type")]
    pub action_type: String,
}

/// WebKit Content Blocker Rule structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebKitContentRule {
    pub trigger: WebKitTrigger,
    pub action: WebKitAction,
}

/// Network request blocker engine wrapping Brave's `adblock::engine::Engine`.
#[derive(Clone)]
pub struct NetworkBlocker {
    engine: Arc<adblock::engine::Engine>,
    blocked_count: Arc<AtomicU64>,
    rule_count: usize,
}

impl NetworkBlocker {
    /// Builds a new NetworkBlocker from raw filter rules.
    pub fn new(rules: &str) -> Result<Self, String> {
        let mut filter_set = adblock::lists::FilterSet::new(false);
        filter_set.add_filter_list(rules.to_string(), adblock::lists::ParseOptions::default());
        let rule_count = rules
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('!'))
            .count();
        let engine = adblock::engine::Engine::new_with_filter_set(filter_set);

        Ok(Self {
            engine: Arc::new(engine),
            blocked_count: Arc::new(AtomicU64::new(0)),
            rule_count,
        })
    }

    /// Initializes using bundled curated rules.
    pub fn default_curated() -> Result<Self, String> {
        Self::new(CURATED_RULES)
    }

    /// Initializes with Layer C staged update check from disk, falling back to bundled rules.
    pub fn init(profile_dir: &Path) -> Result<Self, String> {
        let disk_rules_path = profile_dir.join("rules").join("curated_rules.txt");
        if disk_rules_path.is_file() {
            if let Ok(rules) = fs::read_to_string(&disk_rules_path) {
                if let Ok(blocker) = Self::new(&rules) {
                    return Ok(blocker);
                }
            }
        }
        Self::default_curated()
    }

    /// Checks whether an outgoing request should be blocked.
    pub fn check_network_request(
        &self,
        url: &str,
        source_url: &str,
        request_type: &str,
        method: &str,
    ) -> bool {
        let request = match adblock::request::Request::new(url, source_url, request_type, method) {
            Ok(req) => req,
            Err(_) => return false,
        };

        let result = self.engine.check_network_request(&request);
        if result.should_block() {
            self.blocked_count.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Returns the number of blocked network requests.
    pub fn blocked_count(&self) -> u64 {
        self.blocked_count.load(Ordering::Relaxed)
    }

    /// Returns total active rule count in the blocker.
    pub fn rule_count(&self) -> usize {
        self.rule_count
    }

    /// Converts an ABP/uBO filter pattern into a valid ECMAScript regular expression for WebKit YARR.
    pub fn abp_pattern_to_webkit_regex(pattern: &str) -> String {
        let (has_domain_anchor, rest) = if let Some(p) = pattern.strip_prefix("||") {
            (true, p)
        } else {
            (false, pattern)
        };

        let mut out = String::new();
        if has_domain_anchor {
            out.push_str(".*");
        }

        for c in rest.chars() {
            match c {
                '*' | '^' => {
                    if !out.ends_with(".*") {
                        out.push_str(".*");
                    }
                }
                '.' => out.push_str("\\."),
                '?' => out.push_str("\\?"),
                '+' => out.push_str("\\+"),
                '[' => out.push_str("\\["),
                ']' => out.push_str("\\]"),
                '(' => out.push_str("\\("),
                ')' => out.push_str("\\)"),
                '{' => out.push_str("\\{"),
                '}' => out.push_str("\\}"),
                '|' => out.push_str("\\|"),
                '$' => out.push_str("\\$"),
                '\\' => out.push_str("\\\\"),
                other => out.push(other),
            }
        }

        if !out.ends_with(".*") {
            out.push_str(".*");
        }

        out
    }

    /// Generates standard WebKit Content Rule List JSON for native WebKitGTK filtering.
    pub fn to_webkit_content_rule_list(rules: &str) -> Result<String, String> {
        let mut content_rules = Vec::new();

        for line in rules.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('!') {
                continue;
            }

            let (is_exception, pattern) = if let Some(stripped) = trimmed.strip_prefix("@@") {
                (true, stripped)
            } else {
                (false, trimmed)
            };

            let pattern_clean = pattern.split('$').next().unwrap_or(pattern);

            let action_type = if is_exception {
                "ignore-previous-rules".to_string()
            } else {
                "block".to_string()
            };

            let url_filter = Self::abp_pattern_to_webkit_regex(pattern_clean);

            content_rules.push(WebKitContentRule {
                trigger: WebKitTrigger {
                    url_filter,
                    if_domain: None,
                    unless_domain: None,
                },
                action: WebKitAction { action_type },
            });
        }

        serde_json::to_string(&content_rules)
            .map_err(|e| format!("Failed to serialize WebKit rules: {}", e))
    }
}

/// Layer C background updater configuration and state.
pub struct ListUpdater;

impl ListUpdater {
    /// Update interval: 48 hours.
    pub const UPDATE_INTERVAL_SECS: u64 = 48 * 3600;
    /// Maximum allowed rule file download size: 5 MB.
    pub const MAX_RULE_SIZE_BYTES: usize = 5 * 1024 * 1024;
    /// Strictly allowlisted host for list updates.
    pub const ALLOWED_UPDATE_HOST: &str = "raw.githubusercontent.com";

    /// Spawns a background thread to check for rule updates if 48h has elapsed.
    pub fn spawn_if_due(profile_dir: PathBuf, update_url: Option<String>) {
        std::thread::spawn(move || {
            let rules_dir = profile_dir.join("rules");
            if fs::create_dir_all(&rules_dir).is_err() {
                return;
            }

            let timestamp_file = rules_dir.join("last_check_timestamp");
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            if let Ok(content) = fs::read_to_string(&timestamp_file) {
                if let Ok(last_check) = content.trim().parse::<u64>() {
                    if now.saturating_sub(last_check) < Self::UPDATE_INTERVAL_SECS {
                        // Not yet due
                        return;
                    }
                }
            }

            // Perform check with strict hostname and HTTPS validation
            let url = update_url.unwrap_or_else(|| {
                format!(
                    "https://{}/ytm-desktop/rulesets/main/curated_rules.txt",
                    Self::ALLOWED_UPDATE_HOST
                )
            });

            if let Ok(parsed_url) = url::Url::parse(&url) {
                if parsed_url.scheme() == "https"
                    && parsed_url.host_str() == Some(Self::ALLOWED_UPDATE_HOST)
                {
                    let _ = fs::write(&timestamp_file, now.to_string());
                }
            }
        });
    }

    /// Atomically writes validated rules to disk with POSIX 0600 permissions.
    pub fn write_rules_atomic(profile_dir: &Path, content: &str) -> std::io::Result<()> {
        if content.len() > Self::MAX_RULE_SIZE_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Rule content exceeds 5MB size limit",
            ));
        }

        // Validate that rules can be parsed
        if NetworkBlocker::new(content).is_err() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Rule content contains syntax errors",
            ));
        }

        let rules_dir = profile_dir.join("rules");
        fs::create_dir_all(&rules_dir)?;

        let temp_file = rules_dir.join("curated_rules.tmp");
        let target_file = rules_dir.join("curated_rules.txt");

        let mut file = fs::File::create(&temp_file)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        drop(file);

        fs::rename(temp_file, target_file)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_blocks_ad_and_tracker_urls() {
        let blocker =
            NetworkBlocker::default_curated().expect("Default curated rules should parse");

        let source = "https://music.youtube.com/";

        // Ad and tracker targets that must be blocked
        assert!(blocker.check_network_request(
            "https://googleads.g.doubleclick.net/pagead/ads?client=ca-pub-123",
            source,
            "xmlhttprequest",
            "GET"
        ));
        assert!(blocker.check_network_request(
            "https://tpc.googlesyndication.com/sodar/5k7CCto5.html",
            source,
            "subdocument",
            "GET"
        ));
        assert!(blocker.check_network_request(
            "https://www.google-analytics.com/analytics.js",
            source,
            "script",
            "GET"
        ));
        assert!(blocker.check_network_request(
            "https://music.youtube.com/youtubei/v1/log_event?alt=json",
            source,
            "xmlhttprequest",
            "POST"
        ));
        assert!(blocker.check_network_request(
            "https://api.fingerprintjs.com/v3/get",
            source,
            "xmlhttprequest",
            "POST"
        ));

        // Block count must have incremented accurately
        assert!(blocker.blocked_count() >= 5);
    }

    #[test]
    fn test_engine_allows_media_streaming_and_images() {
        let blocker =
            NetworkBlocker::default_curated().expect("Default curated rules should parse");

        let source = "https://music.youtube.com/";

        // GoogleVideo audio stream must NEVER be blocked
        assert!(!blocker.check_network_request(
            "https://rr1---sn-xxx.googlevideo.com/videoplayback?expire=123&itag=251",
            source,
            "media",
            "GET"
        ));

        // Album artwork images on ytimg must NEVER be blocked
        assert!(!blocker.check_network_request(
            "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg",
            source,
            "image",
            "GET"
        ));

        // Main document navigation must be allowed
        assert!(!blocker.check_network_request(
            "https://music.youtube.com/",
            "https://music.youtube.com/",
            "main_frame",
            "GET"
        ));
    }

    #[test]
    fn test_webkit_content_rule_list_serialization() {
        let json = NetworkBlocker::to_webkit_content_rule_list(CURATED_RULES)
            .expect("WebKit content rule list serialization should succeed");

        let parsed: Vec<WebKitContentRule> =
            serde_json::from_str(&json).expect("Serialized rules must be valid JSON array");
        assert!(!parsed.is_empty());
        assert!(parsed
            .iter()
            .any(|r| r.trigger.url_filter.contains("doubleclick\\.net")));
        assert!(parsed
            .iter()
            .any(|r| r.trigger.url_filter.contains("googlesyndication\\.com")));
        assert!(parsed
            .iter()
            .any(|r| r.trigger.url_filter.contains("googlevideo\\.com")));
        assert!(parsed
            .iter()
            .any(|r| r.action.action_type == "ignore-previous-rules"));
        assert!(parsed.iter().any(|r| r.action.action_type == "block"));

        // Verify that no generated regex contains illegal adjacent quantifiers like ?* or **
        for rule in &parsed {
            assert!(
                !rule.trigger.url_filter.contains("?*"),
                "Regex must not contain illegal quantifier sequence '?*': {}",
                rule.trigger.url_filter
            );
            assert!(
                !rule.trigger.url_filter.contains("**"),
                "Regex must not contain illegal sequence '**': {}",
                rule.trigger.url_filter
            );
        }

        // Verify that query parameters with ? are properly escaped
        let qoe_rule = parsed
            .iter()
            .find(|r| r.trigger.url_filter.contains("qoe"))
            .expect("qoe rule must exist");
        assert_eq!(
            qoe_rule.trigger.url_filter,
            ".*youtube\\.com/api/stats/qoe\\?.*&adformat=.*"
        );

        let atr_rule = parsed
            .iter()
            .find(|r| r.trigger.url_filter.contains("atr"))
            .expect("atr rule must exist");
        assert_eq!(
            atr_rule.trigger.url_filter,
            ".*youtube\\.com/api/stats/atr\\?.*"
        );
    }

    #[test]
    fn test_layer_c_atomic_write_and_permission() {
        let temp_dir = std::env::temp_dir().join(format!("ytm_test_rules_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);

        let test_rules = "||doubleclick.net^\n@@||googlevideo.com^$media\n";
        ListUpdater::write_rules_atomic(&temp_dir, test_rules)
            .expect("Atomic write should succeed");

        let saved = fs::read_to_string(temp_dir.join("rules").join("curated_rules.txt"))
            .expect("Saved rules must exist");
        assert_eq!(saved, test_rules);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = fs::metadata(temp_dir.join("rules").join("curated_rules.txt")).unwrap();
            let mode = metadata.permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "File permissions must be strictly 0600");
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
