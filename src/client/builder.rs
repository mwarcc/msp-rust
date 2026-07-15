use std::sync::Arc;
use std::time::Duration;

use wreq::header::{HeaderMap, HeaderValue};
use wreq::Proxy;
use wreq_util::{Emulation, Platform, Profile};

use crate::errors::{MspError, Result};
use crate::models::MspSession;
use crate::state::SessionState;
use super::cookies::PersistentJar;
use super::stealth::StealthConfig;
use super::MspClient;
use rand::seq::SliceRandom;

pub struct MspClientBuilder {
    device_id:               Option<String>,
    profile:                 Profile,
    platform:                Platform,
    randomize_platform:      bool,
    randomize_profile_brand: Option<BrowserBrand>,
    proxy_url:               Option<String>,
    enforce_proxy:           bool,
    timeout:                 Duration,
    connect_timeout:         Duration,
    locale:                  Option<String>,
    stealth:                 StealthConfig,
    restore_state:           Option<SessionState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserBrand { Chrome, Firefox, Any }

impl Default for MspClientBuilder {
    fn default() -> Self {
        Self {
            device_id:               None,
            profile:                 Profile::Chrome137,
            platform:                Platform::Windows,
            randomize_platform:      false,
            randomize_profile_brand: None,
            proxy_url:               None,
            enforce_proxy:           false,
            timeout:                 Duration::from_secs(30),
            connect_timeout:         Duration::from_secs(10),
            locale:                  None,
            stealth:                 StealthConfig::default(),
            restore_state:           None,
        }
    }
}

impl MspClientBuilder {
    pub fn device_id(mut self, id: impl Into<String>) -> Self {
        self.device_id = Some(id.into());
        self
    }

    pub fn profile(mut self, profile: Profile) -> Self {
        self.profile = profile;
        self.randomize_profile_brand = None;
        self
    }

    pub fn platform(mut self, platform: Platform) -> Self {
        self.platform = platform;
        self.randomize_platform = false;
        self
    }

    pub fn random_platform(mut self) -> Self {
        self.randomize_platform = true;
        self
    }

    pub fn random_profile(mut self, brand: BrowserBrand) -> Self {
        self.randomize_profile_brand = Some(brand);
        self
    }

    pub fn proxy(mut self, proxy_url: impl Into<String>) -> Self {
        self.proxy_url = Some(proxy_url.into());
        self
    }

    pub fn enforce_proxy(mut self, enforce: bool) -> Self {
        self.enforce_proxy = enforce;
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    pub fn locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = Some(locale.into());
        self
    }

    pub fn stealth(mut self, min: Duration, max: Duration) -> Self {
        self.stealth = StealthConfig::enabled(min, max);
        self
    }

    pub fn from_state(mut self, state: SessionState) -> Self {
        self.restore_state = Some(state);
        self
    }

    pub fn build(self) -> Result<MspClient> {
        if let Some(state) = self.restore_state {
            return Self::build_from_state(
                state,
                self.timeout,
                self.connect_timeout,
                self.locale,
                self.stealth,
            );
        }

        if self.enforce_proxy && self.proxy_url.is_none() {
            return Err(MspError::InvalidProxy(
                "Proxy enforcement is enabled but no proxy URL was provided".into()
            ));
        }

        let mut rng = rand::thread_rng();

        let platform = if self.randomize_platform {
            let platforms = [Platform::Windows, Platform::MacOS, Platform::Linux];
            *platforms.choose(&mut rng).unwrap_or(&Platform::Windows)
        } else {
            self.platform
        };

        let profile = match self.randomize_profile_brand {
            Some(BrowserBrand::Chrome) => {
                let p = [
                    Profile::Chrome133,
                    Profile::Chrome135,
                    Profile::Chrome136,
                    Profile::Chrome137,
                ];
                *p.choose(&mut rng).unwrap_or(&Profile::Chrome137)
            }
            Some(BrowserBrand::Firefox) => {
                let p = [
                    Profile::Firefox133,
                    Profile::Firefox136,
                    Profile::Firefox139,
                ];
                *p.choose(&mut rng).unwrap_or(&Profile::Firefox139)
            }
            Some(BrowserBrand::Any) => {
                let p = [
                    Profile::Chrome136,
                    Profile::Chrome137,
                    Profile::Firefox139,
                ];
                *p.choose(&mut rng).unwrap_or(&Profile::Chrome137)
            }
            None => self.profile,
        };

        let platform = coherent_platform(profile, platform);

        let device_id = self.device_id.unwrap_or_else(|| {
            uuid::Uuid::new_v4().to_string().replace('-', "").to_uppercase()
        });

        let accept_language = self.locale.clone().unwrap_or_else(default_locale);

        let jar = Arc::new(PersistentJar::new());

        let http = build_http_client(
            profile,
            platform,
            &self.proxy_url,
            self.enforce_proxy,
            self.timeout,
            self.connect_timeout,
            jar.clone(),
            &accept_language,
        )?;

        Ok(MspClient::from_parts(
            http,
            device_id,
            profile,
            platform,
            self.proxy_url,
            self.enforce_proxy,
            jar,
            self.stealth,
        ))
    }

