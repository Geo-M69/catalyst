use crate::application::contracts::auth::PublicUser;
use crate::application::error::{AppError, AppResult};
use crate::application::ports::auth::{AuthPort, SteamAuthFlowOutcome};
use crate::AppState;

#[derive(Clone)]
pub(crate) struct InfrastructureAuthPort {
    state: AppState,
}

impl InfrastructureAuthPort {
    pub(crate) fn new(state: &AppState) -> Self {
        Self {
            state: state.clone(),
        }
    }

    fn to_contract_public_user(value: crate::PublicUser) -> PublicUser {
        PublicUser {
            id: value.id,
            email: value.email,
            steam_linked: value.steam_linked,
            steam_id: value.steam_id,
        }
    }
}

impl AuthPort for InfrastructureAuthPort {
    fn get_state_session_token(&self) -> AppResult<Option<String>> {
        crate::get_state_session_token(&self.state).map_err(AppError::from)
    }

    fn clear_active_session(&self) -> AppResult<()> {
        crate::clear_active_session(&self.state).map_err(AppError::from)
    }

    fn persist_active_session(&self, session_token: &str) -> AppResult<()> {
        crate::persist_active_session(&self.state, session_token).map_err(AppError::from)
    }

    fn bootstrap_local_session(&self) -> AppResult<Option<PublicUser>> {
        let user = crate::bootstrap_local_session(&self.state).map_err(AppError::from)?;
        Ok(user.map(|row| Self::to_contract_public_user(crate::public_user_from_row(&row))))
    }

    fn cleanup_expired_sessions(&self) -> AppResult<()> {
        let connection = crate::open_connection(&self.state.db_path).map_err(AppError::from)?;
        crate::cleanup_expired_sessions(&connection).map_err(AppError::from)
    }

    fn invalidate_session_by_token(&self, session_token: &str) -> AppResult<()> {
        let connection = crate::open_connection(&self.state.db_path).map_err(AppError::from)?;
        crate::invalidate_session_by_token(&connection, session_token).map_err(AppError::from)
    }

    fn find_user_by_session_token(&self, session_token: &str) -> AppResult<Option<PublicUser>> {
        let connection = crate::open_connection(&self.state.db_path).map_err(AppError::from)?;
        let user = crate::find_user_by_session_token(&connection, session_token)
            .map_err(AppError::from)?;
        Ok(user.map(|row| Self::to_contract_public_user(crate::public_user_from_row(&row))))
    }

    fn complete_steam_auth_flow(
        &self,
        current_session_token: Option<String>,
    ) -> AppResult<SteamAuthFlowOutcome> {
        let outcome = crate::complete_steam_auth_flow(
            &self.state.db_path,
            self.state.steam_api_key.clone(),
            self.state.steam_local_install_detection,
            self.state.steam_root_override.clone(),
            current_session_token,
        )
        .map_err(AppError::from)?;

        Ok(SteamAuthFlowOutcome {
            user: Self::to_contract_public_user(crate::public_user_from_row(&outcome.user)),
            synced_games: outcome.synced_games,
            session_token: outcome.session_token,
        })
    }
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
