//! `edgecomments` GraphQL endpoint — posting user comments to a thread.

use serde::Deserialize;

use crate::errors::{MspError, Result};
use crate::models::SentComment;
use crate::session::SessionStore;
use super::super::http::{build_headers, ContentType};

use wreq::Client;

const COMMENTS_ENDPOINT: &str = "https://eu.mspapis.com/edgecomments/graphql";

const ENTITY_TYPE_UGC: &str = "UGC";


const POST_COMMENT_MUTATION: &str = "\
mutation SendComment($entityType: String!, $threadId: String!, $text: String!, $author: String!) {
  postComment(input: { entityType: $entityType, threadId: $threadId, text: $text, author: $author }) {
    success
    error
    comment {
      commentId
      created
      author
      text
    }
  }
}";


pub struct CommentsEndpoint<'c> {
    pub(crate) http:    &'c Client,
    pub(crate) session: &'c SessionStore,
}

impl<'c> CommentsEndpoint<'c> {
    pub async fn post(&self, thread_id: &str, text: &str) -> Result<SentComment> {
        let session = self.session.get().await?;

        let payload = serde_json::json!({
            "query": POST_COMMENT_MUTATION,
            "variables": {
                "entityType": ENTITY_TYPE_UGC,
                "threadId":   thread_id,
                "text":       text,
                "author":     session.profile_id,
            }
        });

        let envelope: GraphQlResponse = self
            .http
            .post(COMMENTS_ENDPOINT)
            .headers(build_headers(ContentType::Json, Some(&session.bearer())))
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        envelope.into_sent_comment()
    }
}


#[derive(Debug, Deserialize)]
struct GraphQlResponse {
    #[serde(default)]
    data:   Option<ResponseData>,
    #[serde(default)]
    errors: Vec<GraphQlError>,
}

#[derive(Debug, Deserialize)]
struct ResponseData {
    #[serde(rename = "postComment")]
    post_comment: PostCommentResult,
}

#[derive(Debug, Deserialize)]
struct PostCommentResult {
    success: bool,
    error:   Option<String>,
    comment: Option<SentComment>,
}

#[derive(Debug, Deserialize)]
struct GraphQlError {
    message: String,
}

impl GraphQlResponse {
    fn into_sent_comment(self) -> Result<SentComment> {
        if !self.errors.is_empty() {
            let body = self
                .errors
                .into_iter()
                .map(|e| e.message)
                .collect::<Vec<_>>()
                .join("; ");
            return Err(MspError::Api { status: 400, body });
        }

        let result = self
            .data
            .map(|d| d.post_comment)
            .ok_or_else(|| MspError::Api {
                status: 422,
                body:   "GraphQL response contained neither 'data' nor 'errors'".into(),
            })?;

        if !result.success {
            let body = result
                .error
                .unwrap_or_else(|| "postComment reported success = false".into());
            return Err(MspError::Api { status: 400, body });
        }

        result.comment.ok_or_else(|| MspError::Api {
            status: 422,
            body:   "postComment succeeded but returned no comment".into(),
        })
    }
}