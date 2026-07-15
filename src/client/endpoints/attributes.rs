use serde_json::Value;
use wreq::Client;

use crate::errors::Result;
use crate::models::ProfileAttributes;
use crate::session::SessionStore;
use super::super::http::{build_headers, ensure_no_error, optional_str, required_str, ContentType};

const ATTRIBUTES_ENDPOINT: &str =
    "https://eu.mspapis.com/profileattributes/v1/profiles/{profileId}/games/{gameId}/attributes";

const GAME_ID: &str = "j68d";

const MOOD_KEY: &str = "Mood";

pub struct AttributesEndpoint<'c> {
    pub(crate) http:    &'c Client,
    pub(crate) session: &'c SessionStore,
}

impl<'c> AttributesEndpoint<'c> {
    /// Fetches the attributes for `profile_id`, defaulting to the currently
    /// authenticated profile when `None`.
    ///
    /// # Errors
    /// Fails if the session is unavailable, the transport errors, the response
    /// is not valid JSON, the API returns an error body, or required fields
    /// (`profileId` / `gameId`) are missing.
    pub async fn get(&self, profile_id: Option<&str>) -> Result<ProfileAttributes> {
        let session   = self.session.get().await?;
        let target_id = profile_id.unwrap_or(&session.profile_id);
        let url       = Self::url_for(target_id);

        let response = self.fetch_raw(&url, &session.bearer()).await?;
        parse_attributes(&response)
    }

    pub async fn update_additional_data_key(
        &self,
        key:   &str,
        value: impl Into<Value>,
    ) -> Result<ProfileAttributes> {
        let session = self.session.get().await?;
        let bearer  = session.bearer();
        let url     = Self::url_for(&session.profile_id);

        let mut attributes = self.fetch_raw(&url, &bearer).await?;

        let entry = attributes
            .get_mut("additionalData")
            .filter(|v| v.is_object())
            .map(Value::as_object_mut)
            .flatten();

        match entry {
            Some(obj) => {
                obj.insert(key.to_owned(), value.into());
            }
            None => {
                let mut map = serde_json::Map::new();
                map.insert(key.to_owned(), value.into());
                attributes["additionalData"] = Value::Object(map);
            }
        }

        let response: Value = self
            .http
            .put(&url)
            .headers(build_headers(ContentType::Json, Some(&bearer)))
            .json(&attributes)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        parse_attributes(&ensure_no_error(response)?)
    }

    /// Sets the profile's mood to `mood`.
    ///
    /// Convenience wrapper over [`update_additional_data_key`] targeting the
    /// `Mood` key in `additionalData`.
    ///
    /// [`update_additional_data_key`]: Self::update_additional_data_key
    pub async fn set_mood(&self, mood: &str) -> Result<ProfileAttributes> {
        self.update_additional_data_key(MOOD_KEY, mood).await
    }

    fn url_for(profile_id: &str) -> String {
        ATTRIBUTES_ENDPOINT
            .replace("{profileId}", profile_id)
            .replace("{gameId}", GAME_ID)
    }

    async fn fetch_raw(&self, url: &str, bearer: &str) -> Result<Value> {
        let response: Value = self
            .http
            .get(url)
            .headers(build_headers(ContentType::Json, Some(bearer)))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        ensure_no_error(response)
    }
}

/// Maps a raw attribute document into a typed [`ProfileAttributes`].
///
/// `profileId` and `gameId` are required; `avatarId` is optional (defaults to
/// empty). `additionalData` is preserved verbatim as raw JSON.
fn parse_attributes(value: &Value) -> Result<ProfileAttributes> {
    Ok(ProfileAttributes {
        profile_id:      required_str(value, "profileId")?.to_owned(),
        game_id:         required_str(value, "gameId")?.to_owned(),
        avatar_id:       optional_str(value, "avatarId").unwrap_or_default().to_owned(),
        additional_data: value.get("additionalData").cloned().unwrap_or(Value::Null),
    })
}