use crate::application::contracts::steam::{
    GameBetaAccessCodeValidationResponse,
    GameVersionBetasResponse,
    SteamCollectionsImportResponse,
};
use crate::application::error::AppResult;
use crate::application::ports::steam::SteamPort;
use crate::application::use_cases::steam::SteamUseCase;

pub(crate) struct SteamService<P> {
    port: P,
}

impl<P> SteamService<P>
where
    P: SteamPort,
{
    pub(crate) fn new(port: P) -> Self {
        Self { port }
    }
}

impl<P> SteamUseCase for SteamService<P>
where
    P: SteamPort,
{
    fn list_game_versions_betas(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<GameVersionBetasResponse> {
        self.port.list_game_versions_betas(provider, external_id)
    }

    fn validate_game_beta_access_code(
        &self,
        provider: String,
        external_id: String,
        access_code: String,
    ) -> AppResult<GameBetaAccessCodeValidationResponse> {
        self.port
            .validate_game_beta_access_code(provider, external_id, access_code)
    }

    fn import_steam_collections(&self) -> AppResult<SteamCollectionsImportResponse> {
        self.port.import_steam_collections()
    }
}
