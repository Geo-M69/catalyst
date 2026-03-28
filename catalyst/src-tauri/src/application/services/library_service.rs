use crate::application::error::AppResult;
use crate::infrastructure::cache_adapter::CacheAdapter;
use crate::{
	AppState,
	FeatureResponse,
	LibraryResponse,
	STEAM_APP_DETAILS_CACHE_TTL_HOURS,
	SteamDownloadProgressResponse,
	SteamSyncResponse,
	build_http_client,
	cache_steam_app_details,
	cleanup_expired_sessions,
	collect_steam_download_progress_from_steamapps_dir,
	ensure_owned_game_exists,
	find_cached_steam_app_details,
	find_cached_steam_app_features,
	get_authenticated_user,
	list_games_by_user,
	load_owned_steam_games_by_app_id,
	normalize_backend_warning_message,
	normalize_game_identity_input,
	open_connection,
	remove_game_favorite,
	resolve_steam_root_paths,
	resolve_steamapps_directories,
	sync_steam_games_for_user,
	upsert_game_favorite,
};
use chrono::{Duration as ChronoDuration, Utc};
use rusqlite::{Connection, params};
use scraper::{Html, Selector};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use url::Url;

const STEAM_WEB_API_FRIEND_LIST_ENDPOINT: &str =
    "https://api.steampowered.com/ISteamUser/GetFriendList/v1/";
const STEAM_WEB_API_PLAYER_SUMMARIES_ENDPOINT: &str =
    "https://api.steampowered.com/ISteamUser/GetPlayerSummaries/v2/";
const STEAM_WEB_API_OWNED_GAMES_ENDPOINT: &str =
    "https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/";
const STEAM_WEB_API_NEWS_FOR_APP_ENDPOINT: &str =
    "https://api.steampowered.com/ISteamNews/GetNewsForApp/v2/";
const STEAM_WEB_API_PLAYER_ACHIEVEMENTS_ENDPOINT: &str =
    "https://api.steampowered.com/ISteamUserStats/GetPlayerAchievements/v1/";
const STEAM_WEB_API_GAME_SCHEMA_ENDPOINT: &str =
    "https://api.steampowered.com/ISteamUserStats/GetSchemaForGame/v2/";
const STEAM_WEB_API_BADGES_ENDPOINT: &str =
    "https://api.steampowered.com/IPlayerService/GetBadges/v1/";
