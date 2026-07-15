use std::sync::Arc;
use tokio::sync::RwLock;
use crate::{errors::{MspError, Result}, models::MspSession};

#[derive(Debug, Clone, Default)]
pub struct SessionStore(Arc<RwLock<Option<MspSession>>>);

impl SessionStore {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(None)))
    }

    pub async fn set(&self, session: MspSession) {
        *self.0.write().await = Some(session);
    }

    pub async fn get(&self) -> Result<MspSession> {
        self.0
            .read()
            .await
            .clone()
            .ok_or(MspError::NoSession)
    }

    pub async fn clear(&self) {
        *self.0.write().await = None;
    }

    pub fn inner(&self) -> Arc<RwLock<Option<MspSession>>> {
        Arc::clone(&self.0)
    }
}