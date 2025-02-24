// src-tauri/src/audio/processor.rs

use cpal::traits::{DeviceTrait, HostTrait};
use opus::{Encoder, Decoder, Channels, Application, Bitrate};
use tokio::sync::mpsc;
use ringbuf::{HeapRb, Producer};
use std::sync::Arc;
use tokio::sync::Mutex;
use atomic_float::AtomicF32;
use std::thread;

// Audio configuration constants
// Using more common sample rates that most devices support
const PREFERRED_SAMPLE_RATES: [u32; 4] = [48000, 44100, 16000, 8000];
const DEFAULT_SAMPLE_RATE: u32 = 48000;
const CHANNELS: u16 = 1;
// Frame size is calculated based on the actual sample rate used
const FRAME_SIZE_MS: usize = 20;  // 20ms frames for better compatibility
const RING_BUFFER_SIZE_MS: usize = 60;  // 60ms buffer for better compatibility
const MAX_PACKET_SIZE: usize = 1275;  // Maximum Opus packet size
const OPUS_BITRATE: i32 = 32000;  // 32 kbps is enough for good voice quality

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
    sample_rate: Arc<std::sync::atomic::AtomicU32>, // Store actual sample rate used
    frame_size: Arc<std::sync::atomic::AtomicUsize>, // Calculated frame size based on sample rate
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
            sample_rate: self.sample_rate.clone(),
            frame_size: self.frame_size.clone(),
        }
    }
}

