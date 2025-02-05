// src-tauri/src/main.rs
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod room;
mod state;
mod config;
mod audio;

use tauri::{State, Manager, Emitter};
use std::sync::Arc;
use tokio::sync::Mutex; 
use uuid::Uuid;
use crate::room::{RoomManager, Room, User};
use crate::audio::{AudioProcessor, AudioNetwork};
use crate::config::TurnConfig;
use crate::state::StateManager;
use tokio::sync::mpsc;
use parking_lot::Mutex as PLMutex;
use serde::Serialize;

type SafeAudioProcessor = Arc<Mutex<Option<AudioProcessor>>>;
type SafeAudioNetwork = Arc<Mutex<Option<AudioNetwork>>>;

#[derive(Clone, Serialize)]
pub struct RoomEventPayload {
    room_id: String,
    action: String,
    participants: Vec<User>,
}

#[derive(Clone, Serialize)]
pub struct AudioEventPayload {
    is_connected: bool,
    input_level: f32,
    output_level: f32,
}

#[derive(Clone, Serialize)]
pub struct ErrorEventPayload {
    code: String,
    message: String,
}
pub struct AppState {
    room_manager: Arc<Mutex<RoomManager>>,
    audio_processor: SafeAudioProcessor,
    network: SafeAudioNetwork,
    state_manager: Arc<Mutex<StateManager>>,
    app_handle: tauri::AppHandle,
}

impl AppState {
    pub async fn new(app_handle: tauri::AppHandle) -> Self {  // Add app_handle parameter
        let state_manager = StateManager::new()
            .await
            .expect("Failed to initialize StateManager");
        Self {
            room_manager: Arc::new(Mutex::new(RoomManager::new())),
            audio_processor: Arc::new(Mutex::new(None)),
            network: Arc::new(Mutex::new(None)),
            state_manager: Arc::new(Mutex::new(state_manager)),
            app_handle,  // Now app_handle is available to use
        }
    }
    
    // Rest of the implementation remains the same
    pub fn emit_event<T: Clone + Serialize>(&self, event: &str, payload: T) {
        if let Err(e) = self.app_handle.emit(event, payload) {  // Changed from emit_all to emit
            eprintln!("Failed to emit event {}: {}", event, e);
        }
    }

    pub fn emit_error(&self, code: &str, message: &str) {
        self.emit_event("error", ErrorEventPayload {
            code: code.to_string(),
            message: message.to_string(),
        });
    }
}

#[tauri::command]
async fn add_user(state: State<'_, AppState>, name: String) -> Result<User, String> {
    let mut manager = state.room_manager.lock().await;
    let user = manager.add_user(name);
    println!("User created in backend: {:?}", user);
    Ok(user)
}

#[tauri::command]
async fn create_room(
    state: State<'_, AppState>,
    name: String,
    user_id: String,
) -> Result<Room, String> {
    let mut manager = state.room_manager.lock().await;
    let user_id = Uuid::parse_str(&user_id).map_err(|e| e.to_string())?;
    let room = manager.create_room(name, user_id);

    // Persist the new room to Redis
    {
        let mut state_mgr = state.state_manager.lock().await;
        state_mgr.save_room(&room).await.map_err(|e| e.to_string())?;
    }
    
    Ok(room)
}


async fn init_network(network: &SafeAudioNetwork) -> Result<(), String> {
    let turn_config = TurnConfig::default();
    println!("Initializing with TURN config:");
    println!("URL: {}", turn_config.url);
    println!("Username: {}", turn_config.username);
    println!("Realm: {}", turn_config.realm);
    let mut network_lock = network.lock().await;
    if network_lock.is_none() {
        let new_network = AudioNetwork::new("0.0.0.0:0", turn_config)
            .await
            .map_err(|e| e.to_string())?;
        *network_lock = Some(new_network);
    }
    Ok(())
}

