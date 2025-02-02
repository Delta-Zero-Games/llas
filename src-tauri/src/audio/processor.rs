// src-tauri/src/audio/processor.rs

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use opus::{Encoder, Decoder, Channels};
use tokio::sync::mpsc;
use ringbuf::ring_buffer::{RingBuffer, DefaultRb};
use std::sync::Arc;
use tokio::sync::Mutex; // We use Tokio's Mutex for async safety.
use atomic_float::AtomicF32; // From the atomic_float crate

// A simple wrapper for cpal::Stream to mark it Send + Sync.
#[derive(Default)]
struct StreamWrapper(Option<cpal::Stream>);
unsafe impl Send for StreamWrapper {}
unsafe impl Sync for StreamWrapper {}

pub struct AudioProcessor {
    encoder: Arc<Mutex<Encoder>>,
    decoder: Arc<Mutex<Decoder>>,
    input_stream: Arc<Mutex<StreamWrapper>>,
    output_stream: Arc<Mutex<StreamWrapper>>,
    sample_rate: u32,
    channels: u16,
    tx: mpsc::Sender<Vec<u8>>,
    output_volume: Arc<AtomicF32>,
    is_muted: Arc<std::sync::atomic::AtomicBool>,
    // Specify both generic parameters for the Producer.
    pub output_producer: Option<Arc<Mutex<ringbuf::Producer<f32, DefaultRb<f32>>>>>,
}

// In our Clone implementation, we don’t clone the output_producer.
impl Clone for AudioProcessor {
    fn clone(&self) -> Self {
        Self {
            encoder: self.encoder.clone(),
            decoder: self.decoder.clone(),
            input_stream: Arc::new(Mutex::new(StreamWrapper(None))),
            output_stream: Arc::new(Mutex::new(StreamWrapper(None))),
            sample_rate: self.sample_rate,
            channels: self.channels,
            tx: self.tx.clone(),
            output_volume: self.output_volume.clone(),
            is_muted: self.is_muted.clone(),
            output_producer: None,
        }
    }
}

impl AudioProcessor {
    pub fn new(tx: mpsc::Sender<Vec<u8>>) -> Result<Self, Box<dyn std::error::Error>> {
        let encoder = Encoder::new(48000, Channels::Mono, opus::Application::Voip)?;
        let decoder = Decoder::new(48000, Channels::Mono)?;
        Ok(Self {
            encoder: Arc::new(Mutex::new(encoder)),
            decoder: Arc::new(Mutex::new(decoder)),
            input_stream: Arc::new(Mutex::new(StreamWrapper(None))),
            output_stream: Arc::new(Mutex::new(StreamWrapper(None))),
            sample_rate: 48000,
            channels: 1,
            tx,
            output_volume: Arc::new(AtomicF32::new(1.0)),
            is_muted: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            output_producer: None,
        })
    }

    pub fn setup_output_stream(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("No output device available")?;
        let config = cpal::StreamConfig {
            channels: self.channels,
            sample_rate: cpal::SampleRate(self.sample_rate),
            buffer_size: cpal::BufferSize::Fixed(480),
        };

        let ring_size = 4800; // e.g. 100ms of audio buffer
        let (producer, consumer) = RingBuffer::<f32, DefaultRb<f32>>::new(ring_size).split();
        let producer = Arc::new(Mutex::new(producer));
        self.output_producer = Some(producer.clone());

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
            None,
        )?;
        output_stream.play()?;
        *self.output_stream.lock().blocking_lock() = StreamWrapper(Some(output_stream));
        Ok(())
    }

    pub fn process_incoming(&self, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        let mut pcm_data = vec![0f32; 480];
        {
            let mut decoder = self.decoder.blocking_lock();
            // Pass the data slice directly instead of wrapping in Some(…)
            decoder.decode_float(data, &mut pcm_data, false)?;
        }
        if let Some(producer) = &self.output_producer {
            let mut prod = producer.blocking_lock();
            for sample in pcm_data {
                let _ = prod.push(sample);
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
        let device = host
            .default_input_device()
            .ok_or("No input device available")?;
        let config = cpal::StreamConfig {
            channels: self.channels,
            sample_rate: cpal::SampleRate(self.sample_rate),
            buffer_size: cpal::BufferSize::Fixed(480),
        };
        let tx = self.tx.clone();
        let encoder = self.encoder.clone();
        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &_| {
                let mut opus_data = vec![0u8; 1275]; // Maximum opus frame size.
                let mut enc = encoder.blocking_lock();
                if let Ok(size) = enc.encode_float(data, &mut opus_data) {
                    let _ = tx.try_send(opus_data[..size].to_vec());
                }
            },
            |err| eprintln!("Audio capture error: {}", err),
            None,
        )?;
        stream.play()?;
        *self.input_stream.lock().blocking_lock() = StreamWrapper(Some(stream));
        Ok(())
    }

    pub fn cleanup(&mut self) {
        *self.input_stream.lock().blocking_lock() = StreamWrapper(None);
        *self.output_stream.lock().blocking_lock() = StreamWrapper(None);
        self.output_producer = None;
    }

    pub fn set_input_device(&mut self, _device_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        // For simplicity, stop and restart capture.
        *self.input_stream.lock().blocking_lock() = StreamWrapper(None);
        self.start_capture()
    }

    pub fn set_input_volume(&self, volume: f32) -> Result<(), Box<dyn std::error::Error>> {
        let vol = volume.clamp(0.0, 1.0);
        self.output_volume.store(vol, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }
}

// Mark AudioProcessor as Send + Sync unsafely.
unsafe impl Send for AudioProcessor {}
unsafe impl Sync for AudioProcessor {}
