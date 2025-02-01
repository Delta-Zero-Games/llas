// src-tauri/src/config.rs
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnConfig {
    pub url: String,
    pub username: String,
    pub credential: String,
    pub realm: String,
}

impl Default for TurnConfig {
    fn default() -> Self {
        Self {
            url: "turn:137.184.122.169:3478".to_string(),
            username: "brokenhypocrite".to_string(),
            credential: "formula1".to_string(),
            realm: "137.184.122.169".to_string(),
        }
    }
}