    fn build_from_state(
        state:           SessionState,
        timeout:         Duration,
        connect_timeout: Duration,
        locale:          Option<String>,
        stealth:         StealthConfig,
    ) -> Result<MspClient> {
        if !state.is_schema_compatible() {
            return Err(MspError::InvalidProxy(format!(
                "SessionState schema version {} is not compatible with the current library (v{})",
                state.schema_version,
                SessionState::SCHEMA_VERSION,
            )));
        }

        if state.enforce_proxy && state.proxy_url.is_none() {
            return Err(MspError::InvalidProxy(
                "Restored state enforces a proxy, but no proxy URL was found in the session".into()
            ));
        }

        let profile  = parse_profile(&state.browser_profile);
        let platform = coherent_platform(profile, parse_platform(&state.platform));

        let accept_language = locale
            .unwrap_or_else(|| locale_for_region(&state.region).to_string());


        let jar = Arc::new(PersistentJar::from_state(&state.cookies));

        let http = build_http_client(
            profile,
            platform,
            &state.proxy_url,
            state.enforce_proxy,
            timeout,
            connect_timeout,
            jar.clone(),
            &accept_language,
        )?;

        let session = MspSession {
            access_token:            state.access_token,
            refresh_token:           state.refresh_token,
            profile_id:              state.profile_id,
            sub_id:                  state.sub_id,
            device_id:               state.device_id.clone(),
            access_token_expires_at: state.access_token_expires_at,
            region:                  state.region,
        };

        let client = MspClient::from_parts(
            http,
            state.device_id,
            profile,
            platform,
            state.proxy_url,
            state.enforce_proxy,
            jar,
            stealth,
        );

        {
            let store = client.session().inner();
            let rt = tokio::runtime::Handle::try_current();
            match rt {
                Ok(handle) => {
                    tokio::task::block_in_place(|| {
                        handle.block_on(async move {
                            *store.write().await = Some(session);
                        });
                    });
                }
                Err(_) => {
                    tokio::runtime::Runtime::new()
                        .expect("Failed to build temporary Tokio runtime")
                        .block_on(async move {
                            *store.write().await = Some(session);
                        });
                }
            }
        }

        Ok(client)
    }
}


#[allow(clippy::too_many_arguments)]
fn build_http_client(
    profile:         Profile,
    platform:        Platform,
    proxy_url:       &Option<String>,
    enforce_proxy:   bool,
    timeout:         Duration,
    connect_timeout: Duration,
    jar:             Arc<PersistentJar>,
    accept_language: &str,
) -> Result<wreq::Client> {
    let emulation = Emulation::builder()
        .profile(profile)
        .platform(platform)
        .build();

    let mut builder = wreq::Client::builder()
        .emulation(emulation)
        .cookie_provider(jar)
        .timeout(timeout)
        .connect_timeout(connect_timeout)
        .tcp_nodelay(true)
        .https_only(true)
        .referer(false)
        .default_headers(locale_override(accept_language));

    if let Some(ref url) = proxy_url {
        let normalized = normalize_proxy_string(url);
        let proxy = Proxy::all(&normalized)
            .map_err(|e| MspError::InvalidProxy(format!("{e}: {normalized}")))?;
        builder = builder.proxy(proxy);
    } else if enforce_proxy {
        return Err(MspError::InvalidProxy(
            "Enforce proxy constraint is active, but proxy selection is unassigned".into()
        ));
    }

    Ok(builder.build()?)
}

fn locale_override(accept_language: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(accept_language) {
        h.insert("accept-language", v);
    }
    h
}

fn default_locale() -> String {
    "fr-FR,fr;q=0.9,en-US;q=0.8,en;q=0.7".to_string()
}


fn locale_for_region(region: &str) -> &'static str {
    match region.to_ascii_lowercase().as_str() {
        "us" | "en" | "english" => "en-US,en;q=0.9",
        "gb" | "uk"             => "en-GB,en;q=0.9",
        "fr" | "french"         => "fr-FR,fr;q=0.9,en-US;q=0.8,en;q=0.7",
        "de" | "german"         => "de-DE,de;q=0.9,en-US;q=0.8,en;q=0.7",
        "es" | "spanish"        => "es-ES,es;q=0.9,en-US;q=0.8,en;q=0.7",
        "it" | "italian"        => "it-IT,it;q=0.9,en-US;q=0.8,en;q=0.7",
        "nl" | "dutch"          => "nl-NL,nl;q=0.9,en;q=0.8",
        "da" | "danish"         => "da-DK,da;q=0.9,en;q=0.8",
        "sv" | "swedish"        => "sv-SE,sv;q=0.9,en;q=0.8",
        "no" | "norwegian"      => "nb-NO,nb;q=0.9,en;q=0.8",
        "fi" | "finnish"        => "fi-FI,fi;q=0.9,en;q=0.8",
        "pl" | "polish"         => "pl-PL,pl;q=0.9,en;q=0.8",
        "tr" | "turkish"        => "tr-TR,tr;q=0.9,en;q=0.8",
        _                       => "fr-FR,fr;q=0.9,en-US;q=0.8,en;q=0.7",
    }
}

