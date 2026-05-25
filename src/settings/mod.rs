pub mod persistence;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub command: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct AppSettings {
    #[serde(default)]
    pub agents: Vec<Agent>,
}
