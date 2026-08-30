//! OS Keyring interface for securely storing and retrieving Antigravity credentials.
//!
//! On Linux systems, this module communicates with the `secret-tool` binary (part of `libsecret`)
//! to manage the active OAuth credential token in the system keystore.

use anyhow::{bail, Result};
use std::io::Write;
use std::process::{Command, Stdio};

/// Binary executable name for the Linux secret-service CLI tool.
pub const KEYRING_TOOL_BIN: &str = "secret-tool";

/// Keyring lookup and storage attribute key for the service name.
pub const KEYRING_SERVICE_KEY: &str = "service";

/// Keyring lookup and storage attribute value for the service name.
pub const KEYRING_SERVICE_VAL: &str = "gemini";

/// Keyring lookup and storage attribute key for the username.
pub const KEYRING_USER_KEY: &str = "username";

/// Keyring lookup and storage attribute value for the username.
pub const KEYRING_USER_VAL: &str = "antigravity";

/// Display label assigned to stored Antigravity secret items in the system keystore.
pub const KEYRING_LABEL_ARG: &str = "--label=Password for 'antigravity' on 'gemini'";

/// Subcommand name for querying secrets from secret-tool.
pub const KEYRING_CMD_LOOKUP: &str = "lookup";

/// Subcommand name for storing secrets in secret-tool.
pub const KEYRING_CMD_STORE: &str = "store";

/// Subcommand name for clearing secrets from secret-tool.
pub const KEYRING_CMD_CLEAR: &str = "clear";

/// Retrieves the active Antigravity OAuth token JSON from the system keyring.
///
/// Returns `Some(token_json)` if a non-empty secret is found, or `None` if
/// the lookup fails or the keyring does not contain an active credential.
pub fn get_current_keyring_token() -> Option<String> {
    let output = Command::new(KEYRING_TOOL_BIN)
        .args([
            KEYRING_CMD_LOOKUP,
            KEYRING_SERVICE_KEY,
            KEYRING_SERVICE_VAL,
            KEYRING_USER_KEY,
            KEYRING_USER_VAL,
        ])
        .output()
        .ok()?;

    if output.status.success() {
        let secret = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !secret.is_empty() {
            return Some(secret);
        }
    }

    None
}

/// Stores an Antigravity OAuth token JSON string into the system keyring.
///
/// Pipes the token payload into `secret-tool store` via standard input
/// and returns `true` if the process exits with a successful status code.
pub fn write_keyring_token(token_json: &str) -> bool {
    let child = Command::new(KEYRING_TOOL_BIN)
        .args([
            KEYRING_CMD_STORE,
            KEYRING_LABEL_ARG,
            KEYRING_SERVICE_KEY,
            KEYRING_SERVICE_VAL,
            KEYRING_USER_KEY,
            KEYRING_USER_VAL,
        ])
        .stdin(Stdio::piped())
        .spawn();

    if let Ok(mut proc) = child {
        if let Some(mut stdin) = proc.stdin.take() {
            let _ = stdin.write_all(token_json.as_bytes());
        }
        return proc.wait().map(|s| s.success()).unwrap_or(false);
    }

    false
}

/// Clears the Antigravity OAuth credential from the system keyring.
///
/// Executes `secret-tool clear` to remove the stored password for the `gemini`/`antigravity` entry.
pub fn clear_keyring_token() -> Result<()> {
    let status = Command::new(KEYRING_TOOL_BIN)
        .args([
            KEYRING_CMD_CLEAR,
            KEYRING_SERVICE_KEY,
            KEYRING_SERVICE_VAL,
            KEYRING_USER_KEY,
            KEYRING_USER_VAL,
        ])
        .status()?;

    if !status.success() {
        bail!(
            "Failed to clear keyring token: secret-tool exited with status {}",
            status
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyring_constants() {
        assert_eq!(KEYRING_TOOL_BIN, "secret-tool");
        assert_eq!(KEYRING_SERVICE_KEY, "service");
        assert_eq!(KEYRING_SERVICE_VAL, "gemini");
        assert_eq!(KEYRING_USER_KEY, "username");
        assert_eq!(KEYRING_USER_VAL, "antigravity");
        assert!(KEYRING_LABEL_ARG.contains("Password for 'antigravity' on 'gemini'"));
    }

    #[test]
    fn test_keyring_lookup_args_construction() {
        let args = [
            KEYRING_CMD_LOOKUP,
            KEYRING_SERVICE_KEY,
            KEYRING_SERVICE_VAL,
            KEYRING_USER_KEY,
            KEYRING_USER_VAL,
        ];
        assert_eq!(
            args,
            ["lookup", "service", "gemini", "username", "antigravity"]
        );
    }

    #[test]
    fn test_keyring_store_args_construction() {
        let args = [
            KEYRING_CMD_STORE,
            KEYRING_LABEL_ARG,
            KEYRING_SERVICE_KEY,
            KEYRING_SERVICE_VAL,
            KEYRING_USER_KEY,
            KEYRING_USER_VAL,
        ];
        assert_eq!(args.len(), 6);
        assert_eq!(args[0], "store");
        assert_eq!(args[1], "--label=Password for 'antigravity' on 'gemini'");
    }

    #[test]
    fn test_keyring_clear_args_construction() {
        let args = [
            KEYRING_CMD_CLEAR,
            KEYRING_SERVICE_KEY,
            KEYRING_SERVICE_VAL,
            KEYRING_USER_KEY,
            KEYRING_USER_VAL,
        ];
        assert_eq!(
            args,
            ["clear", "service", "gemini", "username", "antigravity"]
        );
    }
}
