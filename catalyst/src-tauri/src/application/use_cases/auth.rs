use crate::application::contracts::auth::{PublicUser, SteamAuthResponse};
use crate::application::error::AppResult;

pub(crate) trait AuthUseCase {
    fn logout(&self) -> AppResult<()>;
    fn get_session(&self) -> AppResult<Option<PublicUser>>;
    fn start_steam_auth(&self) -> AppResult<SteamAuthResponse>;
}
