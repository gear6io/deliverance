pub mod http;

use std::sync::Arc;

use async_trait::async_trait;

use crate::components::Settings;
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

/// Factory for creating [`Receiver`] instances. Register one factory per component type;
/// the registry instantiates a fresh receiver for every component ID in the config.
///
/// Mirrors OTel's `receiver.Factory`. `create_default_config` returns the serialized
/// default config (useful for documentation and tooling); `create` handles deserialization
/// and construction.
pub trait ReceiverFactory: Send + Sync {
    fn component_type(&self) -> &'static str;
    fn create_default_config(&self) -> serde_yaml::Value;
    fn create(
        &self,
        settings: &Settings,
        config: &serde_yaml::Value,
    ) -> anyhow::Result<Box<dyn Receiver>>;
}
