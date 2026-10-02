#!/usr/bin/env bash
# ==============================================================================
# YTM Shell (YouTube Music Desktop) - Linux One-Line Installer
# Repository: https://github.com/02bjk/ytm-shell
# ==============================================================================
set -euo pipefail

REPO="02bjk/ytm-shell"
BIN_NAME="ytm-shell"
INSTALL_DIR="${HOME}/.local/bin"
DESKTOP_DIR="${HOME}/.local/share/applications"
ICON_DIR="${HOME}/.local/share/icons/hicolor/128x128/apps"

echo "==> Installing YTM Shell (YouTube Music)..."

# Ensure architecture is x86_64
ARCH="$(uname -m)"
if [ "$ARCH" != "x86_64" ]; then
    echo "Error: Only x86_64 Linux architecture is currently supported (detected: $ARCH)." >&2
    exit 1
fi

# Create target directories
mkdir -p "$INSTALL_DIR" "$DESKTOP_DIR" "$ICON_DIR"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Check if installing from local archive/build or remote release
if [ -f "${SCRIPT_DIR}/../target/release/${BIN_NAME}" ]; then
    echo "--> Installing from local release build..."
    install -m 0755 "${SCRIPT_DIR}/../target/release/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}"
    install -m 0644 "${SCRIPT_DIR}/../packaging/ytm-shell.desktop" "${DESKTOP_DIR}/ytm-shell.desktop"
    install -m 0644 "${SCRIPT_DIR}/../icons/128x128.png" "${ICON_DIR}/ytm-shell.png"
elif [ -f "${SCRIPT_DIR}/${BIN_NAME}" ]; then
    echo "--> Installing from local folder..."
    install -m 0755 "${SCRIPT_DIR}/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}"
    [ -f "${SCRIPT_DIR}/ytm-shell.desktop" ] && install -m 0644 "${SCRIPT_DIR}/ytm-shell.desktop" "${DESKTOP_DIR}/ytm-shell.desktop"
    [ -f "${SCRIPT_DIR}/icon.png" ] && install -m 0644 "${SCRIPT_DIR}/icon.png" "${ICON_DIR}/ytm-shell.png"
else
    echo "--> Fetching latest release asset from GitHub (${REPO})..."
    TEMP_DIR="$(mktemp -d)"
    trap 'rm -rf "$TEMP_DIR"' EXIT

    DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/ytm-shell-linux-x86_64.tar.gz"
    echo "--> Downloading: $DOWNLOAD_URL"
    if curl -sSL -f "$DOWNLOAD_URL" -o "${TEMP_DIR}/release.tar.gz"; then
        tar -xzf "${TEMP_DIR}/release.tar.gz" -C "$TEMP_DIR"
        install -m 0755 "${TEMP_DIR}/ytm-shell" "${INSTALL_DIR}/${BIN_NAME}"
        [ -f "${TEMP_DIR}/ytm-shell.desktop" ] && install -m 0644 "${TEMP_DIR}/ytm-shell.desktop" "${DESKTOP_DIR}/ytm-shell.desktop"
        [ -f "${TEMP_DIR}/128x128.png" ] && install -m 0644 "${TEMP_DIR}/128x128.png" "${ICON_DIR}/ytm-shell.png"
    else
        echo "Error: Failed to download release asset from GitHub." >&2
        echo "Please verify release assets exist on https://github.com/${REPO}/releases." >&2
        exit 1
    fi
fi

# Update desktop and icon caches if utilities exist
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$DESKTOP_DIR" || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -f -t "${HOME}/.local/share/icons/hicolor" 2>/dev/null || true

echo ""
echo "=================================================================="
echo "  YTM Shell installed successfully!"
echo "  Binary:  ${INSTALL_DIR}/${BIN_NAME}"
echo "  Launcher: ${DESKTOP_DIR}/ytm-shell.desktop"
echo "=================================================================="
echo "Make sure ${INSTALL_DIR} is in your PATH."
echo "Run 'ytm-shell' or search for 'YouTube Music' in your app launcher."
