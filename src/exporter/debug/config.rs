use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verbosity {
    #[default]
    Basic,
    Normal,
    Detailed,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub verbosity: Verbosity,
}
