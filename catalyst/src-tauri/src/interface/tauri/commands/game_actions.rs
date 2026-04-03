use crate::application::error::AppResult;
use crate::application::services::game_actions_service::GameActionsService;
use crate::application::use_cases::game_actions::GameActionsUseCase;
use crate::infrastructure::game_actions_port::InfrastructureGameActionsPort;
use crate::interface::tauri::commands::blocking::run_blocking;
use crate::AppState;
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
        let game_actions_use_case =
            GameActionsService::new(InfrastructureGameActionsPort::new(&state));
        game_actions_use_case.play_game(provider, external_id, launch_options)
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
        let game_actions_use_case =
            GameActionsService::new(InfrastructureGameActionsPort::new(&state));
        game_actions_use_case.install_game(
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
        let game_actions_use_case =
            GameActionsService::new(InfrastructureGameActionsPort::new(&state));
        game_actions_use_case.uninstall_game(provider, external_id)
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
        let game_actions_use_case =
            GameActionsService::new(InfrastructureGameActionsPort::new(&state));
        game_actions_use_case.browse_game_installed_files(provider, external_id)
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
        let game_actions_use_case =
            GameActionsService::new(InfrastructureGameActionsPort::new(&state));
        game_actions_use_case.backup_game_files(provider, external_id)
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
        let game_actions_use_case =
            GameActionsService::new(InfrastructureGameActionsPort::new(&state));
        game_actions_use_case.verify_game_files(provider, external_id)
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
        let game_actions_use_case =
            GameActionsService::new(InfrastructureGameActionsPort::new(&state));
        game_actions_use_case.add_game_desktop_shortcut(provider, external_id)
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
        let game_actions_use_case =
            GameActionsService::new(InfrastructureGameActionsPort::new(&state));
        game_actions_use_case.open_game_recording_settings(provider, external_id)
    })
    .await
}
