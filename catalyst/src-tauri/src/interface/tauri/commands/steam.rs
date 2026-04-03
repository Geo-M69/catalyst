use crate::application::contracts::steam::{
    GameBetaAccessCodeValidationResponse, GameVersionBetasResponse, SteamCollectionsImportResponse,
};
use crate::application::error::AppResult;
use crate::application::services::steam_service::SteamService;
use crate::application::use_cases::steam::SteamUseCase;
use crate::infrastructure::steam_port::InfrastructureSteamPort;
use crate::interface::tauri::commands::blocking::run_blocking;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub(crate) async fn list_game_versions_betas(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<GameVersionBetasResponse> {
    let state = state.inner().clone();
    run_blocking(move || {
        let steam_use_case = SteamService::new(InfrastructureSteamPort::new(&state));
        steam_use_case.list_game_versions_betas(provider, external_id)
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
        let steam_use_case = SteamService::new(InfrastructureSteamPort::new(&state));
        steam_use_case.validate_game_beta_access_code(provider, external_id, access_code)
    })
    .await
}

#[tauri::command]
pub(crate) async fn import_steam_collections(
    state: State<'_, AppState>,
) -> AppResult<SteamCollectionsImportResponse> {
    let state = state.inner().clone();
    run_blocking(move || {
        let steam_use_case = SteamService::new(InfrastructureSteamPort::new(&state));
        steam_use_case.import_steam_collections()
    })
    .await
}
