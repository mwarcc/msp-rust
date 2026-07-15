use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MspSession {
    pub access_token:            String,
    pub refresh_token:           String,
    pub profile_id:              String,
    pub sub_id:                  String,
    pub device_id:               String,
    pub access_token_expires_at: i64,
    /// The two-letter region code (e.g. `"FR"`).
    pub region:                  String,
}


impl MspSession {
    #[inline]
    pub fn bearer(&self) -> String {
        format!("Bearer {}", self.access_token)
    }

    pub fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        now >= (self.access_token_expires_at - 30)
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomReservation {
    pub host_url: String,
    pub room_id: String,
    pub socket_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomKind {
    Chatroom,
    Quiz,
}

impl RoomKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Chatroom => "chatroom",
            Self::Quiz => "quiz",
        }
    }

    pub(crate) fn eio_version(self) -> u8 {
        match self {
            Self::Chatroom => 3,
            Self::Quiz => 4,
        }
    }

    pub(crate) fn socket_path(self) -> &'static str {
        match self {
            Self::Chatroom => "/socket.io",
            Self::Quiz => "/",
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileAttributes {
    pub profile_id: String,
    pub game_id: String,
    pub avatar_id: String, 
    pub additional_data: Value,
}



#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentComment {
    pub comment_id: String,
    pub created: String,
    pub author: String,
    pub text: String,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub conversation_id: String,
    pub conversation_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageReceipt {
    pub conversation_id: String,
    pub message_body: String,
    pub sender_profile_id: String,
    pub timestamp: String,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrencyReward {
    pub key: String,
    pub value: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GreetingDefinition {
    pub greeting_type: String,
    pub hard_cost: Option<i64>,
    pub interval_vip: Option<i64>,
    pub sender_xp_formula: Option<String>,
    pub sender_soft_reward: Option<i64>,
    pub receiver_xp_formula: Option<String>,
    pub receiver_soft_reward: Option<i64>,
    pub next_greeting_seconds_remaining: Option<i64>,
    pub greeting_max_xp_level_threshold: Option<i64>,
    pub greeting_max_xp: Option<i64>,
    pub receiver_min_level: Option<i64>,
    pub seasonal_currency: Option<String>,
    pub sender_seasonal_reward: Option<Value>,
    pub receiver_seasonal_reward: Option<Value>,
    pub sender_currency_rewards: Option<Vec<CurrencyReward>>,
    pub receiver_currency_rewards: Option<Vec<CurrencyReward>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendGreetingData {
    pub next_greeting_seconds_remaining: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendGreetingError {
    pub next_greeting_seconds_remaining: Option<i64>,
    pub reason: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendGreetingResult {
    pub data: Option<SendGreetingData>,
    pub success: bool,
    pub error: Option<SendGreetingError>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestMessage {
    #[serde(rename = "ConversationId")]
    pub conversation_id:   String,
    #[serde(rename = "MessageBody")]
    pub message_body:      String,
    #[serde(rename = "MessageType")]
    pub message_type:      String,
    #[serde(rename = "SenderProfileId")]
    pub sender_profile_id: String,
    #[serde(rename = "Timestamp")]
    pub timestamp:         String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationEntry {
    pub conversation_id:          String,
    pub conversation_name:        String,
    pub conversation_status:      String,
    pub conversation_type:        String,
    pub created:                  String,
    pub join_date:                String,
    pub latest_activity:          String,
    pub latest_message:           Option<String>,

    #[serde(skip)]
    pub latest_message_parsed:    Option<LatestMessage>,
    pub leave_date:               String,
    pub muted:                    bool,
    pub number_of_unread_messages: u32,
    pub participants:             Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub conversation_id:   String,
    pub message_body:      String,
    pub message_type:      String,
    pub sender_profile_id: String,
    pub timestamp:         String,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSearchResult {
    pub total_count: u32,
    pub nodes:       Vec<ProfileNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileNode {
    pub id:     String,
    pub avatar: Option<ProfileAvatar>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileAvatar {
    pub game_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileIdentity {
    pub id:                  String,
    pub login:               String,
    pub name:                String,
    pub culture:             String,
    pub created:             String,
    pub latest_state_change: String,
    pub state:               String,
    pub roles:               Vec<String>,
    pub latest_logins:       Vec<ProfileLatestLogin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileLatestLogin {
    pub game:      String,
    pub timestamp: String,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectCurrencyReward {
    #[serde(rename = "type")]
    pub reward_type: String,
    pub amount:      i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildCollect {
    pub guid:             String,
    pub group_id:         String,
    pub xp_reward:        i64,
    pub currency_rewards: Vec<CollectCurrencyReward>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Collect {
    pub owner:            String,
    pub collect_type:     String,
    pub count:            u32,
    pub xp_reward:        i64,
    pub currency_rewards: Vec<CollectCurrencyReward>,
    #[serde(default)]
    pub child_collects:   Vec<ChildCollect>,
    pub child_group_id:   Option<String>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UgcReaction {
    pub reaction_type_id: String,
    pub count:            u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UgcResource {
    #[serde(rename = "type")]
    pub resource_type: String,
    pub id:            String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UgcMembership {
    pub last_tier_expiry: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UgcProfile {
    pub id:         String,
    pub name:       String,
    pub membership: Option<UgcMembership>,
    pub avatar:     Option<ProfileAvatar>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ugc {
    pub id:               String,
    pub title:            Option<String>,
    pub last_edited_date: String,
    pub lifecycle_status: String,
    pub privacy_status:   String,
    pub owner:            String,
    #[serde(rename = "type")]
    pub ugc_type:         String,
    pub comment_count:    u32,
    /// Only present when the UGC is a Movie.
    pub duration:         Option<f64>,
    /// Only present when the UGC is a Movie.
    pub views:            Option<u64>,
    pub reactions:        Vec<UgcReaction>,
    pub resources:        Vec<UgcResource>,
    pub profile:          Option<UgcProfile>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HighScoreProfile {
    pub id:         String,
    pub name:       String,
    pub membership: Option<UgcMembership>,
    pub avatar:     Option<ProfileAvatar>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HighScoreEntry {
    pub entity_id:         String,
    pub rank:              u32,
    pub score:             String,
    pub progression_level: u32,
    pub profile:           Option<HighScoreProfile>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileMembership {
    pub last_tier_expiry: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInfo {
    pub id:         String,
    pub name:       String,
    pub culture:    String,
    pub avatar:     Option<ProfileAvatar>,
    pub membership: Option<ProfileMembership>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quest {
    pub definition_id: String,
    pub state:         String,
    pub progress:      i64,
    #[serde(default = "Quest::default_target")]
    pub target:        i64,
    #[serde(default)]
    pub children:      Vec<Quest>,
}

impl Quest {
    /// `true` when the quest is active and has not been progressed yet.
    #[inline]
    pub fn is_pending(&self) -> bool {
        self.state == "Active" && self.progress == 0
    }

    fn default_target() -> i64 {
        1
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestStateChange {
    pub quest:           Quest,
    #[serde(default)]
    pub unlocked_quests: Vec<Quest>,
    pub parent_quest:    Option<Quest>,
}