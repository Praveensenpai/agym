#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

REPO="Praveensenpai/agym"
BINARY_NAME="agym"
INSTALL_DIR="$HOME/.local/bin"

platform_asset() {
    local os arch
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os:$arch" in
        Darwin:arm64|Darwin:aarch64)
            printf '%s\n' "agym-macos-arm64"
            ;;
        Darwin:x86_64)
            printf '%s\n' "agym-macos-x86_64"
            ;;
        Linux:x86_64|Linux:amd64)
            printf '%s\n' "agym-linux-x86_64"
            ;;
        *)
            return 1
            ;;
    esac
}

build_from_source() {
    local source_dir="$1"
    cd "$source_dir"
    cargo build --release
    cp "target/release/$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME"
}

mkdir -p "$INSTALL_DIR"

if [ -f "Cargo.toml" ]; then
    echo "Building $BINARY_NAME from local source..."
    build_from_source "$PWD"
else
    echo "Installing $BINARY_NAME..."
    TAG=$(curl -4 -sSL -H "Cache-Control: no-cache" --connect-timeout 10 --retry 3 "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' || true)
    ASSET_NAME=$(platform_asset || true)

    if [ -n "$TAG" ] && [ -n "$ASSET_NAME" ] && command -v curl >/dev/null 2>&1; then
        TMP_DIR=$(mktemp -d)
        trap 'rm -rf "$TMP_DIR"' EXIT
        DOWNLOAD_URL="https://github.com/$REPO/releases/download/$TAG/$ASSET_NAME.tar.gz"
        echo "📥 Downloading pre-compiled binary $TAG for $ASSET_NAME..."
        if curl -4 -fsSL -H "Cache-Control: no-cache" --connect-timeout 10 --retry 3 "$DOWNLOAD_URL" | tar -xz -C "$TMP_DIR"; then
            cp "$TMP_DIR/$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME"
            chmod +x "$INSTALL_DIR/$BINARY_NAME"
        else
            echo "⚠️  No compatible pre-compiled asset found. Building from source..."
            command -v cargo >/dev/null 2>&1 || { echo "❌ Cargo is required to build from source."; exit 1; }
            git clone "https://github.com/$REPO.git" "$TMP_DIR/source"
            build_from_source "$TMP_DIR/source"
        fi
    elif command -v cargo >/dev/null 2>&1; then
        echo "⚠️  No compatible release found. Building from source..."
        TMP_DIR=$(mktemp -d)
        trap 'rm -rf "$TMP_DIR"' EXIT
        git clone "https://github.com/$REPO.git" "$TMP_DIR/source"
        build_from_source "$TMP_DIR/source"
    else
        echo "❌ Cargo is required to build from source."
        exit 1
    fi
fi

chmod +x "$INSTALL_DIR/$BINARY_NAME"

echo "Installing shell autocompletions..."
BASH_DIR="$HOME/.local/share/bash-completion/completions"
mkdir -p "$BASH_DIR"
"$INSTALL_DIR/$BINARY_NAME" completions bash > "$BASH_DIR/$BINARY_NAME" 2>/dev/null || true

ZSH_DIR="$HOME/.zsh/completion"
mkdir -p "$ZSH_DIR"
"$INSTALL_DIR/$BINARY_NAME" completions zsh > "$ZSH_DIR/_$BINARY_NAME" 2>/dev/null || true

FISH_DIR="$HOME/.config/fish/completions"
mkdir -p "$FISH_DIR"
"$INSTALL_DIR/$BINARY_NAME" completions fish > "$FISH_DIR/$BINARY_NAME.fish" 2>/dev/null || true

echo "✔ Successfully installed $BINARY_NAME & shell completions!"
"$INSTALL_DIR/$BINARY_NAME" --version
