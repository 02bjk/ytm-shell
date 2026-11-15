#!/usr/bin/env bash
# ==============================================================================
# YTM Shell (YouTube Music Desktop) - Linux Complete Uninstaller
# Repository: https://github.com/02bjk/ytm-shell
# ==============================================================================
set -euo pipefail

echo "=================================================================="
echo "  YTM Shell Complete Uninstaller"
echo "=================================================================="

# Check for non-interactive flag or pipe
FORCE=false
for arg in "$@"; do
    if [ "$arg" = "-f" ] || [ "$arg" = "--force" ] || [ "$arg" = "-y" ]; then
        FORCE=true
    fi
done

if [ "$FORCE" = false ] && [ -t 0 ]; then
    read -r -p "This will completely remove YTM Shell, its desktop entries, and ALL local cache/profile data. Continue? [y/N] " response
    case "$response" in
        [yY][eE][sS]|[yY]) ;;
        *) echo "Uninstallation cancelled."; exit 0 ;;
    esac
fi

echo "--> Terminating any running ytm-shell processes..."
pkill -x ytm-shell 2>/dev/null || true
pkill -f "ytm-shell" 2>/dev/null || true
sleep 0.5

# 1. Remove Binaries
echo "--> Removing executable binaries..."
BIN_PATHS=(
    "${HOME}/.local/bin/ytm-shell"
    "${HOME}/.local/bin/ytm-shell-uninstall"
    "/usr/local/bin/ytm-shell"
    "/usr/local/bin/ytm-shell-uninstall"
    "/usr/bin/ytm-shell"
)
for bin in "${BIN_PATHS[@]}"; do
    if [ -f "$bin" ]; then
        if [ -w "$bin" ] || [ -w "$(dirname "$bin")" ]; then
            rm -f "$bin"
            echo "    Removed: $bin"
        elif command -v sudo >/dev/null 2>&1; then
            sudo rm -f "$bin" && echo "    Removed (sudo): $bin" || true
        fi
    fi
done

# 2. Remove Desktop Launchers
echo "--> Removing desktop launchers..."
DESKTOP_PATHS=(
    "${HOME}/.local/share/applications/ytm-shell.desktop"
    "/usr/local/share/applications/ytm-shell.desktop"
    "/usr/share/applications/ytm-shell.desktop"
)
for desktop in "${DESKTOP_PATHS[@]}"; do
    if [ -f "$desktop" ]; then
        if [ -w "$desktop" ] || [ -w "$(dirname "$desktop")" ]; then
            rm -f "$desktop"
            echo "    Removed: $desktop"
        elif command -v sudo >/dev/null 2>&1; then
            sudo rm -f "$desktop" && echo "    Removed (sudo): $desktop" || true
        fi
    fi
done

# 3. Remove Application Icons
echo "--> Removing application icons..."
ICON_PATTERNS=(
    "${HOME}/.local/share/icons/hicolor/*/apps/ytm-shell.png"
    "${HOME}/.local/share/pixmaps/ytm-shell.png"
    "/usr/share/icons/hicolor/*/apps/ytm-shell.png"
    "/usr/share/pixmaps/ytm-shell.png"
)
for pattern in "${ICON_PATTERNS[@]}"; do
    for icon in $pattern; do
        if [ -f "$icon" ]; then
            if [ -w "$icon" ]; then
                rm -f "$icon"
                echo "    Removed: $icon"
            elif command -v sudo >/dev/null 2>&1; then
                sudo rm -f "$icon" && echo "    Removed (sudo): $icon" || true
            fi
        fi
    done
done

# 4. Remove All Leftover App Data, Profiles, WebKit Caches & Storage
echo "--> Purging all leftover application profiles, caches, and storage..."
DATA_DIRS=(
    "${HOME}/.local/share/com.ytm.shell"
    "${HOME}/.config/com.ytm.shell"
    "${HOME}/.cache/com.ytm.shell"
    "${HOME}/.local/share/ytm-shell"
    "${HOME}/.config/ytm-shell"
    "${HOME}/.cache/ytm-shell"
)
for dir in "${DATA_DIRS[@]}"; do
    if [ -d "$dir" ]; then
        rm -rf "$dir"
        echo "    Purged: $dir"
    fi
done

# 5. Clean Temporary Run/Test Dirs
TEMP_PATTERNS=(
    "/tmp/ytm_shell*"
    "/tmp/ytm-shell*"
)
for pattern in "${TEMP_PATTERNS[@]}"; do
    for tdir in $pattern; do
        if [ -e "$tdir" ] && [ -w "$tdir" ]; then
            rm -rf "$tdir"
            echo "    Cleaned: $tdir"
        fi
    done
done

# 6. Update System Desktop and Icon Caches
echo "--> Updating system desktop and icon databases..."
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "${HOME}/.local/share/applications" 2>/dev/null || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -f -t "${HOME}/.local/share/icons/hicolor" 2>/dev/null || true

echo ""
echo "=================================================================="
echo "  YTM Shell and all leftover files were completely uninstalled!"
echo "=================================================================="
