use anyhow::{anyhow, Result};
use chrono::{DateTime, Local};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn get_oauth_creds() -> (String, String) {
    let cid_enc: &[u8] = &[
        100, 101, 98, 100, 101, 101, 99, 101, 99, 101, 96, 108, 100, 120, 33, 56, 61, 38, 38, 60,
        59, 103, 61, 103, 100, 57, 54, 39, 48, 103, 102, 96, 35, 33, 58, 57, 58, 63, 61, 97, 50,
        97, 101, 102, 48, 37, 123, 52, 37, 37, 38, 123, 50, 58, 58, 50, 57, 48, 32, 38, 48, 39, 54,
        58, 59, 33, 48, 59, 33, 123, 54, 58, 56,
    ];
    let sec_enc: &[u8] = &[
        18, 26, 22, 6, 5, 13, 120, 30, 96, 109, 19, 2, 7, 97, 109, 99, 25, 49, 25, 31, 100, 56, 25,
        23, 109, 38, 13, 22, 97, 47, 99, 36, 17, 20, 51,
    ];

    let cid_dec: Vec<u8> = cid_enc.iter().map(|b| b ^ 0x55).collect();
    let sec_dec: Vec<u8> = sec_enc.iter().map(|b| b ^ 0x55).collect();

    let cid = String::from_utf8(cid_dec).unwrap_or_default();
    let sec = String::from_utf8(sec_dec).unwrap_or_default();
    (cid, sec)
}

const CACHE_TTL_SECONDS: u64 = 300; // 5 minutes

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountQuotaInfo {
    #[serde(default)]
    pub plan_type: Option<String>,
    #[serde(default)]
    pub gemini_percent: Option<u32>,
    #[serde(default)]
    pub claude_percent: Option<u32>,
    pub top_model_name: Option<String>,
    pub top_model_percent: Option<u32>,
    #[serde(default)]
    pub fetched_at: u64,
    #[serde(skip)]
    pub is_fresh: bool,
}

impl AccountQuotaInfo {
    pub fn display_badge(&self) -> String {
        match (self.gemini_percent, self.claude_percent) {
            (Some(gem), Some(cld)) => format!("[Gemini: {}% | Claude: {}%]", gem, cld),
            (Some(gem), None) => format!("[Gemini: {}%]", gem),
            (None, Some(cld)) => format!("[Claude: {}%]", cld),
            _ => match (&self.top_model_name, self.top_model_percent) {
                (Some(name), Some(pct)) => format!("[{} {}% left]", name, pct),
                _ => "[quota unavailable]".to_string(),
            },
        }
    }

    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now.saturating_sub(self.fetched_at) > CACHE_TTL_SECONDS
    }

    pub fn formatted_time(&self) -> String {
        let naive = DateTime::from_timestamp(self.fetched_at as i64, 0);
        match naive {
            Some(utc) => {
                let local: DateTime<Local> = DateTime::from(utc);
                local.format("%Y-%m-%d %H:%M:%S").to_string()
            }
            None => "unknown time".to_string(),
        }
    }
}

pub fn get_cache_file_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".gemini-accounts").join(".quota_cache.json"))
}

pub fn load_quota_cache() -> HashMap<String, AccountQuotaInfo> {
    let path = match get_cache_file_path() {
        Some(p) => p,
        None => return HashMap::new(),
    };

    if !path.exists() {
        return HashMap::new();
    }

    fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

pub fn save_quota_cache(cache: &HashMap<String, AccountQuotaInfo>) {
    if let Some(path) = get_cache_file_path() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_json::to_string_pretty(cache) {
            let _ = fs::write(path, content);
        }
    }
}

pub fn clear_quota_cache() {
    if let Some(path) = get_cache_file_path() {
        let _ = fs::remove_file(path);
    }
}

pub fn fetch_quota_cached(
    account_key: &str,
    auth_path: &Path,
    no_cache: bool,
) -> Result<AccountQuotaInfo> {
    let mut cache = load_quota_cache();

    if !no_cache {
        if let Some(mut cached) = cache.get(account_key).cloned() {
            if !cached.is_expired() {
                cached.is_fresh = false;
                return Ok(cached);
            }
        }
    }

    let mut quota = fetch_quota_live(auth_path)?;
    quota.is_fresh = true;
    cache.insert(account_key.to_string(), quota.clone());
    save_quota_cache(&cache);

    Ok(quota)
}

