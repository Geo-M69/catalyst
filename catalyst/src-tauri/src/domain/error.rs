#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DomainValidationError {
    ProviderRequired,
    ExternalIdRequired,
    MissingIdentityPair,
    CollectionIdRequired,
    CollectionNameRequired,
    CollectionNameTooLong { max_chars: usize },
    SteamExternalIdMustBeNumeric,
}

impl DomainValidationError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::ProviderRequired => "provider_required",
            Self::ExternalIdRequired => "external_id_required",
            Self::MissingIdentityPair => "missing_identity_pair",
            Self::CollectionIdRequired => "collection_id_required",
            Self::CollectionNameRequired => "collection_name_required",
            Self::CollectionNameTooLong { .. } => "collection_name_too_long",
            Self::SteamExternalIdMustBeNumeric => "invalid_external_id",
        }
    }

    pub(crate) fn message(&self) -> String {
        match self {
            Self::ProviderRequired => String::from("Game provider is required"),
            Self::ExternalIdRequired => String::from("Game external ID is required"),
            Self::MissingIdentityPair => {
                String::from("provider and external_id must be supplied together")
            }
            Self::CollectionIdRequired => String::from("Collection ID is required"),
            Self::CollectionNameRequired => String::from("Collection name is required"),
            Self::CollectionNameTooLong { max_chars } => {
                format!("Collection name must be {max_chars} characters or fewer")
            }
            Self::SteamExternalIdMustBeNumeric => {
                String::from("Steam external_id must be a numeric app ID")
            }
        }
    }
}
