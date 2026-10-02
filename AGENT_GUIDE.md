# AGENT_GUIDE.md: AI Developer & Codebase Context Guide

> **Audience**: AI Coding Agents, LLM Pair Programmers, and Automated Verification Systems.  
> **Purpose**: Provides immediate, full-fidelity context of the `ytm-shell` architecture, component boundaries, security invariants, and industrial development practices required to maintain and evolve this codebase.

---

## 1. Project Overview & Design Philosophy

`ytm-shell` is a minimal, hardened, resource-constrained desktop shell wrapping `https://music.youtube.com`. It replaces heavyweight Chromium/Electron wrappers with native platform web engines:
* **Linux**: WebKitGTK 4.1 (`soup3` / WebKit 2.40+) with GStreamer multimedia pipeline.
* **Windows**: Microsoft Edge WebView2 (`x86_64-pc-windows-msvc`).

### Core Design Constraints
1. **Binary Size Budget**: <10 MB release binary (`target/release/ytm-shell` compiles to ~7.3 MB).
2. **Zero IPC Bridge**: Tauri's IPC bridge is completely eliminated. Remote web content has zero ability to invoke native host functions.
3. **Fail-Closed Security**: Network filtering operates fail-closed. If filtering fails to initialize, external navigation is aborted unless `--allow-unfiltered` is explicitly passed.
4. **Zero Telemetry**: No user analytics, metrics, or personal data collection. Outbound tracking requests are blocked and mocked locally.

---

## 2. Codebase Structure & Component Map

```
rustify/
├── .github/workflows/
│   └── release.yml          # Multi-platform CI/CD: builds Linux tarball/deb & Windows exe/zip
├── dist/
│   └── install.sh           # One-line curl-friendly installer for Linux systems
├── docs/
│   ├── SECURITY_AUDIT.md    # Cloudflare Security Audit Specification compliance report
│   ├── bom.json             # CycloneDX v1.5 Software Bill of Materials (SBOM)
│   ├── coverage-ledger.json # Unit-by-unit security verification ledger (100% pass)
│   └── findings.json        # Resolved security findings audit ledger
├── icons/                   # 100% original copyright-free Radiant Crimson Red icon suite
│   ├── icon.png / icon.ico  # Master icons
│   └── 32x32.png .. 128x128 # Multi-resolution raster icons for Linux/Windows desktop
├── packaging/
│   ├── ytm-shell.desktop    # XDG Desktop Entry with MPRIS actions
│   ├── ytm-shell.metainfo.xml # AppStream metadata specification
│   ├── com.youtube.music.ytm_shell.yml # Flatpak manifest
│   └── debian/              # Debian packaging control files
├── src/
│   ├── main.rs              # Application entry, Tauri builder, CLI flag parsing, lifecycle
│   ├── lib.rs               # Library root re-exporting internal modules for unit testing
│   ├── adblock.rs           # Layer A DOM scriptlet, ad skipping state machine, telemetry mocking
│   ├── allowlist.rs         # Navigation allowlist (AllowInternal, OpenExternal, Block)
│   ├── network_block.rs     # Layer B & C Brave adblock engine + WebKit Content Rule generator
│   ├── player.rs            # Player DOM selector scripts, metadata extraction, playback controls
│   ├── mpris.rs             # Linux MPRIS D-Bus interface (org.mpris.MediaPlayer2.ytm_shell)
│   ├── shortcuts.rs         # Hardware media keys & fallback global shortcut registrar
│   ├── tray.rs              # System tray setup, icon attachment, menu actions, close-to-tray
│   ├── platform/
│   │   ├── mod.rs           # Cross-platform module interface
│   │   ├── linux.rs         # WebKitGTK 4.1 FFI, GPU policies, POSIX 0700 file security
│   │   └── windows.rs       # WebView2 platform integration stub
│   └── bin/
│       ├── bench_engine.rs  # Adblock matching performance benchmark harness
│       └── probe_login.rs   # Navigation policy test harness
├── ui/
│   └── error.html           # Secure local offline / fail-closed error fallback page
├── Cargo.toml               # Package manifest and dependency configuration
├── Cargo.lock               # Deterministic dependency lockfile
├── tauri.conf.json          # Tauri configuration (IPC disabled, permissions denied)
├── deny.toml                # cargo-deny security policy (advisories, licenses, bans)
├── README.md                # User-facing installation and usage guide
└── AGENT_GUIDE.md           # This document (AI / agent developer guide)
```

---

## 3. Security Invariants & Threat Model

When implementing features or modifying code, agents **must never violate** the following security invariants:

### Invariant 1: Zero IPC Bridge
* **Rule**: Never register Tauri `#[tauri::command]` or `invoke_handler`.
* **Rationale**: Any IPC bridge between remote webview content (`music.youtube.com`) and host native code creates a critical remote code execution (RCE) vector. All player interactions must execute strictly via DOM scripts evaluated from Rust outward (`window.eval`), never inward.

