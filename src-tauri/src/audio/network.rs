// src-tauri/src/audio/network.rs

use tokio::net::UdpSocket;
use tokio::sync::{mpsc, broadcast};
use parking_lot::Mutex;
use bytes::{BytesMut, BufMut};
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::collections::{HashMap, VecDeque};
use super::processor::AudioProcessor;
use crate::config::TurnConfig;
use std::io::Write;
use byteorder::{BigEndian, WriteBytesExt};
use std::time::{Duration, Instant};

// Constants for TURN
const STUN_MAGIC_COOKIE: u32 = 0x2112A442;
const ALLOCATION_REQUEST: u16 = 0x0003;
const XOR_MAPPED_ADDRESS: u16 = 0x0016;
const LIFETIME: u16 = 0x000D;
const REALM_ATTR: u16 = 0x0014;
const NONCE_ATTR: u16 = 0x0015;

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
    peers: Vec<SocketAddr>,
    buffer_size: usize,
    sequence: std::sync::atomic::AtomicU32,
    audio_tx: broadcast::Sender<(Vec<u8>, SocketAddr)>,
    jitter_buffers: HashMap<SocketAddr, JitterBuffer>,
    quality_monitors: HashMap<SocketAddr, QualityMonitor>,
    stats_tx: broadcast::Sender<(SocketAddr, NetworkStats)>,
}

impl AudioNetwork {
    pub async fn new(bind_addr: &str, turn_config: TurnConfig) -> Result<Self, Box<dyn std::error::Error>> {
        // Bind a UDP socket.
        let socket = UdpSocket::bind(bind_addr).await?;
        socket.set_ttl(32)?;

        // Convert the socket to its std version to clone it.
        let std_socket = socket.into_std()?;
        let std_socket_clone = std_socket.try_clone()?;
        let socket = UdpSocket::from_std(std_socket)?;
        let turn_socket = Self::setup_turn_connection(&turn_config, UdpSocket::from_std(std_socket_clone)?).await?;

        let (audio_tx, _) = broadcast::channel(100);
        let (stats_tx, _) = broadcast::channel(100);

        Ok(Self {
            socket: Arc::new(socket),
            turn_socket: Arc::new(turn_socket),
            peers: Vec::new(),
            buffer_size: 480,
            sequence: std::sync::atomic::AtomicU32::new(0),
            audio_tx,
            jitter_buffers: HashMap::new(),
            quality_monitors: HashMap::new(),
            stats_tx,
        })
    }

    async fn setup_turn_connection(
        config: &TurnConfig,
        local_socket: UdpSocket
    ) -> Result<UdpSocket, Box<dyn std::error::Error>> {
        println!("Starting TURN connection setup...");
        // Remove "turn:" prefix if present.
        let url = config.url.trim_start_matches("turn:");
        let server_addr: SocketAddr = url.parse()?;
        println!("Connecting to TURN server at: {}", server_addr);
        local_socket.connect(server_addr).await?;
        println!("Connected to TURN server");

        // Create TURN allocation request.
        let mut request = Vec::new();
        request.write_u16::<BigEndian>(ALLOCATION_REQUEST)?; // Message Type
        request.write_u16::<BigEndian>(0)?; // Message Length (placeholder)
        request.write_u32::<BigEndian>(STUN_MAGIC_COOKIE)?;
        let transaction_id: [u8; 12] = rand::random();
        request.write_all(&transaction_id)?;

        // Add REQUESTED-TRANSPORT attribute (Required for allocation)
        request.write_u16::<BigEndian>(0x0019)?; // REQUESTED-TRANSPORT
        request.write_u16::<BigEndian>(4)?; // Length
        request.write_u8(17)?; // UDP protocol number
        request.write_all(&[0; 3])?; // Reserved padding

        // Add REALM attribute
        let realm = config.realm.as_bytes();
        request.write_u16::<BigEndian>(REALM_ATTR)?;
        request.write_u16::<BigEndian>(realm.len() as u16)?;
        request.write_all(realm)?;
        pad_to_multiple_of_4(&mut request);

        // Add credentials.
        let username = config.username.as_bytes();
        let credential = config.credential.as_bytes();
        println!("Adding credentials - Username length: {}", username.len());
        request.write_u16::<BigEndian>(0x0006)?; // Username type
        request.write_u16::<BigEndian>(username.len() as u16)?;
        request.write_all(username)?;
        pad_to_multiple_of_4(&mut request);

        // Message Integrity attribute (HMAC-SHA1)
        let key = hmac_key(username, credential, &config.realm);
        let hmac = hmac_sha1(&key, &request);
        request.write_u16::<BigEndian>(0x0008)?; // Message-Integrity type
        request.write_u16::<BigEndian>(20)?; // 20 bytes
        request.write_all(&hmac)?;

        // Update message length.
        let message_length = (request.len() - 20) as u16;
        request[2..4].copy_from_slice(&message_length.to_be_bytes());

        println!("Sending TURN allocation request...");
        println!("Request bytes: {:02x?}", request);
        local_socket.send(&request).await?;
        println!("Request sent, waiting for response...");

        let mut response = vec![0u8; 1024];
        let size = local_socket.recv(&mut response).await?;
        println!("Received response of {} bytes", size);
        println!("Response bytes: {:02x?}", &response[..size]);

        if size >= 2 {
            let message_type = u16::from_be_bytes([response[0], response[1]]);
            println!("Response message type: 0x{:04x}", message_type);
        }

        if let Some(relayed_address) = process_turn_response(&response[..size])? {
            println!("TURN allocation successful. Relayed address: {}", relayed_address);
            Ok(local_socket)
        } else {
            println!("Failed to get relayed address from response");
            Err("Failed to get relayed address from TURN server".into())
        }
    }