const STEAM_APP_REVIEWS_ENDPOINT: &str = "https://store.steampowered.com/appreviews/";
const STEAM_FRIENDS_ACTIVITY_CACHE_TTL_SECONDS: i64 = 15 * 60;
const STEAM_FRIENDS_ACTIVITY_MAX_FRIENDS_TO_SCAN: usize = 48;
const STEAM_ACTIVITY_TIMELINE_CACHE_TTL_SECONDS: i64 = 15 * 60;
const STEAM_ACTIVITY_TIMELINE_MAX_NEWS_ITEMS: usize = 12;
const STEAM_ACTIVITY_TIMELINE_MAX_ACHIEVEMENTS: usize = 12;
const STEAM_ACTIVITY_TIMELINE_MAX_ITEMS: usize = 24;
const STEAM_ACTIVITY_TIMELINE_MAX_NEWS_PAGE_IMAGE_LOOKUPS: usize = 6;
const STEAM_ACTIVITY_TIMELINE_CACHE_VERSION: &str = "v2";
const STEAM_ACHIEVEMENTS_CACHE_TTL_SECONDS: i64 = 15 * 60;
const STEAM_ACHIEVEMENTS_CACHE_VERSION: &str = "v1";
const STEAM_TRADING_CARDS_CACHE_TTL_SECONDS: i64 = 15 * 60;
const STEAM_TRADING_CARDS_CACHE_VERSION: &str = "v1";
const STEAM_REVIEW_CACHE_TTL_SECONDS: i64 = 15 * 60;
const STEAM_REVIEW_CACHE_VERSION: &str = "v2";
const STEAM_CLAN_IMAGE_PLACEHOLDER_MARKER: &str = "{steam_clan_image}/";
const STEAM_CLAN_IMAGE_CDN_BASE: &str = "https://clan.akamai.steamstatic.com/images/";

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameStoreMetadataResponse {
    pub developers: Option<Vec<String>>,
    pub publishers: Option<Vec<String>>,
    pub franchise: Option<String>,
    pub release_date: Option<String>,
    pub short_description: Option<String>,
    pub header_image: Option<String>,
    pub has_achievements: Option<bool>,
    pub achievements_count: Option<i64>,
    pub has_cloud_saves: Option<bool>,
    pub cloud_details: Option<String>,
    pub controller_support: Option<String>,
    pub features: Option<Vec<FeatureResponse>>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameFriendActivityEntryResponse {
    pub steam_id: String,
    pub persona_name: String,
    pub avatar_url: Option<String>,
    pub profile_url: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameFriendsActivityResponse {
    pub provider: String,
    pub external_id: String,
    pub played_friends: Vec<GameFriendActivityEntryResponse>,
    pub owned_friends: Vec<GameFriendActivityEntryResponse>,
    pub friend_list_visibility: String,
    pub warning: Option<String>,
    pub last_synced_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameActivityTimelineItemResponse {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub url: Option<String>,
    pub source_label: Option<String>,
    pub presentation: Option<String>,
    pub occurred_at: String,
    pub is_major_update: Option<bool>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameActivityTimelineResponse {
    pub provider: String,
    pub external_id: String,
    pub items: Vec<GameActivityTimelineItemResponse>,
    pub warning: Option<String>,
    pub last_synced_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameAchievementEntryResponse {
    pub api_name: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub unlocked: bool,
    pub unlocked_at: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameAchievementsResponse {
    pub provider: String,
    pub external_id: String,
    pub total: i64,
    pub unlocked_count: i64,
    pub percent: Option<f64>,
    pub entries: Vec<GameAchievementEntryResponse>,
    pub warning: Option<String>,
    pub last_synced_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameTradingCardEntryResponse {
    pub id: String,
    pub name: String,
    pub image_url: Option<String>,
    pub owned_count: i64,
    pub is_owned: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameTradingCardsResponse {
    pub provider: String,
    pub external_id: String,
    pub supported: bool,
    pub badge_level: Option<i64>,
    pub badge_xp: Option<i64>,
    pub total_cards: i64,
    pub owned_cards: i64,
    pub cards: Vec<GameTradingCardEntryResponse>,
    pub warning: Option<String>,
    pub view_url: String,
    pub last_synced_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameDlcEntryResponse {
    pub id: String,
    pub provider: String,
    pub external_id: String,
    pub name: String,
    pub installed: bool,
    pub in_library: bool,
    pub store_url: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameDlcResponse {
    pub provider: String,
    pub external_id: String,
    pub entries: Vec<GameDlcEntryResponse>,
    pub warning: Option<String>,
    pub last_synced_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameReviewEntryResponse {
    pub id: String,
    pub recommended: bool,
    pub text: String,
    pub playtime_minutes: i64,
    pub created_at: String,
    pub likes: i64,
    pub comments: i64,
    pub source: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameReviewResponse {
    pub provider: String,
    pub external_id: String,
    pub review: Option<GameReviewEntryResponse>,
    pub warning: Option<String>,
    pub last_synced_at: String,
}

#[derive(Clone, Copy)]
enum SteamFriendListVisibility {
    Public,
    Private,
    Unknown,
}

impl SteamFriendListVisibility {
    fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
            Self::Unknown => "unknown",
        }
    }
}

struct SteamFriendListOutcome {
    friend_ids: Vec<String>,
    visibility: SteamFriendListVisibility,
    warning: Option<String>,
}

#[derive(serde::Deserialize)]
struct SteamFriendListApiResponse {
    friendslist: Option<SteamFriendListPayload>,
}

#[derive(serde::Deserialize)]
struct SteamFriendListPayload {
    #[serde(default)]
    friends: Vec<SteamFriendListEntry>,
}

#[derive(serde::Deserialize)]
struct SteamFriendListEntry {
    steamid: String,
    relationship: Option<String>,
}

#[derive(serde::Deserialize)]
struct SteamPlayerSummariesApiResponse {
    response: Option<SteamPlayerSummariesPayload>,
}

#[derive(serde::Deserialize)]
struct SteamPlayerSummariesPayload {
    #[serde(default)]
    players: Vec<SteamPlayerSummary>,
}

#[derive(serde::Deserialize, Clone)]
struct SteamPlayerSummary {
    steamid: String,
    personaname: Option<String>,
    avatarfull: Option<String>,
    profileurl: Option<String>,
}

#[derive(serde::Deserialize)]
struct SteamOwnedGamesApiResponseLite {
    response: Option<SteamOwnedGamesPayloadLite>,
}

#[derive(serde::Deserialize)]
struct SteamOwnedGamesPayloadLite {
    game_count: Option<u64>,
    #[serde(default)]
    games: Vec<SteamOwnedGameLite>,
}

#[derive(serde::Deserialize)]
struct SteamOwnedGameLite {
    appid: u64,
    playtime_forever: Option<u64>,
}

#[derive(serde::Deserialize)]
struct SteamAppReviewsApiResponse {
    success: Option<i64>,
    cursor: Option<String>,
    #[serde(default)]
    reviews: Vec<SteamAppReviewEntry>,
}

#[derive(serde::Deserialize)]
struct SteamAppReviewEntry {
    recommendationid: Option<String>,
    author: Option<SteamAppReviewAuthor>,
    review: Option<String>,
    timestamp_created: Option<i64>,
    voted_up: Option<bool>,
    votes_up: Option<i64>,
    comment_count: Option<i64>,
}

#[derive(serde::Deserialize)]
struct SteamAppReviewAuthor {
    steamid: Option<String>,
    playtime_at_review: Option<i64>,
}

#[derive(serde::Deserialize)]
struct SteamBadgesApiResponse {
    response: Option<SteamBadgesPayload>,
}

#[derive(serde::Deserialize)]
struct SteamBadgesPayload {
    #[serde(default)]
    badges: Vec<SteamBadgeEntry>,
}

#[derive(serde::Deserialize)]
struct SteamBadgeEntry {
    appid: Option<u64>,
    level: Option<i64>,
    xp: Option<i64>,
}

#[derive(Clone, Copy)]
struct FriendOwnedGameStatus {
    owns: bool,
    played: bool,
}

#[derive(Clone)]
struct SteamAchievementSchemaEntry {
    display_name: Option<String>,
    description: Option<String>,
    icon: Option<String>,
}

#[derive(serde::Deserialize)]
struct SteamNewsForAppApiResponse {
    appnews: Option<SteamNewsForAppPayload>,
}

#[derive(serde::Deserialize)]
struct SteamNewsForAppPayload {
    #[serde(default)]
    newsitems: Vec<serde_json::Value>,
}

#[derive(serde::Deserialize)]
struct SteamPlayerAchievementsApiResponse {
    playerstats: Option<SteamPlayerAchievementsPayload>,
}

#[derive(serde::Deserialize)]
struct SteamPlayerAchievementsPayload {
    success: Option<bool>,
    #[serde(default)]
    achievements: Vec<SteamPlayerAchievementEntry>,
}

#[derive(serde::Deserialize)]
struct SteamPlayerAchievementEntry {
    apiname: Option<String>,
    achieved: Option<u8>,
    unlocktime: Option<i64>,
}

#[derive(serde::Deserialize)]
struct SteamGameSchemaApiResponse {
    game: Option<SteamGameSchemaPayload>,
}

#[derive(serde::Deserialize)]
struct SteamGameSchemaPayload {
    #[serde(rename = "availableGameStats")]
    available_game_stats: Option<SteamGameSchemaStats>,
}

#[derive(serde::Deserialize)]
struct SteamGameSchemaStats {
    #[serde(default)]
    achievements: Vec<SteamGameSchemaAchievement>,
}

#[derive(serde::Deserialize)]
struct SteamGameSchemaAchievement {
    name: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    description: Option<String>,
    icon: Option<String>,
}

// FeatureResponse is defined in crate root (`lib.rs`) so it can be shared across responses.

pub(crate) fn get_library(state: &AppState) -> AppResult<LibraryResponse> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state, &connection)?;
    let games = list_games_by_user(&connection, &user.id)?;

    // (removed debug log)

    Ok(LibraryResponse {
        user_id: user.id,
        total: games.len(),
        games,
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

fn empty_game_friends_activity_response(
    provider: &str,
    external_id: &str,
    friend_list_visibility: SteamFriendListVisibility,
    warning: Option<String>,
) -> GameFriendsActivityResponse {
    GameFriendsActivityResponse {
        provider: provider.to_owned(),
        external_id: external_id.to_owned(),
        played_friends: Vec::new(),
        owned_friends: Vec::new(),
        friend_list_visibility: friend_list_visibility.as_str().to_owned(),
        warning,
        last_synced_at: Utc::now().to_rfc3339(),
    }
}

fn append_warning(existing_warning: &mut Option<String>, next_warning: impl Into<String>) {
    let next_warning = next_warning.into();
    let next_warning = next_warning.trim();
    if next_warning.is_empty() {
        return;
    }

    match existing_warning {
        Some(current) => {
            if !current.is_empty() && !current.ends_with(' ') {
                current.push(' ');
            }
            current.push_str(next_warning);
        }
        None => {
            *existing_warning = Some(next_warning.to_owned());
        }
    }
}

fn empty_game_trading_cards_response(
    provider: &str,
    external_id: &str,
    supported: bool,
    view_url: String,
    warning: Option<String>,
) -> GameTradingCardsResponse {
    GameTradingCardsResponse {
        provider: provider.to_owned(),
        external_id: external_id.to_owned(),
        supported,
        badge_level: None,
        badge_xp: None,
        total_cards: 0,
        owned_cards: 0,
        cards: Vec::new(),
        warning,
        view_url,
        last_synced_at: Utc::now().to_rfc3339(),
    }
}

fn empty_game_dlc_response(
    provider: &str,
    external_id: &str,
    warning: Option<String>,
) -> GameDlcResponse {
    GameDlcResponse {
        provider: provider.to_owned(),
        external_id: external_id.to_owned(),
        entries: Vec::new(),
        warning,
        last_synced_at: Utc::now().to_rfc3339(),
    }
}

fn empty_game_review_response(
    provider: &str,
    external_id: &str,
    warning: Option<String>,
) -> GameReviewResponse {
    GameReviewResponse {
        provider: provider.to_owned(),
        external_id: external_id.to_owned(),
        review: None,
        warning,
        last_synced_at: Utc::now().to_rfc3339(),
    }
}

fn fetch_steam_review_for_user(
    client: &reqwest::blocking::Client,
    app_id: u64,
    steam_id: &str,
    steam_cookie_header: Option<&str>,
) -> Result<Option<GameReviewEntryResponse>, String> {
    let trimmed_steam_id = steam_id.trim();
    if trimmed_steam_id.is_empty() {
        return Ok(None);
    }

    let find_review_in_payload =
        |reviews: Vec<SteamAppReviewEntry>| -> Option<GameReviewEntryResponse> {
            for review in reviews {
                let Some(author) = review.author else {
                    continue;
                };
                let author_steam_id = author
                    .steamid
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty());
                if author_steam_id != Some(trimmed_steam_id) {
                    continue;
                }

                let playtime_minutes = author.playtime_at_review.unwrap_or(0).max(0);
                let created_at = review
                    .timestamp_created
                    .and_then(unix_seconds_to_rfc3339)
                    .unwrap_or_else(|| Utc::now().to_rfc3339());
                let text = review
                    .review
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or("")
                    .to_owned();
                let review_id = review
                    .recommendationid
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| format!("steam:{app_id}:{trimmed_steam_id}"));

                return Some(GameReviewEntryResponse {
                    id: review_id,
                    recommended: review.voted_up.unwrap_or(false),
                    text,
                    playtime_minutes,
                    created_at,
                    likes: review.votes_up.unwrap_or(0).max(0),
                    comments: review.comment_count.unwrap_or(0).max(0),
                    source: String::from("steam"),
                });
            }
            None
        };

    let run_query = |filter_value: &str,
                     include_steam_id: bool,
                     max_pages: usize|
     -> Result<Option<GameReviewEntryResponse>, String> {
        let mut cursor = String::from("*");
        for _ in 0..max_pages {
            let mut request_url = url::Url::parse(&format!("{STEAM_APP_REVIEWS_ENDPOINT}{app_id}"))
                .map_err(|error| format!("Failed to parse Steam appreviews endpoint: {error}"))?;
            {
                let mut query = request_url.query_pairs_mut();
                query
                    .append_pair("json", "1")
                    .append_pair("language", "all")
                    .append_pair("filter", filter_value)
                    .append_pair("review_type", "all")
                    .append_pair("purchase_type", "all")
                    .append_pair("day_range", "36500")
                    .append_pair("num_per_page", "100")
                    .append_pair("cursor", &cursor);
                if include_steam_id {
                    // Undocumented but often supported: narrows the result set to one author.
                    query.append_pair("steamid", trimmed_steam_id);
                }
            }

            let mut request = client.get(request_url);
            if let Some(cookie_header) = steam_cookie_header {
                request = request.header(reqwest::header::COOKIE, cookie_header);
            }
            let response = request
                .send()
                .map_err(|error| format!("Steam appreviews request failed: {error}"))?;
            if !response.status().is_success() {
                return Err(format!(
                    "Steam appreviews request failed with status {}",
                    response.status()
                ));
            }

            let payload = response
                .json::<SteamAppReviewsApiResponse>()
                .map_err(|error| format!("Failed to decode Steam appreviews response: {error}"))?;
            if payload.success.unwrap_or(0) != 1 {
                return Err(String::from(
                    "Steam appreviews payload returned an unsuccessful result.",
                ));
            }

            if let Some(found) = find_review_in_payload(payload.reviews) {
                return Ok(Some(found));
            }

            let next_cursor = payload
                .cursor
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            let Some(next_cursor) = next_cursor else {
                break;
            };
            if next_cursor == cursor {
                break;
            }
            cursor = next_cursor;
        }

        Ok(None)
    };

    let mut had_successful_query = false;
    let mut last_query_error: Option<String> = None;
    for (filter_value, include_steam_id, max_pages) in [
        ("updated", true, 6),
        ("recent", true, 6),
        ("all", true, 8),
        ("updated", false, 8),
        ("recent", false, 8),
    ] {
        match run_query(filter_value, include_steam_id, max_pages) {
            Ok(Some(review)) => return Ok(Some(review)),
            Ok(None) => {
                had_successful_query = true;
            }
            Err(error) => {
                last_query_error = Some(error);
            }
        }
    }

    if let Some(profile_review) =
        fetch_steam_profile_review_for_user(client, app_id, trimmed_steam_id, steam_cookie_header)?
    {
        return Ok(Some(profile_review));
    }

    if !had_successful_query {
        if let Some(error) = last_query_error {
            return Err(error);
        }
    }

    Ok(None)
}

fn normalize_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn extract_last_decimal_number(input: &str) -> Option<f64> {
    let mut current = String::new();
    let mut last_value = None;
    for ch in input.chars() {
        if ch.is_ascii_digit() || ch == '.' || ch == ',' {
            current.push(ch);
            continue;
        }
        if !current.is_empty() {
            let normalized = current.replace(',', "");
            if let Ok(parsed) = normalized.parse::<f64>() {
                last_value = Some(parsed);
            }
            current.clear();
        }
    }
    if !current.is_empty() {
        let normalized = current.replace(',', "");
        if let Ok(parsed) = normalized.parse::<f64>() {
            last_value = Some(parsed);
        }
    }
    last_value
}

fn extract_minutes_from_hours_text(playtime_text: &str) -> Option<i64> {
    let lower = playtime_text.to_ascii_lowercase();
    let markers = [
        "hrs at review time",
        "hours at review time",
        "hr at review time",
        "hrs on record",
        "hours on record",
        "hr on record",
    ];
    for marker in markers {
        if let Some(index) = lower.find(marker) {
            let prefix = &playtime_text[..index];
            if let Some(hours) = extract_last_decimal_number(prefix) {
                let minutes = (hours * 60.0).round() as i64;
                return Some(minutes.max(0));
            }
        }
    }
    None
}

fn decrypt_chromium_cookie_v10(encrypted_value: &[u8]) -> Option<String> {
    // Steam's embedded Chromium cookie DB commonly stores cookie values in v10 format.
    // On Linux Steam this is typically AES-128-CBC with the historical Chromium key derivation.
    if encrypted_value.len() <= 3 || &encrypted_value[..3] != b"v10" {
        return None;
    }

    let ciphertext = &encrypted_value[3..];
    let mut child = Command::new("openssl")
        .arg("enc")
        .arg("-d")
        .arg("-aes-128-cbc")
        .arg("-K")
        .arg("fd621fe5a2b402539dfa147ca9272778")
        .arg("-iv")
        .arg("20202020202020202020202020202020")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    if let Some(mut stdin) = child.stdin.take() {
        if stdin.write_all(ciphertext).is_err() {
            return None;
        }
    }

    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }

    let decrypted = String::from_utf8(output.stdout).ok()?;
    let trimmed = decrypted.trim().to_owned();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed)
}

fn load_steam_web_cookie_header(
    steam_root_override: Option<&str>,
    steam_id: &str,
) -> Option<String> {
    let mut candidate_paths: Vec<PathBuf> = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        let home_path = PathBuf::from(home);
        candidate_paths.push(home_path.join(
            ".var/app/com.valvesoftware.Steam/.local/share/Steam/config/htmlcache/Default/Cookies",
        ));
        candidate_paths.push(home_path.join(".local/share/Steam/config/htmlcache/Default/Cookies"));
        candidate_paths.push(home_path.join(".steam/root/config/htmlcache/Default/Cookies"));
    }
    for root in resolve_steam_root_paths(steam_root_override) {
        candidate_paths.push(root.join("config/htmlcache/Default/Cookies"));
    }

    let mut visited = HashSet::new();
    let machine_auth_cookie_name = format!("steamMachineAuth{steam_id}");
    for path in candidate_paths {
        let canonical_path = match path.canonicalize() {
            Ok(canonical) => canonical,
            Err(_) => continue,
        };
        if !visited.insert(canonical_path.clone()) {
            continue;
        }
        if !canonical_path.is_file() {
            continue;
        }

        let Ok(connection) = Connection::open(&canonical_path) else {
            continue;
        };
        let mut statement = match connection.prepare(
            "SELECT host_key, name, COALESCE(value, ''), encrypted_value
			 FROM cookies
			 WHERE (host_key = 'steamcommunity.com'
			        OR host_key = '.steamcommunity.com'
			        OR host_key = 'store.steampowered.com'
			        OR host_key = '.steampowered.com')
			   AND (name IN (
			        'steamLoginSecure',
			        'steamRememberLogin',
			        'sessionid',
			        'steamCountry',
			        'birthtime',
			        'lastagecheckage',
			        'wants_mature_content',
			        'Steam_Language',
			        'timezoneOffset',
			        'timezoneName',
			        'clientsessionid')
			        OR name LIKE 'steamMachineAuth%')
			 ORDER BY CASE host_key
			           WHEN 'steamcommunity.com' THEN 0
			           WHEN '.steamcommunity.com' THEN 1
			           WHEN 'store.steampowered.com' THEN 2
			           ELSE 3
			          END",
        ) {
            Ok(stmt) => stmt,
            Err(_) => continue,
        };

        let rows = match statement.query_map([], |row| {
            let host_key: String = row.get(0)?;
            let name: String = row.get(1)?;
            let value: String = row.get(2)?;
            let encrypted_value: Vec<u8> = row.get(3)?;
            Ok((host_key, name, value, encrypted_value))
        }) {
            Ok(rows) => rows,
            Err(_) => continue,
        };

        let mut cookie_values_by_name: HashMap<String, String> = HashMap::new();
        for row in rows.flatten() {
            let (_, name, value, encrypted_value) = row;
            if name.starts_with("steamMachineAuth") && name != machine_auth_cookie_name {
                continue;
            }

            let resolved_value = if !value.trim().is_empty() {
                Some(value.trim().to_owned())
            } else {
                decrypt_chromium_cookie_v10(&encrypted_value)
            };
            let Some(resolved_value) = resolved_value else {
                continue;
            };
            if resolved_value.is_empty() {
                continue;
            }
            cookie_values_by_name.entry(name).or_insert(resolved_value);
        }

        if !cookie_values_by_name.contains_key("steamLoginSecure")
            || !cookie_values_by_name.contains_key("sessionid")
        {
            continue;
        }

        let ordered_names = [
            "steamLoginSecure",
            "sessionid",
            "steamRememberLogin",
            "steamCountry",
            "birthtime",
            "lastagecheckage",
            "wants_mature_content",
            "Steam_Language",
            "timezoneOffset",
            "timezoneName",
            "clientsessionid",
        ];
        let mut pairs: Vec<String> = Vec::new();
        for name in ordered_names {
            if let Some(value) = cookie_values_by_name.get(name) {
                pairs.push(format!("{name}={value}"));
            }
        }
        if let Some(value) = cookie_values_by_name.get(&machine_auth_cookie_name) {
            pairs.push(format!("{machine_auth_cookie_name}={value}"));
        }

        if !pairs.is_empty() {
            return Some(pairs.join("; "));
        }
    }

    None
}

fn extract_xml_tag_value(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)?;
    let after_open = &xml[start + open.len()..];
    let end = after_open.find(&close)?;
    let mut value = after_open[..end].trim().to_owned();
    if value.starts_with("<![CDATA[") && value.ends_with("]]>") && value.len() >= 12 {
        value = value[9..value.len() - 3].to_owned();
    }
    let normalized = normalize_whitespace(&value);
    if normalized.is_empty() {
        return None;
    }
    Some(normalized)
}

fn pick_text_from_html_document(document: &Html, selectors: &[&str]) -> Option<String> {
    for selector_str in selectors {
        let Ok(selector) = Selector::parse(selector_str) else {
            continue;
        };
        let Some(node) = document.select(&selector).next() else {
            continue;
        };
        let text = normalize_whitespace(&node.text().collect::<Vec<_>>().join(" "));
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}

fn parse_steam_profile_review_html(
    body: &str,
    app_id: u64,
    steam_id: &str,
) -> Option<GameReviewEntryResponse> {
    if body.trim().is_empty() {
        return None;
    }

    let lower_body = body.to_ascii_lowercase();
    if lower_body.contains("there are no reviews to display")
        || lower_body.contains("has not written a review")
        || lower_body.contains("no user review data found")
        || lower_body.contains("error processing your request")
    {
        return None;
    }

    let html = Html::parse_document(body);

    let recommendation_text = pick_text_from_html_document(
        &html,
        &[
            "#ReviewTitle .ratingSummary",
            ".ratingSummaryBlock .ratingSummary",
            ".apphub_Card .title",
            ".apphub_Card .vote_header .title",
        ],
    );
    let recommended = match recommendation_text.as_deref() {
        Some(text) if text.to_ascii_lowercase().contains("not recommended") => false,
        Some(text) if text.to_ascii_lowercase().contains("recommended") => true,
        _ => !lower_body.contains("not recommended"),
    };

    let mut review_text = pick_text_from_html_document(
        &html,
        &[
            "#ReviewText",
            ".review_area #ReviewText",
            ".apphub_Card .apphub_CardTextContent",
            ".apphub_Card .review_box .content",
        ],
    )
    .unwrap_or_default();
    if review_text.to_ascii_lowercase().starts_with("posted:") {
        review_text = review_text
            .split_once(" ")
            .map(|(_, rest)| rest.trim().to_owned())
            .unwrap_or(review_text);
    }

    let hours_text = pick_text_from_html_document(
        &html,
        &[
            "#ReviewTitle .playTime",
            ".ratingSummaryBlock .playTime",
            ".playTime",
            ".apphub_Card .hours",
            ".apphub_Card .hours_content",
        ],
    )
    .unwrap_or_default();
    let playtime_minutes = extract_minutes_from_hours_text(&hours_text)
        .or_else(|| {
            extract_last_decimal_number(&hours_text).map(|hours| (hours * 60.0).round() as i64)
        })
        .unwrap_or(0)
        .max(0);

    if review_text.is_empty() && hours_text.is_empty() {
        return None;
    }

    Some(GameReviewEntryResponse {
        id: format!("steam-profile:{app_id}:{steam_id}"),
        recommended,
        text: review_text,
        playtime_minutes,
        created_at: Utc::now().to_rfc3339(),
        likes: 0,
        comments: 0,
        source: String::from("steam_profile"),
    })
}

#[cfg(test)]
mod review_parsing_tests {
    use super::*;

    #[test]
    fn extracts_review_time_minutes_from_rating_summary_layout() {
        let html = r#"
			<div class="ratingSummaryBlock" id="ReviewTitle">
				<div class="ratingSummaryHeader">
					<div class="ratingSummary">Recommended</div>
					<div class="playTime">
						4.7 hrs last two weeks / 62.3 hrs on record (12.8 hrs at review time)
					</div>
				</div>
				<div class="recommendation_date">Posted: Sep 7, 2022 @ 6:53am</div>
			</div>
			<div class="review_area">
				<div class="review_area_content">
					<div id="ReviewText">Brings memories of my most toxic and civil moments in gaming.</div>
				</div>
			</div>
		"#;

        let parsed = parse_steam_profile_review_html(html, 222880, "76561198000000000")
            .expect("expected review to be parsed");
        assert!(parsed.recommended);
        assert_eq!(parsed.playtime_minutes, 768);
        assert!(parsed.text.contains("Brings memories"));
    }

    #[test]
    fn extract_minutes_prefers_at_review_time_marker() {
        let input = "4.7 hrs last two weeks / 62.3 hrs on record (12.8 hrs at review time)";
        assert_eq!(extract_minutes_from_hours_text(input), Some(768));
    }
}

fn fetch_steam_profile_review_xml_for_user(
    client: &reqwest::blocking::Client,
    app_id: u64,
    steam_id: &str,
    steam_cookie_header: Option<&str>,
) -> Result<Option<GameReviewEntryResponse>, String> {
    let review_xml_url = format!(
        "https://steamcommunity.com/profiles/{steam_id}/recommended/{app_id}/?xml=1&l=english"
    );
    let mut request = client.get(&review_xml_url);
    if let Some(cookie_header) = steam_cookie_header {
        request = request.header(reqwest::header::COOKIE, cookie_header);
    }
    let response = request
        .send()
        .map_err(|error| format!("Steam profile review XML request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam profile review XML request failed with status {}",
            response.status()
        ));
    }

    let body = response.text().map_err(|error| {
        format!("Could not read Steam profile review XML response body: {error}")
    })?;
    if body.trim().is_empty() {
        return Ok(None);
    }

    let lower_body = body.to_ascii_lowercase();
    if lower_body.contains("<error>") || lower_body.contains("no user review data found") {
        return Ok(None);
    }

    let review_text = extract_xml_tag_value(&body, "review").unwrap_or_default();
    let hours_text = extract_xml_tag_value(&body, "hours").unwrap_or_default();
    if review_text.is_empty() && hours_text.is_empty() {
        return Ok(None);
    }

    let playtime_minutes = extract_minutes_from_hours_text(&hours_text)
        .or_else(|| {
            extract_last_decimal_number(&hours_text).map(|hours| (hours * 60.0).round() as i64)
        })
        .unwrap_or(0)
        .max(0);
    let recommended = match extract_xml_tag_value(&body, "recommended")
        .unwrap_or_else(|| String::from("true"))
        .to_ascii_lowercase()
        .as_str()
    {
        "false" | "0" => false,
        _ => true,
    };
    let created_at = extract_xml_tag_value(&body, "timestamp_created")
        .and_then(|value| value.parse::<i64>().ok())
        .and_then(unix_seconds_to_rfc3339)
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    let likes = extract_xml_tag_value(&body, "votes_up")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);
    let comments = extract_xml_tag_value(&body, "comment_count")
        .or_else(|| extract_xml_tag_value(&body, "comments"))
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);
    let review_id = extract_xml_tag_value(&body, "recommendationid")
        .unwrap_or_else(|| format!("steam-profile-xml:{app_id}:{steam_id}"));

    Ok(Some(GameReviewEntryResponse {
        id: review_id,
        recommended,
        text: review_text,
        playtime_minutes,
        created_at,
        likes,
        comments,
        source: String::from("steam_profile_xml"),
    }))
}

fn fetch_steam_profile_review_for_user(
    client: &reqwest::blocking::Client,
    app_id: u64,
    steam_id: &str,
    steam_cookie_header: Option<&str>,
) -> Result<Option<GameReviewEntryResponse>, String> {
    if let Some(xml_review) =
        fetch_steam_profile_review_xml_for_user(client, app_id, steam_id, steam_cookie_header)?
    {
        return Ok(Some(xml_review));
    }

    let mut review_urls = Vec::new();
    review_urls.push(format!(
        "https://steamcommunity.com/profiles/{steam_id}/recommended/{app_id}/?l=english"
    ));
    if steam_cookie_header.is_some() {
        review_urls.push(format!(
            "https://steamcommunity.com/my/recommended/{app_id}/?l=english"
        ));
    }

    let mut had_successful_response = false;
    let mut last_error: Option<String> = None;
    for review_url in review_urls {
        let mut request = client.get(&review_url);
        if let Some(cookie_header) = steam_cookie_header {
            request = request.header(reqwest::header::COOKIE, cookie_header);
        }
        let response = match request.send() {
            Ok(response) => response,
            Err(error) => {
                last_error = Some(format!("Steam profile review request failed: {error}"));
                continue;
            }
        };
        if !response.status().is_success() {
            last_error = Some(format!(
                "Steam profile review request failed with status {}",
                response.status()
            ));
            continue;
        }

        had_successful_response = true;
        let body = match response.text() {
            Ok(body) => body,
            Err(error) => {
                last_error = Some(format!(
                    "Could not read Steam profile review response body: {error}"
                ));
                continue;
            }
        };
        if let Some(parsed_review) = parse_steam_profile_review_html(&body, app_id, steam_id) {
            return Ok(Some(parsed_review));
        }
    }

    if !had_successful_response {
        if let Some(error) = last_error {
            return Err(error);
        }
    }

    Ok(None)
}

fn parse_steam_dlc_app_ids_from_data(data: &serde_json::Value) -> Vec<u64> {
    let mut app_ids = Vec::new();
    let mut seen_app_ids = HashSet::new();

    let Some(raw_dlc_entries) = data.get("dlc").and_then(serde_json::Value::as_array) else {
        return app_ids;
    };

    for raw_dlc_entry in raw_dlc_entries {
        let parsed_app_id = raw_dlc_entry.as_u64().or_else(|| {
            raw_dlc_entry
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .and_then(|value| value.parse::<u64>().ok())
        });

        if let Some(app_id) = parsed_app_id {
            if seen_app_ids.insert(app_id) {
                app_ids.push(app_id);
            }
        }
    }

    app_ids
}

fn extract_steam_app_name_from_details(details: &serde_json::Value) -> Option<String> {
    let data = details.get("data").unwrap_or(details);
    data.get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn resolve_steam_app_names_best_effort(
    connection: &Connection,
    client: &reqwest::blocking::Client,
    app_ids: &[u64],
    force_refresh: bool,
) -> (HashMap<u64, String>, Option<String>) {
    let mut names_by_app_id = HashMap::new();
    let mut warning: Option<String> = None;
    let mut missing_app_ids = Vec::new();
    let mut seen_app_ids = HashSet::new();

    let stale_before = Utc::now() - ChronoDuration::hours(STEAM_APP_DETAILS_CACHE_TTL_HOURS);
    for app_id in app_ids {
        if !seen_app_ids.insert(*app_id) {
            continue;
        }

        if !force_refresh {
            if let Ok(Some(cached_details)) =
                find_cached_steam_app_details(connection, *app_id, stale_before)
            {
                if let Some(cached_name) = extract_steam_app_name_from_details(&cached_details) {
                    names_by_app_id.insert(*app_id, cached_name);
                    continue;
                }
            }
        }

        missing_app_ids.push(*app_id);
    }

    const APP_DETAILS_BATCH_SIZE: usize = 50;
    for app_id_batch in missing_app_ids.chunks(APP_DETAILS_BATCH_SIZE) {
        let mut request_url = match url::Url::parse(crate::STEAM_APP_DETAILS_ENDPOINT) {
            Ok(url) => url,
            Err(error) => {
                append_warning(
                    &mut warning,
                    format!("Could not parse Steam appdetails endpoint: {error}"),
                );
                return (names_by_app_id, warning);
            }
        };
        let app_ids_param = app_id_batch
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        request_url
            .query_pairs_mut()
            .append_pair("appids", &app_ids_param)
            .append_pair("l", "english");

        let response = match client.get(request_url).send() {
            Ok(value) => value,
            Err(error) => {
                append_warning(
                    &mut warning,
                    format!("Steam appdetails request failed: {error}"),
                );
                continue;
            }
        };
        if !response.status().is_success() {
            append_warning(
                &mut warning,
                format!(
                    "Steam appdetails request failed with status {}",
                    response.status()
                ),
            );
            continue;
        }

        let payload = match response.json::<serde_json::Value>() {
            Ok(value) => value,
            Err(error) => {
                append_warning(
                    &mut warning,
                    format!("Could not decode Steam appdetails response: {error}"),
                );
                continue;
            }
        };

        for app_id in app_id_batch {
            let Some(entry) = payload.get(&app_id.to_string()) else {
                continue;
            };
            if !entry
                .get("success")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            {
                continue;
            }

            if let Some(name) = extract_steam_app_name_from_details(entry) {
                names_by_app_id.insert(*app_id, name);
            }

            let _ = cache_steam_app_details(connection, *app_id, entry);
        }
    }

    (names_by_app_id, warning)
}

fn normalize_dlc_name_for_dedupe(name: &str) -> String {
    let cleaned = name
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>();
    let ignored_tokens = [
        "dlc",
        "addon",
        "add",
        "on",
        "content",
        "pack",
        "package",
        "edition",
        "premium",
        "bundle",
        "deluxe",
        "collector",
        "collectors",
        "digital",
        "upgrade",
        "bonus",
        "preorder",
        "pre",
        "order",
        "ost",
        "soundtrack",
        "artbook",
        "mini",
    ];
    let mut normalized_tokens = Vec::new();
    for token in cleaned.split_whitespace() {
        if ignored_tokens.contains(&token) {
            continue;
        }
        normalized_tokens.push(token);
    }

    normalized_tokens.join(" ")
}

fn dlc_entry_is_placeholder_name(name: &str, external_id: &str) -> bool {
    let trimmed_name = name.trim();
    let expected = format!("DLC App {external_id}");
    trimmed_name.eq_ignore_ascii_case(&expected)
}

fn should_replace_collapsed_dlc_entry(
    existing: &GameDlcEntryResponse,
    candidate: &GameDlcEntryResponse,
) -> bool {
    if candidate.in_library != existing.in_library {
        return candidate.in_library;
    }
    if candidate.installed != existing.installed {
        return candidate.installed;
    }

    let existing_is_placeholder =
        dlc_entry_is_placeholder_name(&existing.name, &existing.external_id);
    let candidate_is_placeholder =
        dlc_entry_is_placeholder_name(&candidate.name, &candidate.external_id);
    if candidate_is_placeholder != existing_is_placeholder {
        return !candidate_is_placeholder;
    }

    let existing_len = existing.name.trim().len();
    let candidate_len = candidate.name.trim().len();
    if candidate_len != existing_len {
        // Prefer the shorter/canonical title once install/library status tie-breakers are equal.
        return candidate_len < existing_len;
    }

    candidate.external_id < existing.external_id
}

fn collapse_near_duplicate_dlc_entries(
    entries: Vec<GameDlcEntryResponse>,
) -> Vec<GameDlcEntryResponse> {
    let mut collapsed_entries = Vec::new();
    let mut index_by_key = HashMap::<String, usize>::new();

    for entry in entries {
        let normalized_key = normalize_dlc_name_for_dedupe(&entry.name);
        let dedupe_key = if normalized_key.trim().is_empty() {
            format!("app:{}", entry.external_id)
        } else {
            normalized_key
        };

        if let Some(existing_index) = index_by_key.get(&dedupe_key).copied() {
            if let Some(existing_entry) = collapsed_entries.get(existing_index) {
                if should_replace_collapsed_dlc_entry(existing_entry, &entry) {
                    collapsed_entries[existing_index] = entry;
                }
            }
            continue;
        }

        index_by_key.insert(dedupe_key, collapsed_entries.len());
        collapsed_entries.push(entry);
    }

    collapsed_entries.sort_by(|left, right| {
        left.name
            .to_ascii_lowercase()
            .cmp(&right.name.to_ascii_lowercase())
            .then_with(|| left.external_id.cmp(&right.external_id))
    });
    collapsed_entries
}

fn extract_cached_steam_dlc_app_ids(cached_details: &serde_json::Value) -> Option<Vec<u64>> {
    if let Some(data) = cached_details.get("data") {
        return Some(parse_steam_dlc_app_ids_from_data(data));
    }
    if cached_details.is_object() {
        return Some(parse_steam_dlc_app_ids_from_data(cached_details));
    }
    None
}

fn fetch_steam_dlc_app_ids(
    connection: &Connection,
    client: &reqwest::blocking::Client,
    app_id: u64,
    force_refresh: bool,
) -> Result<Vec<u64>, String> {
    let stale_before = Utc::now() - ChronoDuration::hours(STEAM_APP_DETAILS_CACHE_TTL_HOURS);
    if !force_refresh {
        if let Ok(Some(cached)) = find_cached_steam_app_details(connection, app_id, stale_before) {
            if let Some(cached_dlc_app_ids) = extract_cached_steam_dlc_app_ids(&cached) {
                return Ok(cached_dlc_app_ids);
            }
        }
    }

    let mut request_url = match url::Url::parse(crate::STEAM_APP_DETAILS_ENDPOINT) {
        Ok(url) => url,
        Err(_) => Url::parse("https://store.steampowered.com/api/appdetails")
            .map_err(|error| format!("Failed to parse Steam appdetails URL: {error}"))?,
    };
    request_url
        .query_pairs_mut()
        .append_pair("appids", &app_id.to_string())
        .append_pair("l", "english");

    let response = client
        .get(request_url)
        .send()
        .map_err(|error| format!("Steam appdetails request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam appdetails request failed with status {}",
            response.status()
        ));
    }

    let payload = response
        .json::<serde_json::Value>()
        .map_err(|error| format!("Failed to decode Steam appdetails payload: {error}"))?;
    let Some(entry) = payload.get(&app_id.to_string()) else {
        return Ok(Vec::new());
    };
    if !entry
        .get("success")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(Vec::new());
    }
    let Some(data) = entry.get("data") else {
        return Ok(Vec::new());
    };

    let _ = cache_steam_app_details(connection, app_id, entry);
    Ok(parse_steam_dlc_app_ids_from_data(data))
}

fn load_owned_steam_dlc_by_app_id(
    connection: &Connection,
    user_id: &str,
) -> Result<HashMap<u64, (String, bool)>, String> {
    let mut owned_dlc_by_app_id = HashMap::new();
    let mut statement = connection
        .prepare(
            "SELECT external_id, name, installed
			 FROM games
			 WHERE user_id = ?1 AND provider = 'steam' AND kind = 'dlc'",
        )
        .map_err(|error| format!("Failed to prepare owned Steam DLC query: {error}"))?;

    let rows = statement
        .query_map(params![user_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(|error| format!("Failed to query owned Steam DLC entries: {error}"))?;

    for row in rows {
        let (external_id, name, installed_raw) =
            row.map_err(|error| format!("Failed to decode owned Steam DLC row: {error}"))?;
        let Some(app_id) = external_id.parse::<u64>().ok() else {
            continue;
        };
        owned_dlc_by_app_id.insert(app_id, (name, installed_raw > 0));
    }

    Ok(owned_dlc_by_app_id)
}

fn steam_store_data_supports_trading_cards(data: &serde_json::Value) -> bool {
    if let Some(categories) = data.get("categories").and_then(serde_json::Value::as_array) {
        for category in categories {
            let description = category
                .get("description")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .unwrap_or_default()
                .to_ascii_lowercase();
            if description.contains("trading card") || description.contains("trading cards") {
                return true;
            }
        }
    }

    let serialized = data.to_string().to_ascii_lowercase();
    serialized.contains("trading card") || serialized.contains("trading cards")
}

fn extract_cached_trading_cards_support(cached_details: &serde_json::Value) -> Option<bool> {
    if let Some(data) = cached_details.get("data") {
        return Some(steam_store_data_supports_trading_cards(data));
    }
    if cached_details.is_object() {
        return Some(steam_store_data_supports_trading_cards(cached_details));
    }
    None
}

fn resolve_steam_trading_cards_support(
    connection: &Connection,
    client: &reqwest::blocking::Client,
    app_id: u64,
) -> Result<Option<bool>, String> {
    let stale_before = Utc::now() - ChronoDuration::hours(STEAM_APP_DETAILS_CACHE_TTL_HOURS);
    if let Ok(Some(cached)) = find_cached_steam_app_details(connection, app_id, stale_before) {
        if let Some(cached_support) = extract_cached_trading_cards_support(&cached) {
            return Ok(Some(cached_support));
        }
    }

    let mut request_url = match url::Url::parse(crate::STEAM_APP_DETAILS_ENDPOINT) {
        Ok(url) => url,
        Err(_) => Url::parse("https://store.steampowered.com/api/appdetails")
            .map_err(|error| format!("Failed to parse Steam appdetails URL: {error}"))?,
    };
    request_url
        .query_pairs_mut()
        .append_pair("appids", &app_id.to_string())
        .append_pair("l", "english");

    let response = client
        .get(request_url)
        .send()
        .map_err(|error| format!("Steam appdetails request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam appdetails request failed with status {}",
            response.status()
        ));
    }

    let payload = response
        .json::<serde_json::Value>()
        .map_err(|error| format!("Failed to decode Steam appdetails payload: {error}"))?;
    let Some(entry) = payload.get(&app_id.to_string()) else {
        return Ok(None);
    };
    if !entry
        .get("success")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(None);
    }
    let Some(data) = entry.get("data") else {
        return Ok(None);
    };

    // Keep cache compatible with callers that expect `details_json` to include a top-level `data` field.
    let _ = cache_steam_app_details(connection, app_id, entry);
    Ok(Some(steam_store_data_supports_trading_cards(data)))
}

fn decode_html_entities(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn normalize_steam_community_asset_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }
    let decoded = decode_html_entities(trimmed);
    if decoded.starts_with("http://") || decoded.starts_with("https://") {
        return Some(decoded);
    }
    if decoded.starts_with("//") {
        return Some(format!("https:{decoded}"));
    }
    if decoded.starts_with('/') {
        return Some(format!("https://steamcommunity.com{decoded}"));
    }
    Some(decoded)
}

fn extract_style_background_image_url(style_value: &str) -> Option<String> {
    let lower = style_value.to_ascii_lowercase();
    let url_start = lower.find("url(")?;
    let after_marker = &style_value[(url_start + 4)..];
    let mut end_offset = after_marker.find(')')?;
    while end_offset > 0 && after_marker[..end_offset].ends_with(char::is_whitespace) {
        end_offset -= 1;
    }
    let candidate = after_marker[..end_offset]
        .trim()
        .trim_matches('\'')
        .trim_matches('"');
    normalize_steam_community_asset_url(candidate)
}

fn parse_trading_card_qty(raw_text: &str) -> i64 {
    let digits = raw_text
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return 0;
    }
    digits.parse::<i64>().unwrap_or(0)
}

