# 🚀 agym

Unified session picker, account switcher, and model quota manager for the **Antigravity CLI** (`agy`).

---

## ✨ Features

- 🔍 **Interactive Session Picker**: Search and instantly resume past Antigravity AI sessions using `fzf`.
- 👤 **Account Switcher**: Fast switching between multiple saved Antigravity AI accounts.
- 📊 **Quota & Model Stats**: Live visual progress bars and reset countdowns for Gemini, Claude, and GPT models.

---

## 📦 Installation

> ℹ️ **Note**: AUR submission (`yay -S agym-git`) is currently pending due to temporary AUR maintenance. Please use the one-liner installer below in the meantime.

### Quick One-Liner

```bash
curl -sSL https://raw.githubusercontent.com/Praveensenpai/agym/main/install.sh | bash
```

### 🛠️ Build from Source (Cargo)

Prerequisites: [Rust & Cargo](https://rustup.rs/)

```bash
git clone https://github.com/Praveensenpai/agym.git
cd agym
cargo build --release
cp target/release/agym ~/.local/bin/
```

---

## ⚡ Usage

```bash
# Launch interactive session picker (fzf)
agym

# Launch interactive account switcher (fzf)
agym accounts

# View model quotas & reset timers
agym stats

# View verbose quota details for all internal models
agym stats -v

# Switch to account directly by name/email
agym switch account@example.com
```

---

## 📜 License

MIT
