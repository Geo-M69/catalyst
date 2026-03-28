#![allow(dead_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LaunchOptions(String);

impl LaunchOptions {
    pub(crate) fn parse_optional(raw: Option<&str>) -> Option<Self> {
        let normalized = raw?.trim();
        if normalized.is_empty() {
            return None;
        }

        Some(Self(normalized.to_owned()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}
