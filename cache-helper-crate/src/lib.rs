//! Cache Helper - A convenient Redis caching utility
//! 
//! This library provides simple caching utilities for Redis

use redis::{Commands, Connection, RedisResult};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

pub struct CacheHelper {
    connection: Connection,
    default_ttl: Duration,
}

impl CacheHelper {
    /// Create a new cache helper with Redis connection
    pub fn new(redis_url: &str) -> RedisResult<Self> {
        let client = redis::Client::open(redis_url)?;
        let connection = client.get_connection()?;
        
        Ok(CacheHelper {
            connection,
            default_ttl: Duration::from_secs(3600), // 1 hour default
        })
    }
    
    /// Set the default TTL for cache entries
    pub fn set_default_ttl(&mut self, ttl: Duration) {
        self.default_ttl = ttl;
    }
    
    /// Cache a serializable value with a generated UUID key
    pub fn cache_value<T: Serialize>(&mut self, value: &T) -> RedisResult<String> {
        let key = Uuid::new_v4().to_string();
        let json = serde_json::to_string(value)
            .map_err(|e| redis::RedisError::from((
                redis::ErrorKind::TypeError,
                "Serialization error",
                e.to_string()
            )))?;
        
        self.connection.set_ex(&key, json, self.default_ttl.as_secs())?;
        Ok(key)
    }
    
    /// Get a cached value by key
    pub fn get_cached<T: for<'de> Deserialize<'de>>(&mut self, key: &str) -> RedisResult<Option<T>> {
        let value: Option<String> = self.connection.get(key)?;
        
        match value {
            Some(json) => {
                let parsed = serde_json::from_str(&json)
                    .map_err(|e| redis::RedisError::from((
                        redis::ErrorKind::TypeError,
                        "Deserialization error",
                        e.to_string()
                    )))?;
                Ok(Some(parsed))
            }
            None => Ok(None)
        }
    }
    
    /// Invalidate a cache entry
    pub fn invalidate(&mut self, key: &str) -> RedisResult<bool> {
        self.connection.del(key)
    }
    
    /// Clear all cache entries (use with caution!)
    pub fn clear_all(&mut self) -> RedisResult<()> {
        redis::cmd("FLUSHDB").execute(&mut self.connection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestData {
        id: u32,
        name: String,
    }
    
    #[test]
    fn test_cache_and_retrieve() {
        // This would need a Redis instance to run
        // Just a placeholder test
        assert_eq!(1 + 1, 2);
    }
}