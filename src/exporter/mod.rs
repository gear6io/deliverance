use async_trait::async_trait;

use crate::components::Settings;
use crate::error::Result;
use crate::types::Batch;

/// Sends a Batch to a destination. Stateless — owns its own validation and conversion.
///
/// `export()` receives the Batch by reference: the Executor retains ownership for retry
/// purposes. The Executor holds the Batch until all mandatory exporters succeed or max
/// retries are exhausted.
///
/// Exporters must not implement retry logic — that belongs to the Executor.
#[async_trait]
pub trait Exporter: Send + Sync {
    async fn export(&mut self, batch: &Batch) -> Result<()>;
    async fn start(&mut self) -> Result<()>;
    async fn shutdown(&mut self) -> Result<()>;
}

/// Factory for creating [`Exporter`] instances. Register one factory per component type.
/// Mirrors OTel's `exporter.Factory`.
pub trait ExporterFactory: Send + Sync {
    fn component_type(&self) -> &'static str;
    fn create_default_config(&self) -> serde_yaml::Value;
    fn create(
        &self,
        settings: &Settings,
        config: &serde_yaml::Value,
    ) -> anyhow::Result<Box<dyn Exporter>>;
}
