use super::super::contracts::auth::{PublicUser, SteamAuthResponse};
use super::super::error::AppResult;
use super::super::ports::auth::AuthPort;
use super::super::use_cases::auth::AuthUseCase;

#[derive(Clone)]
pub(crate) struct AuthService<P> {
    port: P,
}

impl<P> AuthService<P>
where
    P: AuthPort,
{
    pub(crate) fn new(port: P) -> Self {
        Self { port }
    }
}

impl<P> AuthUseCase for AuthService<P>
where
    P: AuthPort,
{
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

    fn start_steam_auth(&self) -> AppResult<SteamAuthResponse> {
        let current_session_token = self.port.get_state_session_token()?;
        let outcome = self.port.complete_steam_auth_flow(current_session_token)?;

        self.port.persist_active_session(&outcome.session_token)?;
        Ok(SteamAuthResponse {
            user: outcome.user,
            synced_games: outcome.synced_games,
        })
    }
}
