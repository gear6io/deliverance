pub mod config;

use async_trait::async_trait;
use tracing::info;

use crate::components::{Component, Settings};
use crate::error::Result;
use crate::exporter::{Exporter, ExporterFactory};
use crate::types::Batch;

pub use config::{Config, Verbosity};

pub struct DebugExporter {
    config: Config,
}

impl Component for DebugExporter {
    fn component_type() -> &'static str {
        "debugexporter"
    }
}

#[async_trait]
impl Exporter for DebugExporter {
    async fn start(&mut self) -> Result<()> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }

    async fn export(&mut self, batch: &Batch) -> Result<()> {
        match self.config.verbosity {
            Verbosity::Basic => {
                info!(events = batch.len(), rows = batch.row_count(), "debugexporter");
            }
            Verbosity::Normal => {
                for (i, event) in batch.events.iter().enumerate() {
                    let schema = event.record.schema();
                    let fields: Vec<String> = schema
                        .fields()
                        .iter()
                        .map(|f| format!("{}:{}", f.name(), f.data_type()))
                        .collect();
                    info!(
                        event = i,
                        source = ?event.source,
                        rows = event.record.num_rows(),
                        schema = %fields.join(", "),
                        "debugexporter"
                    );
                }
            }
            Verbosity::Detailed => {
                for (i, event) in batch.events.iter().enumerate() {
                    let table = arrow::util::pretty::pretty_format_batches(&[
                        (*event.record).clone(),
                    ])?
                    .to_string();
                    info!(
                        event = i,
                        source = ?event.source,
                        rows = event.record.num_rows(),
                        "\n{table}"
                    );
                }
            }
        }
        Ok(())
    }
}

pub struct DebugExporterFactory;

impl ExporterFactory for DebugExporterFactory {
    fn component_type(&self) -> &'static str {
        DebugExporter::component_type()
    }

    fn create_default_config(&self) -> serde_yaml::Value {
        serde_yaml::to_value(Config::default()).expect("Config serializes cleanly")
    }

    fn create(
        &self,
        _settings: &Settings,
        config: &serde_yaml::Value,
    ) -> anyhow::Result<Box<dyn Exporter>> {
        let cfg: Config = serde_yaml::from_value(config.clone())?;
        Ok(Box::new(DebugExporter { config: cfg }))
    }
}
