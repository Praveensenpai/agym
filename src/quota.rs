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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountQuotaInfo {
    #[serde(default)]
    pub plan_type: Option<String>,
    #[serde(default)]
    pub gemini_week_percent: Option<u32>,
    #[serde(default)]
    pub gemini_window_percent: Option<u32>,
    #[serde(default)]
    pub gemini_percent: Option<u32>,
    #[serde(default)]
    pub claude_week_percent: Option<u32>,
    #[serde(default)]
    pub claude_window_percent: Option<u32>,
    #[serde(default)]
    pub claude_percent: Option<u32>,
    #[serde(default)]
    pub top_model_name: Option<String>,
    #[serde(default)]
    pub top_model_percent: Option<u32>,
    #[serde(default)]
    pub fetched_at: u64,
    #[serde(skip)]
    pub is_fresh: bool,
}

impl AccountQuotaInfo {
    pub fn display_badge(&self) -> String {
        let has_week = self.gemini_week_percent.is_some() || self.claude_week_percent.is_some();
        if has_week {
            let g = match (
                self.gemini_week_percent,
                self.gemini_window_percent.or(self.gemini_percent),
            ) {
                (Some(w), Some(h)) => format!("Gemini: {w}%(W) {h}%(5h)"),
                (Some(w), None) => format!("Gemini: {w}%(W)"),
                (None, Some(h)) => format!("Gemini: {h}%"),
                (None, None) => "Gemini: —".to_string(),
            };
            let c = match (
                self.claude_week_percent,
                self.claude_window_percent.or(self.claude_percent),
            ) {
                (Some(w), Some(h)) => format!("Claude: {w}%(W) {h}%(5h)"),
                (Some(w), None) => format!("Claude: {w}%(W)"),
                (None, Some(h)) => format!("Claude: {h}%"),
                (None, None) => "Claude: —".to_string(),
            };
            return format!("[{g} | {c}]");
        }

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

fn read_token_data(acc_path: &Path) -> Result<(Value, String, Option<String>)> {
    let content = fs::read_to_string(acc_path)?;
    let tok_json: Value = serde_json::from_str(&content)?;
    let access_tok = tok_json
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
    Ok((tok_json, access_tok, refresh_tok))
}

fn try_refresh_token(
    client: &Client,
    ref_tok: &str,
    acc_path: &Path,
    tok_json: &mut Value,
) -> Option<String> {
    let ref_url = "https://oauth2.googleapis.com/token";
    let (cid, sec) = get_oauth_creds();
    let resp = client
        .post(ref_url)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", ref_tok),
            ("client_id", cid.as_str()),
            ("client_secret", sec.as_str()),
        ])
        .send()
        .ok()?;
    let ref_json: Value = resp.json().ok()?;
    let new_access = ref_json.get("access_token")?.as_str()?;
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
    Some(new_access.to_string())
}

fn parse_quota_summary(val: &Value, now: u64) -> Option<AccountQuotaInfo> {
    let groups = val.get("groups")?.as_array()?;
    let mut info = AccountQuotaInfo {
        plan_type: Some("Pro".to_string()),
        gemini_week_percent: None,
        gemini_window_percent: None,
        gemini_percent: None,
        claude_week_percent: None,
        claude_window_percent: None,
        claude_percent: None,
        top_model_name: Some("Gemini".to_string()),
        top_model_percent: None,
        fetched_at: now,
        is_fresh: true,
    };
    for g in groups {
        let dname = g
            .get("displayName")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_lowercase();
        let is_gemini = dname.contains("gemini");
        let is_claude = dname.contains("claude") || dname.contains("gpt") || dname.contains("3p");
        let buckets = g.get("buckets").and_then(|b| b.as_array());
        for b in buckets.into_iter().flatten() {
            let window = b.get("window").and_then(|w| w.as_str()).unwrap_or_default();
            let frac = b
                .get("remainingFraction")
                .and_then(|f| f.as_f64())
                .unwrap_or(0.0);
            let pct = (frac * 100.0).round() as u32;
            if is_gemini {
                if window == "weekly" {
                    info.gemini_week_percent = Some(pct);
                } else {
                    info.gemini_window_percent = Some(pct);
                    info.gemini_percent = Some(pct);
                    info.top_model_percent = Some(pct);
                }
            } else if is_claude {
                if window == "weekly" {
                    info.claude_week_percent = Some(pct);
                } else {
                    info.claude_window_percent = Some(pct);
                    info.claude_percent = Some(pct);
                }
            }
        }
    }
    Some(info)
}