#[tauri::command]
async fn join_room(
    state: State<'_, AppState>,
    room_id: String,
    user_id: String,
) -> Result<Room, String> {
    println!("Join room request - room_id: {}, user_id: {}", room_id, user_id);
    
    let room_id = Uuid::parse_str(&room_id).map_err(|e| {
        let error = format!("Invalid room UUID: {}", e);
        println!("{}", error);
        state.emit_error("INVALID_UUID", &error);
        error
    })?;
    
    let user_id = Uuid::parse_str(&user_id).map_err(|e| {
        let error = format!("Invalid user UUID: {}", e);
        println!("{}", error);
        state.emit_error("INVALID_UUID", &error);
        error
    })?;

    // Debug: Check if user exists
    {
        let manager = state.room_manager.lock().await;
        if let Some(user) = manager.get_user(&user_id) {
            println!("Found user in backend: {:?}", user);
        } else {
            println!("User {} not found in backend", user_id);
        }
    }
    
    // Initialize network
    if let Err(e) = init_network(&state.network).await {
        state.emit_error("NETWORK_ERROR", &e);
        return Err(e);
    }
    
    let peer_addr = {
        let network = state.network.lock().await;
        match network.as_ref().ok_or_else(|| "Network not initialized".to_string())?.get_local_addr() {
            Ok(addr) => addr,
            Err(e) => {
                state.emit_error("NETWORK_ERROR", &e.to_string());
                return Err(e.to_string());
            }
        }
    };

    let room = {
        let mut manager = state.room_manager.lock().await;
        
        // Add peer address
        if let Err(e) = manager.add_peer_address(user_id, peer_addr) {
            state.emit_error("PEER_ERROR", &e);
            return Err(e);
        }
        
        // Join room
        let room = match manager.join_room(room_id, user_id) {
            Ok(r) => r,
            Err(e) => {
                println!("Failed to join room: {}", e);
                return Err(e);
            }
        };
        
        // Update network peers
        {
            let mut network = state.network.lock().await;
            if let Some(net) = network.as_mut() {
                for participant in &room.participants {
                    if let Some(participant_addr) = participant.peer_addr {
                        net.add_peer(participant_addr);
                    }
                }
            }
        }

        // Emit room update event
        state.emit_event("room:update", RoomEventPayload {
            room_id: room.id.to_string(),
            action: "join".to_string(),
            participants: room.participants.clone(),
        });

        room
    };

    // Update Redis
    {
        let mut state_mgr = state.state_manager.lock().await;
        if let Err(e) = state_mgr.save_room(&room).await {
            state.emit_error("REDIS_ERROR", &e.to_string());
            return Err(e.to_string());
        }
    }
    
    Ok(room)
}