### Invariant 2: Multi-Layer Ad & Tracker Defense
* **Layer A (DOM / Scriptlet)** ([`src/adblock.rs`](file:///workspace/rustify/src/adblock.rs)):
  * Injected at document-start, strictly guarded to hostname `music.youtube.com`.
  * Preemptively returns `200 OK` `{}` for telemetry endpoints (`log_event`, `api/stats/ads`, `api/stats/qoe`, `api/stats/atr`, `pagead`). This prevents Google client loggers from stalling navigation or search keystrokes.
  * Deep ad pruning (`pruneRecursive`) is strictly scoped to YouTube Player Responses (requiring both `videoDetails` and `playabilityStatus`). Search results and Home browse feeds are never recursively pruned.
* **Layer B (Network / Native Filter)** ([`src/network_block.rs`](file:///workspace/rustify/src/network_block.rs)):
  * Evaluates all network requests via Brave's pure-Rust `adblock` engine.
  * Compiles curated ABP rules into WebKit Content Rule List JSON for native C++ request dropping in WebKitGTK.
* **Layer C (Staged Updates)**:
  * Checks for list updates over HTTPS from allowlisted host (`raw.githubusercontent.com`) every 48 hours.
  * Size capped at 5 MB; writes atomically with POSIX `0600` permissions.

### Invariant 3: WebKit YARR Regular Expression Rules
* **Critical YARR Constraint**: WebKit’s Content Extension compiler translates `url-filter` regular expressions into a deterministic finite automaton (DFA).
* **Forbidden**: Disjunctions (`|`) inside non-capturing groups `(?:A|B)` trigger `Disjunctions are not supported yet.`
* **Forbidden**: Quantifier-on-quantifier sequences (such as `?*`, `**`, `+*`) trigger `Internal error in YARR.`
* **Implementation**: ABP patterns are converted using [`NetworkBlocker::abp_pattern_to_webkit_regex`](file:///workspace/rustify/src/network_block.rs#L176-L215), which escapes metacharacters (`\?`, `\.`, `\+`, `\[`, etc.) and handles wildcards cleanly.

### Invariant 4: Strict Navigation & New-Window Allowlist
* **Rule**: Handled via [`allowlist::evaluate_url`](file:///workspace/rustify/src/allowlist.rs):
  * `AllowInternal`: Only `https://music.youtube.com`, `https://accounts.google.com`, and `https://*.recaptcha.net`.
  * `OpenExternal`: Third-party outbound links (privacy policy, terms, artist external links) are launched in the host's default browser via `platform::open_in_browser`.
  * `Block`: All unknown, non-HTTPS, or tracking domains are rejected immediately.
* **Rule**: Downloads are blocked (`.on_download(|_, _| false)`).
* **Rule**: Drag and drop is disabled (`.disable_drag_drop_handler()`).

### Invariant 5: Local Storage Sandboxing
* **Rule**: User data is isolated in a dedicated profile directory under OS app data with POSIX `0700` (`rwx------`) permissions.
* **Rule**: Session wipe (`clear_webview_session` and `wipe_profile_directory`) purges cookies, memory caches, and persistent databases.

---

## 4. Key Subsystems & Implementation Details

### Ad-Skipper & Song Pause Protection ([`src/adblock.rs`](file:///workspace/rustify/src/adblock.rs))
* **Detection**: Monitors `#movie_player` class attributes for `ad-showing` and `ad-interrupting` without DOM subtree recursion.
* **One-Shot Seek**: Ad streams (`dur <= 65`) are seeked to conclusion (`video.currentTime = dur`) exactly once (`adFastForwarded = true`) to prevent hammering WebKit’s GStreamer decoding pipeline.
* **Firm Song Pause**: When an ad is active and the song stream is loaded in the player (`dur > 65`), the song is immediately paused (`video.pause()`) and held at `0:00`. It is never allowed to advance muted in the background.
* **Clean Resumption**: When the ad overlay clears, `video.currentTime = 0` is enforced, audio is unmuted, and `video.play()` resumes the track cleanly from the start.
* **Visual Indicator**: Floating pill badge (`#ytm-shell-ad-indicator`) displays "Bypassing ad..." with a pulsing Radiant Crimson indicator.

### GPU & Hardware Video Acceleration ([`src/platform/linux.rs`](file:///workspace/rustify/src/platform/linux.rs))
* **Default**: `HardwareAccelerationPolicy::OnDemand` with WebGL enabled.
* **Flag `--force-gpu`**: `HardwareAccelerationPolicy::Always`.
* **Flag `--disable-gpu`**: `HardwareAccelerationPolicy::Never`.
* **Video Decoding**: Offloaded via GStreamer VA-API (`gstreamer1.0-vaapi`) to Intel QuickSync, AMD VCN, or NVIDIA NVDEC.

### Linux MPRIS D-Bus Service ([`src/mpris.rs`](file:///workspace/rustify/src/mpris.rs))
* Implements `org.mpris.MediaPlayer2` and `org.mpris.MediaPlayer2.Player` via `zbus`.
* Responds to desktop multimedia keys, lockscreen controls, and GNOME/KDE media widgets.
* Background worker queries track metadata from the DOM every 1.5 seconds.

---

## 5. Industrial Development & Maintenance Rules

### Rule 1: Zero Secret Leakage (Never Commit Credentials)
* **Never commit** API keys, personal access tokens (PATs), passwords, private keys, or `.env` files into git commits or repository files.
* Git remotes must use environment-provided credentials or git credential helpers.

### Rule 2: Release Binaries Belong in GitHub Releases
* **Never commit** compiled binaries (`ytm-shell`, `.exe`, `.tar.gz`) directly into git repository branches.
* Binaries are packaged and published to GitHub Releases via `.github/workflows/release.yml`.

### Rule 3: Verification Commands Before Submitting Changes
Agents modifying this repository must verify that all quality gates pass:
```bash
# 1. Format check
cargo fmt --check

# 2. Linter check (deny warnings)
cargo clippy -- -D warnings

# 3. Comprehensive test suite (23 unit tests)
cargo test

# 4. Dependency & security audit checks
cargo deny check
cargo audit

# 5. Optimized release compilation
cargo build --release
```
