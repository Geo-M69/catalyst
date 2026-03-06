use crate::*;
use crate::application::error::AppResult;
use tauri::State;

#[tauri::command]
pub(crate) fn play_game(
    provider: String,
    external_id: String,
    launch_options: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<()> {
    crate::application::services::game_actions_service::play_game(
        state.inner(),
        provider,
        external_id,
        launch_options,
    )
}

#[tauri::command]
pub(crate) fn install_game(
    provider: String,
    external_id: String,
    install_path: Option<String>,
    create_desktop_shortcut: Option<bool>,
    create_application_shortcut: Option<bool>,
    state: State<'_, AppState>,
) -> AppResult<()> {
    crate::application::services::game_actions_service::install_game(
        state.inner(),
        provider,
        external_id,
        install_path,
        create_desktop_shortcut,
        create_application_shortcut,
    )
}

#[tauri::command]
pub(crate) fn uninstall_game(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    crate::application::services::game_actions_service::uninstall_game(
        state.inner(),
        provider,
        external_id,
    )
}

#[tauri::command]
pub(crate) fn browse_game_installed_files(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    crate::application::services::game_actions_service::browse_game_installed_files(
        state.inner(),
        provider,
        external_id,
    )
}

#[tauri::command]
pub(crate) fn backup_game_files(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    crate::application::services::game_actions_service::backup_game_files(
        state.inner(),
        provider,
        external_id,
    )
}

#[tauri::command]
pub(crate) fn verify_game_files(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    crate::application::services::game_actions_service::verify_game_files(
        state.inner(),
        provider,
        external_id,
    )
}

#[tauri::command]
pub(crate) fn add_game_desktop_shortcut(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    crate::application::services::game_actions_service::add_game_desktop_shortcut(
        state.inner(),
        provider,
        external_id,
    )
}

#[tauri::command]
pub(crate) fn open_game_recording_settings(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    crate::application::services::game_actions_service::open_game_recording_settings(
        state.inner(),
        provider,
        external_id,
    )
}
