use crate::AppState;
use crate::LibraryResponse;
use crate::SteamDownloadProgressResponse;
use crate::SteamSyncResponse;
use crate::application::error::AppResult;
use crate::application::ports::library::LibraryPort;
use crate::application::services::library_types::GameAchievementsResponse;
use crate::application::services::library_types::GameActivityTimelineResponse;
use crate::application::services::library_types::GameDlcResponse;
use crate::application::services::library_types::GameFriendsActivityResponse;
use crate::application::services::library_types::GameReviewResponse;
use crate::application::services::library_types::GameStoreMetadataResponse;
use crate::application::services::library_types::GameTradingCardsResponse;
use crate::infrastructure::library_port::InfrastructureLibraryPort;

struct LibraryService<P> {
    port: P,
}

impl<P> LibraryService<P>
where
    P: LibraryPort,
{
    fn new(port: P) -> Self {
        Self { port }
    }

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

pub(crate) fn get_library(state: &AppState) -> AppResult<LibraryResponse> {
    LibraryService::new(InfrastructureLibraryPort::new(state)).get_library()
}

pub(crate) fn sync_steam_library(state: &AppState) -> AppResult<SteamSyncResponse> {
    LibraryService::new(InfrastructureLibraryPort::new(state)).sync_steam_library()
}

pub(crate) fn set_game_favorite(
    state: &AppState,
    provider: String,
    external_id: String,
    favorite: bool,
) -> AppResult<()> {
    LibraryService::new(InfrastructureLibraryPort::new(state))
        .set_game_favorite(provider, external_id, favorite)
}

pub(crate) fn get_game_friends_activity(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameFriendsActivityResponse> {
    LibraryService::new(InfrastructureLibraryPort::new(state))
        .get_game_friends_activity(provider, external_id, force_refresh)
}

pub(crate) fn get_game_activity_timeline(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameActivityTimelineResponse> {
    LibraryService::new(InfrastructureLibraryPort::new(state))
        .get_game_activity_timeline(provider, external_id, force_refresh)
}

pub(crate) fn get_game_achievements(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameAchievementsResponse> {
    LibraryService::new(InfrastructureLibraryPort::new(state))
        .get_game_achievements(provider, external_id, force_refresh)
}

pub(crate) fn get_game_trading_cards(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameTradingCardsResponse> {
    LibraryService::new(InfrastructureLibraryPort::new(state))
        .get_game_trading_cards(provider, external_id, force_refresh)
}

pub(crate) fn get_game_dlc(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameDlcResponse> {
    LibraryService::new(InfrastructureLibraryPort::new(state))
        .get_game_dlc(provider, external_id, force_refresh)
}

pub(crate) fn get_game_review(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameReviewResponse> {
    LibraryService::new(InfrastructureLibraryPort::new(state))
        .get_game_review(provider, external_id, force_refresh)
}

pub(crate) fn list_steam_downloads(
    state: &AppState,
) -> AppResult<Vec<SteamDownloadProgressResponse>> {
    LibraryService::new(InfrastructureLibraryPort::new(state)).list_steam_downloads()
}

pub(crate) fn get_game_store_metadata(
    state: &AppState,
    provider: String,
    external_id: String,
) -> AppResult<GameStoreMetadataResponse> {
    LibraryService::new(InfrastructureLibraryPort::new(state))
        .get_game_store_metadata(provider, external_id)
}
