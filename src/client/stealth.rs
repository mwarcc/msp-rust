use std::time::Duration;
use rand::Rng;

#[derive(Debug, Clone, Copy)]
pub struct StealthConfig {
    pub enabled:   bool,
    pub min_delay: Duration,
    pub max_delay: Duration,
}

impl Default for StealthConfig {
    fn default() -> Self {
        Self {
            enabled:   false,
            min_delay: Duration::from_millis(120),
            max_delay: Duration::from_millis(650),
        }
    }
}

impl StealthConfig {
    pub fn enabled(min: Duration, max: Duration) -> Self {
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        Self { enabled: true, min_delay: min, max_delay: max }
    }

    pub async fn pace(&self) {
        if !self.enabled {
            return;
        }
        let lo = self.min_delay.as_millis() as u64;
        let hi = self.max_delay.as_millis() as u64;
        let millis = if lo >= hi {
            lo
        } else {
            rand::thread_rng().gen_range(lo..=hi)
        };
        tokio::time::sleep(Duration::from_millis(millis)).await;
    }
}