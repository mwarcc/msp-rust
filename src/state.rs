
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use crate::errors::{MspError, Result};

const TOKEN_EXPIRY_SKEW_SECS: i64 = 30;

pub const SCHEMA_VERSION: u8 = 1;

#[derive(Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub profile_id: String,
    pub sub_id:     String,
    pub device_id:  String,
    pub region:     String,

    pub access_token:            String,
    pub refresh_token:           String,
    pub access_token_expires_at: i64,
    pub browser_profile: String,
    pub platform:        String,

    #[serde(default)]
    pub proxy_url:     Option<String>,
    #[serde(default)]
    pub enforce_proxy: bool,

    #[serde(default)]
    pub cookies: HashMap<String, SerializableCookie>,

    #[serde(default)]
    pub exported_at: i64,

    #[serde(default)]
    pub schema_version: u8,
}

impl SessionState {
    pub const SCHEMA_VERSION: u8 = SCHEMA_VERSION;
    
    pub fn to_json(&self) -> Result<String> {
        let mut snapshot = self.clone();
        snapshot.schema_version = SCHEMA_VERSION;
        snapshot.exported_at = unix_now();
        Ok(serde_json::to_string_pretty(&snapshot)?)
    }

    pub fn from_json(json: &str) -> Result<Self> {
        let state: Self = serde_json::from_str(json)?;

        if !state.is_schema_compatible() {
            return Err(MspError::Api {
                status: 409,
                body: format!(
                    "Incompatible session schema: found v{}, expected v{SCHEMA_VERSION}",
                    state.schema_version
                ),
            });
        }
        if state.access_token.is_empty() || state.refresh_token.is_empty() {
            return Err(MspError::Api {
                status: 422,
                body: "Session state is missing access_token/refresh_token".into(),
            });
        }

        Ok(state)
    }

    #[must_use]
    pub fn is_access_token_expired(&self) -> bool {
        match now_unix() {
            Ok(now) => now >= self.access_token_expires_at - TOKEN_EXPIRY_SKEW_SECS,
            Err(e) => {
                tracing::error!("System clock unavailable ({e}); treating token as expired.");
                true
            }
        }
    }

    #[must_use]
    pub fn seconds_until_expiry(&self) -> i64 {
        now_unix().map(|now| self.access_token_expires_at - now).unwrap_or(0)
    }

    #[must_use]
    pub fn is_schema_compatible(&self) -> bool {
        self.schema_version == 0 || self.schema_version == SCHEMA_VERSION
    }

    pub fn mark_exported(&mut self) {
        self.exported_at = unix_now();
    }

    #[must_use]
    pub fn cookie_key(domain: &str, path: &str, name: &str) -> String {
        format!("{domain}{path}:{name}")
    }

    pub fn upsert_cookie(&mut self, cookie: SerializableCookie) {
        let key = Self::cookie_key(&cookie.domain, &cookie.path, &cookie.name);
        self.cookies.insert(key, cookie);
    }
}

impl std::fmt::Debug for SessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionState")
            .field("profile_id", &self.profile_id)
            .field("sub_id", &self.sub_id)
            .field("device_id", &self.device_id)
            .field("region", &self.region)
            .field("access_token", &redacted(&self.access_token))
            .field("refresh_token", &redacted(&self.refresh_token))
            .field("access_token_expires_at", &self.access_token_expires_at)
            .field("browser_profile", &self.browser_profile)
            .field("platform", &self.platform)
            .field("proxy_url", &self.proxy_url)
            .field("enforce_proxy", &self.enforce_proxy)
            .field("cookies", &self.cookies.len())
            .field("exported_at", &self.exported_at)
            .field("schema_version", &self.schema_version)
            .finish()
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableCookie {
    pub name:      String,
    pub value:     String,
    pub domain:    String,
    pub path:      String,
    pub secure:    bool,
    pub http_only: bool,

    #[serde(default)]
    pub same_site: Option<String>,

    #[serde(default)]
    pub expires_at: Option<i64>,
}

impl SerializableCookie {
    #[must_use]
    pub fn is_expired(&self) -> bool {
        match (self.expires_at, now_unix()) {
            (Some(exp), Ok(now)) => now >= exp,
            _ => false,
        }
    }
}


fn now_unix() -> std::result::Result<i64, std::time::SystemTimeError> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64)
}


#[inline]
fn unix_now() -> i64 {
    now_unix().unwrap_or(0)
}


#[inline]
fn redacted(secret: &str) -> String {
    if secret.is_empty() {
        "<empty>".to_owned()
    } else {
        format!("<redacted:{} chars>", secret.len())
    }
}