#[tauri::command]
async fn leave_room(
    state: State<'_, AppState>,
    room_id: String,
    user_id: String,
) -> Result<(), String> {
    let room_id = Uuid::parse_str(&room_id).map_err(|e| e.to_string())?;
    let user_id = Uuid::parse_str(&user_id).map_err(|e| e.to_string())?;

    // Remove the user from the room in memory
    {
        let mut manager = state.room_manager.lock().await;
        manager.leave_room(room_id, user_id)?;
        
        // Use the public getter method to check if the room still exists.
        if let Some(room) = manager.get_room(&room_id) {
            // Update the room’s participant list in Redis.
            // (Cloning the room might be necessary if you run into lifetime issues.)
            let room_clone = room.clone();
            let mut state_mgr = state.state_manager.lock().await;
            state_mgr.save_room(&room_clone).await.map_err(|e| e.to_string())?;
        } else {
            // If the room was removed, delete it from Redis.
            let mut state_mgr = state.state_manager.lock().await;
            state_mgr.delete_room(&room_id).await.map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
async fn list_rooms(state: State<'_, AppState>) -> Result<Vec<Room>, String> {
    let mut state_mgr = state.state_manager.lock().await;
    state_mgr.list_rooms().await.map_err(|e| e.to_string())
}


async fn setup_processor(processor: &SafeAudioProcessor, tx: mpsc::Sender<Vec<u8>>) -> Result<(), String> {
    let mut processor_lock = processor.lock().await;
    if processor_lock.is_none() {
        *processor_lock = Some(AudioProcessor::new(tx).map_err(|e| e.to_string())?);
    }

    // Get a reference to the processor
    let processor_ref = processor_lock.as_mut().ok_or_else(|| "Processor not initialized".to_string())?;
    
    // Setup streams
    processor_ref.setup_output_stream().await.map_err(|e| e.to_string())?;
    processor_ref.start_capture().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_streaming(
    state: State<'_, AppState>,
    room_id: String
) -> Result<(), String> {
    println!("Starting streaming for room: {}", room_id);
    let (tx, rx) = mpsc::channel(32);

    // Initialize audio processor if not already initialized
    {
        let mut processor = state.audio_processor.lock().await;
        if processor.is_none() {
            println!("Initializing audio processor");
            let (audio_tx, _) = mpsc::channel(32); // Create a separate channel for the audio processor
            *processor = Some(AudioProcessor::new(audio_tx).map_err(|e| e.to_string())?);
            println!("Audio processor initialized successfully");
        }
    }
    
    println!("Setting up processor with channel");
    // Setup processor with the channel
    setup_processor(&state.audio_processor, tx).await?;
    println!("Processor setup complete");
    
    let room_id = Uuid::parse_str(&room_id).map_err(|e| e.to_string())?;
    let peers = {
        let manager = state.room_manager.lock().await;
        let peers = manager.get_room_peers(&room_id);
        println!("Found {} peers in room", peers.len());
        peers
    };

    // Initialize network if not already initialized
    println!("Initializing network");
    init_network(&state.network).await?;
    println!("Network initialized");

    let mut network = state.network.lock().await;
    if let Some(net) = network.as_mut() {
        for peer_addr in peers {
            println!("Adding peer: {}", peer_addr);
            net.add_peer(peer_addr);
        }
        println!("Starting audio streaming");
        net.start_streaming(rx).await;
        println!("Audio streaming started");
        
        // Get the processor reference
        let processor = {
            let guard = state.audio_processor.lock().await;
            guard.as_ref().ok_or_else(|| "Processor not initialized".to_string())?.clone()
        };
        
        // Create a new Arc<Mutex<AudioProcessor>> for the network
        let network_processor = Arc::new(PLMutex::new(processor));
        println!("Starting to handle incoming audio");
        net.handle_incoming(network_processor).await;
        println!("Handling incoming audio started");
    }
    
    println!("Streaming setup complete");
    Ok(())
}

#[tauri::command]
async fn stop_streaming(state: State<'_, AppState>) -> Result<(), String> {
    let mut network = state.network.lock().await;
    let mut processor = state.audio_processor.lock().await;
    *network = None;
    *processor = None;
    Ok(())
}

#[tauri::command]
async fn set_input_device(
    state: State<'_, AppState>,
    device_id: String
) -> Result<(), String> {
    let mut processor_lock = state.audio_processor.lock().await;
    if let Some(proc) = processor_lock.as_mut() {
        proc.set_input_device(&device_id).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn set_input_volume(
    state: State<'_, AppState>,
    volume: f32
) -> Result<(), String> {
    let mut processor_lock = state.audio_processor.lock().await;
    if let Some(proc) = processor_lock.as_mut() {
        proc.set_input_volume(volume).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn set_muted(
    state: State<'_, AppState>,
    muted: bool
) -> Result<(), String> {
    let mut processor_lock = state.audio_processor.lock().await;
    if let Some(proc) = processor_lock.as_mut() {
        proc.set_muted(muted);
    }
    Ok(())
}

#[tauri::command]
async fn set_user_volume(
    state: State<'_, AppState>,
    _user_id: String, // unused for now
    volume: f32
) -> Result<(), String> {
    let mut processor_lock = state.audio_processor.lock().await;
    if let Some(proc) = processor_lock.as_mut() {
        proc.set_output_volume(volume);
    }
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle();
            let app_state = tauri::async_runtime::block_on(AppState::new(app_handle.clone())); // Added .clone()
            app.manage(app_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            add_user,
            create_room,
            join_room,
            leave_room,
            list_rooms,
            start_streaming,
            stop_streaming,
            set_user_volume,
            set_input_device,
            set_input_volume,
            set_muted
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
