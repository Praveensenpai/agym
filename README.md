# agym (Antigravity Manager)

> Ultra-fast Antigravity CLI Account Manager & Switcher written in Rust.

`agym` allows you to switch between multiple Antigravity accounts instantly without logging out and jump directly into previous chat sessions.

---

## ⚡ Features

* 🚀 **Instant Account Switching**: Switch Antigravity accounts in <10ms without logging out.
* 💬 **Session Explorer**: Browse and jump directly into previous Antigravity sessions via `agy resume`.
* 🔑 **Automatic JWT Parsing**: Decodes account email dynamically from your Antigravity token.
* 📊 **Live Model Quota Display**: Shows CloudCode model quota next to accounts.
* ⚡ **Smart 5-Minute Quota Cache**: Caches usage metrics locally in `~/.gemini-accounts/.quota_cache.json` for instant UI execution.
* 🔄 **Cache Bypass**: Supports `--no-cache` (`-n`) to force refreshing live quota on demand.
* ➕ **Seamless New Session Flow**: Backs up your active session so you can log into a new account with zero setup.
* 🗑️ **In-Menu Account Deletion**: Safely delete unused account profiles directly from the interactive TUI menu.
* 🎯 **Interactive TUI Selector**: Select saved accounts or jump into sessions using `inquire`.
* 🛠️ **CLI Subcommands**: Streamlined CLI support for scripting (`agym <email>`, `agym sessions`, `agym new`, `agym save`, `agym list`).
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

### 1. Interactive Menu (Default)
Run `agym` with no arguments to open the interactive selection menu (switch accounts, jump to sessions, save, log into new account, or delete accounts):

```bash
agym
```

### 2. Direct Account Switch
Switch to a saved account directly by name or email:

```bash
agym user@gmail.com
```

### 3. Resume Previous Sessions
Browse previous chat sessions and resume instantly:

```bash
agym sessions # (or agym s)
```

### 4. Bypass Quota Cache
Force fetching fresh live quota directly from CloudCode API:

```bash
agym -n
# or
agym list --no-cache
```

### 5. Log in to a New Account
Back up your current session and prepare a fresh session to log into a new account:

```bash
agym new # (or agym add)
```

### 6. Save Current Account Session
Save your currently active Antigravity login session:

```bash
agym save
```

### 7. List Accounts
List all saved account profiles with model quota and cache timestamps:

```bash
agym list
```

---

## 📜 License

MIT © [Praveensenpai](https://github.com/Praveensenpai)
