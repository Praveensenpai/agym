//! Platform-native credential storage for Antigravity OAuth tokens.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

/// Keyring lookup and storage attribute key for the service name on Linux.
#[cfg(target_os = "linux")]
pub const KEYRING_SERVICE_KEY: &str = "service";

/// Keyring lookup and storage attribute value for the service name.
pub const KEYRING_SERVICE_VAL: &str = "gemini";

/// Keyring lookup and storage attribute key for the username on Linux.
#[cfg(target_os = "linux")]
pub const KEYRING_USER_KEY: &str = "username";

/// Keyring lookup and storage attribute value for the username.
pub const KEYRING_USER_VAL: &str = "antigravity";

/// Display label assigned to stored Antigravity secret items in the system keystore.
pub const KEYRING_LABEL: &str = "Password for 'antigravity' on 'gemini'";

const GO_KEYRING_ENCODING_PREFIX: &str = "go-keyring-base64:";

pub(crate) fn decode_keyring_secret(secret: &[u8]) -> Option<String> {
    let secret = String::from_utf8_lossy(secret).trim().to_string();
    let encoded = secret.strip_prefix(GO_KEYRING_ENCODING_PREFIX)?;
    String::from_utf8(STANDARD.decode(encoded).ok()?).ok()
}

pub(crate) fn normalize_keyring_secret(secret: &[u8]) -> Option<String> {
    let secret = String::from_utf8_lossy(secret).trim().to_string();
    if secret.starts_with(GO_KEYRING_ENCODING_PREFIX) {
        return decode_keyring_secret(secret.as_bytes());
    }

    (!secret.is_empty()).then_some(secret)
}

pub(crate) fn encode_keyring_secret(secret: &str) -> String {
    format!(
        "{GO_KEYRING_ENCODING_PREFIX}{}",
        STANDARD.encode(secret.as_bytes())
    )
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("agym supports Linux and macOS keyring backends only");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyring_secret_round_trip() {
        let token = r#"{"access_token":"test"}"#;
        let encoded = encode_keyring_secret(token);
        assert_eq!(
            normalize_keyring_secret(encoded.as_bytes()),
            Some(token.to_string())
        );
    }

    #[test]
    fn test_plain_keyring_secret_is_accepted() {
        assert_eq!(
            normalize_keyring_secret(b"plain-token\n"),
            Some("plain-token".to_string())
        );
    }

    #[test]
    fn test_invalid_encoded_keyring_secret_is_rejected() {
        assert_eq!(
            normalize_keyring_secret(b"go-keyring-base64:not-base64"),
            None
        );
    }
}
