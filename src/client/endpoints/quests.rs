use serde::Deserialize;
use wreq::Client;

use crate::errors::Result;
use crate::models::{Quest, QuestStateChange};
use crate::session::SessionStore;
use super::super::http::{build_headers, ContentType};

const QUESTS_URL:  &str = "https://eu.mspapis.com/quests/v2/profiles/{p}/games/j68d/quests";
const REWARDS_URL: &str = "https://eu.mspapis.com/timelimitedrewards/v2/profiles/{p}/games/j68d/rewards";

const DAILY_TYPES: &[&str] = &["EventQuest", "StaticDailyQuest", "RandomDailyQuest"];

const RANDOM_DAILY_PARENT: &str = "random_daily_parent";

const PET_ID:       &str = "daily_pet_pets";
const GIFT_ID:      &str = "daily_open_gift_normal";
const GIFT_VIP_ID:  &str = "daily_open_gift_vip";
const PICKUP_ID:    &str = "daily_pickup";
const PICKUP_VIP_ID:&str = "daily_pickup_vip";

const PET_TARGET:      i64 = 10;
const GIFT_TARGET:     i64 = 4;
const GIFT_VIP_TARGET: i64 = 3;

#[derive(Debug, Deserialize)]
struct QuestListResponse {
    #[serde(default)]
    quests: Vec<Quest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DailyRemainingCounters {
    pub pets:        u32,
    pub pickups:     u32,
    pub pickups_vip: u32,
}

pub struct QuestsEndpoint<'c> {
    pub(crate) http:    &'c Client,
    pub(crate) session: &'c SessionStore,
}

impl<'c> QuestsEndpoint<'c> {
    fn url(base: &str, profile_id: &str) -> String {
        base.replace("{p}", profile_id)
    }

    async fn headers(&self) -> Result<(wreq::header::HeaderMap, String)> {
        let s = self.session.get().await?;
        Ok((build_headers(ContentType::Json, Some(&s.bearer())), s.profile_id))
    }

    pub async fn get_daily_quests(&self) -> Result<Vec<Quest>> {
        let (headers, pid) = self.headers().await?;
        let query = DAILY_TYPES.iter().map(|t| format!("questType={t}")).collect::<Vec<_>>().join("&");

        Ok(self.http
            .get(format!("{}?{query}", Self::url(QUESTS_URL, &pid)))
            .headers(headers)
            .send().await?
            .error_for_status()?
            .json::<QuestListResponse>().await?
            .quests)
    }

    pub async fn get_daily_remaining_counters(&self) -> Result<DailyRemainingCounters> {
        let q = self.get_daily_quests().await?;
        Ok(DailyRemainingCounters {
            pets:        remaining(&q, PET_ID,      PET_TARGET),
            pickups:     remaining(&q, GIFT_ID,     GIFT_TARGET),
            pickups_vip: remaining(&q, GIFT_VIP_ID, GIFT_VIP_TARGET),
        })
    }

    pub async fn get_count_of_pets_remaining_to_pet(&self) -> Result<u32> {
        Ok(self.get_daily_remaining_counters().await?.pets)
    }

    pub async fn get_count_pickups_remaining_to_collect(&self) -> Result<u32> {
        Ok(self.get_daily_remaining_counters().await?.pickups)
    }

    pub async fn get_count_pickups_vip_remaining_to_collect(&self) -> Result<u32> {
        Ok(self.get_daily_remaining_counters().await?.pickups_vip)
    }

    pub async fn get_random_daily_quests(&self) -> Result<Vec<Quest>> {
        Ok(random_daily_children(self.get_daily_quests().await?))
    }

    pub async fn get_pending_random_daily_quests(&self) -> Result<Vec<Quest>> {
        Ok(pending_random_daily_children(self.get_daily_quests().await?))
    }

    pub async fn progress_quest(&self, definition_id: &str) -> Result<Quest> {
        self.set_quest_progress(definition_id, 1).await
    }

    pub async fn set_quest_progress(&self, definition_id: &str, progress: i64) -> Result<Quest> {
        let (headers, pid) = self.headers().await?;
        self.http
            .put(format!("{}/{definition_id}/progress", Self::url(QUESTS_URL, &pid)))
            .headers(headers)
            .json(&serde_json::json!({ "progress": progress }))
            .send().await?
            .error_for_status()?
            .json().await
            .map_err(Into::into)
    }

    pub async fn state_quest(&self, definition_id: &str, new_state: &str) -> Result<QuestStateChange> {
        let (headers, pid) = self.headers().await?;
        self.http
            .put(format!("{}/{definition_id}/state", Self::url(QUESTS_URL, &pid)))
            .headers(headers)
            .json(&serde_json::json!({ "state": new_state }))
            .send().await?
            .error_for_status()?
            .json().await
            .map_err(Into::into)
    }

    pub async fn collect_pickup(&self)     -> Result<()> { self.claim_reward(PICKUP_ID).await }
    pub async fn collect_pickup_vip(&self) -> Result<()> { self.claim_reward(PICKUP_VIP_ID).await }

    async fn claim_reward(&self, reward_id: &str) -> Result<()> {
        let (headers, pid) = self.headers().await?;
        self.http
            .put(format!("{}/{reward_id}", Self::url(REWARDS_URL, &pid)))
            .headers(headers)
            .json(&serde_json::json!({ "state": "Claimed" }))
            .send().await?
            .error_for_status()?;
        Ok(())
    }
}

#[must_use]
pub fn random_daily_children(quests: Vec<Quest>) -> Vec<Quest> {
    quests.into_iter()
        .find(|q| q.definition_id == RANDOM_DAILY_PARENT)
        .map(|q| q.children)
        .unwrap_or_default()
}

#[must_use]
pub fn pending_random_daily_children(quests: Vec<Quest>) -> Vec<Quest> {
    let mut children = random_daily_children(quests);
    children.retain(Quest::is_pending);
    children
}

#[must_use]
fn remaining(quests: &[Quest], id: &str, target: i64) -> u32 {
    let progress = quests.iter().find(|q| q.definition_id == id).map_or(0, |q| q.progress);
    (target - progress).clamp(0, target) as u32
}