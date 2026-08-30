//! Token parsing and authentication utilities for Google OAuth / JWT tokens.
//!
//! This module handles decoding Google OAuth `id_token` JWTs, base64 payload
//! parsing with padding fallback, and tokeninfo endpoint querying.

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use std::time::Duration;

/// Google OAuth2 tokeninfo validation endpoint used to resolve account email
/// when no `id_token` JWT is present in newer token payloads.
pub const GOOGLE_TOKENINFO_URL: &str =
    "https://www.googleapis.com/oauth2/v1/tokeninfo?access_token=";

/// Minimum number of dot-separated segments in a standard JWT structure (header and payload).
pub const JWT_MIN_PARTS: usize = 2;

/// Zero-indexed position of the payload segment in a standard JWT.
pub const JWT_PAYLOAD_INDEX: usize = 1;

/// Request timeout in seconds for tokeninfo network validation queries.
pub const TOKENINFO_TIMEOUT_SECS: u64 = 5;

/// Extracts the user email from a raw token JSON string.
///
/// Supports legacy `id_token` JWT payloads (both top-level and nested under `token`),
/// and falls back to Google's `tokeninfo` endpoint when only an `access_token` is available.
pub fn extract_email_from_token_json(token_json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(token_json).ok()?;

    // Try id_token JWT path first (legacy format)
    let id_token = v.get("id_token").and_then(|t| t.as_str()).or_else(|| {
        v.get("token")
            .and_then(|t| t.get("id_token"))
            .and_then(|t| t.as_str())
    });

    if let Some(id_token) = id_token {
        if let Some(email) = extract_email_from_jwt(id_token) {
            return Some(email);
        }
    }

    // Fallback: new token format has no id_token — resolve email via Google tokeninfo API
    let access_token = v.get("access_token").and_then(|t| t.as_str()).or_else(|| {
        v.get("token")
            .and_then(|t| t.get("access_token"))
            .and_then(|t| t.as_str())
    })?;

    fetch_email_from_tokeninfo(access_token)
}

/// Extracts the email claim from an encoded JWT `id_token` string.
///
/// Splits the JWT by `.` separators, extracts the payload segment,
/// base64-decodes it, and reads the `"email"` field from the JSON payload.
pub fn extract_email_from_jwt(id_token: &str) -> Option<String> {
    let parts: Vec<&str> = id_token.split('.').collect();
    if parts.len() < JWT_MIN_PARTS {
        return None;
    }

    let payload_b64 = parts[JWT_PAYLOAD_INDEX];
    let bytes = decode_jwt_payload(payload_b64)?;

    let payload: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    payload
        .get("email")
        .and_then(|e| e.as_str())
        .map(ToString::to_string)
}

/// Decodes a base64-encoded JWT payload segment, handling missing padding gracefully.
///
/// First attempts unpadded URL-safe base64 decoding. If decoding fails, normalizes
/// padding characters (`=`) and attempts standard base64 decoding.
pub fn decode_jwt_payload(payload_b64: &str) -> Option<Vec<u8>> {
    if let Ok(bytes) = URL_SAFE_NO_PAD.decode(payload_b64) {
        return Some(bytes);
    }

    let mut padded = payload_b64.to_string();
    let rem = padded.len() % 4;
    if rem == 2 {
        padded.push_str("==");
    } else if rem == 3 {
        padded.push('=');
    }

    STANDARD.decode(padded).ok()
}

/// Fetches the user email associated with an OAuth `access_token` from Google's tokeninfo API.
///
/// Uses a strict timeout to avoid stalling execution in offline or high-latency environments.
pub fn fetch_email_from_tokeninfo(access_token: &str) -> Option<String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(TOKENINFO_TIMEOUT_SECS))
        .build()
        .ok()?;

    let url = format!("{}{}", GOOGLE_TOKENINFO_URL, access_token);
    let resp = client.get(&url).send().ok()?;
    if !resp.status().is_success() {
        return None;
    }

    let info: serde_json::Value = resp.json().ok()?;
    info.get("email")
        .and_then(|e| e.as_str())
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_email_from_jwt_unpadded() {
        // {"alg":"none"}.{"email":"user@example.com","sub":"12345"}
        // Payload base64 (URL-safe unpadded): eyJlbWFpbCI6InVzZXJAZXhhbXBsZS5jb20iLCJzdWIiOiIxMjM0NSJ9
        let jwt =
            "eyJhbGciOiJub25lIn0.eyJlbWFpbCI6InVzZXJAZXhhbXBsZS5jb20iLCJzdWIiOiIxMjM0NSJ9.sig";
        let email = extract_email_from_jwt(jwt);
        assert_eq!(email, Some("user@example.com".to_string()));
    }

    #[test]
    fn test_extract_email_from_jwt_padded() {
        // Payload: {"email":"a@b.c"} -> eyJlbWFpbCI6ImFAYi5jIn0 (len 22 -> rem 2)
        let jwt = "header.eyJlbWFpbCI6ImFAYi5jIn0.sig";
        let email = extract_email_from_jwt(jwt);
        assert_eq!(email, Some("a@b.c".to_string()));
    }

    #[test]
    fn test_extract_email_nested_token_structure() {
        let token_json = r#"{
            "token": {
                "id_token": "header.eyJlbWFpbCI6Im5lc3RlZEBleGFtcGxlLmNvbSJ9.sig"
            }
        }"#;
        let email = extract_email_from_token_json(token_json);
        assert_eq!(email, Some("nested@example.com".to_string()));
    }

    #[test]
    fn test_extract_email_top_level_id_token() {
        let token_json = r#"{
            "id_token": "header.eyJlbWFpbCI6InRvcEBleGFtcGxlLmNvbSJ9.sig"
        }"#;
        let email = extract_email_from_token_json(token_json);
        assert_eq!(email, Some("top@example.com".to_string()));
    }

    #[test]
    fn test_extract_email_invalid_tokens() {
        assert_eq!(extract_email_from_token_json(""), None);
        assert_eq!(extract_email_from_token_json("not json"), None);
        assert_eq!(extract_email_from_token_json(r#"{"other": "data"}"#), None);
        assert_eq!(extract_email_from_jwt("only_one_part"), None);
        assert_eq!(extract_email_from_jwt("header.invalid_base64!@#.sig"), None);
    }

    #[test]
    fn test_decode_jwt_payload_malformed() {
        assert_eq!(decode_jwt_payload("invalid base64 chars %%%"), None);
        assert!(decode_jwt_payload("").is_some());
    }

    #[test]
    fn test_extract_email_from_jwt_missing_email_claim() {
        // {"sub":"12345"}
        let jwt = "header.eyJzdWIiOiIxMjM0NSJ9.sig";
        assert_eq!(extract_email_from_jwt(jwt), None);
    }
}
