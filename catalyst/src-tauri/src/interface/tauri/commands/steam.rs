use crate::*;
use crate::application::error::AppResult;
use tauri::State;

#[tauri::command]
pub(crate) fn list_game_versions_betas(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<GameVersionBetasResponse> {
    crate::application::services::steam_service::list_game_versions_betas(
        state.inner(),
        provider,
        external_id,
    )
}

#[tauri::command]
pub(crate) fn validate_game_beta_access_code(
    provider: String,
    external_id: String,
    access_code: String,
    state: State<'_, AppState>,
) -> AppResult<GameBetaAccessCodeValidationResponse> {
    crate::application::services::steam_service::validate_game_beta_access_code(
        state.inner(),
        provider,
        external_id,
        access_code,
    )
}

#[tauri::command]
pub(crate) fn import_steam_collections(state: State<'_, AppState>) -> AppResult<SteamCollectionsImportResponse> {
    crate::application::services::steam_service::import_steam_collections(state.inner())
}
