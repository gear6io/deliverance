use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct HttpReceiverConfig {
    /// TCP address to listen on, e.g. "0.0.0.0:4318".
    pub endpoint: String,
}

impl Default for HttpReceiverConfig {
    fn default() -> Self {
        Self {
            endpoint: "0.0.0.0:4318".to_string(),
        }
    }
}
