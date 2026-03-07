use crate::*;
use crate::application::error::{AppError, AppResult};
use tauri::State;


// `register` and `login` local credential commands removed. Authentication
// is performed via Steam SSO (`start_steam_auth`).

#[tauri::command]
pub(crate) fn logout(state: State<'_, AppState>) -> AppResult<()> {
    let session_token = get_state_session_token(state.inner())?;
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;

    if let Some(token) = session_token {
        invalidate_session_by_token(&connection, &token)?;
    }

    Ok(clear_active_session(state.inner())?)
}

#[tauri::command]
pub(crate) fn get_session(state: State<'_, AppState>) -> AppResult<Option<PublicUser>> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;

    let Some(session_token) = get_state_session_token(state.inner())? else {
        return Ok(None);
    };

    let user = find_user_by_session_token(&connection, &session_token)?;
    if user.is_none() {
        clear_active_session(state.inner())?;
    }

    Ok(user.map(|row| public_user_from_row(&row)))
}

#[tauri::command]
pub(crate) async fn start_steam_auth(state: State<'_, AppState>) -> AppResult<SteamAuthResponse> {
    let db_path = state.db_path.clone();
    let steam_api_key = state.steam_api_key.clone();
    let steam_local_install_detection = state.steam_local_install_detection;
    let steam_root_override = state.steam_root_override.clone();
    let current_session_token = get_state_session_token(state.inner())?;

    let outcome = tauri::async_runtime::spawn_blocking(move || {
        complete_steam_auth_flow(
            &db_path,
            steam_api_key,
            steam_local_install_detection,
            steam_root_override,
            current_session_token,
        )
    })
    .await
    .map_err(|error| format!("Steam auth task failed: {error}"))??;

    persist_active_session(state.inner(), &outcome.session_token)?;

    Ok(SteamAuthResponse {
        user: public_user_from_row(&outcome.user),
        synced_games: outcome.synced_games,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn persist_session_token_file_has_restrictive_permissions() {
        #[cfg(unix)]
        {
            let dir = tempdir().expect("tempdir");
            let db_path = dir.path().join("test.db");
            let session_path = dir.path().join("session.token");
            let state = AppState::new(
                db_path,
                session_path.clone(),
                None,
                false,
                false,
                None,
            );

            // Persist a dummy token
            persist_active_session(&state, "dummy.session.token").expect("persist ok");

            let metadata = fs::metadata(&session_path).expect("session file exists");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = metadata.permissions().mode() & 0o777;
                assert_eq!(mode, 0o600, "session file should be rw-------");
            }
        }
    }
}
