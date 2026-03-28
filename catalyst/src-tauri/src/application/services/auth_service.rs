use crate::AppState;
use crate::PublicUser;
use crate::SteamAuthResponse;
use crate::application::error::{AppError, AppResult};
use crate::application::ports::auth::AuthPort;
use crate::infrastructure::auth_port::InfrastructureAuthPort;

#[derive(Clone)]
struct AuthService<P> {
    port: P,
}

impl<P> AuthService<P>
where
    P: AuthPort,
{
    fn new(port: P) -> Self {
        Self { port }
    }

    fn logout(&self) -> AppResult<()> {
        let session_token = self.port.get_state_session_token()?;
        self.port.cleanup_expired_sessions()?;

        if let Some(token) = session_token {
            self.port.invalidate_session_by_token(&token)?;
        }

        self.port.clear_active_session()?;
        Ok(())
    }

    fn get_session(&self) -> AppResult<Option<PublicUser>> {
        self.port.cleanup_expired_sessions()?;

        let Some(session_token) = self.port.get_state_session_token()? else {
            return Ok(None);
        };

        let user = self.port.find_user_by_session_token(&session_token)?;
        if user.is_none() {
            self.port.clear_active_session()?;
        }

        Ok(user)
    }

    async fn start_steam_auth(&self) -> AppResult<SteamAuthResponse> {
        let current_session_token = self.port.get_state_session_token()?;
        let task_port = self.port.clone();
        let outcome = tauri::async_runtime::spawn_blocking(move || {
            task_port.complete_steam_auth_flow(current_session_token)
        })
        .await
        .map_err(|error| {
            AppError::internal(
                "steam_auth_task_failed",
                format!("Steam auth task failed: {error}"),
            )
        })??;

        self.port.persist_active_session(&outcome.session_token)?;
        Ok(SteamAuthResponse {
            user: outcome.user,
            synced_games: outcome.synced_games,
        })
    }
}

pub(crate) fn logout(state: &AppState) -> AppResult<()> {
    AuthService::new(InfrastructureAuthPort::new(state)).logout()
}

pub(crate) fn get_session(state: &AppState) -> AppResult<Option<PublicUser>> {
    AuthService::new(InfrastructureAuthPort::new(state)).get_session()
}

pub(crate) async fn start_steam_auth(state: &AppState) -> AppResult<SteamAuthResponse> {
    AuthService::new(InfrastructureAuthPort::new(state))
        .start_steam_auth()
        .await
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
            let port = InfrastructureAuthPort::new(&state);
            port.persist_active_session("dummy.session.token")
                .expect("persist ok");

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
