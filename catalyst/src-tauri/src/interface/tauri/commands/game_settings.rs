use crate::*;
use crate::application::error::AppResult;
use tauri::State;

#[tauri::command]
pub(crate) fn list_game_languages(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<String>> {
    crate::application::services::game_settings_service::list_game_languages(
        state.inner(),
        provider,
        external_id,
    )
}

#[tauri::command]
pub(crate) fn list_game_compatibility_tools(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<GameCompatibilityToolResponse>> {
    crate::application::services::game_settings_service::list_game_compatibility_tools(
        state.inner(),
        provider,
        external_id,
    )
}

#[tauri::command]
pub(crate) fn get_game_privacy_settings(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<GamePrivacySettingsResponse> {
    crate::application::services::game_settings_service::get_game_privacy_settings(
        state.inner(),
        provider,
        external_id,
    )
}

#[tauri::command]
pub(crate) fn set_game_privacy_settings(
    provider: String,
    external_id: String,
    hide_in_library: bool,
    mark_as_private: bool,
    state: State<'_, AppState>,
) -> AppResult<()> {
    crate::application::services::game_settings_service::set_game_privacy_settings(
        state.inner(),
        provider,
        external_id,
        hide_in_library,
        mark_as_private,
    )
}

#[tauri::command]
pub(crate) fn clear_game_overlay_data(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    crate::application::services::game_settings_service::clear_game_overlay_data(
        state.inner(),
        provider,
        external_id,
    )
}

#[tauri::command]
pub(crate) fn get_game_properties_settings(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<GamePropertiesSettingsPayload> {
    crate::application::services::game_settings_service::get_game_properties_settings(
        state.inner(),
        provider,
        external_id,
    )
}

#[tauri::command]
pub(crate) fn set_game_properties_settings(
    provider: String,
    external_id: String,
    settings: GamePropertiesSettingsPayload,
    state: State<'_, AppState>,
) -> AppResult<()> {
    crate::application::services::game_settings_service::set_game_properties_settings(
        state.inner(),
        provider,
        external_id,
        settings,
    )
}

#[tauri::command]
pub(crate) fn get_game_customization_artwork(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<GameCustomizationArtworkResponse> {
    crate::application::services::game_settings_service::get_game_customization_artwork(
        state.inner(),
        provider,
        external_id,
    )
}

#[tauri::command]
pub(crate) fn get_game_installation_details(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<GameInstallationDetailsResponse> {
    crate::application::services::game_settings_service::get_game_installation_details(
        state.inner(),
        provider,
        external_id,
    )
}

#[tauri::command]
pub(crate) fn get_game_install_size_estimate(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<Option<u64>> {
    crate::application::services::game_settings_service::get_game_install_size_estimate(
        state.inner(),
        provider,
        external_id,
    )
}

#[tauri::command]
pub(crate) fn list_game_install_locations(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<GameInstallLocationResponse>> {
    crate::application::services::game_settings_service::list_game_install_locations(
        state.inner(),
        provider,
        external_id,
    )
}
