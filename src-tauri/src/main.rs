// src-tauri/src/main.rs
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::State;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use crate::room::{RoomManager, Room, User};
use crate::audio::{AudioProcessor, AudioNetwork};
use crate::config::TurnConfig;
use tokio::sync::mpsc;

// Define AppState to hold our shared state
pub struct AppState {
    room_manager: Arc<Mutex<RoomManager>>,
    audio_processor: Arc<Mutex<Option<AudioProcessor>>>,
    network: Arc<Mutex<Option<AudioNetwork>>>,
}

// User management commands
#[tauri::command]
async fn add_user(
    state: State<'_, AppState>,
    name: String,
) -> Result<User, String> {
    let mut manager = state.room_manager.lock().await;
    Ok(manager.add_user(name))
}

// Room management commands
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

#[tauri::command]
async fn join_room(
    state: State<'_, AppState>,
    room_id: String,
    user_id: String,
) -> Result<Room, String> {
    let turn_config = TurnConfig::default();
    let mut manager = state.room_manager.lock().await;
    let mut network = state.network.lock().await;
    
    let room_id = Uuid::parse_str(&room_id).map_err(|e| e.to_string())?;
    let user_id = Uuid::parse_str(&user_id).map_err(|e| e.to_string())?;

    // Initialize network if needed
    if network.is_none() {
        *network = Some(AudioNetwork::new("0.0.0.0:0", turn_config).await
            .map_err(|e| e.to_string())?);
    }

    // Get allocated address from TURN server
    if let Some(net) = network.as_mut() {
        let peer_addr = net.get_local_addr()
            .map_err(|e| e.to_string())?;
        
        manager.add_peer_address(user_id, peer_addr)?;
    }

    let room = manager.join_room(room_id, user_id)?;

    // Add existing participants as peers
    if let Some(net) = network.as_mut() {
        for participant in &room.participants {
            if let Some(peer_addr) = participant.peer_addr {
                net.add_peer(peer_addr);
            }
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

// Audio commands
#[tauri::command]
async fn start_streaming(
    state: State<'_, AppState>,
    room_id: String
) -> Result<(), String> {
    let (tx, rx) = mpsc::channel(32);
    
    // Initialize audio processor
    let processor = {
        let mut processor_lock = state.audio_processor.lock().await;
        if processor_lock.is_none() {
            *processor_lock = Some(AudioProcessor::new(tx)?);
        }
        
        let processor = processor_lock.as_mut().unwrap();
        processor.setup_output_stream()?;
        processor.start_capture()?;
        
        Arc::new(Mutex::new(processor.clone()))
    };

    // Get room information and peers
    let room_id = Uuid::parse_str(&room_id).map_err(|e| e.to_string())?;
    let peers = {
        let manager = state.room_manager.lock().await;
        manager.get_room_peers(&room_id)
    };

    // Start network streaming
    let mut network = state.network.lock().await;
    if let Some(net) = network.as_mut() {
        // Add all peers from room
        for peer_addr in peers {
            net.add_peer(peer_addr);
        }
        
        net.start_streaming(rx).await;
        net.handle_incoming(processor).await;
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
    let mut processor = state.audio_processor.lock().await;
    if let Some(proc) = processor.as_mut() {
        proc.set_input_device(&device_id).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn set_input_volume(
    state: State<'_, AppState>,
    volume: f32
) -> Result<(), String> {
    let mut processor = state.audio_processor.lock().await;
    if let Some(proc) = processor.as_mut() {
        proc.set_input_volume(volume).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn set_muted(
    state: State<'_, AppState>,
    muted: bool
) -> Result<(), String> {
    let mut processor = state.audio_processor.lock().await;
    if let Some(proc) = processor.as_mut() {
        proc.set_muted(muted).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn set_user_volume(
    state: State<'_, AppState>,
    user_id: String,
    volume: f32
) -> Result<(), String> {
    let mut processor = state.audio_processor.lock().await;
    if let Some(proc) = processor.as_mut() {
        proc.set_output_volume(volume).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            room_manager: Arc::new(Mutex::new(RoomManager::new())),
            audio_processor: Arc::new(Mutex::new(None)),
            network: Arc::new(Mutex::new(None)),
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