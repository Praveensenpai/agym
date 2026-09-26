# CODEBASE.md: agym Semantic Digest

> **Notice**: This file is an AI-optimized semantic index. Do not write narrative prose. Keep token density high.

## 1. System Topology & Data Flow
```text
main.rs ──> clap CLI Parser ──┬──> [Save/Switch Account] ──> account::{store, auth, keyring} ──> OS Keyring / ~/.gemini-accounts
                              └──> [Interactive TUI]      ──┬──> ui::run_accounts_tui ──> ui::accounts ──> quota::fetch_quota_cached
                                                            └──> ui::run_sessions_tui ──> ui::sessions ──> session::scan_sessions ──> agy --conversation
```

## 2. Global Constraints & Architecture Patterns
- **Primary Language & Edition**: Rust 2021 edition
- **Architectural Paradigm**: Modular subsystem separation: `account/`, `quota.rs`, `session.rs`, `ui/`
- **Hard Constraints**: <400 lines/file, <60 lines/fn, zero production unwrap(), 0 compiler/clippy warnings.
- **Target Distribution**: Linux x86_64 standalone binary via GitHub Actions release (`~/.local/bin/agym`).

## 3. Module & Interface Skeleton

### `src/main.rs` (Role: cli, Lines: 75)
- **Responsibility**: Application entrypoint, CLI argument definition, dispatch to account switching, token saving, shell completion, or TUI.
- **Imports**: `crate::account::*`, `anyhow::Result`, `clap::{CommandFactory, Parser, Subcommand}`, `clap_complete::{generate, Shell}`, `crate::ui`
- **Types & Enums**:
  ```rust
  struct Cli { version: Option<bool>, account: Option<String>, command: Option<Commands> }
  enum Commands { Save, Completions { shell: Shell } }
  ```
- **Public Functions & Signatures**:
  ```rust
  fn main() -> Result<()>
  ```
- **Consumers**: OS process execution.
- **Side Effects**: stdout/stderr output, TUI initialization, process exit.

### `src/account/auth.rs` (Role: domain/infra, Lines: 179)
- **Responsibility**: Google OAuth2 `id_token` JWT decoding, unpadded/padded base64 parsing, tokeninfo network resolution fallback.
- **Imports**: `base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD}`, `reqwest::blocking::Client`, `serde_json::Value`
- **Public Functions & Signatures**:
  ```rust
  pub fn extract_email_from_token_json(token_json: &str) -> Option<String>
  pub fn extract_email_from_jwt(id_token: &str) -> Option<String>
  pub fn decode_jwt_payload(payload_b64: &str) -> Option<Vec<u8>>
  pub fn fetch_email_from_tokeninfo(access_token: &str) -> Option<String>
  ```
- **Consumers**: `account::store`, `account::mod`
- **Side Effects**: HTTPS GET to `www.googleapis.com/oauth2/v1/tokeninfo`.

### `src/account/keyring.rs` (Role: infra, Lines: 80)
- **Responsibility**: System keyring credential read/write/clear operations via `secret-tool` CLI subprocess.
- **Imports**: `std::process::Command`, `anyhow::Result`
- **Public Functions & Signatures**:
  ```rust
  pub fn get_current_keyring_token() -> Option<String>
  pub fn write_keyring_token(token_json: &str) -> bool
  pub fn clear_keyring_token() -> Result<()>
  ```
- **Consumers**: `account::store`, `account::mod`
- **Side Effects**: Spawns `secret-tool` process with stdin/stdout piping.

### `src/account/store.rs` (Role: domain/infra, Lines: 373)
- **Responsibility**: Account file persistence in `~/.gemini-accounts/`, profile symlink management in `~/.gemini-profiles/`, and SQLite sync.
- **Imports**: `super::auth`, `super::keyring`, `crate::quota::{clear_quota_cache, fetch_quota_cached, load_quota_cache, AccountQuotaInfo}`, `rusqlite::Connection`
- **Types & Enums**:
  ```rust
  pub struct AccountInfo { pub email: String, pub is_active: bool, pub quota: Option<AccountQuotaInfo>, pub file_path: PathBuf }
  ```
