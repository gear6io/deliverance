pub mod config;

use async_trait::async_trait;
use sysinfo::System;
use tokio::time::{sleep, Duration};

use crate::error::Result;
use crate::processor::Processor;
use crate::types::Batch;

pub use config::Config;

/// OOM circuit breaker (F3). Mirrors OTel's memorylimiter.
///
/// When system used memory is at or above the configured ceiling, `process()` parks
/// the owned Batch and polls until memory drops below the threshold. While parked:
/// - nothing reads from the pipeline channel
/// - the channel fills up
/// - `ChannelReceiverHost::emit()` awaits
/// - the Receiver's caller (e.g. HTTP handler) blocks
/// - the client request stalls
/// This chain propagates backpressure without any extra signaling.
pub struct MemoryProcessor {
    config: Config,
    system: System,
}

impl MemoryProcessor {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            system: System::new(),
        }
    }

    /// Construct from a raw YAML value — called by the ComponentRegistry factory.
    pub fn from_config(raw: &serde_yaml::Value) -> anyhow::Result<Box<dyn Processor>> {
        let cfg: Config = serde_yaml::from_value(raw.clone())?;
        Ok(Box::new(Self::new(cfg)))
    }

    fn used_memory_mib(&mut self) -> u64 {
        self.system.refresh_memory();
        self.system.used_memory() / (1024 * 1024)
    }
}

#[async_trait]
impl Processor for MemoryProcessor {
    async fn start(&mut self) -> Result<()> {
        Ok(())
    }

    async fn process(&mut self, batch: Batch) -> Result<Batch> {
        let check_interval = Duration::from_millis(self.config.check_interval_ms);
        loop {
            let used_mib = self.used_memory_mib();
            if used_mib < self.config.limit_mib {
                return Ok(batch);
            }
            tracing::warn!(
                used_mib,
                limit_mib = self.config.limit_mib,
                "memory ceiling hit — blocking pipeline reads"
            );
            sleep(check_interval).await;
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}
