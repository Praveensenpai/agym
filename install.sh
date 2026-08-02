#!/bin/bash

CYAN='\033[0;36m'
GREEN='\033[0;32m'
PURPLE='\033[0;35m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BOLD='\033[1m'
NC='\033[0m'

echo -e "${PURPLE}🚀 Installing agym...${NC}\n"

BIN_DIR="$HOME/.local/bin"
mkdir -p "$BIN_DIR"

REPO="Praveensenpai/agym"
RELEASE_URL="https://github.com/${REPO}/releases/latest/download/agym-linux-x86_64.tar.gz"

LOCAL_DIR=""
if [ -n "${BASH_SOURCE[0]}" ] && [ -f "${BASH_SOURCE[0]}" ]; then
    LOCAL_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" 2>/dev/null && pwd)"
fi

if [ -n "$LOCAL_DIR" ] && [ -f "$LOCAL_DIR/Cargo.toml" ] && command -v cargo >/dev/null 2>&1; then
    VERSION=$(grep -m1 '^version' "$LOCAL_DIR/Cargo.toml" | cut -d '"' -f2 2>/dev/null || echo "latest")
    echo -e "${BLUE}📦 Local source detected. Building agym v${VERSION} with Cargo...${NC}"
    cargo build --release --manifest-path "$LOCAL_DIR/Cargo.toml"
    cp "$LOCAL_DIR/target/release/agym" "$BIN_DIR/agym"
    INSTALLED_VER="v${VERSION}"
else
    LATEST_TAG=$(curl -4 -sSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null | grep -o '"tag_name": "[^"]*"' | cut -d'"' -f4)
    [ -z "$LATEST_TAG" ] && LATEST_TAG="latest"
    echo -e "${BLUE}📦 Downloading agym ${LATEST_TAG} pre-compiled binary from GitHub Releases...${NC}"
    TMP_DIR=$(mktemp -d)
    if curl -4 -fL --connect-timeout 10 --retry 3 -sS "$RELEASE_URL" -o "$TMP_DIR/agym.tar.gz"; then
        tar -xzf "$TMP_DIR/agym.tar.gz" -C "$TMP_DIR"
        if [ -f "$TMP_DIR/agym" ]; then
            cp "$TMP_DIR/agym" "$BIN_DIR/agym"
        elif [ -f "$TMP_DIR/dist/agym" ]; then
            cp "$TMP_DIR/dist/agym" "$BIN_DIR/agym"
        fi
        rm -rf "$TMP_DIR"
        INSTALLED_VER="${LATEST_TAG}"
    else
        rm -rf "$TMP_DIR"
        echo -e "${RED}❌ Failed to download pre-compiled release.${NC}"
        exit 1
    fi
fi

if [ ! -f "$BIN_DIR/agym" ] || [ ! -s "$BIN_DIR/agym" ]; then
    echo -e "${RED}❌ Error: Failed to install agym binary!${NC}"
    exit 1
fi

chmod +x "$BIN_DIR/agym"
echo -e "${GREEN}✔ Installed agym ${INSTALLED_VER} to ${BIN_DIR}/agym${NC}"

# Shell alias setup
SHELL_CONFIGS=("$HOME/.bashrc" "$HOME/.zshrc")
ALIAS_LINE="alias agym='$HOME/.local/bin/agym'"

for config in "${SHELL_CONFIGS[@]}"; do
    if [ -f "$config" ]; then
        sed -i '/alias agy-ls=/d' "$config" 2>/dev/null
        sed -i '/alias agy-account=/d' "$config" 2>/dev/null
        if ! grep -q "alias agym=" "$config" 2>/dev/null; then
            echo "" >> "$config"
            echo "$ALIAS_LINE" >> "$config"
            echo -e "${BLUE}📝 Added agym alias to $config${NC}"
        fi
    fi
done

echo -e "\n${GREEN}${BOLD}🎉 agym ${INSTALLED_VER} installation completed!${NC}"
echo -e "Usage:"
echo -e "  ${CYAN}agym${NC}            - Interactive session picker"
echo -e "  ${CYAN}agym accounts${NC}   - Interactive account switcher"
echo -e "  ${CYAN}agym stats${NC}      - Quota & model status"
echo -e "  ${CYAN}agym --version${NC}  - Show agym version"
