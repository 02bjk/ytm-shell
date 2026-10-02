# Threat Model: YTM Shell

## 1. Executive Summary & Scope

YTM Shell is a minimal, hardened native desktop wrapper around YouTube Music (`https://music.youtube.com`) for Linux (WebKitGTK 4.1) and Windows (WebView2). Its security architecture treats the remotely loaded web page as an untrusted environment executing arbitrary third-party and first-party JavaScript.

### In Scope
- **Ad, Tracker & Telemetry Scripts:** Network tracking, telemetry beaconing, surveillance scripts, and fingerprinting routines executing in the web context.
- **Malicious Navigation & Popups:** Cross-site redirects, rogue `window.open` requests, clickjacking, protocol smuggling, and outbound link exfiltration.
- **Remote Page Abuse of Tauri IPC:** Hostile or compromised web scripts attempting to invoke Tauri commands, access native OS resources, manipulate filesystems, or execute arbitrary code via IPC.
- **Tampered Filter Lists:** Supply-chain poisoning, man-in-the-middle tampering, or syntax-injection attacks via remote adblock/tracker rulesets.
- **Supply-Chain & Dependency Compromise:** Malicious crates, compromised build tools, untrusted CI actions, unsigned distribution binaries, or tampered dependencies.
- **Local Data Theft & Snooping:** Extraction of persistent authentication cookies, cached profile tokens, or local browsing state by unauthorized local non-root users or malware.

### Out of Scope
- **Hiding Identity from Google while Signed In:** When the user explicitly authenticates with Google, Google knows the account identity server-side. The threat model mitigates cross-site profiling and excessive client-side fingerprinting, not Google knowing who is logged into their service.

---

## 2. Threat Vectors, Actors, and Mitigations

| Threat Vector | Threat Actor / Origin | Potential Impact | Architecture Mitigation |
| :--- | :--- | :--- | :--- |
| **IPC Bridge Exploitation** | Malicious script or XSS on remote web page | Remote Code Execution (RCE) or file leakage via Tauri IPC | `withGlobalTauri = false`; zero Tauri capabilities granted to remote origin; remote IPC bridge stripped entirely; strict fail-closed capability boundaries. |
| **Navigation & Popup Smuggling** | Third-party ad script or rogue redirect | Phishing, drive-by malware, exfiltration to untrusted domains | Strict compile-time HTTPS navigation allowlist (`music.youtube.com`, `accounts.google.com`, etc.); exact host matching; all unlisted links open in system default browser; popups blocked unless explicitly allowlisted login flows. |
| **Excessive Web Permissions** | Compromised web context requesting capabilities | Unauthorized access to microphone, webcam, geolocation, clipboard, or local files | Deny-all permission handler at native webview level (WebView2 `PermissionRequested`, WebKitGTK `permission-request`); no file protocol (`file://`); download requests rejected; TLS errors strictly non-ignorable. |
| **Ad / Tracker Injection** | Google ad servers, DoubleClick, telemetry networks | Privacy loss, bandwidth waste, audio/video ad interruption | Multi-layered defense: Layer A DOM & API response pruning (`/youtubei/v1/player` JSON hooking) before JS execution; Layer B native network rule engine (Brave `adblock` / WebKit content filter) returning empty 403; cosmetic CSS suppression. |
| **Device Fingerprinting** | Fingerprinting scripts (fingerprintjs, canvas readback, WebRTC) | Persistent device tracking across contexts | Deterministic consistency over random spoofing: WebRTC disabled (no local/public IP leak); hardware concurrency/screen normalized; canvas readback seeded per session; no WebGL if unnecessary; third-party fingerprinting hosts dropped at Layer B. |
| **Tampered Ruleset Injection** | Compromised CDN, MitM attacker | Malicious rule injection, filter bypass, or execution of arbitrary code | HTTPS-only list updates to allowlisted hosts; strict size caps and hash/ETag verification; untrusted remote lists forbidden from executing scriptlets (scriptlets are compile-time immutable native code); bundled fallback snapshot; atomic writes with `0600` permissions. |
| **Local Data Theft** | Unprivileged local processes / malware | Theft of session tokens or Google login cookies | Profile directory restricted to user application data directory (`0700` POSIX permissions on Linux, user ACL on Windows); secure "Sign out & wipe data" routine clearing cookies, cache, and storage; zero app-stored credentials. |
| **Supply Chain & Build Integrity** | Compromised crate registry or build action | Backdoored binary release | `#![forbid(unsafe_code)]` with isolated FFI module; minimal features (`default-features = false` everywhere); pinned CI action SHAs; locked dependency tree (`Cargo.lock`); `cargo deny` license/advisory checks; `cargo audit` in CI; SBOM generation; signed binary releases. |

---

## 3. Defense-in-Depth Architecture Principles

1. **Least Privilege IPC:** The remote web application is treated as completely untrusted. No internal Tauri IPC endpoints are exposed to `https://music.youtube.com` unless strictly mandated with rigorous schema validation.
2. **Fail-Closed Enforcement:** If the ad-blocking engine or security initialization script fails to load, navigation to remote endpoints aborts immediately and presents a local secure error screen (`ui/index.html`).
3. **Sandbox Preservation:** The operating system and webview sandboxes remain strictly intact. `WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS` is forbidden under all circumstances. Evergreen WebView2 runtime ensures timely OS security patches.
4. **Data Minimization:** No telemetry, metrics, or error tracking egresses from the application binary. Logs scrub all query parameters, URLs, and sensitive session tokens.
