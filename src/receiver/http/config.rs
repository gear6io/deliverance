use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct HttpReceiverConfig {
    /// TCP address to listen on, e.g. "0.0.0.0:4318".
    pub endpoint: String,
}
