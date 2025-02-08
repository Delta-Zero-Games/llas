// src-tauri/src/main.rs
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod room;
mod state;
mod config;
mod audio;

use tauri::{State, Manager, Emitter, AppHandle};
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
use serde_json::json;

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
    println!("Creating room '{}' for user '{}'", name, user_id);
    
    let user_id = Uuid::parse_str(&user_id).map_err(|e| {
        let error = format!("Invalid user UUID: {}", e);
        state.emit_error("INVALID_UUID", &error);
        error
    })?;

    // First create the room in memory
    let room = {
        let mut manager = state.room_manager.lock().await;
        manager.create_room(name, user_id)
    };

    // Then persist it to Redis
    {
        let mut state_mgr = state.state_manager.lock().await;
        state_mgr.save_room(&room).await.map_err(|e| {
            let error = format!("Failed to save room to Redis: {}", e);
            println!("{}", error);
            state.emit_error("REDIS_ERROR", &error);
            error
        })?;
        println!("Room saved to Redis: {}", room.id);
    }

    // Emit room creation event
    state.emit_event("room:update", RoomEventPayload {
        room_id: room.id.to_string(),
        action: "create".to_string(),
        participants: room.participants.clone(),
    });
    
    println!("Room created successfully: {:?}", room);
    Ok(room)
}

#[tauri::command]
async fn cleanup_rooms(state: State<'_, AppState>) -> Result<(), String> {
    let mut state_mgr = state.state_manager.lock().await;
    state_mgr.cleanup_stale_rooms().await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn init_network(network: &SafeAudioNetwork, app_handle: AppHandle) -> Result<(), String> {
    let turn_config = TurnConfig::default();
    println!("Initializing with TURN config:");
    println!("URL: {}", turn_config.url);
    println!("Username: {}", turn_config.username);
    println!("Realm: {}", turn_config.realm);
    let mut network_lock = network.lock().await;
    if network_lock.is_none() {
        let new_network = AudioNetwork::new("0.0.0.0:0", turn_config, app_handle)
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

    // First check if user exists in memory
    {
        let manager = state.room_manager.lock().await;
        if manager.get_user(&user_id).is_none() {
            let error = format!("User {} not found", user_id);
            println!("{}", error);
            state.emit_error("USER_NOT_FOUND", &error);
            return Err(error);
        }
    }

    // Check Redis for room
    let redis_room = {
        let mut state_mgr = state.state_manager.lock().await;
        state_mgr.get_room(&room_id).await.map_err(|e| {
            let error = format!("Failed to check Redis for room: {}", e);
            println!("{}", error);
            error
        })?
    };

    // Validate room exists
    let redis_room = redis_room.ok_or_else(|| {
        let error = format!("Room {} not found", room_id);
        println!("{}", error);
        state.emit_error("ROOM_NOT_FOUND", &error);
        error
    })?;

    // Sync room to RoomManager if necessary
    {
        let mut manager = state.room_manager.lock().await;
        if manager.get_room(&room_id).is_none() {
            println!("Room found in Redis but not in memory, syncing...");
            manager.sync_room(redis_room);
        }
    }

    // Initialize network
    if let Err(e) = init_network(&state.network, state.app_handle.clone()).await {
        state.emit_error("NETWORK_ERROR", &e);
        return Err(e);
    }
    
    let peer_addr = {
        let network = state.network.lock().await;
        network.as_ref()
            .ok_or_else(|| "Network not initialized".to_string())?
            .get_local_addr()
            .map_err(|e| e.to_string())?
    };

    // Join room and update both memory and Redis
    let updated_room = {
        let mut manager = state.room_manager.lock().await;
        manager.add_peer_address(user_id, peer_addr)?;
        manager.join_room(room_id, user_id)?
    };

    // Update Redis with new room state
    {
        let mut state_mgr = state.state_manager.lock().await;
        state_mgr.save_room(&updated_room).await.map_err(|e| e.to_string())?;
    }

    // Emit room update event
    state.emit_event("room:update", RoomEventPayload {
        room_id: updated_room.id.to_string(),
        action: "join".to_string(),
        participants: updated_room.participants.clone(),
    });

    println!("Successfully joined room: {}", room_id);
    Ok(updated_room)
}

#[tauri::command]
async fn leave_room(
    state: State<'_, AppState>,
    room_id: String,
    user_id: String,
) -> Result<(), String> {
    let room_id = Uuid::parse_str(&room_id).map_err(|e| e.to_string())?;
    let user_id = Uuid::parse_str(&user_id).map_err(|e| e.to_string())?;

    // Stop streaming first
    stop_streaming(state.clone()).await?;

    // Remove the user from the room in memory
    {
        let mut manager = state.room_manager.lock().await;
        manager.leave_room(room_id, user_id)?;
        
        // Use the public getter method to check if the room still exists.
        if let Some(room) = manager.get_room(&room_id) {
            let room_clone = room.clone();
            let mut state_mgr = state.state_manager.lock().await;
            state_mgr.save_room(&room_clone).await.map_err(|e| e.to_string())?;
        } else {
            let mut state_mgr = state.state_manager.lock().await;
            state_mgr.delete_room(&room_id).await.map_err(|e| e.to_string())?;
        }
    }

    // Emit disconnected status
    state.app_handle.emit("audio:status", json!({ "connected": false }))
        .map_err(|e| e.to_string())?;

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
            *processor = Some(AudioProcessor::new(tx.clone()).map_err(|e| e.to_string())?);
            println!("Audio processor initialized successfully");
        }
    }
    
    println!("Setting up processor with channel");
    // Setup processor with the channel
    setup_processor(&state.audio_processor, tx.clone()).await?;
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
    init_network(&state.network, state.app_handle.clone()).await?;
    println!("Network initialized");

    // Emit connected status
    state.app_handle.emit("audio:status", json!({ "connected": true }))
        .map_err(|e| e.to_string())?;

    let mut network = state.network.lock().await;
    if let Some(net) = network.as_mut() {
        // First set up the incoming audio handler
        let processor = {
            let guard = state.audio_processor.lock().await;
            guard.as_ref().ok_or_else(|| "Processor not initialized".to_string())?.clone()
        };
        
        // Create a new Arc<Mutex<AudioProcessor>> for the network
        let network_processor = Arc::new(PLMutex::new(processor));
        
        // Add peers
        for peer_addr in peers {
            println!("Adding peer: {}", peer_addr);
            net.add_peer(peer_addr);
        }

        // Start handling incoming audio first
        println!("Starting to handle incoming audio");
        net.handle_incoming(network_processor).await;
        println!("Handling incoming audio started");
        
        // Then start streaming
        println!("Starting audio streaming");
        net.start_streaming(rx).await;
        println!("Audio streaming started");
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
    // Emit disconnected status
    state.app_handle.emit("audio:status", json!({ "connected": false }))
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
async fn set_input_device(
    state: State<'_, AppState>,
    device_id: String
) -> Result<(), String> {
    let mut processor = state.audio_processor.lock().await;
    if let Some(proc) = processor.as_mut() {
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
        proc.set_input_volume(volume);
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
            let app_state = tauri::async_runtime::block_on(AppState::new(app_handle.clone()));
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
            set_muted,
            cleanup_rooms
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
