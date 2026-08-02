#!/bin/bash

CYAN='\033[0;36m'
GREEN='\033[0;32m'
PURPLE='\033[0;35m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
NC='\033[0m'

echo -e "${PURPLE}🚀 Installing agym (Unified Antigravity CLI Manager)...${NC}\n"

BIN_DIR="$HOME/.local/bin"
mkdir -p "$BIN_DIR"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
AGYM_SRC="$SCRIPT_DIR/bin/agym"

if [ ! -f "$AGYM_SRC" ]; then
    echo -e "${RED}❌ Error: $AGYM_SRC not found!${NC}"
    exit 1
fi

chmod +x "$AGYM_SRC"
cp "$AGYM_SRC" "$BIN_DIR/agym"
chmod +x "$BIN_DIR/agym"

echo -e "${GREEN}✔ Installed agym to ${BIN_DIR}/.${NC}"

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

echo -e "\n${GREEN}${BOLD}🎉 agym installation completed!${NC}"
echo -e "Usage:"
echo -e "  ${CYAN}agym${NC}            - Interactive session picker"
echo -e "  ${CYAN}agym accounts${NC}   - Interactive account switcher"
echo -e "  ${CYAN}agym stats${NC}      - Quota & model status"
