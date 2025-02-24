use std::error::Error;
use redis::{Client, Commands};
use redis::RedisError;

fn main() -> Result<(), Box<dyn Error>> {
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
    println!("Connecting to Redis at {}", redis_url);

    let client = Client::open(redis_url)?;
    let mut conn = client.get_connection()?;

    println!("Connected to Redis. Cleaning all rooms...");

    // Get all room IDs
    let room_ids: Vec<String> = conn.smembers("rooms")?;
    
    if room_ids.is_empty() {
        println!("No rooms found in Redis.");
        return Ok(());
    }

    println!("Found {} rooms to clean", room_ids.len());

    // Delete each room
    for id in &room_ids {
        let room_key = format!("room:{}", id);
        conn.del(&room_key)?;
        println!("Deleted room {}", id);
    }

    // Clear the rooms set
    conn.del::<_, ()>("rooms")?;
    println!("Cleared rooms index.");
    
    println!("Successfully cleaned all rooms from Redis.");
    Ok(())
}