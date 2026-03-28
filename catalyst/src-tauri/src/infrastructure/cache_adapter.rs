use serde_json::Value;

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
}
