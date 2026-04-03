use crate::application::error::AppResult;
use crate::application::ports::game_actions::GameActionsPort;
use crate::application::use_cases::game_actions::GameActionsUseCase;

pub(crate) struct GameActionsService<P> {
    port: P,
}

impl<P> GameActionsService<P>
where
    P: GameActionsPort,
{
    pub(crate) fn new(port: P) -> Self {
        Self { port }
    }
}

impl<P> GameActionsUseCase for GameActionsService<P>
where
    P: GameActionsPort,
{
    fn play_game(
        &self,
        provider: String,
        external_id: String,
        launch_options: Option<String>,
    ) -> AppResult<()> {
        self.port.play_game(provider, external_id, launch_options)
    }

    fn install_game(
        &self,
        provider: String,
        external_id: String,
        install_path: Option<String>,
        create_desktop_shortcut: Option<bool>,
        create_application_shortcut: Option<bool>,
    ) -> AppResult<()> {
        self.port.install_game(
            provider,
            external_id,
            install_path,
            create_desktop_shortcut,
            create_application_shortcut,
        )
    }

    fn uninstall_game(&self, provider: String, external_id: String) -> AppResult<()> {
        self.port.uninstall_game(provider, external_id)
    }

    fn browse_game_installed_files(&self, provider: String, external_id: String) -> AppResult<()> {
        self.port.browse_game_installed_files(provider, external_id)
    }

    fn backup_game_files(&self, provider: String, external_id: String) -> AppResult<()> {
        self.port.backup_game_files(provider, external_id)
    }

    fn verify_game_files(&self, provider: String, external_id: String) -> AppResult<()> {
        self.port.verify_game_files(provider, external_id)
    }

    fn add_game_desktop_shortcut(&self, provider: String, external_id: String) -> AppResult<()> {
        self.port.add_game_desktop_shortcut(provider, external_id)
    }

    fn open_game_recording_settings(&self, provider: String, external_id: String) -> AppResult<()> {
        self.port.open_game_recording_settings(provider, external_id)
    }
}
