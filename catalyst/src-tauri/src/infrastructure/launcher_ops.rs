use std::path::Path;

use crate::application::error::{AppError, AppResult};
use crate::infrastructure::runtime_launcher_ops;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct LauncherOps;

impl LauncherOps {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn open_path_in_file_manager(&self, path: &Path) -> AppResult<()> {
        runtime_launcher_ops::open_path_in_file_manager(path).map_err(AppError::from)
    }

    pub(crate) fn open_provider_game_uri(
        &self,
        provider: &str,
        external_id: &str,
        action: &str,
        launch_options: Option<&str>,
    ) -> AppResult<()> {
        runtime_launcher_ops::open_provider_game_uri(provider, external_id, action, launch_options)
            .map_err(AppError::from)
    }

    pub(crate) fn create_provider_game_desktop_shortcut(
        &self,
        provider: &str,
        external_id: &str,
        game_name: &str,
    ) -> AppResult<()> {
        runtime_launcher_ops::create_provider_game_desktop_shortcut(
            provider,
            external_id,
            game_name,
        )
        .map_err(AppError::from)
    }

    pub(crate) fn open_steam_game_recording_settings(&self) -> AppResult<()> {
        runtime_launcher_ops::open_steam_game_recording_settings().map_err(AppError::from)
    }
}
