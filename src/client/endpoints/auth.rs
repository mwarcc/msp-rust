use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tokio::task::AbortHandle;
use wreq::ws::message::Message;
use wreq::Client;

use crate::{
    errors::{MspError, Result},
    event_bus::EventBus,
    events::parse_frame,
    models::MspSession,
    session::SessionStore,
};

use super::super::http::{build_headers, ensure_no_error, required_str, ContentType, ORIGIN};

const TOKEN_ENDPOINT:    &str = "https://eu-secure.mspapis.com/loginidentity/connect/token";
const PROFILES_ENDPOINT: &str = "https://eu.mspapis.com/profileidentity/v1/logins/{sub}/profiles";
const PRESENCE_WS_URL:    &str =
    "wss://gameserver-eu.mspapis.com/presenceserver/instance/socket.io?EIO=3&transport=websocket";

const GAME_ID:       &str = "j68d";
const CLIENT_ID:     &str = "unity.client";
const CLIENT_SECRET: &str = "secret";

const RELOGIN_MIN_SECS: u64 = 2 * 60 * 60;
const RELOGIN_MAX_SECS: u64 = 3 * 60 * 60;

const LOGIN_MAX_ATTEMPTS: u32      = 4;
const LOGIN_BACKOFF_BASE: Duration = Duration::from_secs(2);
const LOGIN_BACKOFF_MAX:  Duration = Duration::from_secs(30);

const PRESENCE_BACKOFF_MIN: Duration = Duration::from_secs(2);
const PRESENCE_BACKOFF_MAX: Duration = Duration::from_secs(60);

const PRESENCE_PING_INTERVAL: Duration = Duration::from_secs(5);
const PRESENCE_READ_TIMEOUT:  Duration = Duration::from_secs(45);
const PRESENCE_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);

const DEFAULT_TOKEN_LIFETIME_SECS: i64 = 3600;

type PresenceSlot = Arc<Mutex<Option<AbortHandle>>>;


pub struct AuthEndpoint<'c> {
    pub(crate) http:      &'c Client,
    pub(crate) session:   &'c SessionStore,
    pub(crate) device_id: &'c str,
    pub(crate) event_bus: &'c EventBus,
}

