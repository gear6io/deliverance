pub mod config;

use async_trait::async_trait;
use tokio::time::{Duration, Instant};

use crate::error::Result;
use crate::processor::Processor;
use crate::types::Batch;

pub use config::Config;

/// Accumulator without transformation (F4, F5). Mirrors OTel's batchprocessor.
///
/// Collects incoming events into an internal buffer. Returns:
/// - A non-empty `Batch` (the accumulated buffer) when flush conditions are met.
/// - An empty `Batch` when not yet ready — the pipeline loop treats this as "not yet"
///   and skips the Executor for this tick.
///
/// Flush triggers: accumulated row count >= `batch_size` OR time since last flush
/// >= `flush_interval_ms`. Note (v1): the timer fires only when an event arrives,
/// so sparse traffic may accumulate past the deadline until the next event.
pub struct BatchingProcessor {
    config: Config,
    buffer: Batch,
    last_flush: Instant,
}

impl BatchingProcessor {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            buffer: Batch::new(),
            last_flush: Instant::now(),
        }
    }

    /// Construct from a raw YAML value — called by the ComponentRegistry factory.
    pub fn from_config(raw: &serde_yaml::Value) -> anyhow::Result<Box<dyn Processor>> {
        let cfg: Config = serde_yaml::from_value(raw.clone())?;
        Ok(Box::new(Self::new(cfg)))
    }

    fn should_flush(&self) -> bool {
        let elapsed = self.last_flush.elapsed();
        self.buffer.row_count() >= self.config.batch_size
            || elapsed >= Duration::from_millis(self.config.flush_interval_ms)
    }

    fn flush(&mut self) -> Batch {
        self.last_flush = Instant::now();
        std::mem::replace(&mut self.buffer, Batch::new())
    }

    /// Drain the internal buffer regardless of flush conditions.
    /// Called by the Engine during shutdown to emit any remaining events.
    pub fn drain(&mut self) -> Batch {
        self.flush()
    }
}

#[async_trait]
impl Processor for BatchingProcessor {
    async fn start(&mut self) -> Result<()> {
        Ok(())
    }

    async fn process(&mut self, batch: Batch) -> Result<Batch> {
        for event in batch.events {
            self.buffer.push(event);
        }

        if self.should_flush() {
            Ok(self.flush())
        } else {
            Ok(Batch::new())
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}
