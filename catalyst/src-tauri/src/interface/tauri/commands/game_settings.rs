use crate::application::contracts::game_settings::{
    GameCompatibilityToolResponse, GameCustomizationArtworkResponse, GameInstallLocationResponse,
    GameInstallationDetailsResponse, GamePrivacySettingsResponse, GamePropertiesSettingsPayload,
    GameScreenshotResponse,
};
use crate::application::error::AppResult;
use crate::application::services::game_settings_service::GameSettingsService;
use crate::application::use_cases::game_settings::GameSettingsUseCase;
use crate::infrastructure::game_settings_port::InfrastructureGameSettingsPort;
use crate::interface::tauri::commands::blocking::run_blocking;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub(crate) async fn list_game_languages(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<String>> {
    let state = state.inner().clone();
    run_blocking(move || {
        let game_settings_use_case =
            GameSettingsService::new(InfrastructureGameSettingsPort::new(&state));
        game_settings_use_case.list_game_languages(provider, external_id)
    })
    .await
}

#[tauri::command]
pub(crate) async fn list_game_compatibility_tools(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<GameCompatibilityToolResponse>> {
    let state = state.inner().clone();
    run_blocking(move || {
        let game_settings_use_case =
            GameSettingsService::new(InfrastructureGameSettingsPort::new(&state));
        game_settings_use_case.list_game_compatibility_tools(provider, external_id)
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_game_privacy_settings(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<GamePrivacySettingsResponse> {
    let state = state.inner().clone();
    run_blocking(move || {
        let game_settings_use_case =
            GameSettingsService::new(InfrastructureGameSettingsPort::new(&state));
        game_settings_use_case.get_game_privacy_settings(provider, external_id)
    })
    .await
}

#[tauri::command]
pub(crate) async fn set_game_privacy_settings(
    provider: String,
    external_id: String,
    hide_in_library: bool,
    mark_as_private: bool,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let state = state.inner().clone();
    run_blocking(move || {
        let game_settings_use_case =
            GameSettingsService::new(InfrastructureGameSettingsPort::new(&state));
        game_settings_use_case.set_game_privacy_settings(
            provider,
            external_id,
            hide_in_library,
            mark_as_private,
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn clear_game_overlay_data(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let state = state.inner().clone();
    run_blocking(move || {
        let game_settings_use_case =
            GameSettingsService::new(InfrastructureGameSettingsPort::new(&state));
        game_settings_use_case.clear_game_overlay_data(provider, external_id)
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_game_properties_settings(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<GamePropertiesSettingsPayload> {
    let state = state.inner().clone();
    run_blocking(move || {
        let game_settings_use_case =
            GameSettingsService::new(InfrastructureGameSettingsPort::new(&state));
        game_settings_use_case.get_game_properties_settings(provider, external_id)
    })
    .await
}

#[tauri::command]
pub(crate) async fn set_game_properties_settings(
    provider: String,
    external_id: String,
    settings: GamePropertiesSettingsPayload,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let state = state.inner().clone();
    run_blocking(move || {
        let game_settings_use_case =
            GameSettingsService::new(InfrastructureGameSettingsPort::new(&state));
        game_settings_use_case.set_game_properties_settings(provider, external_id, settings)
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_game_customization_artwork(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<GameCustomizationArtworkResponse> {
    let state = state.inner().clone();
    run_blocking(move || {
        let game_settings_use_case =
            GameSettingsService::new(InfrastructureGameSettingsPort::new(&state));
        game_settings_use_case.get_game_customization_artwork(provider, external_id)
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_game_screenshots(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<GameScreenshotResponse>> {
    let state = state.inner().clone();
    run_blocking(move || {
        let game_settings_use_case =
            GameSettingsService::new(InfrastructureGameSettingsPort::new(&state));
        game_settings_use_case.get_game_screenshots(provider, external_id)
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_game_installation_details(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<GameInstallationDetailsResponse> {
    let state = state.inner().clone();
    run_blocking(move || {
        let game_settings_use_case =
            GameSettingsService::new(InfrastructureGameSettingsPort::new(&state));
        game_settings_use_case.get_game_installation_details(provider, external_id)
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_game_install_size_estimate(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<Option<u64>> {
    let state = state.inner().clone();
    run_blocking(move || {
        let game_settings_use_case =
            GameSettingsService::new(InfrastructureGameSettingsPort::new(&state));
        game_settings_use_case.get_game_install_size_estimate(provider, external_id)
    })
    .await
}

#[tauri::command]
pub(crate) async fn list_game_install_locations(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<GameInstallLocationResponse>> {
    let state = state.inner().clone();
    run_blocking(move || {
        let game_settings_use_case =
            GameSettingsService::new(InfrastructureGameSettingsPort::new(&state));
        game_settings_use_case.list_game_install_locations(provider, external_id)
    })
    .await
}
