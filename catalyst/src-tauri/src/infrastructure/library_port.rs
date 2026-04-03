use crate::application::contracts::library::{
    FeatureResponse,
    GameAchievementsResponse,
    GameActivityTimelineResponse,
    GameDlcResponse,
    GameFriendsActivityResponse,
    GameResponse,
    GameReviewResponse,
    GameStoreMetadataResponse,
    GameTradingCardsResponse,
    LibraryResponse,
    SteamDownloadProgressResponse,
    SteamSyncResponse,
};
use crate::application::error::AppResult;
use crate::application::ports::library::LibraryPort;
use crate::{
    AppState,
    build_http_client,
    cleanup_expired_sessions,
    ensure_owned_game_exists,
    get_authenticated_user,
    list_games_by_user,
    normalize_game_identity_input,
    open_connection,
    remove_game_favorite,
    sync_steam_games_for_user,
    upsert_game_favorite,
};

#[derive(Clone)]
pub(crate) struct InfrastructureLibraryPort {
    state: AppState,
}

impl InfrastructureLibraryPort {
    pub(crate) fn new(state: &AppState) -> Self {
        Self {
            state: state.clone(),
        }
    }

    fn to_contract_feature(value: crate::FeatureResponse) -> FeatureResponse {
        FeatureResponse {
            key: value.key,
            label: value.label,
            icon: value.icon,
            tooltip: value.tooltip,
        }
    }

    fn to_contract_game(value: crate::GameResponse) -> GameResponse {
        GameResponse {
            id: value.id,
            provider: value.provider,
            external_id: value.external_id,
            name: value.name,
            kind: value.kind,
            playtime_minutes: value.playtime_minutes,
            installed: value.installed,
            artwork_url: value.artwork_url,
            last_synced_at: value.last_synced_at,
            last_played_at: value.last_played_at,
            favorite: value.favorite,
            steam_tags: value.steam_tags,
            genres: value.genres,
            collections: value.collections,
            hide_in_library: value.hide_in_library,
            developers: value.developers,
            publishers: value.publishers,
            franchise: value.franchise,
            release_date: value.release_date,
            short_description: value.short_description,
            header_image: value.header_image,
            has_achievements: value.has_achievements,
            has_cloud_saves: value.has_cloud_saves,
            controller_support: value.controller_support,
            achievements_count: value.achievements_count,
            cloud_details: value.cloud_details,
            features: value
                .features
                .into_iter()
                .map(Self::to_contract_feature)
                .collect(),
        }
    }

    fn to_contract_download(
        value: crate::SteamDownloadProgressResponse,
    ) -> SteamDownloadProgressResponse {
        SteamDownloadProgressResponse {
            game_id: value.game_id,
            provider: value.provider,
            external_id: value.external_id,
            name: value.name,
            state: value.state,
            bytes_downloaded: value.bytes_downloaded,
            bytes_total: value.bytes_total,
            progress_percent: value.progress_percent,
            progress_source: value.progress_source,
        }
    }
}

impl LibraryPort for InfrastructureLibraryPort {
    fn get_library(&self) -> AppResult<LibraryResponse> {
        self::get_library(&self.state)
    }

    fn sync_steam_library(&self) -> AppResult<SteamSyncResponse> {
        self::sync_steam_library(&self.state)
    }

    fn set_game_favorite(
        &self,
        provider: String,
        external_id: String,
        favorite: bool,
    ) -> AppResult<()> {
        self::set_game_favorite(&self.state, provider, external_id, favorite)
    }

    fn get_game_friends_activity(
        &self,
        provider: String,
        external_id: String,
        force_refresh: bool,
    ) -> AppResult<GameFriendsActivityResponse> {
        self::get_game_friends_activity(&self.state, provider, external_id, force_refresh)
    }

    fn get_game_activity_timeline(
        &self,
        provider: String,
        external_id: String,
        force_refresh: bool,
    ) -> AppResult<GameActivityTimelineResponse> {
        self::get_game_activity_timeline(&self.state, provider, external_id, force_refresh)
    }

    fn get_game_achievements(
        &self,
        provider: String,
        external_id: String,
        force_refresh: bool,
    ) -> AppResult<GameAchievementsResponse> {
        self::get_game_achievements(&self.state, provider, external_id, force_refresh)
    }