fn parse_steam_gamecards_page_cards(html: &str, app_id: u64) -> Vec<GameTradingCardEntryResponse> {
    let document = Html::parse_document(html);
    let card_selector = match Selector::parse(".badge_card_set_card") {
        Ok(selector) => selector,
        Err(_) => return Vec::new(),
    };
    let title_selector = Selector::parse(".badge_card_set_title").ok();
    let qty_selector = Selector::parse(".badge_card_set_text_qty").ok();
    let image_selector = Selector::parse("img").ok();
    let all_descendants_selector = Selector::parse("*").ok();

    let mut cards = Vec::new();
    for (index, card_element) in document.select(&card_selector).enumerate() {
        let card_name = title_selector
            .as_ref()
            .and_then(|selector| card_element.select(selector).next())
            .map(|title| compact_whitespace(&title.text().collect::<Vec<_>>().join(" ")))
            .unwrap_or_default();

        let qty_text = qty_selector
            .as_ref()
            .and_then(|selector| card_element.select(selector).next())
            .map(|qty| qty.text().collect::<Vec<_>>().join(" "))
            .unwrap_or_default();
        let owned_count = parse_trading_card_qty(&qty_text);

        let mut image_url = image_selector
            .as_ref()
            .and_then(|selector| card_element.select(selector).next())
            .and_then(|image| image.value().attr("src"))
            .and_then(normalize_steam_community_asset_url);
        if image_url.is_none() {
            if let Some(selector) = all_descendants_selector.as_ref() {
                for node in card_element.select(selector) {
                    let Some(style_value) = node.value().attr("style") else {
                        continue;
                    };
                    if let Some(url) = extract_style_background_image_url(style_value) {
                        image_url = Some(url);
                        break;
                    }
                }
            }
        }

        if card_name.is_empty() && image_url.is_none() {
            continue;
        }

        let fallback_name = format!("Card {}", index + 1);
        cards.push(GameTradingCardEntryResponse {
            id: format!("steam:{app_id}:card:{}", index + 1),
            name: if card_name.is_empty() {
                fallback_name
            } else {
                card_name
            },
            image_url,
            owned_count,
            is_owned: owned_count > 0,
        });
    }

    cards
}

