use serde_json::Value;
use wreq::{Client, StatusCode};

use crate::{
    errors::{MspError, Result},
    models::{ChatMessage, Conversation, ConversationEntry, LatestMessage, MessageReceipt},
    session::SessionStore,
};
use super::super::http::{build_headers, ContentType};

const CONVERSATIONS_BY_PROFILE_ENDPOINT: &str =
    "https://eu.mspapis.com/gamemessaging/v1/profiles/{profileId}/conversations/profiles/{otherProfileId}";
const CONVERSATIONS_CREATE_ENDPOINT: &str =
    "https://eu.mspapis.com/gamemessaging/v1/conversations?creator={profileId}";
const MESSAGES_ENDPOINT: &str =
    "https://eu.mspapis.com/gamemessaging/v1/conversations/{conversationId}/history";
const CONVERSATIONS_LIST_ENDPOINT: &str =
    "https://eu.mspapis.com/gamemessaging/v1/participants/{profileId}/conversations";
const CHAT_HISTORY_ENDPOINT: &str =
    "https://eu.mspapis.com/gamemessaging/v1/conversations/{conversationId}/history";
const CONVERSATION_PARTICIPANT_ENDPOINT: &str =
    "https://eu.mspapis.com/gamemessaging/v1/conversations/{conversationId}/participants/{profileId}";


pub struct MessagingEndpoint<'c> {
    pub(crate) http:    &'c Client,
    pub(crate) session: &'c SessionStore,
}

impl<'c> MessagingEndpoint<'c> {
    pub async fn find_conversation(
        &self,
        other_profile_id: &str,
    ) -> Result<Option<Conversation>> {
        let session = self.session.get().await?;

        let url = CONVERSATIONS_BY_PROFILE_ENDPOINT
            .replace("{profileId}",      &session.profile_id)
            .replace("{otherProfileId}", other_profile_id);

        let response = self
            .http
            .get(&url)
            .headers(build_headers(ContentType::Json, Some(&session.bearer())))
            .send()
            .await?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !response.status().is_success() {
            return Err(MspError::Api {
                status: response.status().as_u16(),
                body:   "unexpected response from conversation lookup".into(),
            });
        }

        let data: Value = response.json().await?;
        Ok(Some(parse_conversation(&data)))
    }

        pub async fn mark_conversation_as_read(
        &self,
        conversation_id: &str,
    ) -> Result<ConversationEntry> {
        let session = self.session.get().await?;

        let url = CONVERSATION_PARTICIPANT_ENDPOINT
            .replace("{conversationId}", conversation_id)
            .replace("{profileId}",      &session.profile_id);

        let payload = serde_json::json!({
            "numUnread": 0,
            "isMuted":   false,
        });

        let raw: Value = self
            .http
            .put(&url)
            .headers(build_headers(ContentType::Json, Some(&session.bearer())))
            .json(&payload)
            .send()
            .await?
            .json()
            .await?;

        let mut entry: ConversationEntry = serde_json::from_value(raw)
            .map_err(MspError::Json)?;

        if let Some(ref raw_str) = entry.latest_message {
            if let Ok(parsed) = serde_json::from_str::<LatestMessage>(raw_str) {
                entry.latest_message_parsed = Some(parsed);
            }
        }

        Ok(entry)
    }

    pub async fn create_conversation(
        &self,
        other_profile_id: &str,
    ) -> Result<Conversation> {
        let session = self.session.get().await?;

        let url = CONVERSATIONS_CREATE_ENDPOINT
            .replace("{profileId}", &session.profile_id);

        let payload = serde_json::json!({
            "name":         "name",
            "message":      null,
            "type":         "OneToOne",
            "participants": [session.profile_id, other_profile_id],
        });

        let response: Value = self
            .http
            .post(&url)
            .headers(build_headers(ContentType::Json, Some(&session.bearer())))
            .json(&payload)
            .send()
            .await?
            .json()
            .await?;

        Ok(Conversation {
            conversation_id:   response["id"].as_str().unwrap_or_default().to_owned(),
            conversation_name: response["conversationName"].as_str().unwrap_or_default().to_owned(),
        })
    }

