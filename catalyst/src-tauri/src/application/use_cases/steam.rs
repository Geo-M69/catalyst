use crate::application::contracts::steam::{
    GameBetaAccessCodeValidationResponse,
    GameVersionBetasResponse,
    SteamCollectionsImportResponse,
};
use crate::application::error::AppResult;

pub(crate) trait SteamUseCase {
    fn list_game_versions_betas(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<GameVersionBetasResponse>;

    fn validate_game_beta_access_code(
        &self,
        provider: String,
        external_id: String,
        access_code: String,
    ) -> AppResult<GameBetaAccessCodeValidationResponse>;

    fn import_steam_collections(&self) -> AppResult<SteamCollectionsImportResponse>;
}
