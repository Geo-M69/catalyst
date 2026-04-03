use crate::application::contracts::library::{
    LibraryResponse,
    SteamDownloadProgressResponse,
    SteamSyncResponse,
};
use crate::application::error::AppResult;
use crate::application::ports::library::LibraryPort;
use crate::application::contracts::library::GameAchievementsResponse;
use crate::application::contracts::library::GameActivityTimelineResponse;
use crate::application::contracts::library::GameDlcResponse;
use crate::application::contracts::library::GameFriendsActivityResponse;
use crate::application::contracts::library::GameReviewResponse;
use crate::application::contracts::library::GameStoreMetadataResponse;
use crate::application::contracts::library::GameTradingCardsResponse;
use crate::application::use_cases::library::LibraryUseCase;

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
