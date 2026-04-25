pub mod batch;
pub mod memory;

use async_trait::async_trait;

use crate::error::Result;
use crate::types::Batch;

/// Transforms or gate-checks a Batch in the pipeline.
///
/// `process()` takes ownership of the Batch and returns it (possibly modified or
/// accumulated). Two built-in conventions:
///
/// - **Passthrough** (MemoryProcessor): await until safe, return the Batch unchanged.
/// - **Accumulate** (BatchingProcessor): absorb events, return an empty `Batch` when
///   not yet ready to flush, or the accumulated `Batch` when flush conditions are met.
///
/// The pipeline loop checks `batch.is_empty()` after each processor and skips the
/// Executor if the Batch is empty — this is the "not yet" signal from BatchingProcessor.
#[async_trait]
pub trait Processor: Send + Sync {
    async fn process(&mut self, batch: Batch) -> Result<Batch>;
    async fn start(&mut self) -> Result<()>;
    async fn shutdown(&mut self) -> Result<()>;
}
