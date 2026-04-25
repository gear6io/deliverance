use serde::Deserialize;

/// Configuration for the built-in MemoryProcessor.
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// Memory ceiling in mebibytes. Pipeline blocks when system used memory exceeds this.
    pub limit_mib: u64,
    /// How often to re-check memory while blocking, in milliseconds.
    #[serde(default = "default_check_interval_ms")]
    pub check_interval_ms: u64,
}

fn default_check_interval_ms() -> u64 {
    500
}