impl<'c> AuthEndpoint<'c> {
    #[tracing::instrument(
        name  = "msp_login",
        skip_all,
        fields(username = %username, region = %region, device_id = %self.device_id)
    )]
    pub async fn login(&self, username: &str, password: &str, region: &str) -> Result<MspSession> {
        let region = region.to_uppercase();

        let session = self.run_login_flow(username, password, &region).await?;
        self.session.set(session.clone()).await;

        tracing::info!(profile_id = %session.profile_id, "Session successfully initialized.");

        let presence_slot: PresenceSlot = Arc::new(Mutex::new(None));
        self.spawn_presence(session.clone(), region.clone(), presence_slot.clone()).await;
        self.spawn_relogin(username.to_owned(), password.to_owned(), region, presence_slot);

        Ok(session)
    }

    /// Silently rotate the access token using the stored refresh token.
    ///
    /// Updates the session store in-place; subsequent calls pick up the new token.
    #[tracing::instrument(name = "msp_refresh", skip_all)]
    pub async fn refresh(&self) -> Result<()> {
        let session  = self.session.get().await?;
        let acr_base = self.acr_base();

        tracing::debug!(profile_id = %session.profile_id, "Rotating access token silently...");

        let new_session = self
            .refresh_grant(
                &session.refresh_token,
                &acr_base,
                &session.profile_id,
                &session.sub_id,
                &session.region,
            )
            .await?;

        self.session.set(new_session).await;
        tracing::info!("Access token rotated successfully.");
        Ok(())
    }


    #[inline]
    fn acr_base(&self) -> String {
        format!("gameId:{GAME_ID} deviceId:{}", self.device_id)
    }

    async fn run_login_flow(&self, username: &str, password: &str, region: &str) -> Result<MspSession> {
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            match self.login_flow_once(username, password, region).await {
                Ok(session) => return Ok(session),
                Err(e) if attempt >= LOGIN_MAX_ATTEMPTS => {
                    tracing::error!(attempt, "Login flow exhausted retries.");
                    return Err(e);
                }
                Err(e) => {
                    let backoff = backoff_with_jitter(LOGIN_BACKOFF_BASE, LOGIN_BACKOFF_MAX, attempt);
                    tracing::warn!(
                        attempt,
                        retry_in_ms = %backoff.as_millis(),
                        "Login flow failed: {e:?}. Retrying."
                    );
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    }

    async fn login_flow_once(&self, username: &str, password: &str, region: &str) -> Result<MspSession> {
        let acr_base = self.acr_base();

        tracing::debug!("Running password grant...");
        let (access_token, refresh_token, sub) =
            self.password_grant(region, username, password, &acr_base).await?;

        tracing::debug!(sub = %sub, "Resolving profile ID...");
        let profile_id = self.fetch_profile_id(&sub, region, &access_token).await?;

        tracing::debug!(profile_id = %profile_id, "Exchanging refresh token for profile-scoped session...");
        self.refresh_grant(&refresh_token, &acr_base, &profile_id, &sub, region).await
    }

    async fn spawn_presence(&self, session: MspSession, region: String, slot: PresenceSlot) {
        let http = self.http.clone();
        let bus  = self.event_bus.clone();

        let handle = tokio::spawn(async move {
            presence_supervisor(http, session, region, bus).await;
        });

        let mut guard = slot.lock().await;
        if let Some(previous) = guard.replace(handle.abort_handle()) {
            previous.abort();
            tracing::debug!("Aborted stale presence task before installing a new one.");
        }
    }

    fn spawn_relogin(&self, username: String, password: String, region: String, slot: PresenceSlot) {
        let http      = self.http.clone();
        let session   = self.session.clone();
        let event_bus = self.event_bus.clone();
        let device_id = self.device_id.to_owned();

        tokio::spawn(async move {
            loop {
                let delay_secs = {
                    use rand::Rng;
                    rand::thread_rng().gen_range(RELOGIN_MIN_SECS..=RELOGIN_MAX_SECS)
                };
                tracing::debug!(delay_secs, "Next automatic re-login scheduled.");
                tokio::time::sleep(Duration::from_secs(delay_secs)).await;

                tracing::info!("Executing scheduled re-login for '{username}'...");

                let endpoint = AuthEndpoint {
                    http:      &http,
                    session:   &session,
                    device_id: &device_id,
                    event_bus: &event_bus,
                };

                match endpoint.run_login_flow(&username, &password, &region).await {
                    Ok(new_session) => {
                        session.set(new_session.clone()).await;
                        tracing::info!(
                            profile_id = %new_session.profile_id,
                            "Re-login successful. Session refreshed."
                        );
                        endpoint.spawn_presence(new_session, region.clone(), slot.clone()).await;
                    }
                    Err(e) => {
                        tracing::error!("Scheduled re-login failed: {e:?}. Will retry next cycle.");
                    }
                }
            }
        });
    }

    async fn password_grant(
        &self,
        region:   &str,
        username: &str,
        password: &str,
        acr_base: &str,
    ) -> Result<(String, String, String)> {
        #[derive(Serialize)]
        struct Body<'a> {
            client_id:     &'a str,
            client_secret: &'a str,
            grant_type:    &'a str,
            scope:         &'a str,
            username:      String,
            password:      &'a str,
            acr_values:    &'a str,
        }

        let body = serde_urlencoded::to_string(Body {
            client_id:     client_id(),
            client_secret: client_secret(),
            grant_type:    "password",
            scope:         "openid nebula offline_access",
            username:      format!("{region}|{username}"),
            password,
            acr_values:    acr_base,
        })?;

        let response = self
            .post_token(build_headers(ContentType::Form, None), body)
            .await?;

        let access_token  = required_str(&response, "access_token")?.to_owned();
        let refresh_token = required_str(&response, "refresh_token")?.to_owned();
        let sub           = decode_jwt_sub(&access_token)?;

        Ok((access_token, refresh_token, sub))
    }

    async fn fetch_profile_id(&self, sub: &str, region: &str, bearer_token: &str) -> Result<String> {
        let url    = format!("{}?region={region}&pageSize=1", PROFILES_ENDPOINT.replace("{sub}", sub));
        let bearer = format!("Bearer {bearer_token}");

        let response: Value = self
            .http
            .get(&url)
            .headers(build_headers(ContentType::Json, Some(&bearer)))
            .send()
            .await?
            .json()
            .await?;

        response
            .get(0)
            .and_then(|p| p.get("id"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| MspError::Api {
                status: 404,
                body:   format!("Profile resolution failed — sub='{sub}' region='{region}'"),
            })
    }

    async fn refresh_grant(
        &self,
        refresh_token: &str,
        acr_base:      &str,
        profile_id:    &str,
        sub_id:        &str,
        region:        &str,
    ) -> Result<MspSession> {
        #[derive(Serialize)]
        struct Body<'a> {
            grant_type:    &'a str,
            refresh_token: &'a str,
            acr_values:    String,
        }

        let body = serde_urlencoded::to_string(Body {
            grant_type:    "refresh_token",
            refresh_token,
            acr_values:    format!("{acr_base} profileId:{profile_id}"),
        })?;

        let basic    = basic_auth();
        let response = self
            .post_token(build_headers(ContentType::Form, Some(&basic)), body)
            .await?;

        let access_token  = required_str(&response, "access_token")?.to_owned();
        let refresh_token = required_str(&response, "refresh_token")?.to_owned();
        let expires_at    = decode_jwt_exp(&access_token)
            .unwrap_or_else(|_| unix_now() + DEFAULT_TOKEN_LIFETIME_SECS);

        Ok(MspSession {
            access_token,
            refresh_token,
            profile_id:              profile_id.to_owned(),
            sub_id:                  sub_id.to_owned(),
            device_id:               self.device_id.to_owned(),
            access_token_expires_at: expires_at,
            region:                  region.to_owned(),
        })
    }

    async fn post_token(&self, headers: wreq::header::HeaderMap, body: String) -> Result<Value> {
        let response: Value = self
            .http
            .post(TOKEN_ENDPOINT)
            .headers(headers)
            .body(body)
            .send()
            .await?
            .json()
            .await?;

        ensure_no_error(response)
    }
}

