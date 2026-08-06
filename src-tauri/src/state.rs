use moka::future::Cache;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub memory_cache: Cache<String, String>,
}

impl AppState {
    pub fn new(db: Connection) -> Self {
        let memory_cache = Cache::builder()
            .max_capacity(500)
            .time_to_live(Duration::from_secs(300))
            .build();

        Self {
            db: Arc::new(Mutex::new(db)),
            memory_cache,
        }
    }
}