fn coherent_platform(profile: Profile, requested: Platform) -> Platform {
    let name = format!("{profile:?}");
    if name.starts_with("Safari") && !(name.contains("Ios") || name.contains("IPad")) {
        Platform::MacOS
    } else {
        requested
    }
}

fn normalize_proxy_string(raw: &str) -> String {
    let (scheme, rest) = if let Some(stripped) = raw.strip_prefix("http://") {
        ("http", stripped)
    } else if let Some(stripped) = raw.strip_prefix("https://") {
        ("https", stripped)
    } else if let Some(stripped) = raw.strip_prefix("socks5://") {
        ("socks5", stripped)
    } else if let Some(stripped) = raw.strip_prefix("socks5h://") {
        ("socks5h", stripped)
    } else {
        ("http", raw)
    };

    let parts: Vec<&str> = rest.split(':').collect();
    if parts.len() == 4 {
        let host = parts[0];
        let port = parts[1];
        let user = parts[2];
        let pass = parts[3];
        format!("{scheme}://{user}:{pass}@{host}:{port}")
    } else {
        raw.to_string()
    }
}

fn parse_profile(s: &str) -> Profile {
    match s {
        "Chrome118"  => Profile::Chrome118,
        "Chrome124"  => Profile::Chrome124,
        "Chrome128"  => Profile::Chrome128,
        "Chrome131"  => Profile::Chrome131,
        "Chrome133"  => Profile::Chrome133,
        "Chrome135"  => Profile::Chrome135,
        "Chrome136"  => Profile::Chrome136,
        "Chrome137"  => Profile::Chrome137,
        "Firefox117" => Profile::Firefox117,
        "Firefox128" => Profile::Firefox128,
        "Firefox133" => Profile::Firefox133,
        "Firefox136" => Profile::Firefox136,
        "Firefox139" => Profile::Firefox139,
        other => {
            tracing::warn!(
                "Unknown browser profile '{}' in SessionState — falling back to Chrome137", other
            );
            Profile::Chrome137
        }
    }
}

fn parse_platform(s: &str) -> Platform {
    match s {
        "Windows" => Platform::Windows,
        "MacOS"   => Platform::MacOS,
        "Linux"   => Platform::Linux,
        other => {
            tracing::warn!(
                "Unknown platform '{}' in SessionState — falling back to Windows", other
            );
            Platform::Windows
        }
    }
}

