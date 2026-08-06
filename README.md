# agym (Antigravity Manager)

> Ultra-fast Antigravity CLI Account Manager & Switcher written in Rust.

`agym` allows you to switch between multiple Antigravity accounts instantly without logging out and jump directly into previous chat sessions.

---

## ⚡ Features

* 🖥️ **Interactive Ratatui TUI Dashboard**: High-performance full-screen TUI dashboard.
* 🚀 **Instant Account Switching**: Switch Antigravity accounts in <10ms without logging out.
* 💬 **Embedded Session Explorer**: Press `[s]` or `[Tab]` inside the TUI dashboard to browse and jump directly into previous Antigravity sessions (`agy resume`).
* 📊 **Dual Model Quota Display**: Tracks both Gemini and Claude model quota pools (`[Gemini: X% | Claude: Y%]`).
* ⚡ **Non-Blocking Background Refresh**: Press `[r]` inside TUI to fetch fresh live quota metrics asynchronously with 15s debouncing.
* ➕ **Seamless New Session Flow**: Press `[n]` inside TUI to back up your active session and log into a new account.
* 🗑️ **In-Menu Account Deletion**: Safely delete unused account profiles directly from the interactive TUI menu (`[d]`).
* 🛠️ **CLI Subcommands**: Streamlined CLI support for direct account switching (`agym <email>`) and saving sessions (`agym save`).
* 🐚 **Shell Autocompletions**: Native autocompletion support for `bash`, `zsh`, and `fish`.
* 📦 **Single Standalone Binary**: Zero runtime dependencies.

---

## 📥 Installation

### Quick One-Liner (Pre-compiled Binary)

```bash
curl -sSL -H "Accept: application/vnd.github.v3.raw" https://api.github.com/repos/Praveensenpai/agym/contents/install.sh | bash
```

### Build from Source

```bash
git clone https://github.com/Praveensenpai/agym.git
cd agym
chmod +x install.sh
./install.sh
```

---

## ⚡ Shell Autocompletions Setup

`install.sh` automatically installs completions for `bash`, `zsh`, and `fish`.

If installing manually from source or via `cargo install`, generate completions for your shell:

### Bash
```bash
mkdir -p ~/.local/share/bash-completion/completions
agym completions bash > ~/.local/share/bash-completion/completions/agym
```

### Zsh
```bash
mkdir -p ~/.zsh/completion
agym completions zsh > ~/.zsh/completion/_agym
```

### Fish
```bash
mkdir -p ~/.config/fish/completions
agym completions fish > ~/.config/fish/completions/agym.fish
```

---

## 🚀 Usage

### 1. Interactive Ratatui TUI (Default)
Run `agym` with no arguments to launch the full-screen Ratatui TUI dashboard:

```bash
agym
```

#### TUI Dashboard Keybindings:
- **`j` / `k` or `↓` / `↑`**: Navigate through accounts/sessions
- **`Enter`**: Switch account / Resume session
- **`s` / `Tab`**: Switch between Accounts View and Sessions Explorer
- **`Space` / `v`**: Toggle session detail preview panel
- **`n`**: Log into a new account
- **`d`**: Delete selected account
- **`r`**: Refresh live quota metrics (background thread, 15s debouncing)
- **`/`**: Live search & filter
- **`q` or `Esc`**: Quit TUI

### 2. Direct Account Switch
Switch to a saved account directly by name or email:

```bash
agym user@gmail.com
```

### 3. Save Current Account Session
Save your currently active Antigravity login session:

```bash
agym save
```

---

## 📜 License

MIT © [Praveensenpai](https://github.com/Praveensenpai)