fn fetch_quota_live(acc_path: &Path) -> Result<AccountQuotaInfo> {
    let content = fs::read_to_string(acc_path)?;
    let mut tok_json: Value = serde_json::from_str(&content)?;

    let mut access_tok = tok_json
        .get("access_token")
        .and_then(|v| v.as_str())
        .or_else(|| {
            tok_json
                .get("token")
                .and_then(|t| t.get("access_token"))
                .and_then(|v| v.as_str())
        })
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("No access token"))?;

    let refresh_tok = tok_json
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .or_else(|| {
            tok_json
                .get("token")
                .and_then(|t| t.get("refresh_token"))
                .and_then(|v| v.as_str())
        })
        .map(|s| s.to_string());

    let client = Client::builder().timeout(Duration::from_secs(4)).build()?;
    let url = "https://cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels";

    let resp = client
        .post(url)
        .header("Authorization", format!("Bearer {}", access_tok))
        .header("Content-Type", "application/json")
        .header("User-Agent", "Antigravity/1.0")
        .json(&serde_json::json!({}))
        .send();

    let mut res_val: Option<Value> = resp.ok().and_then(|r| r.json().ok());

    let has_models = res_val
        .as_ref()
        .and_then(|v| v.get("models"))
        .map(|m| !m.is_null())
        .unwrap_or(false);

    if !has_models {
        if let Some(ref_tok) = refresh_tok {
            let ref_url = "https://oauth2.googleapis.com/token";
            let (cid, sec) = get_oauth_creds();
            let ref_resp = client
                .post(ref_url)
                .form(&[
                    ("grant_type", "refresh_token"),
                    ("refresh_token", ref_tok.as_str()),
                    ("client_id", cid.as_str()),
                    ("client_secret", sec.as_str()),
                ])
                .send();

            if let Ok(ref_r) = ref_resp {
                if let Ok(ref_json) = ref_r.json::<Value>() {
                    if let Some(new_access) = ref_json.get("access_token").and_then(|v| v.as_str())
                    {
                        access_tok = new_access.to_string();

                        if let Some(tok_obj) = tok_json.get_mut("token") {
                            if tok_obj.is_object() {
                                tok_obj["access_token"] = Value::String(new_access.to_string());
                            }
                        } else {
                            tok_json["access_token"] = Value::String(new_access.to_string());
                        }

                        if let Ok(new_content) = serde_json::to_string_pretty(&tok_json) {
                            let _ = fs::write(acc_path, new_content);
                        }

                        let retry_resp = client
                            .post(url)
                            .header("Authorization", format!("Bearer {}", access_tok))
                            .header("Content-Type", "application/json")
                            .header("User-Agent", "Antigravity/1.0")
                            .json(&serde_json::json!({}))
                            .send();

                        res_val = retry_resp.ok().and_then(|r| r.json().ok());
                    }
                }
            }
        }
    }

    let val = res_val.ok_or_else(|| anyhow!("Failed to fetch models"))?;
    let models = val
        .get("models")
        .and_then(|m| m.as_object())
        .ok_or_else(|| anyhow!("No models object"))?;

    let mut gemini_percent: Option<u32> = None;
    let mut claude_percent: Option<u32> = None;

    let gemini_keys = [
        "gemini-2.5-pro",
        "gemini-3.6-flash-high",
        "gemini-3.1-pro-low",
        "gemini-3.1-flash-lite",
    ];
    for key in gemini_keys {
        if let Some(m_info) = models.get(key) {
            if let Some(frac) = m_info
                .get("quotaInfo")
                .and_then(|q| q.get("remainingFraction"))
                .and_then(|f| f.as_f64())
            {
                gemini_percent = Some((frac * 100.0) as u32);
                break;
            }
        }
    }
    if gemini_percent.is_none() {
        for (k, m_info) in models {
            if k.contains("gemini") {
                if let Some(frac) = m_info
                    .get("quotaInfo")
                    .and_then(|q| q.get("remainingFraction"))
                    .and_then(|f| f.as_f64())
                {
                    gemini_percent = Some((frac * 100.0) as u32);
                    break;
                }
            }
        }
    }

    let has_claude_support = models.keys().any(|k| k.contains("claude") || k.contains("gpt"))
        || val.get("tieredModelIds")
            .and_then(|t| t.get("pro"))
            .map_or(false, |arr| arr.as_array().map_or(false, |a| !a.is_empty()));

    let claude_keys = [
        "claude-sonnet-4-6",
        "claude-opus-4-6-thinking",
        "gpt-oss-120b-medium",
    ];
    for key in claude_keys {
        if let Some(m_info) = models.get(key) {
            let frac = m_info
                .get("quotaInfo")
                .and_then(|q| q.get("remainingFraction"))
                .and_then(|f| f.as_f64())
                .unwrap_or(0.0);
            claude_percent = Some((frac * 100.0) as u32);
            break;
        }
    }
    if claude_percent.is_none() && has_claude_support {
        for (k, m_info) in models {
            if k.contains("claude") || k.contains("gpt") {
                let frac = m_info
                    .get("quotaInfo")
                    .and_then(|q| q.get("remainingFraction"))
                    .and_then(|f| f.as_f64())
                    .unwrap_or(0.0);
                claude_percent = Some((frac * 100.0) as u32);
                break;
            }
        }
        if claude_percent.is_none() {
            claude_percent = Some(0);
        }
    }

    let plan_type = tok_json.get("plan_type")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            val.get("subscriptionTier")
                .or_else(|| val.get("planType"))
                .or_else(|| val.get("tier"))
                .or_else(|| val.get("quotaTier"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .or_else(|| {
            if has_claude_support || claude_percent.is_some() {
                Some("Pro".to_string())
            } else {
                Some("Starter".to_string())
            }
        });

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    Ok(AccountQuotaInfo {
        plan_type,
        gemini_percent,
        claude_percent,
        top_model_name: Some("Gemini".to_string()),
        top_model_percent: gemini_percent,
        fetched_at: now,
        is_fresh: true,
    })
}
