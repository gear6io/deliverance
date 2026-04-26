use serde::{Deserialize, Serialize};

/// Configuration for the built-in MemoryProcessor.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    /// Memory ceiling in mebibytes. Pipeline blocks when system used memory exceeds this.
    pub limit_mib: u64,
    /// How often to re-check memory while blocking, in milliseconds.
    #[serde(default = "default_check_interval_ms")]
    pub check_interval_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            limit_mib: 512,
            check_interval_ms: default_check_interval_ms(),
        }
    }
}

fn default_check_interval_ms() -> u64 {
    500
}
