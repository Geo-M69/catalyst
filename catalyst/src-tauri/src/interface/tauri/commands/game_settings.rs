use crate::application::error::AppResult;
use crate::{
    AppState,
    GameCompatibilityToolResponse,
    GameCustomizationArtworkResponse,
    GameInstallLocationResponse,
    GameInstallationDetailsResponse,
    GamePrivacySettingsResponse,
    GamePropertiesSettingsPayload,
    GameScreenshotResponse,
};
use crate::interface::tauri::commands::blocking::run_blocking;
use tauri::State;

#[tauri::command]
pub(crate) async fn list_game_languages(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<String>> {
    let state = state.inner().clone();
    run_blocking(move || {
        crate::application::services::game_settings_service::list_game_languages(
            &state,
            provider,
            external_id,
        )
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
        crate::application::services::game_settings_service::list_game_compatibility_tools(
            &state,
            provider,
            external_id,
        )
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
        crate::application::services::game_settings_service::get_game_privacy_settings(
            &state,
            provider,
            external_id,
        )
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
        crate::application::services::game_settings_service::set_game_privacy_settings(
            &state,
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
        crate::application::services::game_settings_service::clear_game_overlay_data(
            &state,
            provider,
            external_id,
        )
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
        crate::application::services::game_settings_service::get_game_properties_settings(
            &state,
            provider,
            external_id,
        )
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
        crate::application::services::game_settings_service::set_game_properties_settings(
            &state,
            provider,
            external_id,
            settings,
        )
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
        crate::application::services::game_settings_service::get_game_customization_artwork(
            &state,
            provider,
            external_id,
        )
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
        crate::application::services::game_settings_service::get_game_screenshots(
            &state,
            provider,
            external_id,
        )
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
        crate::application::services::game_settings_service::get_game_installation_details(
            &state,
            provider,
            external_id,
        )
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
        crate::application::services::game_settings_service::get_game_install_size_estimate(
            &state,
            provider,
            external_id,
        )
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
        crate::application::services::game_settings_service::list_game_install_locations(
            &state,
            provider,
            external_id,
        )
    })
    .await
}