fn fetch_steam_gamecards_page_cards(
    client: &reqwest::blocking::Client,
    steam_id: &str,
    app_id: u64,
) -> Result<Vec<GameTradingCardEntryResponse>, String> {
    let trimmed_steam_id = steam_id.trim();
    if trimmed_steam_id.is_empty() {
        return Err(String::from(
            "Steam account ID is missing; cannot resolve trading-card page.",
        ));
    }

    let gamecards_url = format!(
        "https://steamcommunity.com/profiles/{trimmed_steam_id}/gamecards/{app_id}/?l=english"
    );
    let response = client
        .get(&gamecards_url)
        .send()
        .map_err(|error| format!("Steam gamecards page request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam gamecards page request failed with status {}",
            response.status()
        ));
    }

    let html = response
        .text()
        .map_err(|error| format!("Failed to read Steam gamecards page HTML: {error}"))?;
    Ok(parse_steam_gamecards_page_cards(&html, app_id))
}

fn empty_game_activity_timeline_response(
    provider: &str,
    external_id: &str,
    warning: Option<String>,
) -> GameActivityTimelineResponse {
    GameActivityTimelineResponse {
        provider: provider.to_owned(),
        external_id: external_id.to_owned(),
        items: Vec::new(),
        warning,
        last_synced_at: Utc::now().to_rfc3339(),
    }
}

fn unix_seconds_to_rfc3339(unix_seconds: i64) -> Option<String> {
    chrono::DateTime::<Utc>::from_timestamp(unix_seconds, 0).map(|value| value.to_rfc3339())
}

fn compact_whitespace(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_owned()
}

fn truncate_text(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_owned();
    }
    let mut short = trimmed
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    short.push_str("...");
    short
}

fn strip_bracket_tags(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        if ch == '[' {
            in_tag = true;
            continue;
        }
        if ch == ']' && in_tag {
            in_tag = false;
            continue;
        }
        if !in_tag {
            output.push(ch);
        }
    }
    output
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn is_steam_clan_image_path_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-')
}

fn extract_first_steam_clan_image_path(input: &str) -> Option<String> {
    let lower = input.to_ascii_lowercase();
    let marker_start = lower.find(STEAM_CLAN_IMAGE_PLACEHOLDER_MARKER)?;
    let path_start = marker_start + STEAM_CLAN_IMAGE_PLACEHOLDER_MARKER.len();
    if path_start >= input.len() {
        return None;
    }

    let remainder = &input[path_start..];
    let mut path_end = 0usize;
    for (index, ch) in remainder.char_indices() {
        if !is_steam_clan_image_path_char(ch) {
            break;
        }
        path_end = index + ch.len_utf8();
    }
    if path_end == 0 {
        return None;
    }

    Some(remainder[..path_end].to_owned())
}

fn steam_clan_image_path_to_url(path: &str) -> Option<String> {
    let normalized = path.trim().trim_start_matches('/');
    if normalized.is_empty() {
        return None;
    }

    let has_clan_and_asset = normalized
        .split('/')
        .filter(|part| !part.is_empty())
        .count()
        >= 2;
    if !has_clan_and_asset {
        return None;
    }

    Some(format!("{STEAM_CLAN_IMAGE_CDN_BASE}{normalized}"))
}

fn normalize_news_image_url(candidate: &str) -> Option<String> {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return None;
    }
    if is_http_url(trimmed) {
        return Some(trimmed.to_owned());
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with(STEAM_CLAN_IMAGE_PLACEHOLDER_MARKER) {
        let rest = &trimmed[STEAM_CLAN_IMAGE_PLACEHOLDER_MARKER.len()..];
        return steam_clan_image_path_to_url(rest);
    }

    if let Some(path) = extract_first_steam_clan_image_path(trimmed) {
        return steam_clan_image_path_to_url(&path);
    }

    None
}

fn strip_steam_clan_image_tokens(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0usize;

    while cursor < input.len() {
        let Some(relative_start) = lower[cursor..].find(STEAM_CLAN_IMAGE_PLACEHOLDER_MARKER) else {
            break;
        };
        let marker_start = cursor + relative_start;
        output.push_str(&input[cursor..marker_start]);

        let mut marker_end = marker_start + STEAM_CLAN_IMAGE_PLACEHOLDER_MARKER.len();
        while marker_end < input.len() {
            let Some(next_char) = input[marker_end..].chars().next() else {
                break;
            };
            if !is_steam_clan_image_path_char(next_char) {
                break;
            }
            marker_end += next_char.len_utf8();
        }
        cursor = marker_end;
    }

    output.push_str(&input[cursor..]);
    output
}

fn strip_angle_tags(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        if ch == '<' {
            in_tag = true;
            continue;
        }
        if ch == '>' && in_tag {
            in_tag = false;
            continue;
        }
        if !in_tag {
            output.push(ch);
        }
    }
    output
}

fn extract_news_preview(contents: &str) -> Option<String> {
    let without_image_tokens = strip_steam_clan_image_tokens(contents);
    let without_brackets = strip_bracket_tags(&without_image_tokens);
    let without_html = strip_angle_tags(&without_brackets);
    let compact = compact_whitespace(&without_html);
    if compact.is_empty() {
        return None;
    }
    Some(truncate_text(&compact, 220))
}

fn extract_first_news_image_url(contents: &str) -> Option<String> {
    let lower = contents.to_ascii_lowercase();
    if let Some(start) = lower.find("[img]") {
        let image_start = start + 5;
        if image_start < contents.len() {
            let rest = &contents[image_start..];
            if let Some(end_rel) = rest.to_ascii_lowercase().find("[/img]") {
                let candidate = rest[..end_rel].trim();
                if let Some(url) = normalize_news_image_url(candidate) {
                    return Some(url);
                }
            }
        }
    }

    if let Some(img_pos) = lower.find("<img") {
        let rest = &contents[img_pos..];
        let rest_lower = rest.to_ascii_lowercase();
        if let Some(src_pos) = rest_lower.find("src=\"") {
            let start = src_pos + 5;
            if start < rest.len() {
                let tail = &rest[start..];
                if let Some(end) = tail.find('"') {
                    let candidate = tail[..end].trim();
                    if let Some(url) = normalize_news_image_url(candidate) {
                        return Some(url);
                    }
                }
            }
        }
    }

    if let Some(clan_image_path) = extract_first_steam_clan_image_path(contents) {
        if let Some(url) = steam_clan_image_path_to_url(&clan_image_path) {
            return Some(url);
        }
    }

    None
}

fn read_news_image_url_from_payload(news_item: &serde_json::Value) -> Option<String> {
    for key in [
        "previewurl",
        "preview_url",
        "image",
        "image_url",
        "header_image",
    ] {
        let candidate = news_item
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if let Some(url) = candidate {
            if let Some(normalized) = normalize_news_image_url(url) {
                return Some(normalized);
            }
        }
    }

    None
}

fn extract_html_meta_image_value(html: &str, marker: &str) -> Option<String> {
    let start = html.find(marker)?;
    let value_start = start + marker.len();
    if value_start >= html.len() {
        return None;
    }
    let tail = &html[value_start..];
    let quote = marker.chars().last().unwrap_or('"');
    let end = tail.find(quote)?;
    let candidate = tail[..end].trim();
    if is_http_url(candidate) {
        return Some(candidate.to_owned());
    }
    None
}

fn fetch_news_preview_image_from_announcement_page(
    client: &reqwest::blocking::Client,
    news_url: &str,
) -> Result<Option<String>, String> {
    let response = client
        .get(news_url)
        .send()
        .map_err(|error| format!("Steam announcement page request failed: {error}"))?;
    if !response.status().is_success() {
        return Ok(None);
    }

    let html = response
        .text()
        .map_err(|error| format!("Failed to read Steam announcement page HTML: {error}"))?;

    for marker in [
        "property=\"og:image\" content=\"",
        "property='og:image' content='",
        "name=\"twitter:image\" content=\"",
        "name='twitter:image' content='",
        "rel=\"image_src\" href=\"",
        "rel='image_src' href='",
    ] {
        if let Some(value) = extract_html_meta_image_value(&html, marker) {
            return Ok(Some(value));
        }
    }

    Ok(None)
}

fn looks_like_patch_notes(title: &str, source: &str, description: &str) -> bool {
    let combined = format!("{title} {source} {description}").to_ascii_lowercase();
    [
        "patch notes",
        "patchnote",
        "changelog",
        "hotfix",
        "update",
        "version ",
        "build ",
    ]
    .iter()
    .any(|needle| combined.contains(needle))
}

fn should_use_compact_news_presentation(
    title: &str,
    source: &str,
    description: &str,
    has_post_image: bool,
) -> bool {
    let normalized_source = source.to_ascii_lowercase();
    let combined = format!("{title} {source} {description}").to_ascii_lowercase();

    let has_regular_update_label = ["regular update", "major update", "featured update"]
        .iter()
        .any(|needle| normalized_source.contains(needle));
    if has_regular_update_label {
        return false;
    }

    // If the post itself carries preview media, prefer featured layout.
    if has_post_image {
        return false;
    }

    let has_small_update_label = [
        "small update",
        "patch notes",
        "patchnote",
        "hotfix",
        "minor update",
    ]
    .iter()
    .any(|needle| normalized_source.contains(needle));
    if has_small_update_label {
        return true;
    }

    [
        "hotfix",
        "bugfix",
        "bug fix",
        "maintenance",
        "balance patch",
        "balance update",
    ]
    .iter()
    .any(|needle| combined.contains(needle))
}

