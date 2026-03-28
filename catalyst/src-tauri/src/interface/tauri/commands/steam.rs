use crate::application::error::AppResult;
use crate::{
    AppState,
    GameBetaAccessCodeValidationResponse,
    GameVersionBetasResponse,
    SteamCollectionsImportResponse,
};
use crate::interface::tauri::commands::blocking::run_blocking;
use tauri::State;

#[tauri::command]
pub(crate) async fn list_game_versions_betas(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<GameVersionBetasResponse> {
    let state = state.inner().clone();
    run_blocking(move || {
        crate::application::services::steam_service::list_game_versions_betas(
            &state,
            provider,
            external_id,
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn validate_game_beta_access_code(
    provider: String,
    external_id: String,
    access_code: String,
    state: State<'_, AppState>,
) -> AppResult<GameBetaAccessCodeValidationResponse> {
    let state = state.inner().clone();
    run_blocking(move || {
        crate::application::services::steam_service::validate_game_beta_access_code(
            &state,
            provider,
            external_id,
            access_code,
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn import_steam_collections(state: State<'_, AppState>) -> AppResult<SteamCollectionsImportResponse> {
    let state = state.inner().clone();
    run_blocking(move || crate::application::services::steam_service::import_steam_collections(&state)).await
}
