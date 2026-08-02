# 🚀 agym

Unified session picker, account switcher, and model quota manager for the **Antigravity CLI** (`agy`).

---

## ✨ Features

- 🔍 **Interactive Session Picker**: Search and instantly resume past Antigravity AI sessions using `fzf`.
- 👤 **Account Switcher**: Fast switching between multiple saved Antigravity AI accounts.
- 📊 **Quota & Model Stats**: Live visual progress bars and reset countdowns for Gemini, Claude, and GPT models.

---

## 📦 Installation

### Arch Linux / Omarchy (`yay` / AUR)

```bash
yay -S agym-git
```

### Manual / One-Liner

```bash
curl -sSL https://raw.githubusercontent.com/Praveensenpai/agym/main/install.sh | bash
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
