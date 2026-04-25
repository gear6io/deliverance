use std::collections::HashMap;

use crate::components::{ComponentID, Settings};
use crate::config::{Config, ExporterEntry, PipelineConfig};
use crate::error::{IngestionError, Result};
use crate::exporter::{Exporter, ExporterFactory};
use crate::pipeline::{ExporterGroup, Pipeline, DEFAULT_CHANNEL_CAPACITY};
use crate::processor::{Processor, ProcessorFactory};
use crate::receiver::{Receiver, ReceiverFactory};

/// Static component registry. Factories are registered at startup (compile-time imports);
/// the Engine uses the registry to construct pipeline instances from YAML config at runtime.
///
/// Component IDs follow the OTel `{type}/{name}` convention — the type selects the factory,
/// the name disambiguates multiple instances of the same type within a config file.
/// A bare `{type}` with no name is valid and treated as a single unnamed instance.
pub struct ComponentRegistry {
    receivers: HashMap<String, Box<dyn ReceiverFactory>>,
    processors: HashMap<String, Box<dyn ProcessorFactory>>,
    exporters: HashMap<String, Box<dyn ExporterFactory>>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self {
            receivers: HashMap::new(),
            processors: HashMap::new(),
            exporters: HashMap::new(),
        }
    }

    /// Register a receiver factory. Keyed by `factory.component_type()`.
    pub fn register_receiver(&mut self, factory: Box<dyn ReceiverFactory>) {
        self.receivers
            .insert(factory.component_type().to_string(), factory);
    }

    /// Register a processor factory. Keyed by `factory.component_type()`.
    pub fn register_processor(&mut self, factory: Box<dyn ProcessorFactory>) {
        self.processors
            .insert(factory.component_type().to_string(), factory);
    }

    /// Register an exporter factory. Keyed by `factory.component_type()`.
    pub fn register_exporter(&mut self, factory: Box<dyn ExporterFactory>) {
        self.exporters
            .insert(factory.component_type().to_string(), factory);
    }

    /// Construct all pipelines from the config, resolving component IDs via registered factories.
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

    fn build_receiver(&self, id_str: &str, config: &Config) -> Result<Box<dyn Receiver>> {
        let id = parse_id(id_str)?;
        let factory = self.receivers.get(id.type_str()).ok_or_else(|| {
            IngestionError::Config(format!(
                "no receiver factory registered for type '{}' (from id '{id_str}')",
                id.type_str()
            ))
        })?;
        let raw = config.receivers.get(id_str).ok_or_else(|| {
            IngestionError::Config(format!("receiver '{id_str}' missing from config"))
        })?;
        factory
            .create(&Settings::new(id), raw)
            .map_err(IngestionError::Component)
    }

    fn build_processor(&self, id_str: &str, config: &Config) -> Result<Box<dyn Processor>> {
        let id = parse_id(id_str)?;
        let factory = self.processors.get(id.type_str()).ok_or_else(|| {
            IngestionError::Config(format!(
                "no processor factory registered for type '{}' (from id '{id_str}')",
                id.type_str()
            ))
        })?;
        let raw = config.processors.get(id_str).ok_or_else(|| {
            IngestionError::Config(format!("processor '{id_str}' missing from config"))
        })?;
        factory
            .create(&Settings::new(id), raw)
            .map_err(IngestionError::Component)
    }

    fn build_exporter(&self, id_str: &str, config: &Config) -> Result<Box<dyn Exporter>> {
        let id = parse_id(id_str)?;
        let factory = self.exporters.get(id.type_str()).ok_or_else(|| {
            IngestionError::Config(format!(
                "no exporter factory registered for type '{}' (from id '{id_str}')",
                id.type_str()
            ))
        })?;
        let raw = config.exporters.get(id_str).ok_or_else(|| {
            IngestionError::Config(format!("exporter '{id_str}' missing from config"))
        })?;
        factory
            .create(&Settings::new(id), raw)
            .map_err(IngestionError::Component)
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

fn parse_id(s: &str) -> Result<ComponentID> {
    s.parse()
        .map_err(|e: anyhow::Error| IngestionError::Config(e.to_string()))
}