- **Public Functions & Signatures**:
  ```rust
  pub fn get_home() -> PathBuf
  pub fn get_accounts_dir() -> PathBuf
  pub fn get_profiles_dir() -> PathBuf
  pub fn get_db_path() -> PathBuf
  pub fn get_gemini_link() -> PathBuf
  pub fn save_current_account() -> Option<String>
  pub fn update_sqlite_db(email: &str) -> Result<()>
  pub fn update_gemini_profile(email: &str) -> Result<()>
  pub fn list_account_infos_with<F>(quota_for: F, detect_active: bool) -> Vec<AccountInfo>
  pub fn list_account_infos(no_cache: bool) -> Vec<AccountInfo>
  pub fn list_account_infos_cached() -> Vec<AccountInfo>
  pub fn set_active_account(target: &str) -> Result<()>
  pub fn remove_account(account_name: &str) -> Result<()>
  ```
- **Consumers**: `main.rs`, `account::mod`, `ui::accounts`
- **Side Effects**: Filesystem reads/writes, SQLite database updates, symlink replacement.

### `src/quota.rs` (Role: infra/domain, Lines: 394)
- **Responsibility**: Live CloudCode quota fetching via `daily-cloudcode-pa.googleapis.com` (`retrieveUserQuotaSummary`, `fetchAvailableModels`), OAuth token auto-refresh, and local JSON disk caching (5-min TTL).
- **Imports**: `chrono::{DateTime, Local}`, `reqwest::blocking::Client`, `serde::{Deserialize, Serialize}`, `serde_json::Value`
- **Types & Enums**:
  ```rust
  pub struct AccountQuotaInfo {
      pub plan_type: Option<String>,
      pub gemini_week_percent: Option<u32>,
      pub gemini_window_percent: Option<u32>,
      pub gemini_percent: Option<u32>,
      pub claude_week_percent: Option<u32>,
      pub claude_window_percent: Option<u32>,
      pub claude_percent: Option<u32>,
      pub top_model_name: Option<String>,
      pub top_model_percent: Option<u32>,
      pub fetched_at: u64,
      pub is_fresh: bool,
  }
  ```
- **Public Functions & Signatures**:
  ```rust
  pub fn get_cache_file_path() -> Option<PathBuf>
  pub fn load_quota_cache() -> HashMap<String, AccountQuotaInfo>
  pub fn save_quota_cache(cache: &HashMap<String, AccountQuotaInfo>)
  pub fn clear_quota_cache()
  pub fn fetch_quota_cached(account_key: &str, auth_path: &Path, no_cache: bool) -> Result<AccountQuotaInfo>
  ```
- **Consumers**: `account::store`, `ui::accounts`
- **Side Effects**: HTTPS POST to `daily-cloudcode-pa.googleapis.com`, disk read/write to `~/.gemini-accounts/.quota_cache.json`.

### `src/session.rs` (Role: domain/infra, Lines: 283)
- **Responsibility**: Discovers conversation sessions across brain directories, strips terminal artifacts / executor errors, and sanitizes prompt summaries.
- **Imports**: `chrono::{DateTime, Local}`, `serde_json::Value`, `walkdir::WalkDir`
- **Types & Enums**:
  ```rust
  pub struct SessionInfo {
      pub cid: String,
      pub short_cid: String,
      pub datetime: String,
      pub timestamp: u64,
      pub size_bytes: u64,
      pub line_count: usize,
      pub summary: String,
      pub full_prompt: String,
  }
  ```
- **Public Functions & Signatures**:
  ```rust
  pub fn format_bytes(bytes: u64) -> String
  pub fn clean_user_text(raw: &str) -> String
  pub fn sanitize_summary(raw: &str) -> String
  pub fn scan_sessions() -> Vec<SessionInfo>
  ```
