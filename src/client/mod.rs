pub mod builder;
mod cookies;
mod http;
mod stealth;
pub mod endpoints;

pub use builder::{BrowserBrand, MspClientBuilder};
pub use endpoints::quests::{pending_random_daily_children, random_daily_children, DailyRemainingCounters};
pub use endpoints::highscores::TimeScope;
pub use stealth::StealthConfig;

use std::sync::Arc;

use wreq::Client;
use wreq_util::{Profile, Platform};
use tokio::sync::broadcast;

use crate::event_bus::EventBus;
use crate::events::MspEvent;
use crate::session::SessionStore;
use crate::state::SessionState;
use cookies::PersistentJar;
use endpoints::{
    auth::AuthEndpoint,
    attributes::AttributesEndpoint,
    collects::CollectsEndpoint,
    comments::CommentsEndpoint,
    greetings::GreetingsEndpoint,
    highscores::HighscoresEndpoint,
    messaging::MessagingEndpoint,
    profiles::ProfilesEndpoint,
    reservations::ReservationsEndpoint,
    quests::QuestsEndpoint,
    ugcs::UgcsEndpoint,
};

pub struct MspClient {
    http:          Client,
    device_id:     String,
    session:       SessionStore,
    profile:       Profile,
    platform:      Platform,
    event_bus:     EventBus,
    proxy_url:     Option<String>,
    enforce_proxy: bool,
    jar:           Arc<PersistentJar>,
    stealth:       StealthConfig,
}

impl MspClient {
    pub fn builder() -> MspClientBuilder { MspClientBuilder::default() }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_parts(
        http:          Client,
        device_id:     String,
        profile:       Profile,
        platform:      Platform,
        proxy_url:     Option<String>,
        enforce_proxy: bool,
        jar:           Arc<PersistentJar>,
        stealth:       StealthConfig,
    ) -> Self {
        Self {
            http,
            device_id,
            session:   SessionStore::new(),
            profile,
            platform,
            event_bus: EventBus::new(),
            proxy_url,
            enforce_proxy,
            jar,
            stealth,
        }
    }

    pub fn events(&self) -> broadcast::Receiver<MspEvent> {
        self.event_bus.subscribe()
    }

    #[inline]
    pub async fn pace(&self) {
        self.stealth.pace().await;
    }

    #[inline]
    pub fn stealth(&self) -> &StealthConfig { &self.stealth }

    pub async fn export_state(&self) -> Option<SessionState> {
        let session     = self.session.get().await.ok()?;
        let exported_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        Some(SessionState {
            profile_id:              session.profile_id,
            sub_id:                  session.sub_id,
            device_id:               session.device_id,
            region:                  session.region,
            access_token:            session.access_token,
            refresh_token:           session.refresh_token,
            access_token_expires_at: session.access_token_expires_at,
            browser_profile:         format!("{:?}", self.profile),
            platform:                format!("{:?}", self.platform),
            proxy_url:               self.proxy_url.clone(),
            enforce_proxy:           self.enforce_proxy,
            cookies:                 self.jar.export(),
            exported_at,
            schema_version:          SessionState::SCHEMA_VERSION,
        })
    }

    #[inline] pub fn profile(&self)  -> Profile  { self.profile  }
    #[inline] pub fn platform(&self) -> Platform { self.platform }

    #[inline]
    pub fn auth(&self) -> AuthEndpoint<'_> {
        AuthEndpoint {
            http:      &self.http,
            session:   &self.session,
            device_id: &self.device_id,
            event_bus: &self.event_bus,
        }
    }

    #[inline]
    pub fn attributes(&self) -> AttributesEndpoint<'_> {
        AttributesEndpoint { http: &self.http, session: &self.session }
    }

    #[inline]
    pub fn collects(&self) -> CollectsEndpoint<'_> {
        CollectsEndpoint { http: &self.http, session: &self.session }
    }

    #[inline]
    pub fn comments(&self) -> CommentsEndpoint<'_> {
        CommentsEndpoint { http: &self.http, session: &self.session }
    }

    #[inline]
    pub fn greetings(&self) -> GreetingsEndpoint<'_> {
        GreetingsEndpoint { http: &self.http, session: &self.session }
    }

    #[inline]
    pub fn highscores(&self) -> HighscoresEndpoint<'_> {
        HighscoresEndpoint { http: &self.http, session: &self.session }
    }

    #[inline]
    pub fn messaging(&self) -> MessagingEndpoint<'_> {
        MessagingEndpoint { http: &self.http, session: &self.session }
    }

    #[inline]
    pub fn profiles(&self) -> ProfilesEndpoint<'_> {
        ProfilesEndpoint { http: &self.http, session: &self.session }
    }

    #[inline]
    pub fn quests(&self) -> QuestsEndpoint<'_> {
        QuestsEndpoint { http: &self.http, session: &self.session }
    }

    #[inline]
    pub fn reservations(&self) -> ReservationsEndpoint<'_> {
        ReservationsEndpoint { http: &self.http, session: &self.session }
    }

    #[inline]
    pub fn ugcs(&self) -> UgcsEndpoint<'_> {
        UgcsEndpoint { http: &self.http, session: &self.session }
    }

    #[inline]
    pub fn session(&self) -> &SessionStore { &self.session }

    #[inline]
    pub fn raw_http(&self) -> &wreq::Client {
        &self.http
    }
}