fn fetch_steam_news_timeline_items(
    client: &reqwest::blocking::Client,
    app_id: u64,
) -> Result<Vec<GameActivityTimelineItemResponse>, String> {
    let mut request_url = url::Url::parse(STEAM_WEB_API_NEWS_FOR_APP_ENDPOINT)
        .map_err(|error| format!("Failed to parse Steam news endpoint: {error}"))?;
    request_url
        .query_pairs_mut()
        .append_pair("appid", &app_id.to_string())
        .append_pair("count", &STEAM_ACTIVITY_TIMELINE_MAX_NEWS_ITEMS.to_string())
        .append_pair("maxlength", "400")
        .append_pair("format", "json");

    let response = client
        .get(request_url)
        .send()
        .map_err(|error| format!("Steam news request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam news request failed with status {}",
            response.status()
        ));
    }

    let payload = response
        .json::<SteamNewsForAppApiResponse>()
        .map_err(|error| format!("Failed to decode Steam news response: {error}"))?;
    let Some(appnews) = payload.appnews else {
        return Ok(Vec::new());
    };

    let mut items = Vec::new();
    let mut remaining_news_page_image_lookups = STEAM_ACTIVITY_TIMELINE_MAX_NEWS_PAGE_IMAGE_LOOKUPS;
    for news_item in appnews.newsitems {
        let title = news_item
            .get("title")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("Steam News")
            .to_owned();
        let occurred_at = news_item
            .get("date")
            .and_then(serde_json::Value::as_i64)
            .and_then(unix_seconds_to_rfc3339)
            .unwrap_or_else(|| Utc::now().to_rfc3339());
        let source_label = news_item
            .get("feedlabel")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let raw_contents = news_item
            .get("contents")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let description = extract_news_preview(raw_contents);
        let source_for_match = source_label.as_deref().unwrap_or_default();
        let description_for_match = description.as_deref().unwrap_or_default();
        let url = news_item
            .get("url")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let mut extracted_image_url = read_news_image_url_from_payload(&news_item)
            .or_else(|| extract_first_news_image_url(raw_contents));
        if extracted_image_url.is_none() && remaining_news_page_image_lookups > 0 {
            if let Some(news_url) = url.as_deref() {
                let is_external_community_post =
                    news_url.contains("/externalpost/steam_community_announcements/");
                let is_regular_update = source_for_match
                    .to_ascii_lowercase()
                    .contains("regular update");
                if is_external_community_post || is_regular_update {
                    remaining_news_page_image_lookups -= 1;
                    match fetch_news_preview_image_from_announcement_page(client, news_url) {
                        Ok(found_url) => {
                            if found_url.is_some() {
                                extracted_image_url = found_url;
                            }
                        }
                        Err(error) => {
                            eprintln!(
								"Could not fetch Steam announcement preview image from {news_url}: {error}"
							);
                        }
                    }
                }
            }
        }
        let gid = news_item
            .get("gid")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let source_text = source_label.clone().unwrap_or_else(|| String::from("News"));
        let is_compact = should_use_compact_news_presentation(
            &title,
            source_for_match,
            description_for_match,
            extracted_image_url.is_some(),
        );
        let is_major_update =
            looks_like_patch_notes(&title, source_for_match, description_for_match);
        items.push(GameActivityTimelineItemResponse {
            id: gid.unwrap_or_else(|| format!("news:{app_id}:{occurred_at}:{title}")),
            kind: String::from("news"),
            title,
            subtitle: None,
            description: description.clone(),
            image_url: extracted_image_url,
            url,
            source_label: Some(source_text.clone()),
            presentation: Some(if is_compact {
                String::from("compact")
            } else {
                String::from("featured")
            }),
            occurred_at,
            is_major_update: Some(!is_compact && is_major_update),
        });
    }

    items.sort_by(|left, right| right.occurred_at.cmp(&left.occurred_at));
    Ok(items)
}

fn fetch_steam_achievement_schema(
    client: &reqwest::blocking::Client,
    api_key: &str,
    app_id: u64,
) -> Result<HashMap<String, SteamAchievementSchemaEntry>, String> {
    let mut request_url = url::Url::parse(STEAM_WEB_API_GAME_SCHEMA_ENDPOINT)
        .map_err(|error| format!("Failed to parse Steam schema endpoint: {error}"))?;
    request_url
        .query_pairs_mut()
        .append_pair("key", api_key)
        .append_pair("appid", &app_id.to_string())
        .append_pair("l", "english")
        .append_pair("format", "json");

    let response = client
        .get(request_url)
        .send()
        .map_err(|error| format!("Steam achievement schema request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam achievement schema request failed with status {}",
            response.status()
        ));
    }

    let payload = response
        .json::<SteamGameSchemaApiResponse>()
        .map_err(|error| format!("Failed to decode Steam achievement schema response: {error}"))?;
    let mut schema_by_name = HashMap::new();
    let Some(game_schema) = payload.game else {
        return Ok(schema_by_name);
    };
    let Some(stats) = game_schema.available_game_stats else {
        return Ok(schema_by_name);
    };
    for achievement in stats.achievements {
        let Some(api_name) = achievement
            .name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        schema_by_name.insert(
            api_name.to_owned(),
            SteamAchievementSchemaEntry {
                display_name: achievement
                    .display_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned),
                description: achievement
                    .description
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned),
                icon: achievement
                    .icon
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned),
            },
        );
    }
    Ok(schema_by_name)
}

fn fetch_steam_achievement_timeline_items(
    client: &reqwest::blocking::Client,
    api_key: &str,
    steam_id: &str,
    app_id: u64,
    schema_by_name: &HashMap<String, SteamAchievementSchemaEntry>,
) -> Result<Vec<GameActivityTimelineItemResponse>, String> {
    let mut request_url = url::Url::parse(STEAM_WEB_API_PLAYER_ACHIEVEMENTS_ENDPOINT)
        .map_err(|error| format!("Failed to parse Steam player achievements endpoint: {error}"))?;
    request_url
        .query_pairs_mut()
        .append_pair("key", api_key)
        .append_pair("steamid", steam_id)
        .append_pair("appid", &app_id.to_string())
        .append_pair("l", "english")
        .append_pair("format", "json");

    let response = client
        .get(request_url)
        .send()
        .map_err(|error| format!("Steam player achievements request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam player achievements request failed with status {}",
            response.status()
        ));
    }

    let payload = response
        .json::<SteamPlayerAchievementsApiResponse>()
        .map_err(|error| format!("Failed to decode Steam player achievements response: {error}"))?;
    let Some(playerstats) = payload.playerstats else {
        return Ok(Vec::new());
    };
    if !playerstats.success.unwrap_or(false) {
        return Ok(Vec::new());
    }

    let mut items = Vec::new();
    for achievement in playerstats.achievements {
        if achievement.achieved.unwrap_or(0) != 1 {
            continue;
        }
        let Some(unlock_unix_time) = achievement.unlocktime else {
            continue;
        };
        if unlock_unix_time <= 0 {
            continue;
        }
        let Some(api_name) = achievement
            .apiname
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let occurred_at =
            unix_seconds_to_rfc3339(unlock_unix_time).unwrap_or_else(|| Utc::now().to_rfc3339());
        let schema_entry = schema_by_name.get(api_name);
        let title = schema_entry
            .and_then(|entry| entry.display_name.clone())
            .unwrap_or_else(|| api_name.to_owned());
        let description = schema_entry.and_then(|entry| entry.description.clone());
        let image_url = schema_entry.and_then(|entry| entry.icon.clone());
        items.push(GameActivityTimelineItemResponse {
            id: format!("achievement:{app_id}:{api_name}:{unlock_unix_time}"),
            kind: String::from("achievement"),
            title,
            subtitle: Some(String::from("Achievement unlocked")),
            description,
            image_url,
            url: None,
            source_label: Some(String::from("Achievements")),
            presentation: None,
            occurred_at,
            is_major_update: None,
        });
    }

    items.sort_by(|left, right| right.occurred_at.cmp(&left.occurred_at));
    items.truncate(STEAM_ACTIVITY_TIMELINE_MAX_ACHIEVEMENTS);
    Ok(items)
}

fn fetch_steam_friend_list(
    client: &reqwest::blocking::Client,
    api_key: &str,
    steam_id: &str,
) -> Result<SteamFriendListOutcome, String> {
    let mut request_url = url::Url::parse(STEAM_WEB_API_FRIEND_LIST_ENDPOINT)
        .map_err(|error| format!("Failed to parse Steam friend list endpoint: {error}"))?;
    request_url
        .query_pairs_mut()
        .append_pair("key", api_key)
        .append_pair("steamid", steam_id)
        .append_pair("relationship", "friend")
        .append_pair("format", "json");

    let response = client
        .get(request_url)
        .send()
        .map_err(|error| format!("Steam friend list request failed: {error}"))?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Ok(SteamFriendListOutcome {
            friend_ids: Vec::new(),
            visibility: SteamFriendListVisibility::Private,
            warning: Some(String::from(
                "Your Steam friend list is private, so friends activity is unavailable.",
            )),
        });
    }

    if !response.status().is_success() {
        return Ok(SteamFriendListOutcome {
            friend_ids: Vec::new(),
            visibility: SteamFriendListVisibility::Unknown,
            warning: Some(format!(
                "Could not load your Steam friend list right now (status {}).",
                response.status()
            )),
        });
    }

    let payload = response
        .json::<SteamFriendListApiResponse>()
        .map_err(|error| format!("Failed to decode Steam friend list response: {error}"))?;
    let mut friend_ids = Vec::new();
    let mut seen_ids = HashSet::new();

    if let Some(list_payload) = payload.friendslist {
        for entry in list_payload.friends {
            if !entry
                .relationship
                .as_deref()
                .map(|value| value.eq_ignore_ascii_case("friend"))
                .unwrap_or(true)
            {
                continue;
            }

            let steam_id_value = entry.steamid.trim();
            if steam_id_value.is_empty() || !seen_ids.insert(steam_id_value.to_owned()) {
                continue;
            }
            friend_ids.push(steam_id_value.to_owned());
        }
    }

    Ok(SteamFriendListOutcome {
        friend_ids,
        visibility: SteamFriendListVisibility::Public,
        warning: None,
    })
}

fn fetch_steam_player_summaries(
    client: &reqwest::blocking::Client,
    api_key: &str,
    steam_ids: &[String],
) -> Result<HashMap<String, SteamPlayerSummary>, String> {
    let mut summaries_by_id = HashMap::new();
    if steam_ids.is_empty() {
        return Ok(summaries_by_id);
    }

    for steam_ids_chunk in steam_ids.chunks(100) {
        let mut request_url = url::Url::parse(STEAM_WEB_API_PLAYER_SUMMARIES_ENDPOINT)
            .map_err(|error| format!("Failed to parse Steam player summaries endpoint: {error}"))?;
        request_url
            .query_pairs_mut()
            .append_pair("key", api_key)
            .append_pair("steamids", &steam_ids_chunk.join(","))
            .append_pair("format", "json");

        let response = client
            .get(request_url)
            .send()
            .map_err(|error| format!("Steam player summaries request failed: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "Steam player summaries request failed with status {}",
                response.status()
            ));
        }

        let payload = response
            .json::<SteamPlayerSummariesApiResponse>()
            .map_err(|error| {
                format!("Failed to decode Steam player summaries response: {error}")
            })?;
        if let Some(response_payload) = payload.response {
            for summary in response_payload.players {
                if summary.steamid.trim().is_empty() {
                    continue;
                }
                summaries_by_id.insert(summary.steamid.clone(), summary);
            }
        }
    }

    Ok(summaries_by_id)
}

fn fetch_friend_owned_game_status(
    client: &reqwest::blocking::Client,
    api_key: &str,
    friend_steam_id: &str,
    app_id: u64,
) -> Result<FriendOwnedGameStatus, String> {
    let mut request_url = url::Url::parse(STEAM_WEB_API_OWNED_GAMES_ENDPOINT)
        .map_err(|error| format!("Failed to parse Steam owned games endpoint: {error}"))?;
    request_url
        .query_pairs_mut()
        .append_pair("key", api_key)
        .append_pair("steamid", friend_steam_id)
        .append_pair("appids_filter", &app_id.to_string())
        .append_pair("include_played_free_games", "true")
        .append_pair("format", "json");

    let response = client
        .get(request_url)
        .send()
        .map_err(|error| format!("Steam owned games request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam owned games request failed with status {}",
            response.status()
        ));
    }

    let payload = response
        .json::<SteamOwnedGamesApiResponseLite>()
        .map_err(|error| format!("Failed to decode Steam owned games response: {error}"))?;
    let Some(response_payload) = payload.response else {
        return Ok(FriendOwnedGameStatus {
            owns: false,
            played: false,
        });
    };

    let owns = response_payload
        .games
        .iter()
        .any(|game| game.appid == app_id)
        || response_payload.game_count.unwrap_or(0) > 0;
    let played = response_payload
        .games
        .iter()
        .filter(|game| game.appid == app_id)
        .any(|game| game.playtime_forever.unwrap_or(0) > 0);

    Ok(FriendOwnedGameStatus { owns, played })
}

pub(crate) fn get_game_friends_activity(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameFriendsActivityResponse> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state, &connection)?;
    let (normalized_provider, normalized_external_id) =
        normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )?;

    if normalized_provider != "steam" {
        return Ok(empty_game_friends_activity_response(
            &normalized_provider,
            &normalized_external_id,
            SteamFriendListVisibility::Unknown,
            Some(String::from(
                "Friends activity is currently available for Steam titles only.",
            )),
        ));
    }

    let app_id = match normalized_external_id.parse::<u64>() {
        Ok(value) => value,
        Err(_) => {
            return Ok(empty_game_friends_activity_response(
                &normalized_provider,
                &normalized_external_id,
                SteamFriendListVisibility::Unknown,
                Some(String::from("This Steam app ID is invalid.")),
            ));
        }
    };

    let Some(steam_id) = user
        .steam_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(empty_game_friends_activity_response(
            &normalized_provider,
            &normalized_external_id,
            SteamFriendListVisibility::Unknown,
            Some(String::from("Connect Steam to view friends activity.")),
        ));
    };

    let Some(api_key) = state
        .steam_api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(empty_game_friends_activity_response(
            &normalized_provider,
            &normalized_external_id,
            SteamFriendListVisibility::Unknown,
            Some(String::from(
                "Set STEAM_API_KEY to enable Steam friends activity sync.",
            )),
        ));
    };

    let cache_key = format!("steam_friends_activity:{steam_id}:{app_id}");
    if !force_refresh {
        if let Some(cached_value) =
            CacheAdapter::new().get_json(&cache_key, STEAM_FRIENDS_ACTIVITY_CACHE_TTL_SECONDS)
        {
            if let Ok(cached_response) =
                serde_json::from_value::<GameFriendsActivityResponse>(cached_value)
            {
                return Ok(cached_response);
            }
        }
    }

    let client = build_http_client()?;
    let friend_list_outcome = fetch_steam_friend_list(&client, api_key, steam_id)?;
    let mut response = empty_game_friends_activity_response(
        &normalized_provider,
        &normalized_external_id,
        friend_list_outcome.visibility,
        friend_list_outcome.warning,
    );
    let total_friend_count = friend_list_outcome.friend_ids.len();
    let friend_ids = friend_list_outcome
        .friend_ids
        .into_iter()
        .take(STEAM_FRIENDS_ACTIVITY_MAX_FRIENDS_TO_SCAN)
        .collect::<Vec<_>>();

    if total_friend_count > STEAM_FRIENDS_ACTIVITY_MAX_FRIENDS_TO_SCAN {
        append_warning(
            &mut response.warning,
            format!(
                "Showing activity from the first {} friends for performance.",
                STEAM_FRIENDS_ACTIVITY_MAX_FRIENDS_TO_SCAN
            ),
        );
    }

    if friend_ids.is_empty()
        || !matches!(
            friend_list_outcome.visibility,
            SteamFriendListVisibility::Public
        )
    {
        if let Ok(serialized_response) = serde_json::to_value(&response) {
            CacheAdapter::new().set_json(&cache_key, serialized_response);
        }
        return Ok(response);
    }

    let player_summaries_by_id = match fetch_steam_player_summaries(&client, api_key, &friend_ids) {
        Ok(summaries) => summaries,
        Err(error) => {
            append_warning(
                &mut response.warning,
                format!(
                    "Could not load some Steam profile details: {}",
                    normalize_backend_warning_message(&error)
                ),
            );
            HashMap::new()
        }
    };

    let mut played_friends = Vec::new();
    let mut owned_friends = Vec::new();
    for friend_id in &friend_ids {
        let status = match fetch_friend_owned_game_status(&client, api_key, friend_id, app_id) {
            Ok(value) => value,
            Err(error) => {
                append_warning(
                    &mut response.warning,
                    format!(
                        "Could not check all friend game ownership data: {}",
                        normalize_backend_warning_message(&error)
                    ),
                );
                break;
            }
        };

        if !status.owns {
            continue;
        }

        let summary = player_summaries_by_id.get(friend_id);
        let persona_name = summary
            .and_then(|value| value.personaname.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(friend_id)
            .to_owned();
        let friend_entry = GameFriendActivityEntryResponse {
            steam_id: friend_id.clone(),
            persona_name,
            avatar_url: summary.and_then(|value| {
                value
                    .avatarfull
                    .as_deref()
                    .map(str::trim)
                    .filter(|avatar| !avatar.is_empty())
                    .map(ToOwned::to_owned)
            }),
            profile_url: summary.and_then(|value| {
                value
                    .profileurl
                    .as_deref()
                    .map(str::trim)
                    .filter(|profile| !profile.is_empty())
                    .map(ToOwned::to_owned)
            }),
        };

        if status.played {
            played_friends.push(friend_entry.clone());
        }
        owned_friends.push(friend_entry);
    }

    played_friends.sort_by(|left, right| {
        left.persona_name
            .to_ascii_lowercase()
            .cmp(&right.persona_name.to_ascii_lowercase())
    });
    owned_friends.sort_by(|left, right| {
        left.persona_name
            .to_ascii_lowercase()
            .cmp(&right.persona_name.to_ascii_lowercase())
    });

    response.played_friends = played_friends;
    response.owned_friends = owned_friends;
    response.last_synced_at = Utc::now().to_rfc3339();

    if let Ok(serialized_response) = serde_json::to_value(&response) {
        CacheAdapter::new().set_json(&cache_key, serialized_response);
    }

    Ok(response)
}

