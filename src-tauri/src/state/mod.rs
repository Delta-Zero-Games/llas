// src-tauri/src/state/mod.rs

use redis::{aio::Connection, AsyncCommands, RedisError};
use serde_json;
use crate::room::{Room, User};
use uuid::Uuid;
use crate::config::RedisConfig;

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

        pipe.query_async(&mut self.conn).await?;
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
                rooms.push(room);
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

        pipe.query_async(&mut self.conn).await?;
        Ok(())
    }

    /// Update the participants of a room in Redis.
    pub async fn update_room_participants(&mut self, room_id: &Uuid, participants: &[User]) -> Result<(), RedisError> {
        if let Some(mut room) = self.get_room(room_id).await? {
            room.participants = participants.to_vec();
            self.save_room(&room).await?;
        }
        Ok(())
    }
}
