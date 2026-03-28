#![allow(dead_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionToken(String);

impl SessionToken {
    pub(crate) fn parse(raw: &str) -> Option<Self> {
        let normalized = raw.trim();
        if normalized.is_empty() {
            return None;
        }

        Some(Self(normalized.to_owned()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}