    pub async fn get_or_create_conversation(
        &self,
        other_profile_id: &str,
    ) -> Result<Conversation> {
        if let Some(conv) = self.find_conversation(other_profile_id).await? {
            return Ok(conv);
        }
        self.create_conversation(other_profile_id).await
    }

    pub async fn send_message(
        &self,
        conversation_id: &str,
        body: &str,
    ) -> Result<MessageReceipt> {
        let session = self.session.get().await?;

        let url = MESSAGES_ENDPOINT.replace("{conversationId}", conversation_id);

        let payload = serde_json::json!({
            "Author":      session.profile_id,
            "MessageType": "ChatMessageV2",
            "MessageBody": body,
        });

        let response: Value = self
            .http
            .post(&url)
            .headers(build_headers(ContentType::Json, Some(&session.bearer())))
            .json(&payload)
            .send()
            .await?
            .json()
            .await?;

        Ok(MessageReceipt {
            conversation_id:   response["conversationId"].as_str().unwrap_or_default().to_owned(),
            message_body:      response["messageBody"].as_str().unwrap_or_default().to_owned(),
            sender_profile_id: response["senderProfileId"].as_str().unwrap_or_default().to_owned(),
            timestamp:         response["timestamp"].as_str().unwrap_or_default().to_owned(),
        })
    }

    pub async fn get_conversations(
        &self,
        page: u32,
        page_size: u32,
    ) -> Result<ConversationPage> {
        let session = self.session.get().await?;

        let url = format!(
            "{}?&page={}&pageSize={}",
            CONVERSATIONS_LIST_ENDPOINT.replace("{profileId}", &session.profile_id),
            page,
            page_size,
        );

        let raw: Value = self
            .http
            .get(&url)
            .headers(build_headers(ContentType::Json, Some(&session.bearer())))
            .send()
            .await?
            .json()
            .await?;

        let entries_raw = raw
            .as_array()
            .ok_or_else(|| MspError::Api {
                status: 200,
                body:   "expected a JSON array from conversations list endpoint".into(),
            })?;

        let mut conversations: Vec<ConversationEntry> = Vec::with_capacity(entries_raw.len());
        let mut unread_conversation_ids: Vec<String> = Vec::new();

        for entry in entries_raw {
            let mut conv: ConversationEntry = serde_json::from_value(entry.clone())
                .map_err(MspError::Json)?;

            if let Some(ref raw_str) = conv.latest_message {
                if let Ok(parsed) = serde_json::from_str::<LatestMessage>(raw_str) {
                    conv.latest_message_parsed = Some(parsed);
                }
            }

            if conv.number_of_unread_messages > 0 {
                unread_conversation_ids.push(conv.conversation_id.clone());
            }

            conversations.push(conv);
        }

        Ok(ConversationPage {
            conversations,
            unread_conversation_ids,
        })
    }

    pub async fn get_chat_history(
        &self,
        conversation_id: &str,
        page_size: u32,
    ) -> Result<Vec<ChatMessage>> {
        let session = self.session.get().await?;

        let url = format!(
            "{}?&profileId={}&pageSize={}",
            CHAT_HISTORY_ENDPOINT.replace("{conversationId}", conversation_id),
            session.profile_id,
            page_size,
        );

        let raw: Value = self
            .http
            .get(&url)
            .headers(build_headers(ContentType::Json, Some(&session.bearer())))
            .send()
            .await?
            .json()
            .await?;

        let messages: Vec<ChatMessage> = serde_json::from_value(raw)
            .map_err(MspError::Json)?;

        Ok(messages)
    }
}


fn parse_conversation(data: &Value) -> Conversation {
    Conversation {
        conversation_id:   data["conversationId"].as_str().unwrap_or_default().to_owned(),
        conversation_name: data["conversationName"].as_str().unwrap_or_default().to_owned(),
    }
}


#[derive(Debug, Clone)]
pub struct ConversationPage {
    pub conversations: Vec<ConversationEntry>,
    pub unread_conversation_ids: Vec<String>,
}