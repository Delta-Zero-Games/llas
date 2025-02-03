// src-tauri/src/audio/network.rs

use tokio::net::UdpSocket;
use tokio::sync::{broadcast, mpsc, Mutex};
use std::collections::HashMap;
use bytes::{BytesMut, BufMut};
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use std::io::Write;
use byteorder::{BigEndian, WriteBytesExt};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use serde_json;

// Constants for TURN
const STUN_MAGIC_COOKIE: u32 = 0x2112A442;
const ALLOCATION_REQUEST: u16 = 0x0003;
const REQUESTED_TRANSPORT: u16 = 0x0019;
const REALM: u16 = 0x0014;
const RELAYED_ADDRESS: u16 = 0x0016;

#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub latency: Duration,
    pub packet_loss: f32,
    pub jitter: Duration,
    pub connection_quality: ConnectionQuality,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionQuality {
    Excellent,  // < 50ms latency, < 1% packet loss
    Good,       // < 100ms latency, < 2% packet loss
    Fair,       // < 150ms latency, < 5% packet loss
    Poor,       // < 200ms latency, < 10% packet loss
    Critical,   // >= 200ms latency or >= 10% packet loss
}

#[derive(Clone)]
struct QualityMonitor {
    last_sequence: u32,
    packets_received: u32,
    packets_lost: u32,
    last_packet_time: Instant,
    latency_samples: VecDeque<Duration>,
    jitter_samples: VecDeque<Duration>,
}

impl QualityMonitor {
    fn new() -> Self {
        Self {
            last_sequence: 0,
            packets_received: 0,
            packets_lost: 0,
            last_packet_time: Instant::now(),
            latency_samples: VecDeque::with_capacity(100),
            jitter_samples: VecDeque::with_capacity(100),
        }
    }

    fn update(&mut self, sequence: u32, received_time: Instant) {
        if self.last_sequence != 0 {
            let expected = sequence - self.last_sequence;
            if expected > 1 {
                self.packets_lost += expected - 1;
            }
        }
        self.last_sequence = sequence;
        self.packets_received += 1;
        let latency = received_time - self.last_packet_time;
        self.latency_samples.push_back(latency);
        if self.latency_samples.len() > 100 {
            self.latency_samples.pop_front();
        }
        if let Some(last_latency) = self.latency_samples.get(self.latency_samples.len().saturating_sub(2)) {
            let jitter = if latency > *last_latency {
                latency - *last_latency
            } else {
                *last_latency - latency
            };
            self.jitter_samples.push_back(jitter);
            if self.jitter_samples.len() > 100 {
                self.jitter_samples.pop_front();
            }
        }
        self.last_packet_time = received_time;
    }

    fn get_stats(&self) -> NetworkStats {
        let avg_latency = self.calculate_average_latency();
        let packet_loss = self.calculate_packet_loss();
        let avg_jitter = self.calculate_average_jitter();

        let quality = if avg_latency.as_millis() < 50 && packet_loss < 0.01 {
            ConnectionQuality::Excellent
        } else if avg_latency.as_millis() < 100 && packet_loss < 0.02 {
            ConnectionQuality::Good
        } else if avg_latency.as_millis() < 150 && packet_loss < 0.05 {
            ConnectionQuality::Fair
        } else if avg_latency.as_millis() < 200 && packet_loss < 0.10 {
            ConnectionQuality::Poor
        } else {
            ConnectionQuality::Critical
        };

        NetworkStats {
            latency: avg_latency,
            packet_loss,
            jitter: avg_jitter,
            connection_quality: quality,
        }
    }

    fn calculate_average_latency(&self) -> Duration {
        if self.latency_samples.is_empty() {
            Duration::from_millis(0)
        } else {
            let sum: Duration = self.latency_samples.iter().sum();
            sum / self.latency_samples.len() as u32
        }
    }

    fn calculate_packet_loss(&self) -> f32 {
        if self.packets_received == 0 {
            0.0
        } else {
            self.packets_lost as f32 / (self.packets_received + self.packets_lost) as f32
        }
    }

    fn calculate_average_jitter(&self) -> Duration {
        if self.jitter_samples.is_empty() {
            Duration::from_millis(0)
        } else {
            let sum: Duration = self.jitter_samples.iter().sum();
            sum / self.jitter_samples.len() as u32
        }
    }
}

#[derive(Clone)]
pub struct JitterBuffer {
    buffer: VecDeque<(u32, Vec<u8>)>,
    min_delay: u32,
    max_delay: u32,
    current_delay: u32,
    last_sequence: u32,
}

impl JitterBuffer {
    fn new(min_delay: u32, max_delay: u32) -> Self {
        Self {
            buffer: VecDeque::new(),
            min_delay,
            max_delay,
            current_delay: min_delay,
            last_sequence: 0,
        }
    }

    fn add_packet(&mut self, sequence: u32, data: Vec<u8>) {
        let pos = self.buffer.iter()
            .position(|(seq, _)| *seq > sequence)
            .unwrap_or(self.buffer.len());
        self.buffer.insert(pos, (sequence, data));
        self.adapt_delay(sequence);
    }