fn parse_available_models(
    val: &Value,
    plan_type: Option<String>,
    now: u64,
) -> Result<AccountQuotaInfo> {
    let models = val
        .get("models")
        .and_then(|m| m.as_object())
        .ok_or_else(|| anyhow!("No models in response"))?;
    let mut gemini_percent: Option<u32> = None;
    let gem_keys = ["gemini-2.5-pro", "gemini-3.1-pro-high", "gemini-3-flash"];
    for key in gem_keys {
        if let Some(m) = models.get(key) {
            if let Some(f) = m
                .pointer("/quotaInfo/remainingFraction")
                .and_then(|v| v.as_f64())
            {
                gemini_percent = Some((f * 100.0).round() as u32);
                break;
            }
        }
    }
    let mut claude_percent: Option<u32> = None;
    let cld_keys = ["claude-sonnet-4-6", "claude-opus-4-6-thinking"];
    for key in cld_keys {
        if let Some(m) = models.get(key) {
            if let Some(f) = m
                .pointer("/quotaInfo/remainingFraction")
                .and_then(|v| v.as_f64())
            {
                claude_percent = Some((f * 100.0).round() as u32);
                break;
            }
        }
    }
    Ok(AccountQuotaInfo {
        plan_type,
        gemini_week_percent: None,
        gemini_window_percent: gemini_percent,
        gemini_percent,
        claude_week_percent: None,
        claude_window_percent: claude_percent,
        claude_percent,
        top_model_name: Some("Gemini".to_string()),
        top_model_percent: gemini_percent,
        fetched_at: now,
        is_fresh: true,
    })
}

fn fetch_quota_live(acc_path: &Path) -> Result<AccountQuotaInfo> {
    let (mut tok_json, mut access_tok, refresh_tok) = read_token_data(acc_path)?;
    let client = Client::builder().timeout(Duration::from_secs(5)).build()?;
    let summary_url = "https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary";
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut resp = client
        .post(summary_url)
        .header("Authorization", format!("Bearer {access_tok}"))
        .header("Content-Type", "application/json")
        .header("User-Agent", "Antigravity/1.0")
        .json(&serde_json::json!({}))
        .send();

    if resp.as_ref().map(|r| r.status().as_u16()).unwrap_or(0) == 401 {
        if let Some(ref ref_tok) = refresh_tok {
            if let Some(new_tok) = try_refresh_token(&client, ref_tok, acc_path, &mut tok_json) {
                access_tok = new_tok;
                resp = client
                    .post(summary_url)
                    .header("Authorization", format!("Bearer {access_tok}"))
                    .header("Content-Type", "application/json")
                    .header("User-Agent", "Antigravity/1.0")
                    .json(&serde_json::json!({}))
                    .send();
            }
        }
    }

    if let Ok(r) = resp {
        if r.status().is_success() {
            if let Ok(summary_val) = r.json::<Value>() {
                if let Some(info) = parse_quota_summary(&summary_val, now) {
                    return Ok(info);
                }
            }
        }
    }

    let models_url = "https://cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels";
    let models_resp = client
        .post(models_url)
        .header("Authorization", format!("Bearer {access_tok}"))
        .header("Content-Type", "application/json")
        .header("User-Agent", "Antigravity/1.0")
        .json(&serde_json::json!({}))
        .send()?;
    let models_val: Value = models_resp.json()?;
    let plan = tok_json
        .get("plan_type")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    parse_available_models(&models_val, plan, now)
}
