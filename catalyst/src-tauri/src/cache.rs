use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use serde_json::Value;

static CACHE: Lazy<Mutex<HashMap<String, (DateTime<Utc>, Value)>>> = Lazy::new(|| Mutex::new(HashMap::new()));

pub fn get_cached(key: &str, max_age_seconds: i64) -> Option<Value> {
    let guard = CACHE.lock().ok()?;
    let entry = guard.get(key)?.clone();
    let (ts, value) = entry;
    let age = Utc::now().signed_duration_since(ts).num_seconds();
    if age <= max_age_seconds {
        Some(value)
    } else {
        None
    }
}

pub fn set_cached(key: &str, value: Value) {
    if let Ok(mut guard) = CACHE.lock() {
        guard.insert(key.to_string(), (Utc::now(), value));
    }
}
