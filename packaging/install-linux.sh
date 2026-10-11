#!/usr/bin/env bash
# Build Souls-like and install it for the current user (binary, icon and app-menu entry).
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release
install -Dm755 target/release/souls-like "$HOME/.local/bin/souls-like"
install -Dm644 packaging/souls-like.svg "$HOME/.local/share/icons/hicolor/scalable/apps/souls-like.svg"
install -Dm644 packaging/souls-like.desktop "$HOME/.local/share/applications/souls-like.desktop"
update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
echo "Installed. Launch 'Souls-like' from your app menu, or run: souls-like"
