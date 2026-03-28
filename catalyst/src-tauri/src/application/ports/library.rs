use crate::LibraryResponse;
use crate::SteamDownloadProgressResponse;
use crate::SteamSyncResponse;
use crate::application::error::AppResult;
use crate::application::services::library_types::GameAchievementsResponse;
use crate::application::services::library_types::GameActivityTimelineResponse;
use crate::application::services::library_types::GameDlcResponse;
use crate::application::services::library_types::GameFriendsActivityResponse;
use crate::application::services::library_types::GameReviewResponse;
use crate::application::services::library_types::GameStoreMetadataResponse;
use crate::application::services::library_types::GameTradingCardsResponse;

pub(crate) trait LibraryPort {
    fn get_library(&self) -> AppResult<LibraryResponse>;
    fn sync_steam_library(&self) -> AppResult<SteamSyncResponse>;
    fn set_game_favorite(
        &self,
        provider: String,
        external_id: String,
        favorite: bool,
    ) -> AppResult<()>;
    fn get_game_friends_activity(
        &self,
        provider: String,
        external_id: String,
        force_refresh: bool,
    ) -> AppResult<GameFriendsActivityResponse>;
    fn get_game_activity_timeline(
        &self,
        provider: String,
        external_id: String,
        force_refresh: bool,
    ) -> AppResult<GameActivityTimelineResponse>;
    fn get_game_achievements(
        &self,
        provider: String,
        external_id: String,
        force_refresh: bool,
    ) -> AppResult<GameAchievementsResponse>;
    fn get_game_trading_cards(
        &self,
        provider: String,
        external_id: String,
        force_refresh: bool,
    ) -> AppResult<GameTradingCardsResponse>;
    fn get_game_dlc(
        &self,
        provider: String,
        external_id: String,
        force_refresh: bool,
    ) -> AppResult<GameDlcResponse>;
    fn get_game_review(
        &self,
        provider: String,
        external_id: String,
        force_refresh: bool,
    ) -> AppResult<GameReviewResponse>;
    fn list_steam_downloads(&self) -> AppResult<Vec<SteamDownloadProgressResponse>>;
    fn get_game_store_metadata(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<GameStoreMetadataResponse>;
}
