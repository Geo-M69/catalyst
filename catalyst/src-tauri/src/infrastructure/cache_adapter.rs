#![allow(dead_code)]

use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use crate::application::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct CacheAdapter;

impl CacheAdapter {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn get_json(&self, key: &str, max_age_seconds: i64) -> Option<Value> {
        crate::cache::get_cached(key, max_age_seconds)
    }

    pub(crate) fn set_json(&self, key: &str, value: Value) {
        crate::cache::set_cached(key, value);
    }

    pub(crate) fn get_typed<T>(&self, key: &str, max_age_seconds: i64) -> AppResult<Option<T>>
    where
        T: DeserializeOwned,
    {
        match self.get_json(key, max_age_seconds) {
            Some(value) => serde_json::from_value(value)
                .map(Some)
                .map_err(|error| AppError::from(format!("Failed to decode cached value: {error}"))),
            None => Ok(None),
        }
    }

    pub(crate) fn set_typed<T>(&self, key: &str, value: &T) -> AppResult<()>
    where
        T: Serialize,
    {
        let encoded = serde_json::to_value(value)
            .map_err(|error| AppError::from(format!("Failed to encode cached value: {error}")))?;
        self.set_json(key, encoded);
        Ok(())
    }
}
