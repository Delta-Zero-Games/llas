// src-tauri/src/audio/processor.rs

use cpal::traits::{DeviceTrait, HostTrait};
use opus::{Encoder, Decoder, Channels, Application, Bitrate};
use tokio::sync::mpsc;
use ringbuf::{HeapRb, Producer};
use std::sync::Arc;
use tokio::sync::Mutex;
use atomic_float::AtomicF32;

// Audio configuration constants
const SAMPLE_RATE: u32 = 48000;
const CHANNELS: u16 = 1;
const FRAME_SIZE: usize = 480;  // 10ms at 48kHz
const RING_BUFFER_SIZE: usize = 1440;  // 30ms buffer (reduced from 100ms for lower latency)
const MAX_PACKET_SIZE: usize = 1275;  // Maximum Opus packet size
const OPUS_BITRATE: i32 = 64000;  // 64 kbps for high quality voice

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
    tx: mpsc::Sender<Vec<u8>>,
    output_volume: Arc<AtomicF32>,
    is_muted: Arc<std::sync::atomic::AtomicBool>,
    input_peak: Arc<AtomicF32>,  // For input level monitoring
    output_peak: Arc<AtomicF32>, // For output level monitoring
    pub output_producer: Option<Arc<Mutex<Producer<f32, Arc<HeapRb<f32>>>>>>,
    input_volume: Arc<AtomicF32>,
}

impl Clone for AudioProcessor {
    fn clone(&self) -> Self {
        Self {
            encoder: self.encoder.clone(),
            decoder: self.decoder.clone(),
            input_stream: Arc::new(Mutex::new(StreamWrapper(None))),
            output_stream: Arc::new(Mutex::new(StreamWrapper(None))),
            tx: self.tx.clone(),
            output_volume: self.output_volume.clone(),
            is_muted: self.is_muted.clone(),
            input_peak: self.input_peak.clone(),
            output_peak: self.output_peak.clone(),
            output_producer: None,
            input_volume: self.input_volume.clone(),
        }
    }
}

impl AudioProcessor {
    pub fn new(tx: mpsc::Sender<Vec<u8>>) -> Result<Self, Box<dyn std::error::Error>> {
        // Create and configure Opus encoder
        let mut encoder = Encoder::new(SAMPLE_RATE, Channels::Mono, Application::Voip)?;
        
        // Configure Opus for high quality voice
        encoder.set_bitrate(Bitrate::Bits(OPUS_BITRATE))?;  // Fix bitrate setting
        encoder.set_packet_loss_perc(5)?;  // Expect 5% packet loss
        encoder.set_inband_fec(true)?;     // Enable Forward Error Correction

        let decoder = Decoder::new(SAMPLE_RATE, Channels::Mono)?;

        Ok(Self {
            encoder: Arc::new(Mutex::new(encoder)),
            decoder: Arc::new(Mutex::new(decoder)),
            input_stream: Arc::new(Mutex::new(StreamWrapper(None))),
            output_stream: Arc::new(Mutex::new(StreamWrapper(None))),
            tx,
            output_volume: Arc::new(AtomicF32::new(1.0)),
            is_muted: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            input_peak: Arc::new(AtomicF32::new(0.0)),
            output_peak: Arc::new(AtomicF32::new(0.0)),
            output_producer: None,
            input_volume: Arc::new(AtomicF32::new(1.0)),
        })
    }

