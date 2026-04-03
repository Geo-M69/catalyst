use crate::application::contracts::auth::PublicUser;
use crate::application::error::AppResult;

#[derive(Clone)]
pub(crate) struct SteamAuthFlowOutcome {
    pub(crate) user: PublicUser,
    pub(crate) synced_games: usize,
    pub(crate) session_token: String,
}

pub(crate) trait AuthPort: Clone + Send + 'static {
    fn get_state_session_token(&self) -> AppResult<Option<String>>;
    fn clear_active_session(&self) -> AppResult<()>;
    fn persist_active_session(&self, session_token: &str) -> AppResult<()>;
    fn cleanup_expired_sessions(&self) -> AppResult<()>;
    fn invalidate_session_by_token(&self, session_token: &str) -> AppResult<()>;
    fn find_user_by_session_token(&self, session_token: &str) -> AppResult<Option<PublicUser>>;
    fn complete_steam_auth_flow(
        &self,
        current_session_token: Option<String>,
    ) -> AppResult<SteamAuthFlowOutcome>;
}
