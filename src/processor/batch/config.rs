use serde::Deserialize;

/// Configuration for the built-in BatchingProcessor.
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// Flush when accumulated row count reaches this threshold.
    pub batch_size: usize,
    /// Flush when this many milliseconds have elapsed since the last flush.
    pub flush_interval_ms: u64,
}
