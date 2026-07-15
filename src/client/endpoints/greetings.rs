use serde_json::Value;
use wreq::Client;

use crate::{
    errors::{MspError, Result},
    models::{GreetingDefinition, SendGreetingResult},
    session::SessionStore,
};
use super::super::http::{build_headers, ContentType};

const FEDERATION_GATEWAY_ENDPOINT: &str =
    "https://eu.mspapis.com/federationgateway/graphql";

pub struct GreetingsEndpoint<'c> {
    pub(crate) http:    &'c Client,
    pub(crate) session: &'c SessionStore,
}

impl<'c> GreetingsEndpoint<'c> {
    pub async fn get_greeting_definitions(&self) -> Result<Vec<GreetingDefinition>> {
        let session = self.session.get().await?;

        let payload = serde_json::json!({
            "id": "GetGreetingsDefinitions-5FBA528E623526E9F8378521DC7F0623",
            "variables": ""
        });

        let response: Value = self
            .http
            .post(FEDERATION_GATEWAY_ENDPOINT)
            .headers(build_headers(ContentType::Json, Some(&session.bearer())))
            .json(&payload)
            .send()
            .await?
            .json()
            .await?;

        let definitions_raw = &response["data"]["profiles"]["me"]["greetings"]["definitions"];

        let definitions: Vec<GreetingDefinition> = serde_json::from_value(definitions_raw.clone())
            .map_err(|e| MspError::Json(e))?;

        Ok(definitions)
    }

    pub async fn send_greeting(
        &self,
        greeting_type: &str,
        profile_id: &str,
    ) -> Result<SendGreetingResult> {
        let session = self.session.get().await?;

        let payload = serde_json::json!({
            "id": "SendGreetings-159BDD7706D824BB8F14874A7FAE3368",
            "variables": {
                "greetingType":      greeting_type,
                "receiverProfileId": profile_id,
                "ignoreDailyCap":    false,
            }
        });

        let response: Value = self
            .http
            .post(FEDERATION_GATEWAY_ENDPOINT)
            .headers(build_headers(ContentType::Json, Some(&session.bearer())))
            .json(&payload)
            .send()
            .await?
            .json()
            .await?;

        let raw = &response["data"]["greetings"]["sendGreeting"];

        let result: SendGreetingResult = serde_json::from_value(raw.clone())
            .map_err(|e| MspError::Json(e))?;

        if !result.success {
            if let Some(ref err) = result.error {
                return Err(MspError::Api {
                    status: 200,
                    body: format!(
                        "reason={}, next_in={}s, message={}",
                        err.reason,
                        err.next_greeting_seconds_remaining
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "N/A".to_owned()),
                        err.message,
                    ),
                });
            }
        }

        Ok(result)
    }
}