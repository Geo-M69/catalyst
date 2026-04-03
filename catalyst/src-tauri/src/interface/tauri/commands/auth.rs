use crate::application::contracts::auth::{PublicUser, SteamAuthResponse};
use crate::application::error::AppResult;
use crate::application::services::auth_service::AuthService;
use crate::application::use_cases::auth::AuthUseCase;
use crate::infrastructure::auth_port::InfrastructureAuthPort;
use crate::interface::tauri::commands::blocking::run_blocking;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub(crate) fn logout(state: State<'_, AppState>) -> AppResult<()> {
    let auth_use_case = AuthService::new(InfrastructureAuthPort::new(state.inner()));
    auth_use_case.logout()
}

#[tauri::command]
pub(crate) fn get_session(state: State<'_, AppState>) -> AppResult<Option<PublicUser>> {
    let auth_use_case = AuthService::new(InfrastructureAuthPort::new(state.inner()));
    auth_use_case.get_session()
}

#[tauri::command]
pub(crate) async fn start_steam_auth(state: State<'_, AppState>) -> AppResult<SteamAuthResponse> {
    let state = state.inner().clone();
    run_blocking(move || {
        let auth_use_case = AuthService::new(InfrastructureAuthPort::new(&state));
        auth_use_case.start_steam_auth()
    })
    .await
}