async fn presence_supervisor(http: Client, session: MspSession, region: String, bus: EventBus) {
    let mut backoff = PRESENCE_BACKOFF_MIN;

    loop {
        match run_presence_websocket(&http, &session, &region, &bus).await {
            Ok(()) => {
                tracing::warn!("Presence socket closed. Reconnecting.");
                backoff = PRESENCE_BACKOFF_MIN;
            }
            Err(e) => {
                tracing::error!("Presence socket error: {e:?}. Reconnecting.");
            }
        }

        let jitter = Duration::from_millis({
            use rand::Rng;
            rand::thread_rng().gen_range(0..1_000)
        });
        tokio::time::sleep(backoff + jitter).await;
        backoff = (backoff * 2).min(PRESENCE_BACKOFF_MAX);
    }
}

async fn run_presence_websocket(
    http:    &Client,
    session: &MspSession,
    region:  &str,
    bus:     &EventBus,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing::info!("Connecting to Presence WebSocket...");

    let ws = http
        .websocket(PRESENCE_WS_URL)
        .header("origin", ORIGIN)
        .send()
        .await?;

    let (mut write, mut read) = ws.into_websocket().await?.split();

    recv_expecting(&mut read, "server hello").await?;
    write.send(Message::text("2")).await?;
    recv_expecting(&mut read, "socket.io connect (40)").await?;

    let session_uuid = uuid::Uuid::new_v4().to_string();

    write.send(engine_frame("500", json!({ "pingId": 1, "lastPingDelay": 819_447 }))).await?;
    write.send(engine_frame("10", json!({
        "username":      session.profile_id,
        "access_token":  session.access_token,
        "applicationId": GAME_ID,
        "country":       region,
        "sessionId":     session_uuid,
        "version":       5,
    }))).await?;

    tracing::debug!("Presence handshake complete.");

    let mut ping_interval   = tokio::time::interval(PRESENCE_PING_INTERVAL);
    let mut ping_id         = 2u64;
    let mut last_ping_delay = 824_448u64;
    let mut last_activity   = Instant::now();

    loop {
        tokio::select! {
            _ = ping_interval.tick() => {
                if last_activity.elapsed() > PRESENCE_READ_TIMEOUT {
                    return Err("presence inactivity timeout — connection likely dead".into());
                }

                write.send(engine_frame("500", json!({
                    "pingId":        ping_id,
                    "lastPingDelay": last_ping_delay,
                }))).await?;
                tracing::debug!(ping_id, "Heartbeat sent.");
                ping_id         += 1;
                last_ping_delay += 5_000;
            }

            msg = read.next() => match msg {
                Some(Ok(Message::Text(text))) => {
                    last_activity = Instant::now();
                    let text = text.to_string();

                    if text == "2" {
                        write.send(Message::text("3")).await?;
                        continue;
                    }

                    tracing::debug!(frame = %text, "Frame received.");
                    if let Some(event) = parse_frame(&text) {
                        bus.publish(event);
                    }
                }
                Some(Ok(Message::Binary(_))) | Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {
                    last_activity = Instant::now();
                }
                Some(Ok(Message::Close(_))) | None => {
                    tracing::warn!("Presence disconnected.");
                    return Ok(());
                }
                Some(Err(e)) => {
                    return Err(Box::new(e));
                }
            }
        }
    }
}

