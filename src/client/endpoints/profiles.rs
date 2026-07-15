use serde_json::Value;
use wreq::Client;

use crate::{
    errors::{MspError, Result},
    models::{ProfileAvatar, ProfileIdentity, ProfileNode, ProfileSearchResult, ProfileInfo},
    session::SessionStore,
};
use super::super::http::{build_headers, ContentType};

const GRAPHQL_URL:  &str = "https://eu.mspapis.com/edgerelationships/graphql";
const IDENTITY_URL: &str = "https://eu.mspapis.com/profileidentity/v1/profiles/{p}";
const GAME_ID:      &str = "j68d";

const SEARCH_QUERY: &str = "\
query GetProfileSearch(\
  $region: String!, $startsWith: String!, $pageSize: Int, \
  $currentPage: Int, $preferredGameId: String!\
) { findProfiles(region: $region, nameBeginsWith: $startsWith, pageSize: $pageSize, page: $currentPage) { \
    totalCount nodes { id avatar(preferredGameId: $preferredGameId) { gameId } } \
} }";

const GET_PROFILES_QUERY: &str = "\
query GetProfiles($profileIds: [String!]!, $gameId: String!) {\
  profiles(profileIds: $profileIds) {\
    id name culture avatar(preferredGameId: $gameId) { gameId } membership { lastTierExpiry } \
  } \
}";

pub struct ProfilesEndpoint<'c> {
    pub(crate) http:    &'c Client,
    pub(crate) session: &'c SessionStore,
}

impl<'c> ProfilesEndpoint<'c> {
    async fn graphql(&self, payload: &serde_json::Value) -> Result<Value> {
        let session = self.session.get().await?;
        Ok(self.http
            .post(GRAPHQL_URL)
            .headers(build_headers(ContentType::Json, Some(&session.bearer())))
            .json(payload)
            .send().await?
            .json().await?)
    }

    fn graphql_array<'v>(response: &'v Value, key: &str) -> Result<&'v Vec<Value>> {
        response["data"][key].as_array().ok_or_else(|| MspError::Api {
            status: 200,
            body:   format!("Missing '{key}' array in GraphQL response"),
        })
    }

    pub async fn search_profiles(
        &self,
        username:       &str,
        region:         &str,
        page:           u32,
        page_size:      u32,
        game_id_filter: Option<&str>,
    ) -> Result<ProfileSearchResult> {
        let response = self.graphql(&serde_json::json!({
            "query": SEARCH_QUERY,
            "variables": {
                "region":          region.to_uppercase(),
                "startsWith":      username,
                "pageSize":        page_size,
                "currentPage":     page,
                "preferredGameId": GAME_ID,
            }
        })).await?;

        let raw         = &response["data"]["findProfiles"];
        let total_count = raw["totalCount"].as_u64().unwrap_or(0) as u32;

        let mut nodes: Vec<ProfileNode> = raw["nodes"]
            .as_array()
            .ok_or_else(|| MspError::Api { status: 200, body: "Missing 'nodes' in findProfiles".into() })?
            .iter()
            .filter_map(|n| serde_json::from_value(n.clone()).ok())
            .collect();

        for node in &mut nodes {
            node.avatar.get_or_insert_with(|| ProfileAvatar { game_id: GAME_ID.into() });
        }

        if let Some(filter) = game_id_filter {
            nodes.retain(|n| n.avatar.as_ref().map_or(false, |a| a.game_id == filter));
        }

        Ok(ProfileSearchResult { total_count, nodes })
    }

    pub async fn get_profiles(&self, profile_ids: &[&str]) -> Result<Vec<ProfileInfo>> {
        let response = self.graphql(&serde_json::json!({
            "query": GET_PROFILES_QUERY,
            "variables": { "profileIds": profile_ids, "gameId": GAME_ID }
        })).await?;

        if let Some(errors) = response.get("errors") {
            return Err(MspError::Api { status: 200, body: errors.to_string() });
        }

        Ok(Self::graphql_array(&response, "profiles")?
            .iter()
            .filter_map(|p| serde_json::from_value(p.clone()).ok())
            .collect())
    }

    pub async fn get_profile_identity(&self, profile_id: &str) -> Result<Option<ProfileIdentity>> {
        let session = self.session.get().await?;
        let response: Value = self.http
            .get(IDENTITY_URL.replace("{p}", profile_id))
            .headers(build_headers(ContentType::Json, Some(&session.bearer())))
            .send().await?
            .json().await?;

        let array = response.as_array().ok_or_else(|| MspError::Api {
            status: 200,
            body:   "Expected JSON array from profile identity endpoint".into(),
        })?;

        array.first()
            .map(|v| serde_json::from_value(v.clone()).map_err(MspError::Json))
            .transpose()
    }
}