pub(crate) fn get_game_activity_timeline(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameActivityTimelineResponse> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state, &connection)?;
    let (normalized_provider, normalized_external_id) =
        normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )?;

    if normalized_provider != "steam" {
        return Ok(empty_game_activity_timeline_response(
            &normalized_provider,
            &normalized_external_id,
            Some(String::from(
                "Activity timeline is currently available for Steam titles only.",
            )),
        ));
    }

    let app_id = match normalized_external_id.parse::<u64>() {
        Ok(value) => value,
        Err(_) => {
            return Ok(empty_game_activity_timeline_response(
                &normalized_provider,
                &normalized_external_id,
                Some(String::from("This Steam app ID is invalid.")),
            ));
        }
    };

    let steam_id = user
        .steam_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown");
    let cache_key = format!(
        "steam_activity_timeline:{STEAM_ACTIVITY_TIMELINE_CACHE_VERSION}:{steam_id}:{app_id}"
    );
    if !force_refresh {
        if let Some(cached_value) =
            CacheAdapter::new().get_json(&cache_key, STEAM_ACTIVITY_TIMELINE_CACHE_TTL_SECONDS)
        {
            if let Ok(cached_response) =
                serde_json::from_value::<GameActivityTimelineResponse>(cached_value)
            {
                return Ok(cached_response);
            }
        }
    }

    let client = build_http_client()?;
    let mut response =
        empty_game_activity_timeline_response(&normalized_provider, &normalized_external_id, None);

    match fetch_steam_news_timeline_items(&client, app_id) {
        Ok(news_items) => response.items.extend(news_items),
        Err(error) => {
            append_warning(
                &mut response.warning,
                format!(
                    "Could not load Steam news right now: {}",
                    normalize_backend_warning_message(&error)
                ),
            );
        }
    }

    let maybe_api_key = state
        .steam_api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let maybe_steam_id = user
        .steam_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match (maybe_api_key, maybe_steam_id) {
        (Some(api_key), Some(steam_id_value)) => {
            let schema_by_name = match fetch_steam_achievement_schema(&client, api_key, app_id) {
                Ok(schema) => schema,
                Err(error) => {
                    append_warning(
                        &mut response.warning,
                        format!(
                            "Could not load full achievement metadata: {}",
                            normalize_backend_warning_message(&error)
                        ),
                    );
                    HashMap::new()
                }
            };

            match fetch_steam_achievement_timeline_items(
                &client,
                api_key,
                steam_id_value,
                app_id,
                &schema_by_name,
            ) {
                Ok(achievement_items) => response.items.extend(achievement_items),
                Err(error) => {
                    append_warning(
                        &mut response.warning,
                        format!(
                            "Could not load recent achievements: {}",
                            normalize_backend_warning_message(&error)
                        ),
                    );
                }
            }
        }
        (None, Some(_)) => {
            append_warning(
                &mut response.warning,
                "Set STEAM_API_KEY to include recent achievement unlocks.",
            );
        }
        (_, None) => {
            append_warning(
                &mut response.warning,
                "Connect Steam to include recent achievement unlocks.",
            );
        }
    }

    response
        .items
        .sort_by(|left, right| right.occurred_at.cmp(&left.occurred_at));
    if response.items.len() > STEAM_ACTIVITY_TIMELINE_MAX_ITEMS {
        response.items.truncate(STEAM_ACTIVITY_TIMELINE_MAX_ITEMS);
    }
    if response.items.is_empty() && response.warning.is_none() {
        response.warning = Some(String::from("No recent activity found for this game."));
    }
    response.last_synced_at = Utc::now().to_rfc3339();

    if let Ok(serialized_response) = serde_json::to_value(&response) {
        CacheAdapter::new().set_json(&cache_key, serialized_response);
    }

    Ok(response)
}

pub(crate) fn get_game_achievements(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameAchievementsResponse> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state, &connection)?;
    let (normalized_provider, normalized_external_id) =
        normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )?;

    if normalized_provider != "steam" {
        return Ok(GameAchievementsResponse {
            provider: normalized_provider,
            external_id: normalized_external_id,
            total: 0,
            unlocked_count: 0,
            percent: None,
            entries: Vec::new(),
            warning: Some(String::from(
                "Achievements are currently available for Steam titles only.",
            )),
            last_synced_at: Utc::now().to_rfc3339(),
        });
    }

    let app_id = match normalized_external_id.parse::<u64>() {
        Ok(value) => value,
        Err(_) => {
            return Ok(GameAchievementsResponse {
                provider: normalized_provider,
                external_id: normalized_external_id,
                total: 0,
                unlocked_count: 0,
                percent: None,
                entries: Vec::new(),
                warning: Some(String::from("This Steam app ID is invalid.")),
                last_synced_at: Utc::now().to_rfc3339(),
            });
        }
    };

    let steam_id_opt = user
        .steam_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let api_key_opt = state
        .steam_api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let cache_key = format!(
        "steam_achievements:{STEAM_ACHIEVEMENTS_CACHE_VERSION}:{steam_id}:{app_id}",
        steam_id = steam_id_opt.as_deref().unwrap_or("unknown")
    );
    if !force_refresh {
        if let Some(cached_value) =
            CacheAdapter::new().get_json(&cache_key, STEAM_ACHIEVEMENTS_CACHE_TTL_SECONDS)
        {
            if let Ok(cached_response) =
                serde_json::from_value::<GameAchievementsResponse>(cached_value)
            {
                return Ok(cached_response);
            }
        }
    }

    let client = build_http_client()?;
    let mut response = GameAchievementsResponse {
        provider: normalized_provider.clone(),
        external_id: normalized_external_id.clone(),
        total: 0,
        unlocked_count: 0,
        percent: None,
        entries: Vec::new(),
        warning: None,
        last_synced_at: Utc::now().to_rfc3339(),
    };

    match (api_key_opt.as_deref(), steam_id_opt.as_deref()) {
        (Some(api_key), Some(steam_id)) => {
            let schema_by_name = match fetch_steam_achievement_schema(&client, api_key, app_id) {
                Ok(schema) => schema,
                Err(error) => {
                    append_warning(
                        &mut response.warning,
                        format!(
                            "Could not load full achievement metadata: {}",
                            normalize_backend_warning_message(&error)
                        ),
                    );
                    HashMap::new()
                }
            };

            // fetch player achievements (includes achieved flag)
            let mut player_achievements_map = HashMap::new();
            match (|| -> Result<(), String> {
                let mut request_url = url::Url::parse(STEAM_WEB_API_PLAYER_ACHIEVEMENTS_ENDPOINT)
                    .map_err(|e| {
                    format!("Failed to parse Steam player achievements endpoint: {e}")
                })?;
                request_url
                    .query_pairs_mut()
                    .append_pair("key", api_key)
                    .append_pair("steamid", steam_id)
                    .append_pair("appid", &app_id.to_string())
                    .append_pair("l", "english")
                    .append_pair("format", "json");

                let resp = client
                    .get(request_url)
                    .send()
                    .map_err(|e| format!("Steam player achievements request failed: {e}"))?;
                if !resp.status().is_success() {
                    return Err(format!(
                        "Steam player achievements request failed with status {}",
                        resp.status()
                    ));
                }
                let payload = resp
                    .json::<SteamPlayerAchievementsApiResponse>()
                    .map_err(|e| {
                        format!("Failed to decode Steam player achievements response: {e}")
                    })?;
                let Some(playerstats) = payload.playerstats else {
                    return Ok(());
                };
                if !playerstats.success.unwrap_or(false) {
                    return Ok(());
                }
                for ach in playerstats.achievements {
                    if let Some(api_name) = ach
                        .apiname
                        .as_deref()
                        .map(str::trim)
                        .filter(|v| !v.is_empty())
                    {
                        player_achievements_map.insert(api_name.to_owned(), ach);
                    }
                }
                Ok(())
            })() {
                Ok(_) => {}
                Err(error) => {
                    append_warning(
                        &mut response.warning,
                        format!(
                            "Could not load player achievement data: {}",
                            normalize_backend_warning_message(&error)
                        ),
                    );
                }
            }

            // Build entries from schema; include any schema-only entries as locked
            let mut entries: Vec<GameAchievementEntryResponse> = Vec::new();
            for (api_name, schema_entry) in &schema_by_name {
                let player_entry_opt = player_achievements_map.get(api_name.as_str());
                let unlocked = player_entry_opt.and_then(|p| p.achieved).unwrap_or(0) == 1;
                let unlocked_at = player_entry_opt
                    .and_then(|p| p.unlocktime)
                    .and_then(unix_seconds_to_rfc3339);
                entries.push(GameAchievementEntryResponse {
                    api_name: api_name.to_owned(),
                    name: schema_entry
                        .display_name
                        .clone()
                        .unwrap_or_else(|| api_name.to_owned()),
                    description: schema_entry.description.clone(),
                    icon: schema_entry.icon.clone(),
                    unlocked,
                    unlocked_at,
                });
            }

            // If there are player-only entries not in schema, include them
            for (api_name, player_ach) in &player_achievements_map {
                if !schema_by_name.contains_key(api_name) {
                    let unlocked = player_ach.achieved.unwrap_or(0) == 1;
                    let unlocked_at = player_ach.unlocktime.and_then(unix_seconds_to_rfc3339);
                    entries.push(GameAchievementEntryResponse {
                        api_name: api_name.clone(),
                        name: api_name.clone(),
                        description: None,
                        icon: None,
                        unlocked,
                        unlocked_at,
                    });
                }
            }

            // Sort by name
            entries.sort_by(|a, b| {
                a.name
                    .to_ascii_lowercase()
                    .cmp(&b.name.to_ascii_lowercase())
            });

            let total = entries.len() as i64;
            let unlocked_count = entries.iter().filter(|e| e.unlocked).count() as i64;
            let percent = if total > 0 {
                Some((unlocked_count as f64 / total as f64) * 100.0)
            } else {
                None
            };

            response.total = total;
            response.unlocked_count = unlocked_count;
            response.percent = percent;
            response.entries = entries;
        }
        (None, Some(_)) => {
            append_warning(
                &mut response.warning,
                String::from("Set STEAM_API_KEY to include achievements."),
            );
        }
        (_, None) => {
            append_warning(
                &mut response.warning,
                String::from("Connect Steam to include achievements."),
            );
        }
    }

    response.last_synced_at = Utc::now().to_rfc3339();

    if let Ok(serialized_response) = serde_json::to_value(&response) {
        CacheAdapter::new().set_json(&cache_key, serialized_response);
    }

    Ok(response)
}

