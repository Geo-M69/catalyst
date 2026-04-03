use crate::application::error::AppResult;

pub(crate) trait GameActionsUseCase {
    fn play_game(
        &self,
        provider: String,
        external_id: String,
        launch_options: Option<String>,
    ) -> AppResult<()>;

    fn install_game(
        &self,
        provider: String,
        external_id: String,
        install_path: Option<String>,
        create_desktop_shortcut: Option<bool>,
        create_application_shortcut: Option<bool>,
    ) -> AppResult<()>;

    fn uninstall_game(&self, provider: String, external_id: String) -> AppResult<()>;

    fn browse_game_installed_files(&self, provider: String, external_id: String) -> AppResult<()>;

    fn backup_game_files(&self, provider: String, external_id: String) -> AppResult<()>;

    fn verify_game_files(&self, provider: String, external_id: String) -> AppResult<()>;

    fn add_game_desktop_shortcut(&self, provider: String, external_id: String) -> AppResult<()>;

    fn open_game_recording_settings(&self, provider: String, external_id: String) -> AppResult<()>;
}
