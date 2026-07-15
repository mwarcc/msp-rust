use tokio::sync::broadcast;
use crate::events::MspEvent;

const BUS_CAPACITY: usize = 256;

#[derive(Debug, Clone)]
pub struct EventBus {
    sender: broadcast::Sender<MspEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(BUS_CAPACITY);
        Self { sender }
    }

    pub(crate) fn publish(&self, event: MspEvent) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<MspEvent> {
        self.sender.subscribe()
    }

    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}