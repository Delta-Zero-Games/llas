// src-tauri/src/audio/processor.rs
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use opus::{Encoder, Decoder, Channels};
use tokio::sync::mpsc;
use ringbuf::RingBuffer;
use std::sync::Arc;

pub struct AudioProcessor {
    encoder: Encoder,
    decoder: Decoder,
    input_stream: Option<cpal::Stream>,
    output_stream: Option<cpal::Stream>,
    sample_rate: u32,
    channels: u16,
    tx: mpsc::Sender<Vec<u8>>,
    output_volume: Arc<std::sync::atomic::AtomicF32>,
    is_muted: Arc<std::sync::atomic::AtomicBool>,
}

impl AudioProcessor {
    pub fn new(tx: mpsc::Sender<Vec<u8>>) -> Result<Self, Box<dyn std::error::Error>> {
        let encoder = Encoder::new(48000, Channels::Mono, opus::Application::Voip)?;
        let decoder = Decoder::new(48000, Channels::Mono)?;

        Ok(Self {
            encoder,
            decoder,
            input_stream: None,
            output_stream: None,
            sample_rate: 48000,
            channels: 1,
            tx,
            output_volume: Arc::new(std::sync::atomic::AtomicF32::new(1.0)),
            is_muted: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }

    pub fn setup_output_stream(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host.default_output_device()
            .ok_or("No output device available")?;

        let config = cpal::StreamConfig {
            channels: self.channels,
            sample_rate: cpal::SampleRate(self.sample_rate),
            buffer_size: cpal::BufferSize::Fixed(480), // 10ms at 48kHz
        };

        // Create ring buffer for output samples
        let ring_size = 4800; // 100ms buffer
        let (producer, consumer) = RingBuffer::<f32>::new(ring_size).split();
        let producer = Arc::new(parking_lot::Mutex::new(producer));
        
        // Store producer for later use
        self.output_producer = Some(producer.clone());

        // Get volume control
        let volume = self.output_volume.clone();
        let is_muted = self.is_muted.clone();

        let output_stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for sample in data.iter_mut() {
                    if is_muted.load(std::sync::atomic::Ordering::Relaxed) {
                        *sample = 0.0;
                    } else {
                        *sample = consumer
                            .pop()
                            .unwrap_or(0.0) 
                            * volume.load(std::sync::atomic::Ordering::Relaxed);
                    }
                }
            },
            |err| eprintln!("Output error: {}", err),
            None
        )?;

        output_stream.play()?;
        self.output_stream = Some(output_stream);

        Ok(())
    }

    pub fn process_incoming(&mut self, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        let mut pcm_data = vec![0f32; 480]; // 10ms of audio at 48kHz
        self.decoder.decode_float(Some(data), &mut pcm_data, false)?;
        
        // Write to output buffer
        if let Some(producer) = &self.output_producer {
            let mut producer = producer.lock();
            for sample in pcm_data {
                let _ = producer.push(sample);
            }
        }
        
        Ok(())
    }

    pub fn set_output_volume(&self, volume: f32) {
        self.output_volume.store(volume, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn set_muted(&self, muted: bool) {
        self.is_muted.store(muted, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn start_capture(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host.default_input_device()
            .ok_or("No input device available")?;

        let config = cpal::StreamConfig {
            channels: self.channels,
            sample_rate: cpal::SampleRate(self.sample_rate),
            buffer_size: cpal::BufferSize::Fixed(480),
        };

        let tx = self.tx.clone();
        let mut encoder = self.encoder.clone();

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &_| {
                let mut opus_data = vec![0u8; 1275]; // Max opus frame size
                if let Ok(size) = encoder.encode_float(data, &mut opus_data) {
                    let _ = tx.try_send(opus_data[..size].to_vec());
                }
            },
            |err| eprintln!("Audio capture error: {}", err),
            None
        )?;

        stream.play()?;
        self.input_stream = Some(stream);
        Ok(())
    }
}