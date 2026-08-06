#!/usr/bin/env bash
set -e

REPO="Praveensenpai/agym"
BINARY_NAME="agym"
INSTALL_DIR="$HOME/.local/bin"

mkdir -p "$INSTALL_DIR"

if [ -f "Cargo.toml" ]; then
    echo "Building $BINARY_NAME from local source..."
    cargo build --release
    cp target/release/"$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME"
else
    echo "Installing $BINARY_NAME..."
    TAG=$(curl -4 -sSL -H "Cache-Control: no-cache" --connect-timeout 10 --retry 3 "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' || true)

    if [ -n "$TAG" ]; then
        DOWNLOAD_URL="https://github.com/$REPO/releases/download/$TAG/agym-linux-x86_64.tar.gz"
        TMP_DIR=$(mktemp -d)
        trap 'rm -rf "$TMP_DIR"' EXIT
        echo "📥 Downloading pre-compiled binary $TAG..."
        curl -4 -sSL -H "Cache-Control: no-cache" --connect-timeout 10 --retry 3 "$DOWNLOAD_URL" | tar -xz -C "$TMP_DIR"
        cp "$TMP_DIR/$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME"
    else
        echo "⚠️  No release tag found. Building from source..."
        if command -v cargo >/dev/null 2>&1; then
            TMP_DIR=$(mktemp -d)
            trap 'rm -rf "$TMP_DIR"' EXIT
            git clone "https://github.com/$REPO.git" "$TMP_DIR"
            cd "$TMP_DIR"
            cargo build --release
            cp target/release/"$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME"
        else
            echo "❌ Cargo is required to build from source."
            exit 1
        fi
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
