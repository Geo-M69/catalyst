use std::collections::HashSet;
use std::path::PathBuf;

use crate::application::error::AppResult;

#[derive(Debug, Clone, Default)]
pub(crate) struct SteamLocal {
    steam_root_override: Option<String>,
}

impl SteamLocal {
    pub(crate) fn new(steam_root_override: Option<&str>) -> Self {
        Self {
            steam_root_override: steam_root_override
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned),
        }
    }

    pub(crate) fn resolve_install_directory_for_app_id(&self, app_id: u64) -> AppResult<PathBuf> {
        crate::resolve_steam_install_directory_for_app_id(
            self.steam_root_override.as_deref(),
            app_id,
        )
        .map_err(Into::into)
    }

    pub(crate) fn detect_locally_installed_app_ids(&self) -> AppResult<HashSet<u64>> {
        crate::detect_locally_installed_steam_app_ids(self.steam_root_override.as_deref())
            .map_err(Into::into)
    }
}
