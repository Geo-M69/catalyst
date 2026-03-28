use crate::AppState;
use crate::application::error::{AppError, AppResult};
use crate::application::ports::auth::{AuthPort, SteamAuthFlowOutcome};

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

    fn cleanup_expired_sessions(&self) -> AppResult<()> {
        let connection = crate::open_connection(&self.state.db_path).map_err(AppError::from)?;
        crate::cleanup_expired_sessions(&connection).map_err(AppError::from)
    }

    fn invalidate_session_by_token(&self, session_token: &str) -> AppResult<()> {
        let connection = crate::open_connection(&self.state.db_path).map_err(AppError::from)?;
        crate::invalidate_session_by_token(&connection, session_token).map_err(AppError::from)
    }

    fn find_user_by_session_token(&self, session_token: &str) -> AppResult<Option<crate::PublicUser>> {
        let connection = crate::open_connection(&self.state.db_path).map_err(AppError::from)?;
        let user = crate::find_user_by_session_token(&connection, session_token).map_err(AppError::from)?;
        Ok(user.map(|row| crate::public_user_from_row(&row)))
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
            user: crate::public_user_from_row(&outcome.user),
            synced_games: outcome.synced_games,
            session_token: outcome.session_token,
        })
    }
}
