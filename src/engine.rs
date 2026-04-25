use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{error, info};

use crate::error::Result;
use crate::executor::{Executor, RetryConfig};
use crate::pipeline::{ChannelReceiverHost, Pipeline};
use crate::processor::Processor;
use crate::types::Batch;

/// Orchestrates component lifecycle and drives the pipeline loop(s) (F17).
///
/// Start order:  Exporters → Processors → Receivers
/// Shutdown order: Receivers → drain channels → Processors → Exporters
pub struct Engine {
    pipelines: Vec<Pipeline>,
    retry: RetryConfig,
}

impl Engine {
    pub fn new(pipelines: Vec<Pipeline>) -> Self {
        Self {
            pipelines,
            retry: RetryConfig::default(),
        }
    }

    pub fn with_retry(mut self, retry: RetryConfig) -> Self {
        self.retry = retry;
        self
    }

    /// Start all pipelines. Returns task handles for the pipeline loops.
    /// Callers should await these handles (or select on them) to detect errors.
    pub async fn start(&mut self) -> Result<Vec<JoinHandle<()>>> {
        let mut handles = Vec::new();

        for pipeline in &mut self.pipelines {
            info!(pipeline = %pipeline.name, "starting pipeline");

            // Step 1: start exporters
            for group in &mut pipeline.exporter_groups {
                match group {
                    crate::pipeline::ExporterGroup::Flat(e) => e.start().await?,
                    crate::pipeline::ExporterGroup::OneOf(exporters) => {
                        for e in exporters.iter_mut() {
                            e.start().await?;
                        }
                    }
                }
            }

            // Step 2: start processors
            for p in &mut pipeline.processors {
                p.start().await?;
            }

            // Step 3: wire channel, build executor, spawn pipeline loop, start receiver
            let (tx, rx) = mpsc::channel(pipeline.channel_capacity);
            let host = Arc::new(ChannelReceiverHost::new(tx));

            // Move exporter groups out of the pipeline into the executor.
            // Pipeline.exporter_groups is left empty after this point; the executor owns them.
            let groups = std::mem::take(&mut pipeline.exporter_groups);
            let executor = Executor::new(groups, self.retry.clone());

            // Move processors out of the pipeline into the loop task.
            let processors = std::mem::take(&mut pipeline.processors);

            let pipeline_name = pipeline.name.clone();
            let handle = tokio::spawn(pipeline_loop(pipeline_name, rx, processors, executor));
            handles.push(handle);

            pipeline.receiver.start(host).await?;
        }

        Ok(handles)
    }

    /// Graceful shutdown: stop receivers first, then wait for loops to drain, then
    /// shut down processors and exporters.
    ///
    /// Note: after `start()`, processors and exporter_groups have been moved into the
    /// tokio tasks. Shutdown of those components happens inside `pipeline_loop` when
    /// the channel closes (receiver stopped → tx dropped → rx.recv() returns None).
    pub async fn shutdown(&mut self) -> Result<()> {
        for pipeline in &mut self.pipelines {
            info!(pipeline = %pipeline.name, "shutting down receiver");
            pipeline.receiver.shutdown().await?;
        }
        Ok(())
    }
}

/// The inner pipeline loop — runs as a dedicated tokio task per pipeline.
///
/// Reads Events from the channel, runs the processor chain, and calls the executor.
/// Exits when the channel closes (sender dropped after receiver shutdown).
async fn pipeline_loop(
    name: String,
    mut rx: mpsc::Receiver<crate::types::Event>,
    mut processors: Vec<Box<dyn Processor>>,
    mut executor: Executor,
) {
    info!(pipeline = %name, "pipeline loop started");

    while let Some(event) = rx.recv().await {
        let mut slot: Option<Batch> = {
            let mut b = Batch::new();
            b.push(event);
            Some(b)
        };

        // Run the processor chain. `slot` is None after this block when:
        // - a processor returned an error (event dropped), or
        // - BatchingProcessor returned an empty Batch ("not yet").
        'chain: for processor in &mut processors {
            let batch = slot.take().unwrap();
            match processor.process(batch).await {
                Err(e) => {
                    error!(pipeline = %name, error = %e, "processor error — dropping event");
                    break 'chain;
                }
                Ok(b) if b.is_empty() => {
                    break 'chain;
                }
                Ok(b) => {
                    slot = Some(b);
                }
            }
        }

        if let Some(batch) = slot {
            if let Err(e) = executor.flush(&batch).await {
                error!(pipeline = %name, error = %e, "executor flush failed");
            }
        }
    }

    // Channel drained — shut down processors and exporters in order
    for processor in &mut processors {
        if let Err(e) = processor.shutdown().await {
            error!(pipeline = %name, error = %e, "processor shutdown error");
        }
    }

    info!(pipeline = %name, "pipeline loop exiting");
}