pub(crate) fn get_game_trading_cards(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameTradingCardsResponse> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state, &connection)?;
    let (normalized_provider, normalized_external_id) =
        normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )?;

    if normalized_provider != "steam" {
        return Ok(empty_game_trading_cards_response(
            &normalized_provider,
            &normalized_external_id,
            false,
            String::from("https://steamcommunity.com/tradingcards/"),
            Some(String::from(
                "Trading cards are currently available for Steam titles only.",
            )),
        ));
    }

    let app_id = match normalized_external_id.parse::<u64>() {
        Ok(value) => value,
        Err(_) => {
            return Ok(empty_game_trading_cards_response(
                &normalized_provider,
                &normalized_external_id,
                false,
                String::from("https://steamcommunity.com/tradingcards/"),
                Some(String::from("This Steam app ID is invalid.")),
            ));
        }
    };
    let view_url = format!("https://steamcommunity.com/my/gamecards/{app_id}");

    let steam_id_opt = user
        .steam_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let cache_key = format!(
        "steam_trading_cards:{STEAM_TRADING_CARDS_CACHE_VERSION}:{steam_id}:{app_id}",
        steam_id = steam_id_opt.as_deref().unwrap_or("unknown")
    );
    if !force_refresh {
        if let Some(cached_value) =
            CacheAdapter::new().get_json(&cache_key, STEAM_TRADING_CARDS_CACHE_TTL_SECONDS)
        {
            if let Ok(cached_response) =
                serde_json::from_value::<GameTradingCardsResponse>(cached_value)
            {
                return Ok(cached_response);
            }
        }
    }

    let client = build_http_client()?;
    let support_result = resolve_steam_trading_cards_support(&connection, &client, app_id);
    let mut response = GameTradingCardsResponse {
        provider: normalized_provider.clone(),
        external_id: normalized_external_id.clone(),
        supported: true,
        badge_level: None,
        badge_xp: None,
        total_cards: 0,
        owned_cards: 0,
        cards: Vec::new(),
        warning: None,
        view_url,
        last_synced_at: Utc::now().to_rfc3339(),
    };

    match support_result {
        Ok(Some(supported)) => {
            response.supported = supported;
            if !supported {
                append_warning(
                    &mut response.warning,
                    String::from("This title does not appear to support Steam Trading Cards."),
                );
            }
        }
        Ok(None) => {
            append_warning(
                &mut response.warning,
                String::from("Could not verify trading-card support from Steam Store metadata."),
            );
        }
        Err(error) => {
            append_warning(
                &mut response.warning,
                format!(
                    "Could not load trading-card support metadata: {}",
                    normalize_backend_warning_message(&error)
                ),
            );
        }
    }

    match steam_id_opt.as_deref() {
        Some(steam_id) => match fetch_steam_gamecards_page_cards(&client, steam_id, app_id) {
            Ok(cards) => {
                if !cards.is_empty() {
                    response.supported = true;
                    response.owned_cards = cards.iter().filter(|card| card.is_owned).count() as i64;
                    response.total_cards = cards.len() as i64;
                    response.cards = cards;
                }
            }
            Err(error) => {
                append_warning(
                    &mut response.warning,
                    format!(
                        "Could not load card tiles from Steam community page: {}",
                        normalize_backend_warning_message(&error)
                    ),
                );
            }
        },
        None => {
            append_warning(
                &mut response.warning,
                String::from("Connect Steam to include per-card owned/missing tiles."),
            );
        }
    }

    if response.supported && response.cards.is_empty() {
        append_warning(
			&mut response.warning,
			String::from(
				"Trading cards appear supported, but card tiles could not be read from your community profile.",
			),
		);
    }

    let api_key_opt = state
        .steam_api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    match (api_key_opt.as_deref(), steam_id_opt.as_deref()) {
        (Some(api_key), Some(steam_id)) => {
            match (|| -> Result<(), String> {
                let mut request_url = url::Url::parse(STEAM_WEB_API_BADGES_ENDPOINT)
                    .map_err(|error| format!("Failed to parse Steam badges endpoint: {error}"))?;
                request_url
                    .query_pairs_mut()
                    .append_pair("key", api_key)
                    .append_pair("steamid", steam_id)
                    .append_pair("format", "json");
                let api_response = client
                    .get(request_url)
                    .send()
                    .map_err(|error| format!("Steam badges request failed: {error}"))?;
                if !api_response.status().is_success() {
                    return Err(format!(
                        "Steam badges request failed with status {}",
                        api_response.status()
                    ));
                }
                let payload = api_response
                    .json::<SteamBadgesApiResponse>()
                    .map_err(|error| format!("Failed to decode Steam badges payload: {error}"))?;
                let badges = payload
                    .response
                    .map(|response| response.badges)
                    .unwrap_or_default();
                if let Some(badge) = badges
                    .into_iter()
                    .find(|entry| entry.appid.map(|value| value == app_id).unwrap_or(false))
                {
                    response.badge_level = badge.level;
                    response.badge_xp = badge.xp;
                }
                Ok(())
            })() {
                Ok(()) => {}
                Err(error) => {
                    append_warning(
                        &mut response.warning,
                        format!(
                            "Could not load badge progression: {}",
                            normalize_backend_warning_message(&error)
                        ),
                    );
                }
            }
        }
        (None, Some(_)) => {
            append_warning(
                &mut response.warning,
                String::from("Set STEAM_API_KEY to include badge progression."),
            );
        }
        (_, None) => {
            append_warning(
                &mut response.warning,
                String::from("Connect Steam to include badge progression."),
            );
        }
    }

    response.last_synced_at = Utc::now().to_rfc3339();
    if let Ok(serialized_response) = serde_json::to_value(&response) {
        CacheAdapter::new().set_json(&cache_key, serialized_response);
    }
    Ok(response)
}

pub(crate) fn get_game_dlc(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameDlcResponse> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state, &connection)?;
    let (normalized_provider, normalized_external_id) =
        normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )?;

    if normalized_provider != "steam" {
        return Ok(empty_game_dlc_response(
            &normalized_provider,
            &normalized_external_id,
            Some(String::from(
                "DLC details are currently available for Steam titles only.",
            )),
        ));
    }

    let app_id = match normalized_external_id.parse::<u64>() {
        Ok(value) => value,
        Err(_) => {
            return Ok(empty_game_dlc_response(
                &normalized_provider,
                &normalized_external_id,
                Some(String::from("This Steam app ID is invalid.")),
            ));
        }
    };

    let client = build_http_client()?;
    let mut response = empty_game_dlc_response(&normalized_provider, &normalized_external_id, None);

    let dlc_app_ids = match fetch_steam_dlc_app_ids(&connection, &client, app_id, force_refresh) {
        Ok(entries) => entries,
        Err(error) => {
            append_warning(
                &mut response.warning,
                format!(
                    "Could not load DLC metadata from Steam: {}",
                    normalize_backend_warning_message(&error)
                ),
            );
            Vec::new()
        }
    };

    let owned_dlc_by_app_id = match load_owned_steam_dlc_by_app_id(&connection, &user.id) {
        Ok(map) => map,
        Err(error) => {
            append_warning(
                &mut response.warning,
                format!(
                    "Could not resolve owned DLC status: {}",
                    normalize_backend_warning_message(&error)
                ),
            );
            HashMap::new()
        }
    };

    let (resolved_dlc_names_by_app_id, name_resolution_warning) =
        resolve_steam_app_names_best_effort(&connection, &client, &dlc_app_ids, force_refresh);
    if let Some(warning_message) = name_resolution_warning {
        append_warning(
            &mut response.warning,
            format!(
                "Could not resolve names for some DLC entries: {}",
                normalize_backend_warning_message(&warning_message)
            ),
        );
    }

    let mut entries = Vec::new();
    for dlc_app_id in dlc_app_ids {
        let maybe_owned_dlc = owned_dlc_by_app_id.get(&dlc_app_id);
        let external_id_value = dlc_app_id.to_string();
        let entry_name = maybe_owned_dlc
            .map(|(name, _)| name.clone())
            .or_else(|| resolved_dlc_names_by_app_id.get(&dlc_app_id).cloned())
            .unwrap_or_else(|| format!("DLC App {external_id_value}"));
        let installed = maybe_owned_dlc
            .map(|(_, installed_value)| *installed_value)
            .unwrap_or(false);
        let in_library = maybe_owned_dlc.is_some();

        entries.push(GameDlcEntryResponse {
            id: format!("steam:{external_id_value}"),
            provider: String::from("steam"),
            external_id: external_id_value.clone(),
            name: entry_name,
            installed,
            in_library,
            store_url: format!("{}/{}", crate::STEAM_STORE_APP_ENDPOINT, external_id_value),
        });
    }

    response.entries = collapse_near_duplicate_dlc_entries(entries);
    response.last_synced_at = Utc::now().to_rfc3339();
    Ok(response)
}

pub(crate) fn get_game_review(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameReviewResponse> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state, &connection)?;
    let (normalized_provider, normalized_external_id) =
        normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )?;

    if normalized_provider != "steam" {
        return Ok(empty_game_review_response(
            &normalized_provider,
            &normalized_external_id,
            Some(String::from(
                "Reviews are currently available for Steam titles only.",
            )),
        ));
    }

    let app_id = match normalized_external_id.parse::<u64>() {
        Ok(value) => value,
        Err(_) => {
            return Ok(empty_game_review_response(
                &normalized_provider,
                &normalized_external_id,
                Some(String::from("This Steam app ID is invalid.")),
            ));
        }
    };

    let Some(steam_id) = user
        .steam_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(empty_game_review_response(
            &normalized_provider,
            &normalized_external_id,
            Some(String::from("Connect Steam to load your review.")),
        ));
    };

    let cache_key = format!("steam_review:{STEAM_REVIEW_CACHE_VERSION}:{steam_id}:{app_id}");
    if !force_refresh {
        if let Some(cached_value) = CacheAdapter::new().get_json(&cache_key, STEAM_REVIEW_CACHE_TTL_SECONDS) {
            if let Ok(cached_response) = serde_json::from_value::<GameReviewResponse>(cached_value)
            {
                return Ok(cached_response);
            }
        }
    }

    let client = build_http_client()?;
    let mut response =
        empty_game_review_response(&normalized_provider, &normalized_external_id, None);
    let steam_cookie_header =
        load_steam_web_cookie_header(state.steam_root_override.as_deref(), steam_id);
    match fetch_steam_review_for_user(&client, app_id, steam_id, steam_cookie_header.as_deref()) {
        Ok(Some(review)) => {
            response.review = Some(review);
        }
        Ok(None) => {
            append_warning(
                &mut response.warning,
                String::from("No public Steam review found for this game on your account."),
            );
        }
        Err(error) => {
            append_warning(
                &mut response.warning,
                format!(
                    "Could not load your Steam review right now: {}",
                    normalize_backend_warning_message(&error)
                ),
            );
        }
    }
    response.last_synced_at = Utc::now().to_rfc3339();

    if let Ok(serialized_response) = serde_json::to_value(&response) {
        CacheAdapter::new().set_json(&cache_key, serialized_response);
    }

    Ok(response)
}

pub(crate) fn list_steam_downloads(
    state: &AppState,
) -> AppResult<Vec<SteamDownloadProgressResponse>> {
    let owned_games_by_app_id = match open_connection(&state.db_path) {
        Ok(connection) => {
            if let Err(error) = cleanup_expired_sessions(&connection) {
                eprintln!(
					"Steam download tracking: failed to cleanup expired sessions ({error}); continuing without ownership map."
				);
                HashMap::new()
            } else {
                match get_authenticated_user(state, &connection) {
                    Ok(user) => match load_owned_steam_games_by_app_id(&connection, &user.id) {
                        Ok(games) => games,
                        Err(error) => {
                            eprintln!(
								"Steam download tracking: could not load owned Steam games ({error}); continuing without ownership map."
							);
                            HashMap::new()
                        }
                    },
                    Err(error) => {
                        eprintln!(
							"Steam download tracking: could not resolve authenticated user metadata ({error}); continuing without ownership map."
						);
                        HashMap::new()
                    }
                }
            }
        }
        Err(error) => {
            eprintln!(
				"Steam download tracking: could not open app database ({error}); continuing without ownership map."
			);
            HashMap::new()
        }
    };

    let steam_roots = resolve_steam_root_paths(state.steam_root_override.as_deref());
    if steam_roots.is_empty() {
        return Ok(Vec::new());
    }
    let mut downloads = Vec::new();
    let mut seen_external_ids = HashSet::new();

    for steam_root in steam_roots {
        let steamapps_directories = match resolve_steamapps_directories(&steam_root) {
            Ok(paths) => paths,
            Err(error) => {
                eprintln!(
                    "Could not resolve Steam library paths from root {}: {}",
                    steam_root.display(),
                    error
                );
                continue;
            }
        };
        for steamapps_directory in steamapps_directories {
            if let Err(error) = collect_steam_download_progress_from_steamapps_dir(
                &steamapps_directory,
                &owned_games_by_app_id,
                &mut seen_external_ids,
                &mut downloads,
            ) {
                eprintln!(
                    "Could not read Steam download progress from {}: {}",
                    steamapps_directory.display(),
                    error
                );
            }
        }
    }

    downloads.sort_by(|left, right| {
        left.name
            .to_ascii_lowercase()
            .cmp(&right.name.to_ascii_lowercase())
    });
    Ok(downloads)
}

