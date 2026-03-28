use crate::AppState;
use crate::GameBetaAccessCodeValidationResponse;
use crate::GameVersionBetasResponse;
use crate::SteamCollectionsImportResponse;
use crate::application::error::AppResult;
use crate::application::ports::steam::SteamPort;
use crate::infrastructure::steam_port::InfrastructureSteamPort;

struct SteamService<P> {
    port: P,
}

impl<P> SteamService<P>
where
    P: SteamPort,
{
    fn new(port: P) -> Self {
        Self { port }
    }

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

pub(crate) fn list_game_versions_betas(
    state: &AppState,
    provider: String,
    external_id: String,
) -> AppResult<GameVersionBetasResponse> {
    SteamService::new(InfrastructureSteamPort::new(state))
        .list_game_versions_betas(provider, external_id)
}

pub(crate) fn validate_game_beta_access_code(
    state: &AppState,
    provider: String,
    external_id: String,
    access_code: String,
) -> AppResult<GameBetaAccessCodeValidationResponse> {
    SteamService::new(InfrastructureSteamPort::new(state)).validate_game_beta_access_code(
        provider,
        external_id,
        access_code,
    )
}

pub(crate) fn import_steam_collections(
    state: &AppState,
) -> AppResult<SteamCollectionsImportResponse> {
    SteamService::new(InfrastructureSteamPort::new(state)).import_steam_collections()
}
