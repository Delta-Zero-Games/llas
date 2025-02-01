// src-tauri/src/audio/capture.rs
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use opus::{Encoder, Channels};

pub struct AudioCapture {
    encoder: Encoder,
    sample_rate: u32,
    channels: u16,
}

impl AudioCapture {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let encoder = Encoder::new(48000, Channels::Mono, opus::Application::Voip)?;
        
        Ok(Self {
            encoder,
            sample_rate: 48000,
            channels: 1,
        })
    }

    pub fn setup_stream<F>(&self, mut callback: F) -> Result<cpal::Stream, Box<dyn std::error::Error>> 
    where
        F: FnMut(&[f32]) + Send + 'static,
    {
        let host = cpal::default_host();
        let device = host.default_input_device()
            .ok_or("No input device available")?;

        let config = cpal::StreamConfig {
            channels: self.channels,
            sample_rate: cpal::SampleRate(self.sample_rate),
            buffer_size: cpal::BufferSize::Fixed(480), // 10ms at 48kHz
        };

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &_| callback(data),
            |err| eprintln!("Audio capture error: {}", err),
        )?;

        Ok(stream)
    }
}