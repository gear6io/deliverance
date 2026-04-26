use serde::{Deserialize, Serialize};

/// Configuration for the built-in BatchingProcessor.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    /// Flush when accumulated row count reaches this threshold.
    pub batch_size: usize,
    /// Flush when this many milliseconds have elapsed since the last flush.
    pub flush_interval_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            flush_interval_ms: 5000,
        }
    }
}
