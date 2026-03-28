#![allow(dead_code)]

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::application::error::{AppError, AppResult};
use crate::application::ports::effects::FilesystemPort;

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

    pub(crate) fn resolve_root_path(&self) -> AppResult<PathBuf> {
        crate::resolve_steam_root_path(self.steam_root_override.as_deref()).ok_or_else(|| {
            AppError::not_found(
                "steam_install_not_found",
                "Could not locate local Steam installation",
            )
        })
    }

    pub(crate) fn resolve_userdata_directory(&self, steam_id: &str) -> AppResult<PathBuf> {
        let steam_root = self.resolve_root_path()?;
        crate::resolve_steam_userdata_directory(&steam_root, steam_id).map_err(AppError::from)
    }

    pub(crate) fn resolve_install_directory_for_app_id(&self, app_id: u64) -> AppResult<PathBuf> {
        crate::resolve_steam_install_directory_for_app_id(
            self.steam_root_override.as_deref(),
            app_id,
        )
        .map_err(AppError::from)
    }

    pub(crate) fn detect_locally_installed_app_ids(&self) -> AppResult<HashSet<u64>> {
        crate::detect_locally_installed_steam_app_ids(self.steam_root_override.as_deref())
            .map_err(AppError::from)
    }
}

impl FilesystemPort for SteamLocal {
    fn read_to_string(&self, path: &Path) -> AppResult<String> {
        fs::read_to_string(path)
            .map_err(|error| AppError::from(format!("Failed to read {}: {error}", path.display())))
    }

    fn write_string(&self, path: &Path, content: &str) -> AppResult<()> {
        fs::write(path, content)
            .map_err(|error| AppError::from(format!("Failed to write {}: {error}", path.display())))
    }

    fn list_paths(&self, dir: &Path) -> AppResult<Vec<PathBuf>> {
        let entries = fs::read_dir(dir)
            .map_err(|error| AppError::from(format!("Failed to list {}: {error}", dir.display())))?;
        let mut paths = Vec::new();
        for entry in entries {
            let entry = entry
                .map_err(|error| AppError::from(format!("Failed to list {}: {error}", dir.display())))?;
            paths.push(entry.path());
        }
        Ok(paths)
    }
}
