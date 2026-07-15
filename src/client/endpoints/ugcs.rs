use serde_json::Value;
use wreq::Client;
use bson;
use crate::{
    errors::{MspError, Result},
    models::Ugc,
    session::SessionStore,
};
use super::super::http::{build_headers, ContentType};

const UGC_GRAPHQL_ENDPOINT: &str = "https://eu.mspapis.com/edgeugc/graphql";
const COMMENTS_GRAPHQL_ENDPOINT: &str = "https://eu.mspapis.com/edgecomments/graphql";
const UGC_CDN_BASE: &str = "https://ugc-eu.mspcdns.com/";

const GET_COMMENTS_COUNT_QUERY: &str = "\
query GetCommentsCount($entityType: EntityType!, $threadId: ID!) {\
  count(entityType: $entityType, threadId: $threadId) {\
    count\
  }\
}";

const GET_UGC_BY_ID_QUERY: &str = "\
query GetUgcById($ugcId: String!, $gameId: String!) {\
  ugc(input:{ugcId: $ugcId}) {\
    id title lastEditedDate lifecycleStatus privacyStatus owner type commentCount \
    ...on Movie { duration views } \
    reactions { reactionTypeId count } \
    resources { type id } \
    profile {\
      id name \
      membership { lastTierExpiry } \
      avatar(preferredGameId: $gameId) { gameId }\
    }\
  }\
}";

pub struct UgcsEndpoint<'c> {
    pub(crate) http:    &'c Client,
    pub(crate) session: &'c SessionStore,
}

impl<'c> UgcsEndpoint<'c> {
    pub async fn get_status_text(&self, wayd_id: &str) -> Result<Option<String>> {
    let ugc = self
        .get_ugc_by_id(wayd_id)
        .await?
        .ok_or_else(|| MspError::Api {
            status: 404,
            body:   format!("UGC '{wayd_id}' not found"),
        })?;

    let resource_id = ugc
        .resources
        .iter()
        .find(|r| r.resource_type == "PgcV1")
        .map(|r| r.id.as_str())
        .ok_or_else(|| MspError::Api {
            status: 200,
            body:   "No PgcV1 resource found in UGC".into(),
        })?;

    let cdn_url = format!("{UGC_CDN_BASE}{resource_id}");

    let response = self
        .http
        .get(&cdn_url)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(MspError::Api {
            status: response.status().as_u16(),
            body:   format!("CDN request failed for resource '{resource_id}'"),
        });
    }

    let bytes = response.bytes().await.map_err(|e| MspError::Network(e))?;

    let doc = bson::Document::from_reader(&mut bytes.as_ref())
        .map_err(|e| MspError::Api {
            status: 200,
            body:   format!("BSON decode failed: {e}"),
        })?;

    let text = doc
        .get_array("Texts")
        .ok()
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .map(str::to_owned);

    Ok(text)
}


pub async fn get_comments_count(&self, ugc_id: &str) -> Result<u64> {
    let session = self.session.get().await?;

    let payload = serde_json::json!({
        "query":     GET_COMMENTS_COUNT_QUERY,
        "variables": serde_json::json!({
            "entityType": "UGC",
            "threadId":   ugc_id,
        }).to_string(),
    });

    let response: Value = self
        .http
        .post(COMMENTS_GRAPHQL_ENDPOINT)
        .headers(build_headers(ContentType::Json, Some(&session.bearer())))
        .json(&payload)
        .send()
        .await?
        .json()
        .await?;

    if let Some(errors) = response.get("errors") {
        return Err(MspError::Api {
            status: 200,
            body:   errors.to_string(),
        });
    }

    let count = response["data"]["count"]["count"]
        .as_u64()
        .ok_or_else(|| MspError::Api {
            status: 200,
            body:   "Missing 'count' field in GetCommentsCount response".into(),
        })?;

    Ok(count)
}

    pub async fn get_ugc_by_id(&self, ugc_id: &str) -> Result<Option<Ugc>> {
        let session = self.session.get().await?;

        let payload = serde_json::json!({
            "query":     GET_UGC_BY_ID_QUERY,
            "variables": serde_json::json!({
                "ugcId":  ugc_id,
                "gameId": "j68d",
            }).to_string(),
        });

        let response: Value = self
            .http
            .post(UGC_GRAPHQL_ENDPOINT)
            .headers(build_headers(ContentType::Json, Some(&session.bearer())))
            .json(&payload)
            .send()
            .await?
            .json()
            .await?;

        if let Some(errors) = response.get("errors") {
            return Err(MspError::Api {
                status: 200,
                body:   errors.to_string(),
            });
        }

        let raw = &response["data"]["ugc"];

        if raw.is_null() {
            return Ok(None);
        }

        let ugc: Ugc = serde_json::from_value(raw.clone())
            .map_err(MspError::Json)?;

        Ok(Some(ugc))
    }
}