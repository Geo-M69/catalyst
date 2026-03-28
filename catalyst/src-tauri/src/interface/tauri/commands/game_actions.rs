use crate::application::error::AppResult;
use crate::AppState;
use crate::interface::tauri::commands::blocking::run_blocking;
use tauri::State;

#[tauri::command]
pub(crate) async fn play_game(
    provider: String,
    external_id: String,
    launch_options: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let state = state.inner().clone();
    run_blocking(move || {
        crate::application::services::game_actions_service::play_game(
            &state,
            provider,
            external_id,
            launch_options,
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn install_game(
    provider: String,
    external_id: String,
    install_path: Option<String>,
    create_desktop_shortcut: Option<bool>,
    create_application_shortcut: Option<bool>,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let state = state.inner().clone();
    run_blocking(move || {
        crate::application::services::game_actions_service::install_game(
            &state,
            provider,
            external_id,
            install_path,
            create_desktop_shortcut,
            create_application_shortcut,
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn uninstall_game(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let state = state.inner().clone();
    run_blocking(move || {
        crate::application::services::game_actions_service::uninstall_game(
            &state,
            provider,
            external_id,
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn browse_game_installed_files(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let state = state.inner().clone();
    run_blocking(move || {
        crate::application::services::game_actions_service::browse_game_installed_files(
            &state,
            provider,
            external_id,
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn backup_game_files(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let state = state.inner().clone();
    run_blocking(move || {
        crate::application::services::game_actions_service::backup_game_files(
            &state,
            provider,
            external_id,
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn verify_game_files(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let state = state.inner().clone();
    run_blocking(move || {
        crate::application::services::game_actions_service::verify_game_files(
            &state,
            provider,
            external_id,
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn add_game_desktop_shortcut(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let state = state.inner().clone();
    run_blocking(move || {
        crate::application::services::game_actions_service::add_game_desktop_shortcut(
            &state,
            provider,
            external_id,
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn open_game_recording_settings(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let state = state.inner().clone();
    run_blocking(move || {
        crate::application::services::game_actions_service::open_game_recording_settings(
            &state,
            provider,
            external_id,
        )
    })
    .await
}
