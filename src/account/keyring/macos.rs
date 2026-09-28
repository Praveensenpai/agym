//! macOS Keychain backend implemented through Apple's Security framework.

use super::{
    encode_keyring_secret, normalize_keyring_secret, KEYRING_LABEL, KEYRING_SERVICE_VAL,
    KEYRING_USER_VAL,
};
use anyhow::Result;
use security_framework::passwords::{
    generic_password, set_generic_password_options, PasswordOptions,
};

/// Retrieves the active Antigravity OAuth token from the login Keychain.
pub fn get_current_keyring_token() -> Option<String> {
    let secret = generic_password(PasswordOptions::new_generic_password(
        KEYRING_SERVICE_VAL,
        KEYRING_USER_VAL,
    ))
    .ok()?;

    normalize_keyring_secret(&secret)
}

/// Stores the active Antigravity OAuth token in the login Keychain.
pub fn write_keyring_token(token_json: &str) -> bool {
    let encoded_token = encode_keyring_secret(token_json);
    let mut options = PasswordOptions::new_generic_password(KEYRING_SERVICE_VAL, KEYRING_USER_VAL);
    options.set_label(KEYRING_LABEL);

    set_generic_password_options(encoded_token.as_bytes(), options).is_ok()
}

/// Clears the active Antigravity OAuth token from the login Keychain.
pub fn clear_keyring_token() -> Result<()> {
    let options = PasswordOptions::new_generic_password(KEYRING_SERVICE_VAL, KEYRING_USER_VAL);
    security_framework::passwords::delete_generic_password_options(options)
        .map_err(|error| anyhow::anyhow!("Failed to clear keychain token: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keychain_identity() {
        assert_eq!(KEYRING_SERVICE_VAL, "gemini");
        assert_eq!(KEYRING_USER_VAL, "antigravity");
        assert_eq!(KEYRING_LABEL, "Password for 'antigravity' on 'gemini'");
    }
}
