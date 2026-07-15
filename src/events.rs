use serde::{Deserialize, Serialize};
use serde_json::Value;


#[derive(Debug, Clone)]
pub enum MspEvent {
    PingResponse(PingResponseEvent),

    RelationshipRequestCreated(RelationshipRequestCreatedEvent),

    RelationshipRequestChanged(RelationshipRequestChangedEvent),

    PassiveRewardEarned(PassiveRewardEarnedEvent),

    MessageSent(MessageSentEvent),

    Unknown {
        message_type: String,
        payload:      Value,
    },
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingResponseEvent {
    pub ping_id: u64,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipRequestCreatedEvent {
    pub requester_profile_id: String,
    pub profile_id:           String,
    pub game_id:              String,
    pub target_profile_ids:   Vec<String>,
    pub event_name:           String,
    pub event_version:        u32,
    pub trace_parent:         Option<String>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipRequestChangedEvent {
    pub created:              Option<String>,
    pub new_state:            String,
    pub old_state:            String,
    pub requester_profile_id: String,
    pub profile_id:           String,
    pub game_id:              String,
    pub target_profile_ids:   Vec<String>,
    pub event_name:           String,
    pub event_version:        u32,
    pub trace_parent:         Option<String>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PassiveRewardEarnedEvent {
    pub target_profile_ids: Vec<String>,
    pub profile_id:         String,
    pub game_id:            String,
    pub when:               String,
    pub xp:                 i64,
    pub currency_rewards:   Value,
    pub reward_id:          String,
    pub sub_type:           Option<String>,
    pub vip_days:           i64,
    pub collect:            Option<RewardCollect>,
    pub source_profile_id:  Option<String>,
    pub event_name:         String,
    pub event_version:      u32,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewardCollect {
    pub group_id: String,
    pub guid:     String,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageSentEvent {
    pub author:            String,
    pub sender_profile_id: String,
    pub conversation_id:   String,
    pub conversation_name: String,
    pub conversation_type: String,
    pub message_body:      String,
    pub message_id:        String,
    pub message_type:      String,
    pub message_version:   u32,
    pub muted_profile_ids: Vec<String>,
    pub target_profile_ids: Vec<String>,
    pub timestamp:         String,
    pub event_name:        String,
    pub event_version:     u32,
    pub trace_parent:      Option<String>,
}


pub(crate) fn parse_frame(text: &str) -> Option<MspEvent> {
    if !text.starts_with("42") {
        return None;
    }

    let json_part = &text[2..];
    let array: Value = serde_json::from_str(json_part).ok()?;

    let inner: Value = match array.get(1)? {
        Value::String(s) => serde_json::from_str(s).ok()?,
        Value::Object(_) => array.get(1)?.clone(),
        _ => return None,
    };

    let message_type = inner["messageType"].as_str().unwrap_or("").to_owned();
    let content      = &inner["messageContent"];

    match message_type.as_str() {
        "501" => {
            let event: PingResponseEvent =
                serde_json::from_value(content.clone()).ok()?;
            Some(MspEvent::PingResponse(event))
        }

        "100" => {
            let event_name = content["eventName"].as_str().unwrap_or("");

            match event_name {
                "relationshipRequestCreatedEvent" => {
                    let event: RelationshipRequestCreatedEvent =
                        serde_json::from_value(content.clone()).ok()?;
                    Some(MspEvent::RelationshipRequestCreated(event))
                }

                "relationshipRequestChangedEvent" => {
                    let event: RelationshipRequestChangedEvent =
                        serde_json::from_value(content.clone()).ok()?;
                    Some(MspEvent::RelationshipRequestChanged(event))
                }

                "passiveRewardEarnedEvent" => {
                    let event: PassiveRewardEarnedEvent =
                        serde_json::from_value(content.clone()).ok()?;
                    Some(MspEvent::PassiveRewardEarned(event))
                }

                "messageSentEvent" => {
                    let event: MessageSentEvent =
                        serde_json::from_value(content.clone()).ok()?;
                    Some(MspEvent::MessageSent(event))
                }

                _ => Some(MspEvent::Unknown {
                    message_type,
                    payload: inner.clone(),
                }),
            }
        }

        _ => Some(MspEvent::Unknown {
            message_type,
            payload: inner.clone(),
        }),
    }
}