    fn get_next_packet(&mut self) -> Option<Vec<u8>> {
        if self.buffer.len() as u32 * 10 < self.current_delay {
            return None;
        }
        let (seq, data) = self.buffer.pop_front()?;
        self.last_sequence = seq;
        Some(data)
    }

    fn adapt_delay(&mut self, sequence: u32) {
        if sequence > self.last_sequence {
            let jitter = sequence - self.last_sequence - 1;
            if jitter > 0 {
                self.current_delay = (self.current_delay + jitter).min(self.max_delay);
            } else {
                self.current_delay = (self.current_delay - 1).max(self.min_delay);
            }
        }
    }
}

pub struct AudioNetwork {
    socket: Arc<UdpSocket>,
    turn_socket: Arc<UdpSocket>,
    peers: Arc<Mutex<Vec<SocketAddr>>>,
    buffer_size: usize,
    sequence: u32,
    audio_tx: broadcast::Sender<(Vec<u8>, SocketAddr)>,
    jitter_buffers: HashMap<SocketAddr, JitterBuffer>,
    quality_monitors: Arc<Mutex<HashMap<SocketAddr, QualityMonitor>>>,
    stats_tx: Option<broadcast::Sender<(SocketAddr, NetworkStats)>>,
}

impl AudioNetwork {
    pub async fn new(app_handle: AppHandle, bind_addr: &str, turn_config: TurnConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let local_socket = UdpSocket::bind(bind_addr).await?;
        let turn_socket = Self::setup_turn_connection(&turn_config, local_socket).await?;

        // Emit connection status event
        app_handle.emit("connection_status", 
            serde_json::json!({
                "isConnected": true,
                "stats": {
                    "latency": 0,
                    "packetLoss": 0,
                    "jitter": 0,
                    "bufferSize": 0,
                    "connectionQuality": "Good"
                }
            })
        )?;

        let (audio_tx, _) = broadcast::channel(100);
        let (stats_tx, _) = broadcast::channel(100);

        Ok(Self {
            socket: Arc::new(local_socket),
            turn_socket: Arc::new(turn_socket),
            peers: Arc::new(Mutex::new(Vec::new())),
            buffer_size: 480,
            sequence: 0,
            audio_tx,
            jitter_buffers: HashMap::new(),
            quality_monitors: Arc::new(Mutex::new(HashMap::new())),
            stats_tx: Some(stats_tx),
        })
    }

    pub async fn add_peer(&self, addr: SocketAddr) {
        let mut peers = self.peers.lock().await;
        if !peers.contains(&addr) {
            peers.push(addr);
            self.jitter_buffers.insert(addr, JitterBuffer::new(20, 50));
            let mut monitors = self.quality_monitors.lock().await;
            monitors.insert(addr, QualityMonitor::new());
        }
    }

    pub async fn remove_peer(&self, addr: &SocketAddr) {
        let mut peers = self.peers.lock().await;
        peers.retain(|x| x != addr);
        self.jitter_buffers.remove(addr);
    }

    pub async fn start_streaming(&mut self, app_handle: AppHandle, mut rx: mpsc::Receiver<Vec<u8>>) {
        let socket = self.turn_socket.clone();
        let peers = self.peers.clone();

        // Emit connection status event
        app_handle.emit("connection_status",
            serde_json::json!({
                "isConnected": true,
                "stats": {
                    "latency": 0,
                    "packetLoss": 0,
                    "jitter": 0,
                    "bufferSize": 0,
                    "connectionQuality": "Good"
                }
            })
        ).unwrap();

        tokio::spawn(async move {
            while let Some(audio_data) = rx.recv().await {
                let peers_guard = peers.lock().await;
                for peer in peers_guard.iter() {
                    let mut packet = BytesMut::with_capacity(audio_data.len() + 12);
                    packet.put_u32(0);
                    packet.put_u64(std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64);
                    packet.put_slice(&audio_data);
                    if let Err(e) = socket.send_to(&packet, peer).await {
                        eprintln!("Error sending audio to peer {}: {}", peer, e);
                    }
                }
            }
        });
    }