pub(crate) fn get_game_store_metadata(
    state: &AppState,
    provider: String,
    external_id: String,
) -> AppResult<GameStoreMetadataResponse> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state, &connection)?;
    let (normalized_provider, normalized_external_id) =
        normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )?;

    // Only Steam is supported for rich store metadata at the moment
    if normalized_provider != "steam" {
        return Ok(GameStoreMetadataResponse {
            developers: None,
            publishers: None,
            franchise: None,
            release_date: None,
            short_description: None,
            header_image: None,
            has_achievements: None,
            achievements_count: None,
            has_cloud_saves: None,
            cloud_details: None,
            controller_support: None,
            features: None,
        });
    }

    let app_id = match normalized_external_id.parse::<u64>() {
        Ok(v) => v,
        Err(_) => {
            return Ok(GameStoreMetadataResponse {
                developers: None,
                publishers: None,
                franchise: None,
                release_date: None,
                short_description: None,
                header_image: None,
                has_achievements: None,
                achievements_count: None,
                has_cloud_saves: None,
                cloud_details: None,
                controller_support: None,
                features: None,
            })
        }
    };

    let stale_before = Utc::now() - ChronoDuration::hours(STEAM_APP_DETAILS_CACHE_TTL_HOURS);

    let mut response = GameStoreMetadataResponse {
        developers: None,
        publishers: None,
        franchise: None,
        release_date: None,
        short_description: None,
        header_image: None,
        has_achievements: None,
        achievements_count: None,
        has_cloud_saves: None,
        cloud_details: None,
        controller_support: None,
        features: None,
    };

    // Keep a reference to parsed store data (if available) to build normalized feature list later.
    let mut maybe_data: Option<serde_json::Value> = None;

    if let Ok(Some(cached)) = find_cached_steam_app_details(&connection, app_id, stale_before) {
        if let Some(data) = cached.get("data") {
            // capture parsed data for normalized feature building
            maybe_data = Some(data.clone());
            if let Some(devs) = data.get("developers").and_then(serde_json::Value::as_array) {
                let mut out: Vec<String> = Vec::new();
                for d in devs {
                    if let Some(s) = d.as_str() {
                        out.push(s.to_owned());
                    }
                }
                if !out.is_empty() {
                    response.developers = Some(out);
                }
            }
            if let Some(pubs) = data.get("publishers").and_then(serde_json::Value::as_array) {
                let mut out: Vec<String> = Vec::new();
                for p in pubs {
                    if let Some(s) = p.as_str() {
                        out.push(s.to_owned());
                    }
                }
                if !out.is_empty() {
                    response.publishers = Some(out);
                }
            }
            // franchise: prefer `franchise`, fall back to `series` array
            response.franchise = data
                .get("franchise")
                .and_then(serde_json::Value::as_str)
                .map(|s| s.to_owned())
                .or_else(|| {
                    data.get("series").and_then(|v| v.as_array()).map(|arr| {
                        arr.iter()
                            .filter_map(|s| s.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                });

            // release_date: try nested `release_date.date`, then plain string fallback
            response.release_date = data
                .get("release_date")
                .and_then(|v| v.get("date"))
                .and_then(serde_json::Value::as_str)
                .map(|s| s.to_owned())
                .or_else(|| {
                    data.get("release_date")
                        .and_then(serde_json::Value::as_str)
                        .map(|s| s.to_owned())
                });
            if let Some(sd) = data
                .get("short_description")
                .and_then(serde_json::Value::as_str)
            {
                response.short_description = Some(sd.to_owned());
            }
            if let Some(h) = data.get("header_image").and_then(serde_json::Value::as_str) {
                response.header_image = Some(h.to_owned());
            }
        }
    }

    // If no cached details were found, attempt a best-effort live fetch from the Steam Store
    if response.short_description.is_none() || response.developers.is_none() {
        // Prefer using steamcmd if available for an exact client-style appinfo
        if let Ok(output) = Command::new("bash")
            .arg("-lc")
            .arg(format!(
                "steamcmd +login anonymous +app_info_print {} +quit",
                app_id
            ))
            .output()
        {
            if output.status.success() {
                if let Ok(text) = String::from_utf8(output.stdout) {
                    // Simple VDF-like key/value extraction: "key" "value"
                    let re = regex::Regex::new(r#"\"([^\"]+)\"\s+\"([^\"]*)\""#).unwrap();
                    let mut map: std::collections::HashMap<String, String> =
                        std::collections::HashMap::new();
                    for cap in re.captures_iter(&text) {
                        map.insert(cap[1].to_string(), cap[2].to_string());
                    }
                    // populate response when available
                    if response.developers.is_none() {
                        if let Some(dev) = map.get("developer") {
                            response.developers = Some(vec![dev.to_string()]);
                        }
                    }
                    if response.publishers.is_none() {
                        if let Some(pubr) = map.get("publisher") {
                            response.publishers = Some(vec![pubr.to_string()]);
                        }
                    }
                    if response.short_description.is_none() {
                        if let Some(sd) = map.get("short_description") {
                            response.short_description = Some(sd.to_string());
                        }
                    }
                    if response.header_image.is_none() {
                        if let Some(h) = map.get("header_image") {
                            response.header_image = Some(h.to_string());
                        }
                    }
                    if response.franchise.is_none() {
                        if let Some(fr) = map.get("franchise") {
                            response.franchise = Some(fr.to_string());
                        }
                    }
                    // Build a minimal JSON details object to cache so downstream callers can reuse it
                    let mut obj = serde_json::Map::new();
                    let mut data_map = serde_json::Map::new();
                    if let Some(dev) = map.get("developer") {
                        data_map.insert(
                            "developers".to_string(),
                            serde_json::Value::Array(vec![serde_json::Value::String(
                                dev.to_string(),
                            )]),
                        );
                    }
                    if let Some(pubr) = map.get("publisher") {
                        data_map.insert(
                            "publishers".to_string(),
                            serde_json::Value::Array(vec![serde_json::Value::String(
                                pubr.to_string(),
                            )]),
                        );
                    }
                    if let Some(sd) = map.get("short_description") {
                        data_map.insert(
                            "short_description".to_string(),
                            serde_json::Value::String(sd.to_string()),
                        );
                    }
                    if let Some(h) = map.get("header_image") {
                        data_map.insert(
                            "header_image".to_string(),
                            serde_json::Value::String(h.to_string()),
                        );
                    }
                    if let Some(fr) = map.get("franchise") {
                        data_map.insert(
                            "franchise".to_string(),
                            serde_json::Value::String(fr.to_string()),
                        );
                    }
                    obj.insert("data".to_string(), serde_json::Value::Object(data_map));
                    obj.insert("success".to_string(), serde_json::Value::Bool(true));
                    let entry = serde_json::Value::Object(obj);
                    let _ = crate::cache_steam_app_details(&connection, app_id, &entry);
                    // also expose parsed JSON data for later normalized feature building
                    if let Some(d) = entry.get("data") {
                        maybe_data = Some(d.clone());
                    }
                    // If we got any meaningful value, skip the HTTP store fetch.
                    if response.short_description.is_some()
                        || response.developers.is_some()
                        || response.publishers.is_some()
                    {
                        // proceed to feature inference later; we have cached details now
                    } else {
                        // fall through to HTTP fetch below
                    }
                }
            }
        }
        if let Ok(client) = crate::build_http_client() {
            // If steamcmd already provided useful fields, skip the HTTP store fetch.
            if response.short_description.is_some()
                || response.developers.is_some()
                || response.publishers.is_some()
            {
                // skip HTTP fetch: we prefer steamcmd results when present
            } else {
                let mut request_url = match url::Url::parse(crate::STEAM_APP_DETAILS_ENDPOINT) {
                    Ok(u) => u,
                    Err(_) => Url::parse("https://store.steampowered.com/api/appdetails").unwrap(),
                };
                // append query
                request_url
                    .query_pairs_mut()
                    .append_pair("appids", &app_id.to_string())
                    .append_pair("l", "english");
                if let Ok(resp) = client.get(request_url).send() {
                    if resp.status().is_success() {
                        if let Ok(payload) = resp.json::<serde_json::Value>() {
                            if let Some(entry) = payload.get(&app_id.to_string()) {
                                if entry
                                    .get("success")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false)
                                {
                                    if let Some(data) = entry.get("data") {
                                        // capture parsed data for normalized feature building
                                        maybe_data = Some(data.clone());
                                        let _ = crate::cache_steam_app_details(
                                            &connection,
                                            app_id,
                                            data,
                                        );
                                        // infer features similar to cache_steam_app_details implementation
                                        let has_achievements = data.get("achievements").is_some();
                                        let has_cloud = data
                                            .get("cloud")
                                            .and_then(|v| {
                                                v.get("enabled")
                                                    .and_then(serde_json::Value::as_bool)
                                            })
                                            .unwrap_or_else(|| data.get("cloud").is_some());
                                        let mut controller_support: Option<String> = None;
                                        if let Some(categories) = data
                                            .get("categories")
                                            .and_then(serde_json::Value::as_array)
                                        {
                                            for cat in categories {
                                                if let Some(desc) = cat
                                                    .get("description")
                                                    .and_then(serde_json::Value::as_str)
                                                {
                                                    let lowered = desc.to_ascii_lowercase();
                                                    if lowered.contains("full controller")
                                                        || lowered
                                                            .contains("full controller support")
                                                    {
                                                        controller_support =
                                                            Some(String::from("Full"));
                                                        break;
                                                    }
                                                    if lowered.contains("partial controller")
                                                        || lowered
                                                            .contains("partial controller support")
                                                    {
                                                        controller_support =
                                                            Some(String::from("Partial"));
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                        if controller_support.is_none() {
                                            if let Some(cs) = data
                                                .get("controller_support")
                                                .and_then(serde_json::Value::as_str)
                                            {
                                                controller_support = Some(cs.to_owned());
                                            } else if let Some(cs) = data
                                                .get("controller_supports")
                                                .and_then(serde_json::Value::as_str)
                                            {
                                                controller_support = Some(cs.to_owned());
                                            }
                                        }
                                        let _ = crate::cache_steam_app_features(
                                            &connection,
                                            app_id,
                                            has_achievements,
                                            None,
                                            has_cloud,
                                            None,
                                            controller_support.as_deref(),
                                        );

                                        // apply freshly fetched data to response
                                        if let Some(devs) =
                                            data.get("developers").and_then(|v| v.as_array())
                                        {
                                            let mut out: Vec<String> = Vec::new();
                                            for d in devs {
                                                if let Some(s) = d.as_str() {
                                                    out.push(s.to_owned());
                                                }
                                            }
                                            if !out.is_empty() {
                                                response.developers = Some(out);
                                            }
                                        }
                                        if let Some(pubs) =
                                            data.get("publishers").and_then(|v| v.as_array())
                                        {
                                            let mut out: Vec<String> = Vec::new();
                                            for p in pubs {
                                                if let Some(s) = p.as_str() {
                                                    out.push(s.to_owned());
                                                }
                                            }
                                            if !out.is_empty() {
                                                response.publishers = Some(out);
                                            }
                                        }
                                        if let Some(fr) = data
                                            .get("franchise")
                                            .and_then(serde_json::Value::as_str)
                                        {
                                            response.franchise = Some(fr.to_owned());
                                        }
                                        if let Some(rel) = data
                                            .get("release_date")
                                            .and_then(|v| v.get("date"))
                                            .and_then(serde_json::Value::as_str)
                                        {
                                            response.release_date = Some(rel.to_owned());
                                        }
                                        if let Some(sd) = data
                                            .get("short_description")
                                            .and_then(serde_json::Value::as_str)
                                        {
                                            response.short_description = Some(sd.to_owned());
                                        }
                                        if let Some(h) = data
                                            .get("header_image")
                                            .and_then(serde_json::Value::as_str)
                                        {
                                            response.header_image = Some(h.to_owned());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if let Ok(Some((has_ach, ach_count_opt, has_cloud, cloud_details_opt, controller_opt))) =
        find_cached_steam_app_features(&connection, app_id, stale_before)
    {
        response.has_achievements = Some(has_ach);
        response.achievements_count = ach_count_opt;
        response.has_cloud_saves = Some(has_cloud);
        response.cloud_details = cloud_details_opt;
        response.controller_support = controller_opt;
    }

    // Build a normalized features list using parsed store data and inferred flags.
    {
        let mut features: Vec<FeatureResponse> = Vec::new();

        if let Some(ref data) = maybe_data {
            // Categories mapping (Steam often lists these on the right)
            if let Some(categories) = data.get("categories").and_then(serde_json::Value::as_array) {
                let mut seen_keys: std::collections::HashSet<String> =
                    std::collections::HashSet::new();
                // helper to canonicalize description to a preferred feature key/label
                let canonical_from_desc = |desc: &str| -> Option<(String, String)> {
                    let lowered = desc.to_ascii_lowercase();
                    if lowered.contains("remote play together") || lowered.contains("remote play") {
                        // prefer showing Family Sharing instead of Remote Play Together per UX preference
                        return Some(("family-sharing".to_string(), "Family Sharing".to_string()));
                    }
                    if lowered.contains("steam cloud")
                        || lowered.contains("steam cloud saves")
                        || lowered.contains("cloud saves")
                        || lowered == "cloud"
                    {
                        return Some(("cloud-saves".to_string(), "Cloud Saves".to_string()));
                    }
                    // suppress Trading Cards entries — they are redundant in our UI
                    if lowered.contains("trading card") || lowered.contains("trading cards") {
                        return None;
                    }
                    if lowered.contains("multi-player") || lowered.contains("multiplayer") {
                        return Some(("multi-player".to_string(), "Multi-Player".to_string()));
                    }
                    if lowered.contains("co-op") || lowered.contains("cooperative") {
                        return Some(("multi-player".to_string(), "Multi-Player".to_string()));
                    }
                    if lowered.contains("single-player") || lowered.contains("single player") {
                        return Some(("single-player".to_string(), "Single-Player".to_string()));
                    }
                    if lowered.contains("achievements") || lowered.contains("steam achievements") {
                        return Some(("achievements".to_string(), "Achievements".to_string()));
                    }
                    if lowered.contains("full controller") {
                        return Some((
                            "controller-full".to_string(),
                            "Full Controller Support".to_string(),
                        ));
                    }
                    if lowered.contains("partial controller") {
                        return Some((
                            "controller-partial".to_string(),
                            "Partial Controller Support".to_string(),
                        ));
                    }
                    if lowered.contains("workshop") {
                        return Some(("workshop".to_string(), "Steam Workshop".to_string()));
                    }
                    if lowered.contains("family sharing")
                        || lowered.contains("family-share")
                        || lowered.contains("family_share")
                    {
                        return Some(("family-sharing".to_string(), "Family Sharing".to_string()));
                    }
                    // Suppress explicit Trading Cards category by returning a skip marker
                    if lowered.contains("trading card") || lowered.contains("trading cards") {
                        return Some(("__skip__".to_string(), "".to_string()));
                    }
                    None
                };
                for cat in categories {
                    let id_opt = cat.get("id").and_then(|v| v.as_u64());
                    let desc_opt = cat
                        .get("description")
                        .and_then(serde_json::Value::as_str)
                        .map(|s| s.to_string());
                    if let Some(desc) = desc_opt.as_deref() {
                        if let Some((key, label)) = canonical_from_desc(desc) {
                            // allow canonical helper to mark items to skip (e.g., trading cards)
                            if key == "__skip__" {
                                continue;
                            }
                            if seen_keys.insert(key.clone()) {
                                features.push(FeatureResponse {
                                    key: key.clone(),
                                    label: label.clone(),
                                    icon: None,
                                    tooltip: None,
                                });
                            }
                            // don't also add generic category-<id> when a canonical mapping applies
                            continue;
                        }
                    }
                    // no canonical mapping: include category id-based feature so raw ids are available in UI
                    let label = desc_opt
                        .clone()
                        .or_else(|| id_opt.map(|id| format!("Category {}", id)))
                        .unwrap_or_else(|| "Category".to_string());
                    let key = if let Some(id) = id_opt {
                        format!("category-{}", id)
                    } else {
                        label.to_ascii_lowercase().replace(' ', "-")
                    };
                    if seen_keys.insert(key.clone()) {
                        features.push(FeatureResponse {
                            key: key.clone(),
                            label: label.clone(),
                            icon: None,
                            tooltip: None,
                        });
                    }
                }
            }

            // Controller-specific strings (DualShock / DualSense) may appear in other fields
            let as_string = data.to_string().to_ascii_lowercase();
            if as_string.contains("dualshock") {
                features.push(FeatureResponse {
                    key: "controller-dualshock".to_string(),
                    label: "DualShock Support".to_string(),
                    icon: Some("dualshock".to_string()),
                    tooltip: None,
                });
            }
            if as_string.contains("dualsense") {
                features.push(FeatureResponse {
                    key: "controller-dualsense".to_string(),
                    label: "DualSense Support".to_string(),
                    icon: Some("dualsense".to_string()),
                    tooltip: None,
                });
            }
            // Steam Workshop
            if as_string.contains("workshop") || as_string.contains("steam workshop") {
                if !as_string.contains("trading card") && !as_string.contains("trading cards") {
                    features.push(FeatureResponse {
                        key: "workshop".to_string(),
                        label: "Steam Workshop".to_string(),
                        icon: Some("workshop".to_string()),
                        tooltip: None,
                    });
                }
            }
            // Family Sharing eligibility
            if as_string.contains("family sharing")
                || as_string.contains("family-share")
                || as_string.contains("family_share")
            {
                if !as_string.contains("trading card") && !as_string.contains("trading cards") {
                    features.push(FeatureResponse {
                        key: "family-sharing".to_string(),
                        label: "Family Sharing".to_string(),
                        icon: Some("family".to_string()),
                        tooltip: None,
                    });
                }
            }
        }

        // Achievements (use inferred/cached flag if present)
        if response.has_achievements.unwrap_or(false) {
            let tooltip = response
                .achievements_count
                .map(|c| format!("{} achievements", c));
            features.push(FeatureResponse {
                key: "achievements".to_string(),
                label: "Achievements".to_string(),
                icon: Some("trophy".to_string()),
                tooltip,
            });
        }

        // Cloud saves
        if response.has_cloud_saves.unwrap_or(false) {
            features.push(FeatureResponse {
                key: "cloud-saves".to_string(),
                label: "Cloud Saves".to_string(),
                icon: Some("cloud".to_string()),
                tooltip: response.cloud_details.clone(),
            });
        }

        // Controller support summary
        if let Some(ref ctrl) = response.controller_support {
            features.push(FeatureResponse {
                key: "controller-support".to_string(),
                label: format!("Controller: {}", ctrl),
                icon: Some("gamepad".to_string()),
                tooltip: None,
            });
        }

        if !features.is_empty() {
            response.features = Some(features);
        }
    }

    Ok(response)
}
