use crate::AppState;
use crate::application::error::AppResult;
use crate::application::ports::game_actions::GameActionsPort;
use crate::infrastructure::game_actions_port::InfrastructureGameActionsPort;

struct GameActionsService<P> {
    port: P,
}

impl<P> GameActionsService<P>
where
    P: GameActionsPort,
{
    fn new(port: P) -> Self {
        Self { port }
    }

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

pub(crate) fn play_game(
    state: &AppState,
    provider: String,
    external_id: String,
    launch_options: Option<String>,
) -> AppResult<()> {
    GameActionsService::new(InfrastructureGameActionsPort::new(state))
        .play_game(provider, external_id, launch_options)
}

pub(crate) fn install_game(
    state: &AppState,
    provider: String,
    external_id: String,
    install_path: Option<String>,
    create_desktop_shortcut: Option<bool>,
    create_application_shortcut: Option<bool>,
) -> AppResult<()> {
    GameActionsService::new(InfrastructureGameActionsPort::new(state)).install_game(
        provider,
        external_id,
        install_path,
        create_desktop_shortcut,
        create_application_shortcut,
    )
}

pub(crate) fn uninstall_game(
    state: &AppState,
    provider: String,
    external_id: String,
) -> AppResult<()> {
    GameActionsService::new(InfrastructureGameActionsPort::new(state))
        .uninstall_game(provider, external_id)
}

pub(crate) fn browse_game_installed_files(
    state: &AppState,
    provider: String,
    external_id: String,
) -> AppResult<()> {
    GameActionsService::new(InfrastructureGameActionsPort::new(state))
        .browse_game_installed_files(provider, external_id)
}

pub(crate) fn backup_game_files(
    state: &AppState,
    provider: String,
    external_id: String,
) -> AppResult<()> {
    GameActionsService::new(InfrastructureGameActionsPort::new(state))
        .backup_game_files(provider, external_id)
}

pub(crate) fn verify_game_files(
    state: &AppState,
    provider: String,
    external_id: String,
) -> AppResult<()> {
    GameActionsService::new(InfrastructureGameActionsPort::new(state))
        .verify_game_files(provider, external_id)
}

pub(crate) fn add_game_desktop_shortcut(
    state: &AppState,
    provider: String,
    external_id: String,
) -> AppResult<()> {
    GameActionsService::new(InfrastructureGameActionsPort::new(state))
        .add_game_desktop_shortcut(provider, external_id)
}

pub(crate) fn open_game_recording_settings(
    state: &AppState,
    provider: String,
    external_id: String,
) -> AppResult<()> {
    GameActionsService::new(InfrastructureGameActionsPort::new(state))
        .open_game_recording_settings(provider, external_id)
}
