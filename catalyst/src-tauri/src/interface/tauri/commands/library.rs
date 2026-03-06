use crate::*;
use crate::application::error::AppResult;
use tauri::State;

#[tauri::command]
pub(crate) fn get_library(state: State<'_, AppState>) -> AppResult<LibraryResponse> {
    crate::application::services::library_service::get_library(state.inner())
}

#[tauri::command]
pub(crate) fn get_steam_status(state: State<'_, AppState>) -> AppResult<SteamStatusResponse> {
    crate::application::services::library_service::get_steam_status(state.inner())
}

#[tauri::command]
pub(crate) fn sync_steam_library(state: State<'_, AppState>) -> AppResult<SteamSyncResponse> {
    crate::application::services::library_service::sync_steam_library(state.inner())
}

#[tauri::command]
pub(crate) fn set_game_favorite(
    provider: String,
    external_id: String,
    favorite: bool,
    state: State<'_, AppState>,
) -> AppResult<()> {
    crate::application::services::library_service::set_game_favorite(
        state.inner(),
        provider,
        external_id,
        favorite,
    )
}

#[tauri::command]
pub(crate) fn list_steam_downloads(state: State<'_, AppState>) -> AppResult<Vec<SteamDownloadProgressResponse>> {
    crate::application::services::library_service::list_steam_downloads(state.inner())
}
