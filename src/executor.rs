use std::time::Duration;

use tokio::time::sleep;
use tracing::{error, warn};

use crate::error::{IngestionError, Result};
use crate::exporter::Exporter;
use crate::pipeline::ExporterGroup;
use crate::types::Batch;

/// Retry parameters for mandatory exporter groups.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: usize,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff_ms: 100,
            max_backoff_ms: 10_000,
        }
    }
}

/// Fans a Batch out to all exporter groups (F6–F11).
///
/// For each group in order:
/// - `Flat`: send unconditionally; retry with exponential backoff on failure.
/// - `OneOf`: try exporters left-to-right; first success wins (F8). If all fail,
///   retry the group from the head with backoff (F10). Drop after max retries.
///
/// Each new Batch always starts from index 0 of each OneOf group — no sticky
/// failover state persists between flushes (F9).
pub struct Executor {
    groups: Vec<ExporterGroup>,
    retry: RetryConfig,
}

impl Executor {
    pub fn new(groups: Vec<ExporterGroup>, retry: RetryConfig) -> Self {
        Self { groups, retry }
    }

    pub async fn flush(&mut self, batch: &Batch) -> Result<()> {
        let retry = self.retry.clone();
        for group in &mut self.groups {
            match group {
                ExporterGroup::Flat(exporter) => {
                    export_flat(exporter.as_mut(), batch, &retry).await?;
                }
                ExporterGroup::OneOf(exporters) => {
                    export_one_of(exporters, batch, &retry).await?;
                }
            }
        }
        Ok(())
    }
}

/// Retry a single mandatory exporter with exponential backoff.
async fn export_flat(
    exporter: &mut dyn Exporter,
    batch: &Batch,
    retry: &RetryConfig,
) -> Result<()> {
    let mut backoff_ms = retry.initial_backoff_ms;

    for attempt in 0..retry.max_attempts {
        match exporter.export(batch).await {
            Ok(()) => return Ok(()),
            Err(e) if attempt + 1 == retry.max_attempts => {
                error!(
                    error = %e,
                    attempt,
                    "mandatory exporter exhausted retries — dropping Batch"
                );
                return Err(IngestionError::MaxRetriesExhausted {
                    attempts: retry.max_attempts,
                });
            }
            Err(e) => {
                warn!(error = %e, attempt, backoff_ms, "exporter failed — retrying");
                sleep(Duration::from_millis(backoff_ms)).await;
                backoff_ms = (backoff_ms * 2).min(retry.max_backoff_ms);
            }
        }
    }

    unreachable!()
}

/// Try each exporter in order; first success wins. If all fail, retry the group.
async fn export_one_of(
    exporters: &mut Vec<Box<dyn Exporter>>,
    batch: &Batch,
    retry: &RetryConfig,
) -> Result<()> {
    let mut backoff_ms = retry.initial_backoff_ms;

    for attempt in 0..retry.max_attempts {
        for exporter in exporters.iter_mut() {
            match exporter.export(batch).await {
                Ok(()) => return Ok(()),
                Err(ref e) => {
                    warn!(error = %e, attempt, "oneOf exporter failed — trying next");
                }
            }
        }

        if attempt + 1 == retry.max_attempts {
            error!(attempt, "oneOf group exhausted all retries — dropping Batch");
            return Err(IngestionError::MaxRetriesExhausted {
                attempts: retry.max_attempts,
            });
        }

        warn!(
            backoff_ms,
            attempt,
            "all oneOf exporters failed — retrying group after backoff"
        );
        sleep(Duration::from_millis(backoff_ms)).await;
        backoff_ms = (backoff_ms * 2).min(retry.max_backoff_ms);
    }

    unreachable!()
}