- **Consumers**: `ui::sessions`, `ui::widgets`
- **Side Effects**: Recursive directory traversal of `~/.gemini/antigravity-cli/brain`.

### `src/ui/mod.rs` (Role: tui, Lines: 43)
- **Responsibility**: Crossterm raw terminal guard and TUI lifecycle switching between Accounts and Sessions views.
- **Imports**: `crossterm::{execute, terminal::*}, ratatui::backend::CrosstermBackend, ratatui::Terminal`
- **Types & Enums**:
  ```rust
  pub struct TerminalGuard { terminal: Terminal<CrosstermBackend<Stdout>> }
  ```
- **Public Functions & Signatures**:
  ```rust
  pub fn run_accounts_tui() -> Result<()>
  pub fn run_sessions_tui() -> Result<()>
  ```
- **Consumers**: `main.rs`, `ui::accounts`, `ui::sessions`
- **Side Effects**: Terminal raw mode, alternate screen buffer.

### `src/ui/accounts.rs` (Role: tui, Lines: 222)
- **Responsibility**: Interactive account switcher table, live background quota refresh thread, search filtering, and keybindings.
- **Consumers**: `ui::mod`, `ui::sessions`

### `src/ui/sessions.rs` (Role: tui, Lines: 269)
- **Responsibility**: Interactive session explorer table, prompt detail inspection pane, and external `agy --conversation <cid>` execution.
- **Consumers**: `ui::mod`, `ui::accounts`

### `src/ui/widgets.rs` (Role: tui/presentation, Lines: 357)
- **Responsibility**: Pure deterministic layout math, circular glyph quota progress meter formatting, styling constants, and unit testable widget logic.
- **Consumers**: `ui::accounts`, `ui::sessions`, `account::mod`

## 4. Execution Lifecycle Trace
1. **Startup**: `main.rs` parses CLI options via `clap`. If an account name is provided, switches account and exits.
2. **Interactive TUI**: Defaults to `ui::run_accounts_tui()`, initializing raw terminal via `TerminalGuard`.
3. **Session Exploration**: Pressing `s` transitions to `ui::run_sessions_tui()`, which scans transcripts via `session::scan_sessions()`.
4. **Resuming Session**: Pressing Enter on a session restores terminal state and executes `agy --conversation <cid>`.
5. **Auto-Retry Layer**: Managed externally via `agy-retry` wrapper binary (`~/.local/bin/agy-retry`), catching network errors and resuming unattended sessions.

## 5. Verification Commands
```bash
# Build
cargo build --release

# Test
cargo test

# Lint & Format
cargo clippy -- -D warnings
cargo fmt --check
```

## 6. Recent Iteration Changes
- **2026-09-26**: Aligned quota metrics vertical divider (`TARGET_MODEL_BLOCK_WIDTH = 31`) and right-aligned percentage meters (`{:>3}%`) across multi-window and single-window tiers (`v0.0.6`). Re-captured high-resolution showcase screenshot (`assets/agym_dashboard.png`) with continuous Gaussian privacy blur.
- **2026-09-26**: Generated and integrated high-resolution showcase screenshot (`assets/agym_dashboard.png`) into `README.md` with anti-deblur feathered Gaussian privacy blur.
- **2026-09-21**: Resolved inaccurate quota metrics by switching CloudCode PA endpoints from `cloudcode-pa.googleapis.com` to `daily-cloudcode-pa.googleapis.com` matching Antigravity CLI live tracking, and added percentage clamping to prevent out-of-bounds metrics.
- **2026-09-16**: Added `is_noise_line` in `session.rs` to filter `⚠ agent executor error:`, `Error: The stream was interrupted`, and related network error banners from session titles and prompt previews. Added unit tests and created AI-first `CODEBASE.md`.
