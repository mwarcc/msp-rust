use serde_json::Value;
use wreq::Client;

use crate::{
    errors::{MspError, Result},
    models::HighScoreEntry,
    session::SessionStore,
};
use super::super::http::{build_headers, ContentType};

const HIGHSCORES_GRAPHQL_ENDPOINT: &str =
    "https://eu.mspapis.com/edgehighscores/graphql";

const GET_HIGHSCORES_QUERY: &str = "\
query GetHighScores(\
  $gameId: String!, \
  $culture: String!, \
  $contentType: String!, \
  $timeScope: TimeScope!, \
  $pageSize: Int, \
  $pageIndex: Int\
) { \
  topUsers(input: {\
    gameId: $gameId, \
    culture: $culture, \
    contentType: $contentType, \
    timeScope: $timeScope, \
    pageSize: $pageSize, \
    pageIndex: $pageIndex\
  }) { \
    entityId rank score progressionLevel \
    profile { \
      id name \
      membership { lastTierExpiry } \
      avatar(preferredGameId: $gameId) { gameId } \
    } \
  } \
}";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeScope {
    Weekly,
    AllTime,
}

impl TimeScope {
    fn as_str(self) -> &'static str {
        match self {
            Self::Weekly  => "WEEKLY",
            Self::AllTime => "ALL_TIME",
        }
    }
}

pub struct HighscoresEndpoint<'c> {
    pub(crate) http:    &'c Client,
    pub(crate) session: &'c SessionStore,
}

impl<'c> HighscoresEndpoint<'c> {
    pub async fn get_high_scores(
        &self,
        content_type: Option<&str>,
        time_scope:   Option<TimeScope>,
        page_size:    Option<u32>,
        page_index:   Option<u32>,
        culture:      Option<&str>,
    ) -> Result<Vec<HighScoreEntry>> {
        let session = self.session.get().await?;

        let content_type = content_type.unwrap_or("fame");
        let time_scope   = time_scope.unwrap_or(TimeScope::Weekly).as_str();
        let page_size    = page_size.unwrap_or(20);
        let page_index   = page_index.unwrap_or(1);
        let culture      = culture.unwrap_or("fr-FR");

        let variables = serde_json::json!({
            "gameId":      "j68d",
            "culture":     culture,
            "contentType": content_type,
            "timeScope":   time_scope,
            "pageSize":    page_size,
            "pageIndex":   page_index,
        });

        let payload = serde_json::json!({
            "query":     GET_HIGHSCORES_QUERY,
            "variables": variables.to_string(),
        });

        let response: Value = self
            .http
            .post(HIGHSCORES_GRAPHQL_ENDPOINT)
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

        let raw = response["data"]["topUsers"]
            .as_array()
            .ok_or_else(|| MspError::Api {
                status: 200,
                body:   "Missing 'topUsers' array in GetHighScores response".into(),
            })?;

        let entries: Vec<HighScoreEntry> = raw
            .iter()
            .filter_map(|e| serde_json::from_value(e.clone()).ok())
            .collect();

        Ok(entries)
    }
}