# YTM Shell (YouTube Music Desktop)

Minimal, hardened desktop wrapper for [YouTube Music](https://music.youtube.com). Built with pure Rust, WebKitGTK 4.1 (Linux), and WebView2 (Windows).



---

## Features

* **Multi-Layer Ad & Tracker Blocking**:
  * **Layer A**: Document-start DOM scriptlet that prunes ad metadata and intercepts/mocks telemetry endpoints (`log_event`, `api/stats/*`) to return 200 OK locally in 0ms.
  * **Layer B**: Native WebKit Content Rule List / Brave pure-Rust matching engine that blocks DoubleClick, Google Analytics, Firebase, Sentry, and tracking pixels before network dispatch.
  * **Layer C**: Background updater that securely refreshes rule lists every 48 hours over HTTPS with POSIX `0600` atomic file writes.
  * **Intelligent Ad Skipper**: Automatically skips and mutes video ads while firmly holding track playback at `0:00` until the ad clears, so you never lose the beginning of a song.
* **GPU & Video Hardware Acceleration**: Built-in hardware compositing and VA-API / NVDEC hardware video decoding offload.
* **Full Desktop Integration**: Linux MPRIS D-Bus (`org.mpris.MediaPlayer2.ytm_shell`), Windows SMTC, System Tray icon with close-to-tray background playback, and single-instance window focusing.
* **Zero IPC Bridge Attack Surface**: Remote web content has zero ability to execute local commands.
* **Original Copyright-Free Branding**: Custom-crafted Radiant Crimson Red icon suite.

---

## Privacy & Data Handling

| Category | Policy | Details |
| :--- | :--- | :--- |
| **Data Collected** | **None (0%)** | `ytm-shell` collects zero personal data, zero analytics, zero metrics, and zero crash reports. |
| **Data Needed** | **Session Cookies** | Stored strictly in a local, isolated browser cookie store on your device with POSIX `0700` permissions. Cookies are only sent to `music.youtube.com` and `accounts.google.com`. |
| **Optional Data** | **None** | No optional telemetry or cloud sync. |
| **Telemetry Interception** | **Blocked & Mocked** | Outbound  tracking beacons and telemetry requests (`log_event`, `qoe`, `atr`, `pagead`) are blocked and answered locally with a dummy `200 OK` response so  no tracking data and the player never freezes. |

---

## Available Command-Line Flags

```bash
ytm-shell [FLAGS]
```

| Flag | Description |
| :--- | :--- |
| *(default)* | Runs with balanced GPU acceleration (`OnDemand`), activating the GPU during video playback and saving power when idle. |
| `--force-gpu` | Enforces full hardware acceleration (`Always`) and WebKit compositing mode for maximum throughput. |
| `--disable-gpu` | Disables GPU acceleration (`Never`); forces software rasterization for headless VMs or systems with buggy graphics drivers. |
| `--minimized` | Launches the application directly hidden to the system tray without displaying the main window. |
| `--allow-unfiltered` | Security override flag (for debugging only). Bypasses fail-closed adblock filter enforcement. |

---

## Keyboard Shortcuts

### Hardware Multimedia Keys
Physical multimedia keys on your keyboard (**Play / Pause**, **Next Track**, **Previous Track**) are natively supported out-of-the-box via Linux MPRIS D-Bus and Windows SMTC.

### Fallback Shortcuts (When Hardware Keys Are Absent)
When hardware media keys are absent or grabbed exclusively by your window manager, the following global shortcuts are active:

| Shortcut | Action |
| :--- | :--- |
| <kbd>Ctrl</kbd> + <kbd>Alt</kbd> + <kbd>Space</kbd> | Play / Pause |
| <kbd>Ctrl</kbd> + <kbd>Alt</kbd> + <kbd>→</kbd> | Next Track |
| <kbd>Ctrl</kbd> + <kbd>Alt</kbd> + <kbd>←</kbd> | Previous Track |

---

## Download & Installation

### Linux

#### Option 1: One-Line Curl Installer (Recommended)
Installs the standalone release binary to `~/.local/bin`, installs the desktop launcher, and registers the brand icons:
```bash
curl -sSL https://raw.githubusercontent.com/02bjk/ytm-shell/main/dist/install.sh | bash
```

#### Option 2: Pre-Built Release Archive
Download `ytm-shell-linux-x86_64.tar.gz` from [GitHub Releases](https://github.com/02bjk/ytm-shell/releases):
```bash
tar -xzf ytm-shell-linux-x86_64.tar.gz
chmod +x ytm-shell
./ytm-shell
```

#### Linux Runtime Dependencies
If your system does not already have WebKitGTK and GStreamer installed, install them using your package manager:

* **Debian / Ubuntu / Linux Mint**:
  ```bash
  sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0 libayatana-appindicator3-1 \
    gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-gl gstreamer1.0-vaapi
  ```
* **Arch Linux / Manjaro**:
  ```bash
  sudo pacman -S webkit2gtk-4.1 gtk3 libayatana-appindicator \
    gst-plugins-good gst-plugins-bad gst-vaapi
  ```
* **Fedora / RHEL**:
  ```bash
  sudo dnf install webkit2gtk4.1 gtk3 libayatana-appindicator \
    gstreamer1-plugins-good gstreamer1-plugins-bad-free gstreamer1-vaapi
  ```

---

### Windows

1. Download `ytm-shell-windows-x64.zip` from [GitHub Releases](https://github.com/02bjk/ytm-shell/releases).
2. Extract the archive and launch `ytm-shell.exe`.
3. **Dependencies**: Requires Microsoft Edge WebView2 Runtime (pre-installed on Windows 10 & 11).

---

## Building from Source

```bash
# Clone the repository
git clone https://github.com/02bjk/ytm-shell.git
cd ytm-shell

# Run quality checks
cargo fmt --check
cargo clippy -- -D warnings
cargo test

# Compile optimized release binary
cargo build --release
```


---


## License

MIT License. See [LICENSE](LICENSE) for details.
