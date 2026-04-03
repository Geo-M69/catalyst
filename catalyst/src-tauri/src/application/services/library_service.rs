use super::super::contracts::library::{
    LibraryResponse,
    SteamDownloadProgressResponse,
    SteamSyncResponse,
};
use super::super::contracts::library::GameAchievementsResponse;
use super::super::contracts::library::GameActivityTimelineResponse;
use super::super::contracts::library::GameDlcResponse;
use super::super::contracts::library::GameFriendsActivityResponse;
use super::super::contracts::library::GameReviewResponse;
use super::super::contracts::library::GameStoreMetadataResponse;
use super::super::contracts::library::GameTradingCardsResponse;
use super::super::error::AppResult;
use super::super::ports::library::LibraryPort;
use super::super::use_cases::library::LibraryUseCase;

pub(crate) struct LibraryService<P> {
    port: P,
}

impl<P> LibraryService<P>
where
    P: LibraryPort,
{
    pub(crate) fn new(port: P) -> Self {
        Self { port }
    }
}

impl<P> LibraryUseCase for LibraryService<P>
where
    P: LibraryPort,
{
    fn get_library(&self) -> AppResult<LibraryResponse> {
        self.port.get_library()
    }

    fn sync_steam_library(&self) -> AppResult<SteamSyncResponse> {
        self.port.sync_steam_library()
    }

    fn set_game_favorite(
        &self,
        provider: String,
        external_id: String,
        favorite: bool,
    ) -> AppResult<()> {
        self.port
            .set_game_favorite(provider, external_id, favorite)
    }

    fn get_game_friends_activity(
        &self,
        provider: String,
        external_id: String,
        force_refresh: bool,
    ) -> AppResult<GameFriendsActivityResponse> {
        self.port
            .get_game_friends_activity(provider, external_id, force_refresh)
    }

    fn get_game_activity_timeline(
        &self,
        provider: String,
        external_id: String,
        force_refresh: bool,
    ) -> AppResult<GameActivityTimelineResponse> {
        self.port
            .get_game_activity_timeline(provider, external_id, force_refresh)
    }

    fn get_game_achievements(
        &self,
        provider: String,
        external_id: String,
        force_refresh: bool,
    ) -> AppResult<GameAchievementsResponse> {
        self.port
            .get_game_achievements(provider, external_id, force_refresh)
    }

    fn get_game_trading_cards(
        &self,
        provider: String,
        external_id: String,
        force_refresh: bool,
    ) -> AppResult<GameTradingCardsResponse> {
        self.port
            .get_game_trading_cards(provider, external_id, force_refresh)
    }

    fn get_game_dlc(
        &self,
        provider: String,
        external_id: String,
        force_refresh: bool,
    ) -> AppResult<GameDlcResponse> {
        self.port.get_game_dlc(provider, external_id, force_refresh)
    }

    fn get_game_review(
        &self,
        provider: String,
        external_id: String,
        force_refresh: bool,
    ) -> AppResult<GameReviewResponse> {
        self.port
            .get_game_review(provider, external_id, force_refresh)
    }

    fn list_steam_downloads(&self) -> AppResult<Vec<SteamDownloadProgressResponse>> {
        self.port.list_steam_downloads()
    }

    fn get_game_store_metadata(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<GameStoreMetadataResponse> {
        self.port.get_game_store_metadata(provider, external_id)
    }
}
