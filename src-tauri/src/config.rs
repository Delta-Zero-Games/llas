// src-tauri/src/config.rs
use serde::{Serialize, Deserialize};
use std::env;
use dotenv::dotenv;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnConfig {
    pub url: String,
    pub username: String,
    pub credential: String,
    pub realm: String,
}

impl Default for TurnConfig {
    fn default() -> Self {
        dotenv().ok();  // Load .env file if it exists
        
        let url = env::var("TURN_SERVER_URL")
            .expect("TURN_SERVER_URL must be set in environment");
        let username = env::var("TURN_USERNAME")
            .expect("TURN_USERNAME must be set in environment");
        let credential = env::var("TURN_CREDENTIAL")
            .expect("TURN_CREDENTIAL must be set in environment");
        let realm = env::var("TURN_REALM")
            .expect("TURN_REALM must be set in environment");

        Self {
            url: if url.starts_with("turn:") { url } else { format!("turn:{}", url) },
            username,
            credential,
            realm,
        }
    }
}