    pub async fn handle_incoming(&mut self, processor: Arc<Mutex<AudioProcessor>>, app_handle: AppHandle) {
        let socket = self.turn_socket.clone();
        let audio_tx = self.audio_tx.clone();
        let mut audio_rx = self.audio_tx.subscribe();
        let jitter_buffers = Arc::new(Mutex::new(self.jitter_buffers.clone()));
        let quality_monitors = self.quality_monitors.clone();
        let stats_tx = self.stats_tx.clone().unwrap();

        // Task to handle incoming packets.
        let jb_clone = jitter_buffers.clone();
        tokio::spawn(async move {
            let mut buffer = vec![0u8; 2048];
            loop {
                match socket.recv_from(&mut buffer).await {
                    Ok((size, addr)) => {
                        if size < 12 {
                            continue;
                        }
                        let sequence = u32::from_be_bytes([
                            buffer[0], buffer[1], buffer[2], buffer[3]
                        ]);
                        let timestamp = u64::from_be_bytes([
                            buffer[4], buffer[5], buffer[6], buffer[7],
                            buffer[8], buffer[9], buffer[10], buffer[11]
                        ]);
                        let audio_data = buffer[12..size].to_vec();
                        
                        let mut buffers = jb_clone.lock().await;
                        if let Some(jitter_buffer) = buffers.get_mut(&addr) {
                            jitter_buffer.add_packet(sequence, audio_data.clone());
                            while let Some(data) = jitter_buffer.get_next_packet() {
                                let _ = audio_tx.send((data, addr));
                            }
                        }

                        let mut monitors = quality_monitors.lock().await;
                        if let Some(monitor) = monitors.get_mut(&addr) {
                            monitor.update(timestamp);
                            let stats = monitor.get_stats();
                            let _ = stats_tx.send((addr, stats));
                        }

                        let processor_guard = processor.lock().await;
                        if let Err(e) = processor_guard.process_incoming(&audio_data) {
                            eprintln!("Error processing audio: {}", e);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error receiving audio: {}", e);
                    }
                }
            }
        });
    }

    pub async fn local_addr(&self) -> Result<SocketAddr, Box<dyn std::error::Error>> {
        Ok(self.turn_socket.local_addr()?)
    }

    pub fn subscribe_to_stats(&self) -> broadcast::Receiver<(SocketAddr, NetworkStats)> {
        self.stats_tx.clone().unwrap().subscribe()
    }
}

// Helper functions for TURN authentication.
fn pad_to_multiple_of_4(data: &mut Vec<u8>) {
    while data.len() % 4 != 0 {
        data.push(0);
    }
}

fn hmac_key(username: &[u8], credential: &[u8], realm: &str) -> Vec<u8> {
    let mut key = Vec::new();
    key.extend_from_slice(username);
    key.push(b':');
    key.extend_from_slice(realm.as_bytes());
    key.push(b':');
    key.extend_from_slice(credential);
    key
}

fn hmac_sha1(key: &[u8], message: &[u8]) -> [u8; 20] {
    use hmac::{Hmac, Mac};
    use sha1::Sha1;
    let mut mac = Hmac::<Sha1>::new_from_slice(key).unwrap();
    mac.update(message);
    mac.finalize().into_bytes().into()
}

fn process_turn_response(response: &[u8]) -> Result<Option<SocketAddr>, Box<dyn std::error::Error>> {
    if response.len() < 20 {
        println!("Response too short: {} bytes", response.len());
        return Ok(None);
    }

    let message_type = u16::from_be_bytes([response[0], response[1]]);
    println!("Response message type: 0x{:04x}", message_type);

    if message_type != 0x0103 { // Allocation Success
        // Check for error response and handle 401 Unauthorized
        if message_type == 0x0111 {
            println!("Received error response (401 Unauthorized) from TURN server");
            // Print error code if available
            let mut pos = 20;
            while pos + 4 <= response.len() {
                let attr_type = u16::from_be_bytes([response[pos], response[pos + 1]]);
                let attr_len = u16::from_be_bytes([response[pos + 2], response[pos + 3]]) as usize;
                if attr_type == 0x0009 { // ERROR-CODE
                    let error_code = response[pos + 7];
                    println!("Error code: {}", error_code);
                }
                pos += 4 + attr_len;
                if attr_len % 4 != 0 {
                    pos += 4 - (attr_len % 4);
                }
            }
            return Ok(None);
        }
        println!("Unexpected message type: 0x{:04x}", message_type);
        return Ok(None);
    }

    let mut pos = 20;
    println!("Scanning response attributes");
    while pos + 4 <= response.len() {
        let attr_type = u16::from_be_bytes([response[pos], response[pos + 1]]);
        let attr_len = u16::from_be_bytes([response[pos + 2], response[pos + 3]]) as usize;
        println!("Found attribute type: 0x{:04x}, length: {}", attr_type, attr_len);

        if attr_type == RELAYED_ADDRESS {
            println!("Found RELAYED-ADDRESS attribute");
            let family = response[pos + 5];
            let port = u16::from_be_bytes([response[pos + 6], response[pos + 7]])
                ^ ((STUN_MAGIC_COOKIE >> 16) as u16);

            if family == 0x01 { // IPv4
                let mut addr = [0u8; 4];
                for i in 0..4 {
                    addr[i] = response[pos + 8 + i] ^ ((STUN_MAGIC_COOKIE >> (24 - i * 8)) as u8);
                }
                let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::from(addr)), port);
                println!("Successfully parsed relayed address: {}", addr);
                return Ok(Some(addr));
            } else {
                println!("Unsupported address family: {}", family);
            }
        }

        pos += 4 + attr_len;
        if attr_len % 4 != 0 {
            pos += 4 - (attr_len % 4);
        }
    }

    println!("No RELAYED-ADDRESS attribute found");
    Ok(None)
}
