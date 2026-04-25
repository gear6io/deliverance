pub mod http;

use std::sync::Arc;

use async_trait::async_trait;

use crate::error::Result;
use crate::types::Event;

/// The framework-provided bridge between a Receiver and the pipeline channel.
/// The framework supplies a concrete implementation; Receivers call `emit()`.
#[async_trait]
pub trait ReceiverHost: Send + Sync {
    /// Push an Event into the pipeline. Awaits when the channel is full,
    /// which propagates backpressure upstream to the Receiver (and its callers).
    async fn emit(&self, event: Event) -> Result<()>;
}

/// Owns its input format, validation, and transport. Emits Arrow record batches
/// into the pipeline by calling `host.emit()`.
///
/// The framework calls `start()` after all downstream components are ready.
/// `start()` must not block — spawn background tasks internally and return.
#[async_trait]
pub trait Receiver: Send + Sync {
    async fn start(&mut self, host: Arc<dyn ReceiverHost>) -> Result<()>;
    async fn shutdown(&mut self) -> Result<()>;
}
