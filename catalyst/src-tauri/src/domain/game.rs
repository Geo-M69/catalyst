use crate::domain::error::DomainValidationError;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct GameIdentity {
    provider: String,
    external_id: String,
}

impl GameIdentity {
    pub(crate) fn parse(provider: &str, external_id: &str) -> Result<Self, DomainValidationError> {
        let provider = provider.trim().to_ascii_lowercase();
        if provider.is_empty() {
            return Err(DomainValidationError::ProviderRequired);
        }

        let external_id = external_id.trim().to_owned();
        if external_id.is_empty() {
            return Err(DomainValidationError::ExternalIdRequired);
        }

        Ok(Self {
            provider,
            external_id,
        })
    }

    pub(crate) fn into_parts(self) -> (String, String) {
        (self.provider, self.external_id)
    }

    pub(crate) fn provider(&self) -> &str {
        &self.provider
    }

    pub(crate) fn external_id(&self) -> &str {
        &self.external_id
    }
}

pub(crate) fn parse_steam_app_id(external_id: &str) -> Result<u64, DomainValidationError> {
    external_id
        .parse::<u64>()
        .map_err(|_| DomainValidationError::SteamExternalIdMustBeNumeric)
}
