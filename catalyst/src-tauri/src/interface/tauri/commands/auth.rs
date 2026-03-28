use crate::application::error::AppResult;
use crate::{AppState, PublicUser, SteamAuthResponse};
use tauri::State;

#[tauri::command]
pub(crate) fn logout(state: State<'_, AppState>) -> AppResult<()> {
    crate::application::services::auth_service::logout(state.inner())
}

#[tauri::command]
pub(crate) fn get_session(state: State<'_, AppState>) -> AppResult<Option<PublicUser>> {
    crate::application::services::auth_service::get_session(state.inner())
}

#[tauri::command]
pub(crate) async fn start_steam_auth(state: State<'_, AppState>) -> AppResult<SteamAuthResponse> {
    crate::application::services::auth_service::start_steam_auth(state.inner()).await
}