    fn get_game_trading_cards(
        &self,
        provider: String,
        external_id: String,
        force_refresh: bool,
    ) -> AppResult<GameTradingCardsResponse> {
        self::get_game_trading_cards(&self.state, provider, external_id, force_refresh)
    }

    fn get_game_dlc(
        &self,
        provider: String,
        external_id: String,
        force_refresh: bool,
    ) -> AppResult<GameDlcResponse> {
        self::get_game_dlc(&self.state, provider, external_id, force_refresh)
    }

    fn get_game_review(
        &self,
        provider: String,
        external_id: String,
        force_refresh: bool,
    ) -> AppResult<GameReviewResponse> {
        self::get_game_review(&self.state, provider, external_id, force_refresh)
    }

    fn list_steam_downloads(&self) -> AppResult<Vec<SteamDownloadProgressResponse>> {
        self::list_steam_downloads(&self.state)
    }

    fn get_game_store_metadata(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<GameStoreMetadataResponse> {
        self::get_game_store_metadata(&self.state, provider, external_id)
    }
}

pub(crate) fn get_library(state: &AppState) -> AppResult<LibraryResponse> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state, &connection)?;
    let games = list_games_by_user(&connection, &user.id)?;

    Ok(LibraryResponse {
        user_id: user.id,
        total: games.len(),
        games: games
            .into_iter()
            .map(InfrastructureLibraryPort::to_contract_game)
            .collect(),
    })
}

pub(crate) fn sync_steam_library(state: &AppState) -> AppResult<SteamSyncResponse> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state, &connection)?;
    let client = build_http_client()?;
    let synced_games = sync_steam_games_for_user(
        &connection,
        &user,
        state.steam_api_key.as_deref(),
        state.steam_local_install_detection,
        state.steam_root_override.as_deref(),
        &client,
    )?;

    Ok(SteamSyncResponse {
        user_id: user.id,
        provider: String::from("steam"),
        synced_games,
    })
}

pub(crate) fn set_game_favorite(
    state: &AppState,
    provider: String,
    external_id: String,
    favorite: bool,
) -> AppResult<()> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state, &connection)?;
    let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;

    if favorite {
        upsert_game_favorite(&connection, &user.id, &provider, &external_id)?;
    } else {
        remove_game_favorite(&connection, &user.id, &provider, &external_id)?;
    }

    Ok(())
}

pub(crate) fn get_game_friends_activity(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameFriendsActivityResponse> {
    crate::infrastructure::library_steam_social::get_game_friends_activity(
        state,
        provider,
        external_id,
        force_refresh,
    )
}

pub(crate) fn get_game_activity_timeline(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameActivityTimelineResponse> {
    crate::infrastructure::library_steam_social::get_game_activity_timeline(
        state,
        provider,
        external_id,
        force_refresh,
    )
}

pub(crate) fn get_game_achievements(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameAchievementsResponse> {
    crate::infrastructure::library_steam_progress::get_game_achievements(
        state,
        provider,
        external_id,
        force_refresh,
    )
}

pub(crate) fn get_game_trading_cards(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameTradingCardsResponse> {
    crate::infrastructure::library_steam_progress::get_game_trading_cards(
        state,
        provider,
        external_id,
        force_refresh,
    )
}

pub(crate) fn get_game_dlc(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameDlcResponse> {
    crate::infrastructure::library_steam_dlc::get_game_dlc(state, provider, external_id, force_refresh)
}

pub(crate) fn get_game_review(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameReviewResponse> {
    crate::infrastructure::library_steam_review::get_game_review(
        state,
        provider,
        external_id,
        force_refresh,
    )
}

pub(crate) fn list_steam_downloads(
    state: &AppState,
) -> AppResult<Vec<SteamDownloadProgressResponse>> {
    let downloads = crate::infrastructure::library_steam_downloads::list_steam_downloads(state)?;
    Ok(downloads
        .into_iter()
        .map(InfrastructureLibraryPort::to_contract_download)
        .collect())
}

pub(crate) fn get_game_store_metadata(
    state: &AppState,
    provider: String,
    external_id: String,
) -> AppResult<GameStoreMetadataResponse> {
    crate::infrastructure::library_steam_store_metadata::get_game_store_metadata(
        state,
        provider,
        external_id,
    )
}