async fn recv_expecting<S, E>(read: &mut S, label: &str)
    -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    S: StreamExt<Item = std::result::Result<Message, E>> + Unpin,
    E: std::error::Error + Send + Sync + 'static,
{
    match tokio::time::timeout(PRESENCE_HANDSHAKE_TIMEOUT, read.next()).await {
        Ok(Some(Ok(Message::Text(msg)))) => {
            tracing::debug!("{label}: {msg}");
            Ok(())
        }
        Ok(Some(Ok(_)))  => Ok(()),
        Ok(Some(Err(e))) => Err(Box::new(e)),
        Ok(None)         => Err(format!("connection closed while awaiting {label}").into()),
        Err(_)           => Err(format!("timed out awaiting {label}").into()),
    }
}


fn engine_frame(msg_type: &str, content: Value) -> Message {
    let inner = json!({
        "messageType":    msg_type,
        "messageContent": content,
    }).to_string();
    Message::text(format!("42{}", json!([msg_type, inner])))
}


#[inline]
fn client_id() -> &'static str {
    static ID: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ID.get_or_init(|| std::env::var("MSP_CLIENT_ID").unwrap_or_else(|_| CLIENT_ID.to_owned()))
}

#[inline]
fn client_secret() -> &'static str {
    static SECRET: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SECRET.get_or_init(|| std::env::var("MSP_CLIENT_SECRET").unwrap_or_else(|_| CLIENT_SECRET.to_owned()))
}

fn basic_auth() -> String {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let encoded = STANDARD.encode(format!("{}:{}", client_id(), client_secret()));
    format!("Basic {encoded}")
}

fn decode_jwt_sub(token: &str) -> Result<String> {
    decode_jwt_claims(token)?
        .get("sub")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| MspError::Jwt("JWT missing 'sub' claim".into()))
}

fn decode_jwt_exp(token: &str) -> Result<i64> {
    decode_jwt_claims(token)?
        .get("exp")
        .and_then(Value::as_i64)
        .ok_or_else(|| MspError::Jwt("JWT missing 'exp' claim".into()))
}

fn decode_jwt_claims(token: &str) -> Result<Value> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

    let payload = token
        .split('.')
        .nth(1)
        .ok_or_else(|| MspError::Jwt("Malformed JWT — missing payload segment".into()))?;

    let bytes = URL_SAFE_NO_PAD.decode(payload)?;
    Ok(serde_json::from_slice(&bytes)?)
}


#[inline]
fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn backoff_with_jitter(base: Duration, max: Duration, attempt: u32) -> Duration {
    let exp    = base.saturating_mul(1u32 << attempt.saturating_sub(1).min(16));
    let capped = exp.min(max);
    let jitter = Duration::from_millis({
        use rand::Rng;
        rand::thread_rng().gen_range(0..500)
    });
    capped + jitter
}