// src/audio/mod.rs

pub mod network;
pub mod processor;

// Re-export the key types for easier use elsewhere in your crate.
pub use network::AudioNetwork;
pub use processor::AudioProcessor;
