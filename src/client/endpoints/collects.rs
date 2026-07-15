use serde_json::Value;
use wreq::Client;

use crate::{
    errors::{MspError, Result},
    models::Collect,
    session::SessionStore,
};
use super::super::http::{build_headers, ContentType};

const TIME_LIMITED_REWARDS_ENDPOINT: &str =
    "https://eu.mspapis.com/timelimitedrewards/v2/profiles/{profileId}/games/j68d/rewards/{rewardType}";

const PROFILE_COLLECTS_ENDPOINT: &str =
    "https://eu.mspapis.com/profilecollects/v3/profiles/{profileId}/games/j68d/collects";

const PROFILE_COLLECTS_CLAIM_ENDPOINT: &str =
    "https://eu.mspapis.com/profilecollects/v3/profiles/{profileId}/games/j68d/collects/claim";

pub struct CollectsEndpoint<'c> {
    pub(crate) http:    &'c Client,
    pub(crate) session: &'c SessionStore,
}

impl<'c> CollectsEndpoint<'c> {

    pub async fn collect_pickup(&self) -> Result<()> {
        self.claim_reward("daily_pickup").await
    }


    pub async fn collect_pickup_vip(&self) -> Result<()> {
        self.claim_reward("daily_pickup_vip").await
    }

    pub async fn get_collects(&self) -> Result<Vec<Collect>> {
        let session = self.session.get().await?;

        let url = PROFILE_COLLECTS_ENDPOINT
            .replace("{profileId}", &session.profile_id);

        let response = self
            .http
            .get(&url)
            .headers(build_headers(ContentType::Json, Some(&session.bearer())))
            .send()
            .await?;

        let status = response.status();

        if status.is_success() {
            let body: Value = response.json().await?;
            let collects: Vec<Collect> = serde_json::from_value(body)
                .map_err(MspError::Json)?;
            return Ok(collects);
        }

        let body = response.text().await.unwrap_or_default();
        Err(MspError::Api { status: status.as_u16(), body })
    }

    pub async fn claim_collects(&self, collect_list: &[&str]) -> Result<Vec<Collect>> {
        let session = self.session.get().await?;

        let url = PROFILE_COLLECTS_CLAIM_ENDPOINT
            .replace("{profileId}", &session.profile_id);

        let payload = serde_json::json!({
            "collectTypes": collect_list,
        });

        let response = self
            .http
            .post(&url)
            .headers(build_headers(ContentType::Json, Some(&session.bearer())))
            .json(&payload)
            .send()
            .await?;

        let status = response.status();

        if status.is_success() {
            let body: Value = response.json().await?;
            let collects: Vec<Collect> = serde_json::from_value(body)
                .map_err(MspError::Json)?;
            return Ok(collects);
        }

        let body = response.text().await.unwrap_or_default();
        Err(MspError::Api { status: status.as_u16(), body })
    }

    async fn claim_reward(&self, reward_type: &str) -> Result<()> {
        let session = self.session.get().await?;

        let url = TIME_LIMITED_REWARDS_ENDPOINT
            .replace("{profileId}",  &session.profile_id)
            .replace("{rewardType}", reward_type);

        let payload = serde_json::json!({ "state": "Claimed" });

        let response = self
            .http
            .put(&url)
            .headers(build_headers(ContentType::Json, Some(&session.bearer())))
            .json(&payload)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(MspError::Api { status: status.as_u16(), body });
        }

        Ok(())
    }
}