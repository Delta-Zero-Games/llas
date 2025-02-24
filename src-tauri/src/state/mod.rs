// src-tauri/src/state/mod.rs

use redis::{aio::Connection, AsyncCommands, RedisError};
use serde_json;
use crate::room::{Room, User};
use uuid::Uuid;
use crate::config::RedisConfig;
use chrono::{DateTime, Utc, Duration};

pub struct StateManager {
    conn: Connection,
}

impl StateManager {
    /// Initialize the StateManager using the configuration from RedisConfig.
    pub async fn new() -> Result<Self, RedisError> {
        let redis_config = RedisConfig::default();
        let client = redis::Client::open(redis_config.url)?;
        let conn = client.get_async_connection().await?;
        Ok(Self { conn })
    }

    /// Save a room to Redis by storing its JSON representation.
    pub async fn save_room(&mut self, room: &Room) -> Result<(), RedisError> {
        let room_key = format!("room:{}", room.id);
        let room_json = serde_json::to_string(room).unwrap();
    
        let mut pipe = redis::pipe();
        pipe.atomic()
            .set(&room_key, room_json)
            .sadd("rooms", room.id.to_string());
    
        pipe.query_async::<_, ()>(&mut self.conn).await?;
        Ok(())
    }

    /// Retrieve a room from Redis by its UUID.
    pub async fn get_room(&mut self, room_id: &Uuid) -> Result<Option<Room>, RedisError> {
        let room_key = format!("room:{}", room_id);
        let room_json: Option<String> = self.conn.get(&room_key).await?;
        Ok(room_json.map(|json| serde_json::from_str(&json).unwrap()))
    }

    /// List all rooms stored in Redis.
    pub async fn list_rooms(&mut self) -> Result<Vec<Room>, RedisError> {
        let room_ids: Vec<String> = self.conn.smembers("rooms").await?;
        let mut rooms = Vec::new();
    
        for id in room_ids {
            if let Ok(Some(room)) = self.get_room(&Uuid::parse_str(&id).unwrap()).await {
                // Only include rooms that have participants
                if !room.participants.is_empty() {
                    rooms.push(room);
                } else {
                    // Clean up empty room from Redis
                    let room_id = Uuid::parse_str(&id).unwrap();
                    if let Err(e) = self.delete_room(&room_id).await {
                        println!("Failed to delete empty room from Redis: {}", e);
                    } else {
                        println!("Cleaned up empty room {} from Redis", id);
                    }
                }
            }
        }
        Ok(rooms)
    }

    /// Delete a room from Redis.
    pub async fn delete_room(&mut self, room_id: &Uuid) -> Result<(), RedisError> {
        let room_key = format!("room:{}", room_id);
        let mut pipe = redis::pipe();
    
        pipe.atomic()
            .del(&room_key)
            .srem("rooms", room_id.to_string());
    
        pipe.query_async::<_, ()>(&mut self.conn).await?;
        Ok(())
    }

    /// Clean up all rooms in Redis.
    pub async fn clean_all_rooms(&mut self) -> Result<(), RedisError> {
        let room_ids: Vec<String> = self.conn.smembers("rooms").await?;
        println!("Cleaning all {} rooms from Redis", room_ids.len());
        
        // Create a transaction to delete everything in one go
        let mut pipe = redis::pipe();
        pipe.atomic();
        
        // Add delete commands for each room
        for id in &room_ids {
            let room_key = format!("room:{}", id);
            pipe.del(&room_key);
            println!("Queueing delete for room {}", id);
        }
        
        // Finally delete the rooms set itself if there are rooms
        if !room_ids.is_empty() {
            pipe.del("rooms");
        }
        
        // Execute all commands in a single transaction
        pipe.query_async::<_, ()>(&mut self.conn).await?;
        println!("Successfully deleted all {} rooms", room_ids.len());
        
        Ok(())
    }

    pub async fn cleanup_stale_rooms(&mut self) -> Result<(), RedisError> {
        let room_ids: Vec<String> = self.conn.smembers("rooms").await?;
        let stale_threshold = Utc::now() - Duration::hours(24);
        
        println!("Checking {} rooms for cleanup", room_ids.len());
        let mut deleted_count = 0;
        
        for id in room_ids {
            if let Ok(Some(room)) = self.get_room(&Uuid::parse_str(&id).unwrap()).await {
                // Delete rooms that are:
                // 1. Empty 
                // 2. Have test names
                // 3. Are older than 24 hours
                if room.participants.is_empty() || 
                   room.name.to_lowercase().contains("test") ||
                   room.created_at < stale_threshold {
                    
                    println!("Cleaning up stale room: {} ({}) - created: {}", 
                             room.name, id, room.created_at);
                    
                    self.delete_room(&Uuid::parse_str(&id).unwrap()).await?;
                    deleted_count += 1;
                }
            }
        }
        
        println!("Cleanup complete: removed {} stale rooms", deleted_count);
        Ok(())
    }

    /// Update the participants of a room in Redis.
    #[allow(dead_code)]
    pub async fn update_room_participants(&mut self, room_id: &Uuid, participants: &[User]) -> Result<(), RedisError> {
        if let Some(mut room) = self.get_room(room_id).await? {
            room.participants = participants.to_vec();
            self.save_room(&room).await?;
        }
        Ok(())
    }
}