use std::collections::HashMap;

use serde::Deserialize;

/// Top-level pipeline configuration loaded from YAML.
///
/// Component configs (`receivers`, `processors`, `exporters`) are stored as raw
/// `serde_yaml::Value` so the framework stays agnostic to specific component types.
/// Each component's factory in the `ComponentRegistry` deserializes its own config —
/// mirroring OTel's per-component `Config` struct pattern.
#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub receivers: HashMap<String, serde_yaml::Value>,
    #[serde(default)]
    pub processors: HashMap<String, serde_yaml::Value>,
    #[serde(default)]
    pub exporters: HashMap<String, serde_yaml::Value>,
    #[serde(default)]
    pub pipelines: HashMap<String, PipelineConfig>,
}

/// Declarative pipeline definition. One receiver per pipeline (F13).
#[derive(Debug, Deserialize)]
pub struct PipelineConfig {
    /// Name of a single receiver from the top-level `receivers` map.
    pub receiver: String,
    /// Ordered list of processor names from the top-level `processors` map.
    #[serde(default)]
    pub processors: Vec<String>,
    /// Ordered list of exporter entries. Each entry is either:
    /// - a single name string (flat, mandatory)
    /// - a list of names (oneOf group: try left-to-right, first success wins)
    #[serde(default)]
    pub exporters: Vec<ExporterEntry>,
}

/// Handles both `"clickhouse/events"` (flat) and `["clickhouse/events", "s3/events"]`
/// (oneOf) in the pipeline exporters list via serde's untagged enum.
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum ExporterEntry {
    Flat(String),
    OneOf(Vec<String>),
}

impl Config {
    pub fn from_yaml(s: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(s)
    }

    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let s = std::fs::read_to_string(path)?;
        Ok(serde_yaml::from_str(&s)?)
    }
}