    pub async fn setup_output_stream(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("No output device available")?;

        let config = cpal::StreamConfig {
            channels: CHANNELS,
            sample_rate: cpal::SampleRate(SAMPLE_RATE),
            buffer_size: cpal::BufferSize::Fixed(FRAME_SIZE as u32),
        };

        let (producer, mut consumer) = HeapRb::<f32>::new(RING_BUFFER_SIZE).split();
        let producer = Arc::new(Mutex::new(producer));
        self.output_producer = Some(producer.clone());

        let volume = self.output_volume.clone();
        let is_muted = self.is_muted.clone();
        let output_peak = self.output_peak.clone();

        let output_stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut peak = 0.0f32;
                
                for sample in data.iter_mut() {
                    let input = consumer.pop().unwrap_or(0.0);
                    if is_muted.load(std::sync::atomic::Ordering::Relaxed) {
                        *sample = 0.0;
                    } else {
                        let vol = volume.load(std::sync::atomic::Ordering::Relaxed);
                        *sample = input * vol;
                        peak = peak.max(sample.abs());
                    }
                }

                output_peak.store(peak, std::sync::atomic::Ordering::Relaxed);
            },
            |err| eprintln!("Output error: {}", err),
            None,
        )?;

        *self.output_stream.lock().await = StreamWrapper(Some(output_stream));
        Ok(())
    }

    pub async fn start_capture(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Starting audio capture");
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No input device available")?;
            
        println!("Using input device: {}", device.name()?);
        
        let config = cpal::StreamConfig {
            channels: CHANNELS,
            sample_rate: cpal::SampleRate(SAMPLE_RATE),
            buffer_size: cpal::BufferSize::Fixed(FRAME_SIZE as u32),
        };
        
        let tx = self.tx.clone();
        let encoder = self.encoder.clone();
        let input_peak = self.input_peak.clone();
        let is_muted = self.is_muted.clone();

        let input_volume = self.input_volume.clone();
        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &_| {
                if is_muted.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }

                // Apply input volume before encoding
                let vol = input_volume.load(std::sync::atomic::Ordering::Relaxed);
                let mut volume_adjusted_data = Vec::with_capacity(data.len());
                for sample in data {
                    volume_adjusted_data.push(sample * vol);
                }

                // Calculate peak after volume adjustment
                let peak = volume_adjusted_data.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
                input_peak.store(peak, std::sync::atomic::Ordering::Relaxed);

                let mut opus_data = vec![0u8; MAX_PACKET_SIZE];
                let mut enc = encoder.blocking_lock();
                match enc.encode_float(&volume_adjusted_data, &mut opus_data) {
                    Ok(size) => {
                        if let Err(e) = tx.try_send(opus_data[..size].to_vec()) {
                            println!("Failed to send audio data: {}", e);
                        }
                    },
                    Err(e) => println!("Failed to encode audio: {}", e),
                }
            },
            |err| eprintln!("Audio capture error: {}", err),
            None,
        )?;
        
        println!("Built input stream successfully");
        *self.input_stream.lock().await = StreamWrapper(Some(stream));
        println!("Audio capture started");
        Ok(())
    }

    pub fn process_incoming(&self, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        let mut pcm_data = vec![0f32; FRAME_SIZE];
        {
            let mut decoder = self.decoder.blocking_lock();
            decoder.decode_float(data, &mut pcm_data, false)?;
        }
        
        // Apply volume control
        let volume = self.output_volume.load(std::sync::atomic::Ordering::Relaxed);
        for sample in &mut pcm_data {
            *sample *= volume;
        }
        
        if let Some(producer) = &self.output_producer {
            let mut prod = producer.blocking_lock();
            for sample in pcm_data {
                let _ = prod.push(sample);
            }
        }
        Ok(())
    }

    pub async fn cleanup(&mut self) {
        let mut stream = self.input_stream.lock().await;
        *stream = StreamWrapper(None);
        let mut stream = self.output_stream.lock().await;
        *stream = StreamWrapper(None);
        self.output_producer = None;
    }

    pub async fn set_input_device(&mut self, device_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        println!("Setting input device: {}", device_id);
        let host = cpal::default_host();
        
        // Find the requested device or fall back to default
        let device = if device_id == "default" {
            host.default_input_device()
                .ok_or("No default input device available")?
        } else {
            let mut devices = host.input_devices()?;
            devices.find(|d| d.name().map(|n| n == device_id).unwrap_or(false))
                .unwrap_or_else(|| {
                    println!("Device '{}' not found, using default", device_id);
                    host.default_input_device()
                        .expect("No default input device available")
                })
        };
        
        println!("Selected device: {}", device.name()?);
        
        // Stop current stream if any
        self.cleanup().await;
        
        // Create new stream with selected device
        let config = cpal::StreamConfig {
            channels: CHANNELS,
            sample_rate: cpal::SampleRate(SAMPLE_RATE),
            buffer_size: cpal::BufferSize::Fixed(FRAME_SIZE as u32),
        };
        
        let tx = self.tx.clone();
        let encoder = self.encoder.clone();
        let input_peak = self.input_peak.clone();
        let is_muted = self.is_muted.clone();

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &_| {
                if is_muted.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }

                let peak = data.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
                input_peak.store(peak, std::sync::atomic::Ordering::Relaxed);

                let mut opus_data = vec![0u8; MAX_PACKET_SIZE];
                if let Ok(mut enc) = encoder.try_lock() {
                    match enc.encode_float(data, &mut opus_data) {
                        Ok(size) => {
                            if let Err(e) = tx.try_send(opus_data[..size].to_vec()) {
                                println!("Failed to send audio data: {}", e);
                            }
                        },
                        Err(e) => println!("Failed to encode audio: {}", e),
                    }
                }
            },
            |err| eprintln!("Error in input stream: {}", err),
            None,
        )?;
        
        println!("Successfully built input stream");
        *self.input_stream.lock().await = StreamWrapper(Some(stream));
        println!("Set new input stream");
        Ok(())
    }

    pub fn set_output_volume(&self, volume: f32) {
        let vol = volume.clamp(0.0, 1.0);
        self.output_volume.store(vol, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn set_input_volume(&self, volume: f32) {
        let vol = volume.clamp(0.0, 1.0);
        self.input_volume.store(vol, std::sync::atomic::Ordering::Relaxed);
    }
    
    pub fn get_input_volume(&self) -> f32 {
        self.input_volume.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set_muted(&self, muted: bool) {
        self.is_muted.store(muted, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn get_input_level(&self) -> f32 {
        self.input_peak.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn get_output_level(&self) -> f32 {
        self.output_peak.load(std::sync::atomic::Ordering::Relaxed)
    }
}

unsafe impl Send for AudioProcessor {}
unsafe impl Sync for AudioProcessor {}