    pub async fn send_audio(&mut self, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        let sequence = self.sequence.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut packet = Vec::with_capacity(data.len() + 4);
        packet.extend_from_slice(&sequence.to_be_bytes());
        packet.extend_from_slice(data);

        // Send to all peers through TURN server
        let peers = self.peers.clone();
        if peers.is_empty() {
            println!("No peers to send audio to");
            return Ok(());
        }

        for peer in peers {
            println!("Sending {} bytes of audio data to peer {}", packet.len(), peer);
            self.turn_socket.send_to(&packet, peer).await?;
        }
        Ok(())
    }

    pub fn add_peer(&mut self, addr: SocketAddr) {
        if !self.peers.contains(&addr) {
            self.peers.push(addr);
            self.jitter_buffers.insert(addr, JitterBuffer::new(20, 50));
            self.quality_monitors.insert(addr, QualityMonitor::new());
        }
    }

    pub fn remove_peer(&mut self, addr: &SocketAddr) {
        self.peers.retain(|x| x != addr);
        self.jitter_buffers.remove(addr);
    }

    pub async fn start_streaming(&mut self, mut rx: mpsc::Receiver<Vec<u8>>) {
        let socket = self.turn_socket.clone();
        let peers = self.peers.clone();
        tokio::spawn(async move {
            while let Some(audio_data) = rx.recv().await {
                for peer in &peers {
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

    pub async fn handle_incoming(&mut self, processor: Arc<Mutex<AudioProcessor>>) {
        let socket = self.turn_socket.clone();
        let audio_tx = self.audio_tx.clone();
        let mut audio_rx = self.audio_tx.subscribe();
        let jitter_buffers = Arc::new(Mutex::new(self.jitter_buffers.clone()));
        let quality_monitors = Arc::new(Mutex::new(self.quality_monitors.clone()));
        let stats_tx = self.stats_tx.clone();

        // Task to handle incoming packets.
        let jb_clone = jitter_buffers.clone();
        let qm_clone = quality_monitors.clone();
        tokio::spawn(async move {
            let mut buffer = vec![0u8; 2048];
            println!("Started listening for incoming audio packets");
            loop {
                match socket.recv_from(&mut buffer).await {
                    Ok((size, addr)) => {
                        if size < 4 {
                            println!("Received packet too small: {} bytes from {}", size, addr);
                            continue;
                        }

                        let sequence = u32::from_be_bytes([
                            buffer[0], buffer[1], buffer[2], buffer[3]
                        ]);
                        
                        println!("Received {} bytes from {}, sequence: {}", size, addr, sequence);

                        {
                            let mut monitors = qm_clone.lock();
                            if let Some(monitor) = monitors.get_mut(&addr) {
                                monitor.update(sequence, Instant::now());
                                let stats = monitor.get_stats();
                                let _ = stats_tx.send((addr, stats.clone()));
                                println!("Network stats for {}: latency={:?}, packet_loss={:.2}%, jitter={:?}", 
                                    addr, stats.latency, stats.packet_loss * 100.0, stats.jitter);
                            }
                        }

                        let audio_data = &buffer[4..size];
                        let _ = audio_tx.send((audio_data.to_vec(), addr));
                    }
                    Err(e) => {
                        println!("Error receiving audio packet: {}", e);
                    }
                }
            }
        });

        // Task to process audio data.
        tokio::spawn(async move {
            while let Ok((audio_data, _addr)) = audio_rx.recv().await {
                let processor = processor.lock();
                if let Err(e) = processor.process_incoming(&audio_data) {
                    eprintln!("Error processing audio: {}", e);
                }
            }
        });
    }

    pub fn get_local_addr(&self) -> Result<SocketAddr, Box<dyn std::error::Error>> {
        Ok(self.turn_socket.local_addr()?)
    }

    pub fn new_sync() -> Result<Self, Box<dyn std::error::Error>> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            Self::new("0.0.0.0:0", TurnConfig::default()).await
        })
    }

    pub fn subscribe_to_stats(&self) -> broadcast::Receiver<(SocketAddr, NetworkStats)> {
        self.stats_tx.subscribe()
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
        return Ok(None);
    }

    let message_type = u16::from_be_bytes([response[0], response[1]]);
    println!("Processing response with type: 0x{:04x}", message_type);

    // Check if it's an error response
    if message_type & 0x0110 == 0x0110 {
        // Look for ERROR-CODE attribute (0x0009)
        let mut pos = 20;
        while pos + 4 <= response.len() {
            let attr_type = u16::from_be_bytes([response[pos], response[pos + 1]]);
            let attr_len = u16::from_be_bytes([response[pos + 2], response[pos + 3]]) as usize;
            println!("Found attribute type: 0x{:04x}, length: {}", attr_type, attr_len);

            if attr_type == 0x0009 && pos + 8 <= response.len() { // ERROR-CODE
                let error_class = response[pos + 6] as u16;
                let error_number = response[pos + 7] as u16;
                let error_code = error_class * 100 + error_number;
                
                println!("Error bytes: {:02x?}", &response[pos..pos + 8]);
                println!("Error class: {}, number: {}", error_class, error_number);
                println!("Received error response. Error code: {}", error_code);
                match error_code {
                    401 => println!("Unauthorized: Need to include REALM and NONCE attributes"),
                    431 => println!("Integrity Check Failure: Authentication failed"),
                    437 => println!("Allocation Mismatch: Request conflicts with existing allocation"),
                    441 => println!("Wrong Credentials"),
                    486 => println!("Allocation Quota Reached"),
                    508 => println!("Insufficient Port Capacity"),
                    _ => println!("Unknown error code: {}", error_code),
                }
                return Ok(None);
            }
            pos += 4 + attr_len;
            if attr_len % 4 != 0 {
                pos += 4 - (attr_len % 4);
            }
        }
        println!("Error response but no ERROR-CODE attribute found");
        return Ok(None);
    }

    if message_type != 0x0103 { // Not an Allocation Success
        println!("Unexpected message type: 0x{:04x}", message_type);
        return Ok(None);
    }

    let mut pos = 20;
    while pos + 4 <= response.len() {
        let attr_type = u16::from_be_bytes([response[pos], response[pos + 1]]);
        let attr_len = u16::from_be_bytes([response[pos + 2], response[pos + 3]]) as usize;
        println!("Found attribute type: 0x{:04x}, length: {}", attr_type, attr_len);

        if attr_type == XOR_MAPPED_ADDRESS {
            let family = response[pos + 5];
            let port = u16::from_be_bytes([response[pos + 6], response[pos + 7]])
                ^ ((STUN_MAGIC_COOKIE >> 16) as u16);
            if family == 0x01 { // IPv4
                let mut addr = [0u8; 4];
                for i in 0..4 {
                    addr[i] = response[pos + 8 + i] ^ ((STUN_MAGIC_COOKIE >> (24 - i * 8)) as u8);
                }
                return Ok(Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::from(addr)), port)));
            }
        }
        pos += 4 + attr_len;
        if attr_len % 4 != 0 {
            pos += 4 - (attr_len % 4);
        }
    }
    Ok(None)
}
