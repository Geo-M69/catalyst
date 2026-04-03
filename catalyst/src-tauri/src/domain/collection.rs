use crate::domain::error::DomainValidationError;

pub(crate) const MAX_COLLECTION_NAME_CHARS: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct CollectionId(String);

impl CollectionId {
    pub(crate) fn parse(raw: &str) -> Result<Self, DomainValidationError> {
        let normalized = raw.trim();
        if normalized.is_empty() {
            return Err(DomainValidationError::CollectionIdRequired);
        }

        Ok(Self(normalized.to_owned()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct CollectionName(String);

impl CollectionName {
    pub(crate) fn parse(raw: &str) -> Result<Self, DomainValidationError> {
        let normalized = raw.trim();
        if normalized.is_empty() {
            return Err(DomainValidationError::CollectionNameRequired);
        }

        if normalized.chars().count() > MAX_COLLECTION_NAME_CHARS {
            return Err(DomainValidationError::CollectionNameTooLong {
                max_chars: MAX_COLLECTION_NAME_CHARS,
            });
        }

        Ok(Self(normalized.to_owned()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn into_inner(self) -> String {
        self.0
    }
}

pub(crate) fn parse_collection_name_candidate(raw_value: &str) -> Option<String> {
    let normalized = raw_value.replace('\0', "");
    let normalized = normalized.trim();
    if normalized.is_empty() {
        return None;
    }

    let lowered = normalized.to_ascii_lowercase();
    if matches!(lowered.as_str(), "0" | "1" | "true" | "false") {
        return None;
    }
    if normalized
        .chars()
        .all(|character| character.is_ascii_digit())
    {
        return None;
    }

    Some(normalized.to_owned())
}
