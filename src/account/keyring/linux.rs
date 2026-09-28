//! Linux Secret Service backend implemented through `secret-tool`.

use super::{
    normalize_keyring_secret, KEYRING_SERVICE_KEY, KEYRING_SERVICE_VAL, KEYRING_USER_KEY,
    KEYRING_USER_VAL,
};
use anyhow::{bail, Result};
use std::io::Write;
use std::process::{Command, Stdio};

/// Binary executable provided by libsecret.
pub const KEYRING_TOOL_BIN: &str = "secret-tool";
/// Subcommand for querying secrets.
pub const KEYRING_CMD_LOOKUP: &str = "lookup";
/// Subcommand for storing secrets.
pub const KEYRING_CMD_STORE: &str = "store";
/// Subcommand for clearing secrets.
pub const KEYRING_CMD_CLEAR: &str = "clear";
/// Label argument expected by `secret-tool store`.
pub const KEYRING_LABEL_ARG: &str = "--label=Password for 'antigravity' on 'gemini'";

/// Retrieves the active Antigravity OAuth token from Secret Service.
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

    if !output.status.success() {
        return None;
    }

    normalize_keyring_secret(&output.stdout)
}

/// Stores the active Antigravity OAuth token in Secret Service.
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

    let Ok(mut process) = child else {
        return false;
    };

    if let Some(mut stdin) = process.stdin.take() {
        let _ = stdin.write_all(token_json.as_bytes());
    }

    process
        .wait()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Clears the active Antigravity OAuth token from Secret Service.
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
    use crate::account::keyring::KEYRING_LABEL;

    #[test]
    fn test_keyring_constants() {
        assert_eq!(KEYRING_TOOL_BIN, "secret-tool");
        assert_eq!(KEYRING_SERVICE_KEY, "service");
        assert_eq!(KEYRING_SERVICE_VAL, "gemini");
        assert_eq!(KEYRING_USER_KEY, "username");
        assert_eq!(KEYRING_USER_VAL, "antigravity");
        assert_eq!(KEYRING_LABEL, "Password for 'antigravity' on 'gemini'");
        assert_eq!(
            KEYRING_LABEL_ARG,
            "--label=Password for 'antigravity' on 'gemini'"
        );
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