impl AudioProcessor {
    pub fn new(tx: mpsc::Sender<Vec<u8>>) -> Result<Self, Box<dyn std::error::Error>> {
        // Start with default values, we'll adjust these when streams are created
        let sample_rate = DEFAULT_SAMPLE_RATE;
        let frame_size = (sample_rate as usize * FRAME_SIZE_MS) / 1000;
        
        // Create and configure Opus encoder
        let mut encoder = Encoder::new(sample_rate, Channels::Mono, Application::Voip)?;
        
        // Configure Opus for high quality voice
        encoder.set_bitrate(Bitrate::Bits(OPUS_BITRATE))?;
        encoder.set_packet_loss_perc(5)?;  // Expect 5% packet loss
        encoder.set_inband_fec(true)?;     // Enable Forward Error Correction
        // No complexity setting in this version

        let decoder = Decoder::new(sample_rate, Channels::Mono)?;

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
            sample_rate: Arc::new(std::sync::atomic::AtomicU32::new(sample_rate)),
            frame_size: Arc::new(std::sync::atomic::AtomicUsize::new(frame_size)),
        })
    }
    
    // Get the most compatible sample rate for the device
    fn get_compatible_sample_rate(&self, device: &cpal::Device) -> Result<u32, Box<dyn std::error::Error>> {
        let supported_configs = device.supported_output_configs()?;
        let supported_configs_vec: Vec<_> = supported_configs.collect();
        
        // Try to find a configuration that supports one of our preferred sample rates
        for rate in PREFERRED_SAMPLE_RATES.iter() {
            for config_range in &supported_configs_vec {
                let min_rate = config_range.min_sample_rate().0;
                let max_rate = config_range.max_sample_rate().0;
                
                if min_rate <= *rate && *rate <= max_rate {
                    println!("Found compatible sample rate: {}", rate);
                    return Ok(*rate);
                }
            }
        }
        
        // If we can't find a direct match, use the default sample rate from the device
        if let Ok(config) = device.default_output_config() {
            println!("Using device's default sample rate: {}", config.sample_rate().0);
            return Ok(config.sample_rate().0);
        }
        
        // Fall back to our default if nothing else works
        println!("Falling back to default sample rate: {}", DEFAULT_SAMPLE_RATE);
        Ok(DEFAULT_SAMPLE_RATE)
    }

    pub async fn setup_output_stream(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("No output device available")?;
            
        // Get a compatible sample rate for this device
        let sample_rate = self.get_compatible_sample_rate(&device)?;
        self.sample_rate.store(sample_rate, std::sync::atomic::Ordering::Relaxed);
        
        // Calculate frame size based on actual sample rate
        let frame_size = (sample_rate as usize * FRAME_SIZE_MS) / 1000;
        self.frame_size.store(frame_size, std::sync::atomic::Ordering::Relaxed);
        
        // Calculate buffer size
        let buffer_size = (sample_rate as usize * RING_BUFFER_SIZE_MS) / 1000;
        
        println!("Using sample rate: {}, frame size: {}, buffer size: {}", 
                 sample_rate, frame_size, buffer_size);

        // Reconfigure the encoder and decoder for the selected sample rate
        {
            let mut encoder = self.encoder.lock().await;
            *encoder = Encoder::new(sample_rate, Channels::Mono, Application::Voip)?;
            encoder.set_bitrate(Bitrate::Bits(OPUS_BITRATE))?;
            encoder.set_packet_loss_perc(5)?;
            encoder.set_inband_fec(true)?;
            // No complexity setting in this version
            
            let mut decoder = self.decoder.lock().await;
            *decoder = Decoder::new(sample_rate, Channels::Mono)?;
        }

        let config = cpal::StreamConfig {
            channels: CHANNELS,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let (producer, mut consumer) = HeapRb::<f32>::new(buffer_size).split();
        let producer = Arc::new(Mutex::new(producer));
        self.output_producer = Some(producer.clone());

        let volume = self.output_volume.clone();
        let is_muted = self.is_muted.clone();
        let output_peak = self.output_peak.clone();

        let output_stream = match device.build_output_stream(
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
        ) {
            Ok(stream) => stream,
            Err(err) => {
                // Log the error but don't fail - user can still use chat without audio
                eprintln!("Error building output stream: {}. Continuing without audio output.", err);
                // Return success instead of failing
                // Use a temporary empty StreamWrapper to avoid await in assignment
                let empty_wrapper = StreamWrapper(None);
                *self.output_stream.blocking_lock() = empty_wrapper;
                return Ok(());
            }
        };

        // Use blocking_lock instead of await to avoid issues with Tauri commands
        *self.output_stream.blocking_lock() = StreamWrapper(Some(output_stream));
        Ok(())
    }

    // This version handles device creation in a thread-safe way for Tauri commands
    pub async fn start_capture(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Starting audio capture");
        let host = cpal::default_host();
        let device = match host.default_input_device() {
            Some(device) => device,
            None => {
                println!("No input device available. Continuing without audio input.");
                // Return success instead of failing - user can still use chat without audio
                return Ok(());
            }
        };
        
        // Log found device
        println!("Using input device: {}", device.name().unwrap_or_else(|_| "unknown".to_string()));
        
        // Get the current sample rate
        let sample_rate = self.sample_rate.load(std::sync::atomic::Ordering::Relaxed);
        
        // Try to create a compatible config
        let mut config = cpal::StreamConfig {
            channels: CHANNELS,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };
        
        // Check if this config is supported
        let supported_configs = match device.supported_input_configs() {
            Ok(configs) => configs,
            Err(e) => {
                println!("Failed to get supported configs: {}. Continuing without audio input.", e);
                return Ok(());
            }
        };
        
        // Try to find compatible config
        let supported_configs_vec: Vec<_> = supported_configs.collect();
        let mut found_compatible = false;
        for cfg_range in supported_configs_vec {
            if cfg_range.channels() == CHANNELS {
                let min_rate = cfg_range.min_sample_rate().0;
                let max_rate = cfg_range.max_sample_rate().0;
                
                if min_rate <= sample_rate && sample_rate <= max_rate {
                    found_compatible = true;
                    break;
                }
            }
        }
        
        // Fall back to device's default if needed
        if !found_compatible {
            match device.default_input_config() {
                Ok(default_config) => {
                    println!("Using device's default config: {:?}", default_config);
                    config = cpal::StreamConfig {
                        channels: default_config.channels(),
                        sample_rate: default_config.sample_rate(),
                        buffer_size: cpal::BufferSize::Default,
                    };
                },
                Err(e) => {
                    println!("Failed to get default config: {}. Continuing without audio input.", e);
                    return Ok(());
                }
            }
        }
        
        println!("Using audio config: {:?}", config);
        
        // Create stream in a thread-safe way
        let tx = self.tx.clone();
        let encoder = self.encoder.clone();
        let input_peak = self.input_peak.clone();
        let is_muted = self.is_muted.clone();
        let input_volume = self.input_volume.clone();
        let input_stream_arc = self.input_stream.clone();
        
        // Move stream creation to a separate thread to avoid Send issues with Tauri commands
        thread::spawn(move || {
            match device.build_input_stream(
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
            ) {
                Ok(stream) => {
                    println!("Built input stream successfully");
                    // Using blocking_lock here since we're in a new thread
                    let mut input_stream = input_stream_arc.blocking_lock();
                    *input_stream = StreamWrapper(Some(stream));
                    println!("Audio capture started");
                },
                Err(err) => {
                    // Log the error but don't fail
                    eprintln!("Error building input stream: {}. Continuing without audio input.", err);
                    let mut input_stream = input_stream_arc.blocking_lock();
                    *input_stream = StreamWrapper(None);
                }
            }
        });
        
        println!("Audio setup initiated");
        Ok(())
    }

    pub fn process_incoming(&self, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        let frame_size = self.frame_size.load(std::sync::atomic::Ordering::Relaxed);
        let mut pcm_data = vec![0f32; frame_size];
        
        // Try to decode the audio packet
        let decode_result = {
            let mut decoder = self.decoder.blocking_lock();
            decoder.decode_float(data, &mut pcm_data, false)
        };
        
        match decode_result {
            Ok(_) => {
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
            },
            Err(e) => {
                println!("Failed to decode audio: {}", e);
                // Don't propagate the error, just log it and continue
                Ok(())
            }
        }
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
            match host.default_input_device() {
                Some(device) => device,
                None => return Err("No default input device available".into()),
            }
        } else {
            let devices = host.input_devices()?;
            let devices_vec: Vec<_> = devices.collect();
            match devices_vec.into_iter().find(|d| d.name().map(|n| n == device_id).unwrap_or(false)) {
                Some(device) => device,
                None => {
                    println!("Device '{}' not found, using default", device_id);
                    match host.default_input_device() {
                        Some(device) => device,
                        None => return Err("No default input device available".into()),
                    }
                }
            }
        };
        
        println!("Selected device: {}", device.name().unwrap_or_else(|_| "unknown".to_string()));
        
        // Stop current stream if any
        self.cleanup().await;
        
        // Get the current sample rate
        let sample_rate = self.sample_rate.load(std::sync::atomic::Ordering::Relaxed);
        
        // Create the stream in a thread-safe way (similar to start_capture)
        let tx = self.tx.clone();
        let encoder = self.encoder.clone();
        let input_peak = self.input_peak.clone();
        let is_muted = self.is_muted.clone();
        let input_volume = self.input_volume.clone();
        let input_stream_arc = self.input_stream.clone();
        
        // Try to create a compatible config
        let config = cpal::StreamConfig {
            channels: CHANNELS,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };
        
        // Use a separate thread to create the stream to avoid Send issues
        thread::spawn(move || {
            match device.build_input_stream(
                &config,
                move |data: &[f32], _: &_| {
                    if is_muted.load(std::sync::atomic::Ordering::Relaxed) {
                        return;
                    }

                    let peak = data.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
                    input_peak.store(peak, std::sync::atomic::Ordering::Relaxed);
                    
                    // Apply input volume
                    let vol = input_volume.load(std::sync::atomic::Ordering::Relaxed);
                    let mut volume_adjusted_data = Vec::with_capacity(data.len());
                    for sample in data {
                        volume_adjusted_data.push(sample * vol);
                    }

                    let mut opus_data = vec![0u8; MAX_PACKET_SIZE];
                    if let Ok(mut enc) = encoder.try_lock() {
                        match enc.encode_float(&volume_adjusted_data, &mut opus_data) {
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
            ) {
                Ok(stream) => {
                    println!("Successfully built input stream");
                    let mut input_stream = input_stream_arc.blocking_lock();
                    *input_stream = StreamWrapper(Some(stream));
                    println!("Set new input stream");
                },
                Err(e) => {
                    println!("Failed to build input stream: {}. Continuing without audio input.", e);
                    let mut input_stream = input_stream_arc.blocking_lock();
                    *input_stream = StreamWrapper(None);
                }
            }
        });
        
        println!("Input device change initiated");
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

// Make AudioProcessor thread-safe for Tauri commands
unsafe impl Send for AudioProcessor {}
unsafe impl Sync for AudioProcessor {}