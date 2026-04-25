use std::collections::HashMap;

use crate::config::{Config, ExporterEntry, PipelineConfig};
use crate::error::{IngestionError, Result};
use crate::exporter::Exporter;
use crate::pipeline::{ExporterGroup, Pipeline, DEFAULT_CHANNEL_CAPACITY};
use crate::processor::Processor;
use crate::receiver::Receiver;

/// Factory function type for Receivers. Receives the raw component YAML config value.
pub type ReceiverFactory =
    Box<dyn Fn(&serde_yaml::Value) -> anyhow::Result<Box<dyn Receiver>> + Send + Sync>;

/// Factory function type for Processors.
pub type ProcessorFactory =
    Box<dyn Fn(&serde_yaml::Value) -> anyhow::Result<Box<dyn Processor>> + Send + Sync>;

/// Factory function type for Exporters.
pub type ExporterFactory =
    Box<dyn Fn(&serde_yaml::Value) -> anyhow::Result<Box<dyn Exporter>> + Send + Sync>;

/// Static component registry. Components are registered at startup (compile-time imports);
/// the Engine uses the registry to construct pipelines from YAML config at runtime.
///
/// This is the answer to PRD Q2: static compilation, not runtime plugin loading.
/// The enterprise layer imports OSS + private packages and registers all components in main().
pub struct ComponentRegistry {
    receivers: HashMap<String, ReceiverFactory>,
    processors: HashMap<String, ProcessorFactory>,
    exporters: HashMap<String, ExporterFactory>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self {
            receivers: HashMap::new(),
            processors: HashMap::new(),
            exporters: HashMap::new(),
        }
    }

    /// Register a Receiver factory under `name` (must match the YAML receivers map key).
    pub fn register_receiver(&mut self, name: impl Into<String>, factory: ReceiverFactory) {
        self.receivers.insert(name.into(), factory);
    }

    /// Register a Processor factory under `name`.
    pub fn register_processor(&mut self, name: impl Into<String>, factory: ProcessorFactory) {
        self.processors.insert(name.into(), factory);
    }

    /// Register an Exporter factory under `name`.
    pub fn register_exporter(&mut self, name: impl Into<String>, factory: ExporterFactory) {
        self.exporters.insert(name.into(), factory);
    }

    /// Construct all pipelines from the config, resolving component names via registered factories.
    pub fn build_all(&self, config: &Config) -> Result<Vec<Pipeline>> {
        config
            .pipelines
            .iter()
            .map(|(name, pipeline_cfg)| self.build_pipeline(name, pipeline_cfg, config))
            .collect()
    }

    fn build_pipeline(
        &self,
        name: &str,
        pipeline_cfg: &PipelineConfig,
        config: &Config,
    ) -> Result<Pipeline> {
        let receiver = self.build_receiver(&pipeline_cfg.receiver, config)?;
        let processors = pipeline_cfg
            .processors
            .iter()
            .map(|p| self.build_processor(p, config))
            .collect::<Result<Vec<_>>>()?;
        let exporter_groups = pipeline_cfg
            .exporters
            .iter()
            .map(|entry| self.build_exporter_group(entry, config))
            .collect::<Result<Vec<_>>>()?;

        Ok(Pipeline {
            name: name.to_string(),
            receiver,
            processors,
            exporter_groups,
            channel_capacity: DEFAULT_CHANNEL_CAPACITY,
        })
    }

    fn build_receiver(&self, name: &str, config: &Config) -> Result<Box<dyn Receiver>> {
        let factory = self.receivers.get(name).ok_or_else(|| {
            IngestionError::Config(format!("no receiver registered for '{name}'"))
        })?;
        let raw = config.receivers.get(name).ok_or_else(|| {
            IngestionError::Config(format!("receiver '{name}' missing from config"))
        })?;
        factory(raw).map_err(IngestionError::Component)
    }

    fn build_processor(&self, name: &str, config: &Config) -> Result<Box<dyn Processor>> {
        let factory = self.processors.get(name).ok_or_else(|| {
            IngestionError::Config(format!("no processor registered for '{name}'"))
        })?;
        let raw = config.processors.get(name).ok_or_else(|| {
            IngestionError::Config(format!("processor '{name}' missing from config"))
        })?;
        factory(raw).map_err(IngestionError::Component)
    }

    fn build_exporter(&self, name: &str, config: &Config) -> Result<Box<dyn Exporter>> {
        let factory = self.exporters.get(name).ok_or_else(|| {
            IngestionError::Config(format!("no exporter registered for '{name}'"))
        })?;
        let raw = config.exporters.get(name).ok_or_else(|| {
            IngestionError::Config(format!("exporter '{name}' missing from config"))
        })?;
        factory(raw).map_err(IngestionError::Component)
    }

    fn build_exporter_group(
        &self,
        entry: &ExporterEntry,
        config: &Config,
    ) -> Result<ExporterGroup> {
        match entry {
            ExporterEntry::Flat(name) => {
                Ok(ExporterGroup::Flat(self.build_exporter(name, config)?))
            }
            ExporterEntry::OneOf(names) => {
                let exporters = names
                    .iter()
                    .map(|n| self.build_exporter(n, config))
                    .collect::<Result<Vec<_>>>()?;
                Ok(ExporterGroup::OneOf(exporters))
            }
        }
    }
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        Self::new()
    }
}
