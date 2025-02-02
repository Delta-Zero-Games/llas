// src-tauri/src/room/mod.rs
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub name: String,
    pub is_muted: bool,
    pub is_deafened: bool,
    pub peer_addr: Option<SocketAddr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub id: Uuid,
    pub name: String,
    pub creator_id: Uuid,
    pub participants: Vec<User>,
    pub created_at: DateTime<Utc>,
}

pub struct RoomManager {
    rooms: HashMap<Uuid, Room>,
    users: HashMap<Uuid, User>,
    peer_mappings: HashMap<SocketAddr, Uuid>,
}

impl RoomManager {
    pub fn new() -> Self {
        Self {
            rooms: HashMap::new(),
            users: HashMap::new(),
            peer_mappings: HashMap::new(),
        }
    }

    pub fn create_room(&mut self, name: String, creator_id: Uuid) -> Room {
        let room = Room {
            id: Uuid::new_v4(),
            name,
            creator_id,
            participants: Vec::new(),
            created_at: Utc::now(),
        };
        self.rooms.insert(room.id, room.clone());
        room
    }

    pub fn join_room(&mut self, room_id: Uuid, user_id: Uuid) -> Result<Room, String> {
        let room = self.rooms.get_mut(&room_id).ok_or("Room not found")?;
        let user = self.users.get(&user_id).ok_or("User not found")?;
        if !room.participants.iter().any(|p| p.id == user_id) {
            room.participants.push(user.clone());
        }
        Ok(room.clone())
    }

    pub fn leave_room(&mut self, room_id: Uuid, user_id: Uuid) -> Result<(), String> {
        let room = self.rooms.get_mut(&room_id).ok_or("Room not found")?;
        room.participants.retain(|p| p.id != user_id);
        if let Some(user) = self.users.get(&user_id) {
            if let Some(addr) = user.peer_addr {
                self.peer_mappings.remove(&addr);
            }
        }
        if room.participants.is_empty() && room.creator_id != user_id {
            self.rooms.remove(&room_id);
        } else if room.creator_id == user_id && !room.participants.is_empty() {
            room.creator_id = room.participants[0].id;
        }
        Ok(())
    }

    pub fn list_rooms(&self) -> Vec<Room> {
        self.rooms.values().cloned().collect()
    }

    pub fn add_peer_address(&mut self, user_id: Uuid, addr: SocketAddr) -> Result<(), String> {
        if let Some(user) = self.users.get_mut(&user_id) {
            if let Some(old_addr) = user.peer_addr {
                self.peer_mappings.remove(&old_addr);
            }
            user.peer_addr = Some(addr);
            self.peer_mappings.insert(addr, user_id);
            Ok(())
        } else {
            Err("User not found".to_string())
        }
    }

    pub fn get_room_peers(&self, room_id: &Uuid) -> Vec<SocketAddr> {
        if let Some(room) = self.rooms.get(room_id) {
            room.participants.iter().filter_map(|user| user.peer_addr).collect()
        } else {
            Vec::new()
        }
    }
    
    pub fn add_user(&mut self, name: String) -> User {
        let user = User {
            id: Uuid::new_v4(),
            name,
            is_muted: false,
            is_deafened: false,
            peer_addr: None,
        };
        self.users.insert(user.id, user.clone());
        user
    }
}
