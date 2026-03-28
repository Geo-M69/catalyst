use crate::application::error::AppResult;
use crate::{
    AppState,
    PublicUser,
    SteamAuthResponse,
    cleanup_expired_sessions,
    clear_active_session,
    complete_steam_auth_flow,
    find_user_by_session_token,
    get_state_session_token,
    invalidate_session_by_token,
    open_connection,
    persist_active_session,
    public_user_from_row,
};

pub(crate) fn logout(state: &AppState) -> AppResult<()> {
    let session_token = get_state_session_token(state)?;
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;

    if let Some(token) = session_token {
        invalidate_session_by_token(&connection, &token)?;
    }

    clear_active_session(state)?;
    Ok(())
}

pub(crate) fn get_session(state: &AppState) -> AppResult<Option<PublicUser>> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;

    let Some(session_token) = get_state_session_token(state)? else {
        return Ok(None);
    };

    let user = find_user_by_session_token(&connection, &session_token)?;
    if user.is_none() {
        clear_active_session(state)?;
    }

    Ok(user.map(|row| public_user_from_row(&row)))
}

pub(crate) async fn start_steam_auth(state: &AppState) -> AppResult<SteamAuthResponse> {
    let db_path = state.db_path.clone();
    let steam_api_key = state.steam_api_key.clone();
    let steam_local_install_detection = state.steam_local_install_detection;
    let steam_root_override = state.steam_root_override.clone();
    let current_session_token = get_state_session_token(state)?;

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

    persist_active_session(state, &outcome.session_token)?;

    Ok(SteamAuthResponse {
        user: public_user_from_row(&outcome.user),
        synced_games: outcome.synced_games,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn persist_session_token_file_has_restrictive_permissions() {
        #[cfg(unix)]
        {
            let dir = tempdir().expect("tempdir");
            let db_path = dir.path().join("test.db");
            let session_path = dir.path().join("session.token");
            let state = AppState::new(db_path, session_path.clone(), None, false, false, None);

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
