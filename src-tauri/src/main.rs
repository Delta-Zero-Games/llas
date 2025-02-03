// src-tauri/src/main.rs
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod room;
mod config;
mod audio;

use tauri::State;
use std::sync::Arc;
use tokio::sync::Mutex; 
use uuid::Uuid;
use crate::room::{RoomManager, Room, User};
use crate::audio::{AudioProcessor, AudioNetwork};
use crate::config::TurnConfig;
use tokio::sync::mpsc;
use parking_lot::Mutex as PLMutex;
use dotenv::dotenv;

type SafeAudioProcessor = Arc<Mutex<Option<AudioProcessor>>>;
type SafeAudioNetwork = Arc<Mutex<Option<AudioNetwork>>>;

pub struct AppState {
    room_manager: Arc<Mutex<RoomManager>>,
    audio_processor: SafeAudioProcessor,
    network: SafeAudioNetwork,
}

impl AppState {
    fn new() -> Self {
        Self {
            room_manager: Arc::new(Mutex::new(RoomManager::new())),
            audio_processor: Arc::new(Mutex::new(None)),
            network: Arc::new(Mutex::new(None)),
        }
    }
}

#[tauri::command]
async fn add_user(state: State<'_, AppState>, name: String) -> Result<User, String> {
    let mut manager = state.room_manager.lock().await;
    Ok(manager.add_user(name))
}

#[tauri::command]
async fn create_room(
    state: State<'_, AppState>,
    name: String,
    user_id: String,
) -> Result<Room, String> {
    let mut manager = state.room_manager.lock().await;
    let user_id = Uuid::parse_str(&user_id).map_err(|e| e.to_string())?;
    Ok(manager.create_room(name, user_id))
}

async fn init_network(app_handle: tauri::AppHandle, network: &SafeAudioNetwork) -> Result<(), String> {
    let turn_config = TurnConfig::default();
    let mut network_lock = network.lock().await;
    if network_lock.is_none() {
        let new_network = AudioNetwork::new(app_handle, "0.0.0.0:0", turn_config)
            .await
            .map_err(|e| e.to_string())?;
        *network_lock = Some(new_network);
    }
    Ok(())
}

#[tauri::command]
async fn join_room(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    room_id: String,
    user_id: String,
) -> Result<Room, String> {
    let room_id = Uuid::parse_str(&room_id).map_err(|e| e.to_string())?;
    let user_id = Uuid::parse_str(&user_id).map_err(|e| e.to_string())?;
    
    // Initialize network
    init_network(app_handle, &state.network).await?;
    
    let peer_addr = {
        let network = state.network.lock().await;
        network.as_ref()
            .ok_or_else(|| "Network not initialized".to_string())?
            .get_local_addr()
            .map_err(|e| e.to_string())?
    };

    {
        let mut manager = state.room_manager.lock().await;
        manager.add_peer_address(user_id, peer_addr)?;
        let room = manager.join_room(room_id, user_id)?;
        
        // Add peers to network
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
        Ok(room)
    }
}

#[tauri::command]
async fn leave_room(
    state: State<'_, AppState>,
    room_id: String,
    user_id: String,
) -> Result<(), String> {
    let mut manager = state.room_manager.lock().await;
    let room_id = Uuid::parse_str(&room_id).map_err(|e| e.to_string())?;
    let user_id = Uuid::parse_str(&user_id).map_err(|e| e.to_string())?;
    manager.leave_room(room_id, user_id)
}

#[tauri::command]
async fn list_rooms(state: State<'_, AppState>) -> Result<Vec<Room>, String> {
    let manager = state.room_manager.lock().await;
    Ok(manager.list_rooms())
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
async fn start_streaming(app_handle: tauri::AppHandle, state: State<'_, AppState>, room_id: String) -> Result<(), String> {
    println!("Starting streaming for room: {}", room_id);
    let (tx, rx) = mpsc::channel(32);

    // Get the processor reference
    let processor = {
        let mut processor = state.audio_processor.lock().await;
        if processor.is_none() {
            *processor = Some(AudioProcessor::new());
        }
        Arc::new(processor.clone().unwrap())
    };

    // Initialize network if not already initialized
    println!("Initializing network");
    init_network(app_handle.clone(), &state.network).await?;
    println!("Network initialized");

    let mut network = state.network.lock().await;
    if let Some(net) = network.as_mut() {
        // Start audio streaming
        println!("Starting audio streaming");
        net.start_streaming(app_handle.clone(), rx).await;
        println!("Audio streaming started");

        // Start handling incoming audio
        net.handle_incoming(processor, app_handle).await;
        println!("Started handling incoming audio");
    }

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
    // Load environment variables from .env file
    if let Err(e) = dotenv() {
        eprintln!("Warning: Failed to load .env file: {}", e);
    }

    tauri::Builder::default()
        .manage(AppState {
            room_manager: Arc::new(Mutex::new(RoomManager::new())),
            network: Arc::new(Mutex::new(None)),
            audio_processor: Arc::new(Mutex::new(None)),
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
