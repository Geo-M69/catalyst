use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use chrono::{Duration as ChronoDuration, TimeZone, Utc};
use regex::Regex;
use reqwest::blocking::Client;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
#[cfg(target_os = "linux")]
use tauri::Manager;
use url::Url;
#[cfg(target_os = "linux")]
use webkit2gtk::{HardwareAccelerationPolicy, SettingsExt, WebViewExt};

const STEAM_OPENID_ENDPOINT: &str = "https://steamcommunity.com/openid/login";
const STEAM_WEB_API_ENDPOINT: &str =
    "https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/";
const STEAM_APP_DETAILS_ENDPOINT: &str = "https://store.steampowered.com/api/appdetails";
const STEAM_STORE_APP_ENDPOINT: &str = "https://store.steampowered.com/app";
const STEAM_PUBLIC_APP_LIST_ENDPOINTS: [&str; 3] = [
    "https://api.steampowered.com/ISteamApps/GetAppList/v0002/",
    "https://api.steampowered.com/ISteamApps/GetAppList/v2/",
    "http://api.steampowered.com/ISteamApps/GetAppList/v0002/",
];
const STEAM_CALLBACK_PUBLIC_HOST: &str = "catalyst";
const STEAM_APP_BETAS_ENDPOINT: &str = "https://api.steampowered.com/ISteamApps/GetAppBetas/v1/";
const STEAM_APP_BETA_CODE_CHECK_ENDPOINT: &str =
    "https://api.steampowered.com/ISteamApps/CheckAppBetaPassword/v1/";
const STEAM_CALLBACK_TIMEOUT: Duration = Duration::from_secs(180);
const STEAM_APP_DETAILS_BATCH_SIZE: usize = 75;
const STEAM_APP_DETAILS_CACHE_TTL_HOURS: i64 = 24 * 7; // 1 week
const STEAM_APP_METADATA_CACHE_TTL_HOURS: i64 = 24 * 7;
const STEAM_APP_LANGUAGES_CACHE_TTL_HOURS: i64 = 24 * 7;
const STEAM_APP_BETAS_CACHE_TTL_HOURS: i64 = 24 * 7;
const STEAM_APP_STORE_TAGS_CACHE_TTL_HOURS: i64 = 24 * 7;
const STEAM_STORE_TAGS_SYNC_MAX_REQUESTS: usize = 4;
const STEAM_STORE_TAGS_SYNC_TIME_BUDGET_SECS: u64 = 12;
const STEAM_PUBLIC_APP_NAME_CACHE_TTL_HOURS: i64 = 24 * 30;
const STEAM_PUBLIC_APP_NAME_LOOKUP_MAX_REQUESTS_PER_SYNC: usize = 300;
const STEAM_PUBLIC_APP_NAME_LOOKUP_TIME_BUDGET_SECS: u64 = 20;
const STEAM_PUBLIC_APP_NAME_LOOKUP_MAX_CONSECUTIVE_ERRORS: usize = 5;
const SESSION_TTL_DAYS: i64 = 30;
const STEAM_ID64_ACCOUNT_ID_BASE: u64 = 76_561_197_960_265_728;
const STEAM_CALLBACK_FALLBACK_HOST: &str = "127.0.0.1";
const STEAM_BUILTIN_COMPATIBILITY_TOOLS: [(&str, &str); 7] = [
    ("proton_experimental", "Proton Experimental"),
    ("proton_hotfix", "Proton Hotfix"),
    ("proton_9", "Proton 9.0-4"),
    ("proton_8", "Proton 8.0-5"),
    ("proton_7", "Proton 7.0-6"),
    ("sniper", "Steam Linux Runtime 3.0 (sniper)"),
    ("soldier", "Steam Linux Runtime 2.0 (soldier)"),
];
const STEAM_APP_STATE_UPDATE_REQUIRED: u64 = 0x2;
const STEAM_APP_STATE_FULLY_INSTALLED: u64 = 0x4;
const STEAM_APP_STATE_UPDATE_RUNNING: u64 = 0x100;
const STEAM_APP_STATE_UPDATE_PAUSED: u64 = 0x200;
const STEAM_APP_STATE_UPDATE_STARTED: u64 = 0x400;
const STEAM_APP_STATE_VALIDATING: u64 = 0x20_000;
const STEAM_APP_STATE_ADDING_FILES: u64 = 0x40_000;
const STEAM_APP_STATE_PREALLOCATING: u64 = 0x80_000;
const STEAM_APP_STATE_DOWNLOADING: u64 = 0x100_000;
const STEAM_APP_STATE_STAGING: u64 = 0x200_000;
const STEAM_APP_STATE_COMMITTING: u64 = 0x400_000;
const STEAM_DIRECTORY_PROGRESS_MANIFEST_STALE_SECONDS: u64 = 20;
const STEAM_DIRECTORY_PROGRESS_MIN_DELTA_BYTES: u64 = 256 * 1024 * 1024;
const STEAM_DIRECTORY_PROGRESS_BLEND_FACTOR: f64 = 0.5;

#[derive(Debug, Clone)]
struct UserRow {
    id: String,
    email: String,
    steam_id: Option<String>,
}
#[derive(Debug, Clone)]
struct LibraryGameInput {
    external_id: String,
    name: String,
    kind: String,
    playtime_minutes: i64,
    installed: bool,
    artwork_url: Option<String>,
    last_synced_at: String,
    last_played_at: Option<String>,
}

#[derive(Debug, Clone)]
struct LocalSteamAppHistoryEntry {
    app_id: u64,
    name: Option<String>,
    playtime_minutes: i64,
    last_played_at: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PublicUser {
    id: String,
    email: String,
    steam_linked: bool,
    steam_id: Option<String>,
}

struct SteamAuthOutcome {
    user: UserRow,
    synced_games: usize,
    session_token: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GameResponse {
    id: String,
    provider: String,
    external_id: String,
    name: String,
    kind: String,
    playtime_minutes: i64,
    installed: bool,
    artwork_url: Option<String>,
    last_synced_at: String,
    last_played_at: Option<String>,
    favorite: bool,
    steam_tags: Vec<String>,
    genres: Vec<String>,
    collections: Vec<String>,
    hide_in_library: bool,
    // Enriched metadata from store (when available)
    developers: Vec<String>,
    publishers: Vec<String>,
    franchise: Option<String>,
    release_date: Option<String>,
    short_description: Option<String>,
    header_image: Option<String>,
    // Inferred / cached feature flags
    has_achievements: bool,
    has_cloud_saves: bool,
    controller_support: Option<String>,
    achievements_count: Option<i64>,
    cloud_details: Option<String>,
    features: Vec<FeatureResponse>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FeatureResponse {
    key: String,
    label: String,
    icon: Option<String>,
    tooltip: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CollectionResponse {
    id: String,
    name: String,
    game_count: usize,
    contains_game: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SteamCollectionsImportResponse {
    apps_tagged: usize,
    collections_created: usize,
    memberships_added: usize,
    skipped_games: usize,
    tags_discovered: usize,
}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
struct GamePrivacySettingsResponse {
    hide_in_library: bool,
    mark_as_private: bool,
    overlay_data_deleted: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SteamDownloadProgressResponse {
    game_id: String,
    provider: String,
    external_id: String,
    name: String,
    state: String,
    bytes_downloaded: Option<u64>,
    bytes_total: Option<u64>,
    progress_percent: Option<f64>,
    progress_source: Option<String>,
}

#[derive(Clone)]
struct OwnedSteamGameMetadata {
    game_id: String,
    external_id: String,
    name: String,
}

struct SteamManifestDownloadProgressSnapshot {
    state_flags: Option<u64>,
    bytes_downloaded: Option<u64>,
    bytes_to_download: Option<u64>,
    bytes_staged: Option<u64>,
    bytes_to_stage: Option<u64>,
}

struct ResolvedSteamDownloadProgressSnapshot {
    state_flags: Option<u64>,
    bytes_downloaded: Option<u64>,
    bytes_total: Option<u64>,
    progress_source: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GameCompatibilityToolResponse {
    id: String,
    label: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GameGeneralSettingsPayload {
    language: String,
    launch_options: String,
    steam_overlay_enabled: bool,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GameCompatibilitySettingsPayload {
    force_steam_play_compatibility_tool: bool,
    steam_play_compatibility_tool: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GameUpdatesSettingsPayload {
    automatic_updates_mode: String,
    background_downloads_mode: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GameControllerSettingsPayload {
    steam_input_override: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GameVersionsBetasSettingsPayload {
    private_access_code: String,
    selected_version_id: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GameCustomizationSettingsPayload {
    custom_sort_name: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GamePropertiesSettingsPayload {
    general: GameGeneralSettingsPayload,
    compatibility: GameCompatibilitySettingsPayload,
    updates: GameUpdatesSettingsPayload,
    controller: GameControllerSettingsPayload,
    #[serde(default = "default_game_customization_settings_payload")]
    customization: GameCustomizationSettingsPayload,
    game_versions_betas: GameVersionsBetasSettingsPayload,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GameCustomizationArtworkResponse {
    cover: Option<String>,
    background: Option<String>,
    logo: Option<String>,
    wide_cover: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GameVersionBetaOptionResponse {
    id: String,
    name: String,
    description: String,
    last_updated: String,
    build_id: Option<String>,
    requires_access_code: bool,
    is_default: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GameBetaAccessCodeValidationResponse {
    valid: bool,
    message: String,
    branch_id: Option<String>,
    branch_name: Option<String>,
}

#[derive(Deserialize)]
struct SteamOwnedGamesApiResponse {
    response: Option<SteamOwnedGamesPayload>,
}

#[derive(Deserialize)]
struct SteamOwnedGamesPayload {
    games: Option<Vec<SteamOwnedGame>>,
}

#[derive(Deserialize)]
struct SteamOwnedGame {
    appid: u64,
    name: Option<String>,
    playtime_forever: Option<i64>,
    img_logo_url: Option<String>,
    img_icon_url: Option<String>,
    rtime_last_played: Option<i64>,
}

fn complete_steam_auth_flow(
    db_path: &Path,
    steam_api_key: Option<String>,
    steam_local_install_detection: bool,
    steam_root_override: Option<String>,
    current_session_token: Option<String>,
) -> Result<SteamAuthOutcome, String> {
    crate::infrastructure::runtime_auth::complete_steam_auth_flow(
        db_path,
        steam_api_key,
        steam_local_install_detection,
        steam_root_override,
        current_session_token,
    )
}

fn collect_locally_known_steam_games_from_app_ids(
    connection: &Connection,
    user: &UserRow,
    app_ids: &HashSet<u64>,
) -> Result<Vec<LibraryGameInput>, String> {
    if app_ids.is_empty() {
        return Ok(Vec::new());
    }

    let existing_game_names = load_provider_game_names(connection, &user.id, "steam")?;
    let now = Utc::now().to_rfc3339();
    let mut local_games = Vec::with_capacity(app_ids.len());
    for app_id in app_ids {
        let external_id = app_id.to_string();
        let name = existing_game_names
            .get(&external_id)
            .cloned()
            .unwrap_or_else(|| format!("Steam App {external_id}"));
        let kind = classify_steam_game_kind(&name).to_owned();
        let artwork_url = Some(format!(
            "https://cdn.cloudflare.steamstatic.com/steam/apps/{external_id}/capsule_231x87.jpg"
        ));

        local_games.push(LibraryGameInput {
            external_id,
            name,
            kind,
            playtime_minutes: 0,
            installed: false,
            artwork_url,
            last_synced_at: now.clone(),
            last_played_at: None,
        });
    }

    Ok(local_games)
}

fn sync_steam_games_for_user(
    connection: &Connection,
    user: &UserRow,
    steam_api_key: Option<&str>,
    steam_local_install_detection: bool,
    steam_root_override: Option<&str>,
    client: &Client,
) -> Result<usize, String> {
    let steam_id = user
        .steam_id
        .as_deref()
        .ok_or_else(|| String::from("User is not linked to Steam"))?;
    let steam_local_signal_app_ids =
        collect_signal_steam_library_app_ids_from_local_state(steam_root_override, steam_id);

    let locally_installed_games = if steam_local_install_detection {
        match collect_locally_installed_steam_games(steam_root_override) {
            Ok(games) => Some(games),
            Err(error) => {
                eprintln!("Local Steam install detection failed: {error}");
                None
            }
        }
    } else {
        None
    };
    let locally_installed_app_ids = if steam_local_install_detection {
        locally_installed_games.as_ref().map(|games| {
            games
                .iter()
                .filter_map(|game| game.external_id.parse::<u64>().ok())
                .collect::<HashSet<_>>()
        })
    } else {
        Some(HashSet::new())
    };

    let Some(api_key) = steam_api_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        if steam_local_install_detection {
            let mut games_by_external_id = HashMap::new();
            if let Some(local_games) = locally_installed_games.as_ref() {
                for local_game in local_games {
                    games_by_external_id.insert(local_game.external_id.clone(), local_game.clone());
                }
            }
            match collect_locally_known_steam_games_from_app_ids(
                connection,
                user,
                &steam_local_signal_app_ids,
            ) {
                Ok(dynamic_owned_games) => {
                    for dynamic_owned_game in dynamic_owned_games {
                        games_by_external_id
                            .entry(dynamic_owned_game.external_id.clone())
                            .or_insert(dynamic_owned_game);
                    }
                }
                Err(error) => {
                    eprintln!("Local Steam signal app merge failed: {error}");
                }
            }

            match collect_locally_known_steam_games_from_localconfig(
                connection,
                user,
                steam_root_override,
                true,
            ) {
                Ok(localconfig_games) => {
                    for localconfig_game in localconfig_games {
                        if let Some(existing_game) =
                            games_by_external_id.get_mut(&localconfig_game.external_id)
                        {
                            if existing_game.playtime_minutes <= 0
                                && localconfig_game.playtime_minutes > 0
                            {
                                existing_game.playtime_minutes = localconfig_game.playtime_minutes;
                            }
                            if existing_game.last_played_at.is_none()
                                && localconfig_game.last_played_at.is_some()
                            {
                                existing_game.last_played_at = localconfig_game.last_played_at;
                            }
                            if should_replace_placeholder_steam_game_name(
                                &existing_game.name,
                                &existing_game.external_id,
                                &localconfig_game.name,
                                &localconfig_game.external_id,
                            ) {
                                existing_game.name = localconfig_game.name;
                                existing_game.kind = localconfig_game.kind;
                            }
                            if existing_game.artwork_url.is_none()
                                && localconfig_game.artwork_url.is_some()
                            {
                                existing_game.artwork_url = localconfig_game.artwork_url;
                            }
                            continue;
                        }

                        games_by_external_id
                            .insert(localconfig_game.external_id.clone(), localconfig_game);
                    }
                }
                Err(error) => {
                    eprintln!("Local Steam localconfig fallback failed: {error}");
                }
            }

            match collect_locally_known_steam_games_from_sharedconfig(
                connection,
                user,
                steam_root_override,
                true,
            ) {
                Ok(sharedconfig_games) => {
                    for sharedconfig_game in sharedconfig_games {
                        if let Some(existing_game) =
                            games_by_external_id.get_mut(&sharedconfig_game.external_id)
                        {
                            if existing_game.playtime_minutes <= 0
                                && sharedconfig_game.playtime_minutes > 0
                            {
                                existing_game.playtime_minutes = sharedconfig_game.playtime_minutes;
                            }
                            if existing_game.last_played_at.is_none()
                                && sharedconfig_game.last_played_at.is_some()
                            {
                                existing_game.last_played_at = sharedconfig_game.last_played_at;
                            }
                            if should_replace_placeholder_steam_game_name(
                                &existing_game.name,
                                &existing_game.external_id,
                                &sharedconfig_game.name,
                                &sharedconfig_game.external_id,
                            ) {
                                existing_game.name = sharedconfig_game.name;
                                existing_game.kind = sharedconfig_game.kind;
                            }
                            if existing_game.artwork_url.is_none()
                                && sharedconfig_game.artwork_url.is_some()
                            {
                                existing_game.artwork_url = sharedconfig_game.artwork_url;
                            }
                            continue;
                        }

                        games_by_external_id
                            .insert(sharedconfig_game.external_id.clone(), sharedconfig_game);
                    }
                }
                Err(error) => {
                    eprintln!("Local Steam sharedconfig fallback failed: {error}");
                }
            }

            match collect_locally_known_steam_games_from_librarycache(
                connection,
                user,
                steam_root_override,
            ) {
                Ok(librarycache_games) => {
                    let selected_app_ids = collect_selected_steam_library_app_ids_from_localconfig(
                        steam_root_override,
                        steam_id,
                    );
                    let target_app_id = steam_sync_debug_target_app_id();
                    if steam_sync_debug_logging_enabled() {
                        let target_in_librarycache = target_app_id.is_some_and(|target| {
                            librarycache_games
                                .iter()
                                .any(|game| game.external_id.parse::<u64>().ok() == Some(target))
                        });
                        let target_in_selected =
                            target_app_id.is_some_and(|target| selected_app_ids.contains(&target));
                        let target_in_signal_set =
                            target_app_id.is_some_and(|target| steam_local_signal_app_ids.contains(&target));
                        let target_preexisting = target_app_id.is_some_and(|target| {
                            games_by_external_id.contains_key(&target.to_string())
                        });
                        log_steam_sync_debug(&format!(
                            "librarycache fallback merge: games={}, selected_app_ids={}, signal_app_ids={}, target_app_id={:?}, target_in_librarycache={}, target_in_selected={}, target_in_signal_set={}, target_preexisting={}",
                            librarycache_games.len(),
                            selected_app_ids.len(),
                            steam_local_signal_app_ids.len(),
                            target_app_id,
                            target_in_librarycache,
                            target_in_selected,
                            target_in_signal_set,
                            target_preexisting
                        ));
                    }
                    for librarycache_game in librarycache_games {
                        if let Some(existing_game) =
                            games_by_external_id.get_mut(&librarycache_game.external_id)
                        {
                            if should_replace_placeholder_steam_game_name(
                                &existing_game.name,
                                &existing_game.external_id,
                                &librarycache_game.name,
                                &librarycache_game.external_id,
                            ) {
                                existing_game.name = librarycache_game.name;
                                existing_game.kind = librarycache_game.kind;
                            }
                            if existing_game.artwork_url.is_none()
                                && librarycache_game.artwork_url.is_some()
                            {
                                existing_game.artwork_url = librarycache_game.artwork_url;
                            }
                            continue;
                        }

                        let Some(app_id) = librarycache_game.external_id.parse::<u64>().ok() else {
                            continue;
                        };
                        if selected_app_ids.contains(&app_id)
                            || steam_local_signal_app_ids.contains(&app_id)
                        {
                            if steam_sync_debug_logging_enabled() && target_app_id == Some(app_id) {
                                log_steam_sync_debug(&format!(
                                    "including target app {} from librarycache fallback branch",
                                    app_id
                                ));
                            }
                            games_by_external_id.insert(
                                librarycache_game.external_id.clone(),
                                librarycache_game,
                            );
                        } else if steam_sync_debug_logging_enabled() && target_app_id == Some(app_id) {
                            log_steam_sync_debug(&format!(
                                "skipping target app {} from librarycache fallback branch because app is not selected and not in local signal set",
                                app_id
                            ));
                        }
                    }
                }
                Err(error) => {
                    eprintln!("Local Steam librarycache fallback failed: {error}");
                }
            }

            if let Err(error) = hydrate_local_steam_game_names_from_manifests(
                &mut games_by_external_id,
                steam_root_override,
            ) {
                eprintln!("Local Steam manifest name fallback failed: {error}");
            }
            if let Err(error) = hydrate_local_steam_game_names_from_public_catalog(
                connection,
                client,
                &mut games_by_external_id,
                steam_root_override,
            ) {
                eprintln!("Local Steam public catalog name fallback failed: {error}");
            }
            let local_app_ids = games_by_external_id
                .keys()
                .filter_map(|external_id| external_id.parse::<u64>().ok())
                .collect::<Vec<_>>();
            if !local_app_ids.is_empty() {
                match resolve_steam_app_kinds_for_app_ids(connection, client, &local_app_ids) {
                    Ok(kinds_by_app_id) => {
                        for game in games_by_external_id.values_mut() {
                            let Some(app_id) = game.external_id.parse::<u64>().ok() else {
                                continue;
                            };
                            let Some(kind) = kinds_by_app_id.get(&app_id) else {
                                continue;
                            };
                            game.kind = kind.clone();
                        }
                    }
                    Err(error) => {
                        eprintln!("Local Steam app kind fallback failed: {error}");
                    }
                }
            }

            if !games_by_external_id.is_empty() {
                let mut merged_local_games = games_by_external_id
                    .into_values()
                    .collect::<Vec<LibraryGameInput>>();
                merged_local_games.sort_by(|left, right| {
                    right
                        .last_played_at
                        .cmp(&left.last_played_at)
                        .then_with(|| right.playtime_minutes.cmp(&left.playtime_minutes))
                        .then_with(|| left.external_id.cmp(&right.external_id))
                });
                let trusted_external_ids = merged_local_games
                    .iter()
                    .map(|game| game.external_id.clone())
                    .collect::<HashSet<_>>();
                if let Err(error) = prune_untrusted_placeholder_steam_games(
                    connection,
                    &user.id,
                    &trusted_external_ids,
                ) {
                    eprintln!("Could not prune untrusted placeholder Steam games: {error}");
                }
                upsert_provider_games(connection, &user.id, "steam", &merged_local_games)?;
                if let Some(app_ids) = locally_installed_app_ids.as_ref() {
                    refresh_provider_installed_flags(connection, &user.id, "steam", app_ids)?;
                }
                return Ok(merged_local_games.len());
            }

            if let Some(app_ids) = locally_installed_app_ids.as_ref() {
                refresh_provider_installed_flags(connection, &user.id, "steam", app_ids)?;
            }
        } else if let Some(app_ids) = locally_installed_app_ids.as_ref() {
            refresh_provider_installed_flags(connection, &user.id, "steam", app_ids)?;
        }

        return Ok(0);
    };

    let mut request_url = Url::parse(STEAM_WEB_API_ENDPOINT)
        .map_err(|error| format!("Failed to parse Steam games endpoint: {error}"))?;
    request_url
        .query_pairs_mut()
        .append_pair("key", api_key)
        .append_pair("steamid", steam_id)
        .append_pair("include_appinfo", "true")
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
        .json::<SteamOwnedGamesApiResponse>()
        .map_err(|error| format!("Failed to decode Steam owned games response: {error}"))?;

    let steam_owned_games = payload
        .response
        .and_then(|response| response.games)
        .unwrap_or_default();
    let existing_installed_flags = if locally_installed_app_ids.is_none() {
        load_provider_installed_flags(connection, &user.id, "steam")?
    } else {
        HashMap::new()
    };
    let steam_owned_app_ids = steam_owned_games
        .iter()
        .map(|game| game.appid)
        .collect::<Vec<_>>();
    let resolved_kinds = resolve_steam_game_kinds(connection, client, &steam_owned_games)?;
    let games = steam_owned_games
        .into_iter()
        .map(|game| {
            let resolved_kind = resolved_kinds.get(&game.appid).map(String::as_str);
            let installed = locally_installed_app_ids
                .as_ref()
                .map(|app_ids| app_ids.contains(&game.appid))
                .unwrap_or_else(|| {
                    existing_installed_flags
                        .get(&game.appid)
                        .copied()
                        .unwrap_or(false)
                });
            map_steam_game(game, resolved_kind, installed)
        })
        .collect::<Vec<_>>();
    let mut games_by_external_id = games
        .into_iter()
        .map(|game| (game.external_id.clone(), game))
        .collect::<HashMap<_, _>>();
    let steam_owned_app_id_set = steam_owned_app_ids.iter().copied().collect::<HashSet<_>>();
    let signal_only_app_ids = steam_local_signal_app_ids
        .difference(&steam_owned_app_id_set)
        .copied()
        .collect::<HashSet<_>>();
    match collect_locally_known_steam_games_from_app_ids(connection, user, &signal_only_app_ids) {
        Ok(dynamic_owned_games) => {
            for dynamic_owned_game in dynamic_owned_games {
                games_by_external_id
                    .entry(dynamic_owned_game.external_id.clone())
                    .or_insert(dynamic_owned_game);
            }
        }
        Err(error) => {
            eprintln!("Local Steam signal app merge failed: {error}");
        }
    }

    if steam_local_install_detection {
        if let Some(local_games) = locally_installed_games.as_ref() {
            for local_game in local_games {
                if let Some(existing_game) = games_by_external_id.get_mut(&local_game.external_id) {
                    if local_game.installed {
                        existing_game.installed = true;
                    }
                    if existing_game.playtime_minutes <= 0 && local_game.playtime_minutes > 0 {
                        existing_game.playtime_minutes = local_game.playtime_minutes;
                    }
                    if existing_game.last_played_at.is_none() && local_game.last_played_at.is_some() {
                        existing_game.last_played_at = local_game.last_played_at.clone();
                    }
                    if should_replace_placeholder_steam_game_name(
                        &existing_game.name,
                        &existing_game.external_id,
                        &local_game.name,
                        &local_game.external_id,
                    ) {
                        existing_game.name = local_game.name.clone();
                        existing_game.kind = local_game.kind.clone();
                    }
                    if existing_game.artwork_url.is_none() && local_game.artwork_url.is_some() {
                        existing_game.artwork_url = local_game.artwork_url.clone();
                    }
                    continue;
                }

                games_by_external_id.insert(local_game.external_id.clone(), local_game.clone());
            }
        }

        match collect_locally_known_steam_games_from_localconfig(
            connection,
            user,
            steam_root_override,
            true,
        ) {
            Ok(localconfig_games) => {
                for localconfig_game in localconfig_games {
                    if let Some(existing_game) =
                        games_by_external_id.get_mut(&localconfig_game.external_id)
                    {
                        if existing_game.playtime_minutes <= 0
                            && localconfig_game.playtime_minutes > 0
                        {
                            existing_game.playtime_minutes = localconfig_game.playtime_minutes;
                        }
                        if existing_game.last_played_at.is_none()
                            && localconfig_game.last_played_at.is_some()
                        {
                            existing_game.last_played_at = localconfig_game.last_played_at;
                        }
                        if should_replace_placeholder_steam_game_name(
                            &existing_game.name,
                            &existing_game.external_id,
                            &localconfig_game.name,
                            &localconfig_game.external_id,
                        ) {
                            existing_game.name = localconfig_game.name;
                            existing_game.kind = localconfig_game.kind;
                        }
                        if existing_game.artwork_url.is_none()
                            && localconfig_game.artwork_url.is_some()
                        {
                            existing_game.artwork_url = localconfig_game.artwork_url;
                        }
                        continue;
                    }

                    games_by_external_id.insert(localconfig_game.external_id.clone(), localconfig_game);
                }
            }
            Err(error) => {
                eprintln!("Local Steam localconfig merge failed: {error}");
            }
        }

        match collect_locally_known_steam_games_from_sharedconfig(
            connection,
            user,
            steam_root_override,
            true,
        ) {
            Ok(sharedconfig_games) => {
                for sharedconfig_game in sharedconfig_games {
                    if let Some(existing_game) =
                        games_by_external_id.get_mut(&sharedconfig_game.external_id)
                    {
                        if existing_game.playtime_minutes <= 0
                            && sharedconfig_game.playtime_minutes > 0
                        {
                            existing_game.playtime_minutes = sharedconfig_game.playtime_minutes;
                        }
                        if existing_game.last_played_at.is_none()
                            && sharedconfig_game.last_played_at.is_some()
                        {
                            existing_game.last_played_at = sharedconfig_game.last_played_at;
                        }
                        if should_replace_placeholder_steam_game_name(
                            &existing_game.name,
                            &existing_game.external_id,
                            &sharedconfig_game.name,
                            &sharedconfig_game.external_id,
                        ) {
                            existing_game.name = sharedconfig_game.name;
                            existing_game.kind = sharedconfig_game.kind;
                        }
                        if existing_game.artwork_url.is_none()
                            && sharedconfig_game.artwork_url.is_some()
                        {
                            existing_game.artwork_url = sharedconfig_game.artwork_url;
                        }
                        continue;
                    }

                    games_by_external_id
                        .insert(sharedconfig_game.external_id.clone(), sharedconfig_game);
                }
            }
            Err(error) => {
                eprintln!("Local Steam sharedconfig merge failed: {error}");
            }
        }

        match collect_locally_known_steam_games_from_librarycache(
            connection,
            user,
            steam_root_override,
        ) {
            Ok(librarycache_games) => {
                let selected_app_ids = collect_selected_steam_library_app_ids_from_localconfig(
                    steam_root_override,
                    steam_id,
                );
                let target_app_id = steam_sync_debug_target_app_id();
                if steam_sync_debug_logging_enabled() {
                    let target_in_librarycache = target_app_id.is_some_and(|target| {
                        librarycache_games
                            .iter()
                            .any(|game| game.external_id.parse::<u64>().ok() == Some(target))
                    });
                    let target_in_selected =
                        target_app_id.is_some_and(|target| selected_app_ids.contains(&target));
                    let target_in_signal_set =
                        target_app_id.is_some_and(|target| steam_local_signal_app_ids.contains(&target));
                    let target_preexisting =
                        target_app_id.is_some_and(|target| games_by_external_id.contains_key(&target.to_string()));
                    let target_is_owned_by_api =
                        target_app_id.is_some_and(|target| steam_owned_app_id_set.contains(&target));
                    log_steam_sync_debug(&format!(
                        "librarycache api merge: games={}, selected_app_ids={}, signal_app_ids={}, target_app_id={:?}, target_in_librarycache={}, target_in_selected={}, target_in_signal_set={}, target_preexisting={}, target_owned_by_api={}",
                        librarycache_games.len(),
                        selected_app_ids.len(),
                        steam_local_signal_app_ids.len(),
                        target_app_id,
                        target_in_librarycache,
                        target_in_selected,
                        target_in_signal_set,
                        target_preexisting,
                        target_is_owned_by_api
                    ));
                }
                for librarycache_game in librarycache_games {
                    if let Some(existing_game) =
                        games_by_external_id.get_mut(&librarycache_game.external_id)
                    {
                        if should_replace_placeholder_steam_game_name(
                            &existing_game.name,
                            &existing_game.external_id,
                            &librarycache_game.name,
                            &librarycache_game.external_id,
                        ) {
                            existing_game.name = librarycache_game.name;
                            existing_game.kind = librarycache_game.kind;
                        }
                        if existing_game.artwork_url.is_none()
                            && librarycache_game.artwork_url.is_some()
                        {
                            existing_game.artwork_url = librarycache_game.artwork_url;
                        }
                        continue;
                    }

                    let Some(app_id) = librarycache_game.external_id.parse::<u64>().ok() else {
                        continue;
                    };
                    if selected_app_ids.contains(&app_id)
                        || steam_owned_app_id_set.contains(&app_id)
                        || steam_local_signal_app_ids.contains(&app_id)
                    {
                        if steam_sync_debug_logging_enabled() && target_app_id == Some(app_id) {
                            log_steam_sync_debug(&format!(
                                "including target app {} from librarycache api branch",
                                app_id
                            ));
                        }
                        games_by_external_id
                            .insert(librarycache_game.external_id.clone(), librarycache_game);
                    } else if steam_sync_debug_logging_enabled() && target_app_id == Some(app_id) {
                        log_steam_sync_debug(&format!(
                            "skipping target app {} from librarycache api branch because app is not selected, owned, or in local signal set",
                            app_id
                        ));
                    }
                }
            }
            Err(error) => {
                eprintln!("Local Steam librarycache merge failed: {error}");
            }
        }

        if let Err(error) = hydrate_local_steam_game_names_from_manifests(
            &mut games_by_external_id,
            steam_root_override,
        ) {
            eprintln!("Local Steam manifest name merge failed: {error}");
        }
        if let Err(error) = hydrate_local_steam_game_names_from_public_catalog(
            connection,
            client,
            &mut games_by_external_id,
            steam_root_override,
        ) {
            eprintln!("Local Steam public catalog name merge failed: {error}");
        }

        let local_only_app_ids = games_by_external_id
            .keys()
            .filter_map(|external_id| external_id.parse::<u64>().ok())
            .filter(|app_id| !steam_owned_app_id_set.contains(app_id))
            .collect::<Vec<_>>();
        if !local_only_app_ids.is_empty() {
            match resolve_steam_app_kinds_for_app_ids(connection, client, &local_only_app_ids) {
                Ok(kinds_by_app_id) => {
                    for game in games_by_external_id.values_mut() {
                        let Some(app_id) = game.external_id.parse::<u64>().ok() else {
                            continue;
                        };
                        if steam_owned_app_id_set.contains(&app_id) {
                            continue;
                        }
                        let Some(kind) = kinds_by_app_id.get(&app_id) else {
                            continue;
                        };
                        game.kind = kind.clone();
                    }
                }
                Err(error) => {
                    eprintln!("Local Steam app kind merge failed: {error}");
                }
            }
        }
    }

    if let Err(error) = refresh_steam_store_tags_cache(connection, client, &steam_owned_app_ids) {
        eprintln!("Steam Store tag sync failed: {error}");
    }

    let games = games_by_external_id.into_values().collect::<Vec<_>>();
    replace_provider_games(connection, &user.id, "steam", &games)?;
    Ok(games.len())
}

fn load_provider_installed_flags(
    connection: &Connection,
    user_id: &str,
    provider: &str,
) -> Result<HashMap<u64, bool>, String> {
    let mut statement = connection
        .prepare("SELECT external_id, installed FROM games WHERE user_id = ?1 AND provider = ?2")
        .map_err(|error| format!("Failed to prepare installed flag query: {error}"))?;

    let rows = statement
        .query_map(params![user_id, provider], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|error| format!("Failed to query installed flags: {error}"))?;

    let mut installed_flags = HashMap::new();
    for row in rows {
        let (external_id, installed_raw) =
            row.map_err(|error| format!("Failed to decode installed flag row: {error}"))?;
        let Some(app_id) = external_id.parse::<u64>().ok() else {
            continue;
        };
        installed_flags.insert(app_id, installed_raw > 0);
    }

    Ok(installed_flags)
}

fn load_provider_game_names(
    connection: &Connection,
    user_id: &str,
    provider: &str,
) -> Result<HashMap<String, String>, String> {
    let mut statement = connection
        .prepare("SELECT external_id, name FROM games WHERE user_id = ?1 AND provider = ?2")
        .map_err(|error| format!("Failed to prepare provider game names query: {error}"))?;
    let rows = statement
        .query_map(params![user_id, provider], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| format!("Failed to query provider game names: {error}"))?;

    let mut names_by_external_id = HashMap::new();
    for row in rows {
        let (external_id, name) =
            row.map_err(|error| format!("Failed to decode provider game name row: {error}"))?;
        names_by_external_id.insert(external_id, name);
    }

    Ok(names_by_external_id)
}

fn refresh_provider_installed_flags(
    connection: &Connection,
    user_id: &str,
    provider: &str,
    installed_app_ids: &HashSet<u64>,
) -> Result<(), String> {
    let mut statement = connection
        .prepare("SELECT external_id FROM games WHERE user_id = ?1 AND provider = ?2")
        .map_err(|error| format!("Failed to prepare provider game ID query: {error}"))?;

    let rows = statement
        .query_map(params![user_id, provider], |row| row.get::<_, String>(0))
        .map_err(|error| format!("Failed to query provider game IDs: {error}"))?;

    let external_ids = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to decode provider game IDs: {error}"))?;

    let mut update = connection
        .prepare(
            "UPDATE games SET installed = ?1 WHERE user_id = ?2 AND provider = ?3 AND external_id = ?4",
        )
        .map_err(|error| format!("Failed to prepare installed flag update: {error}"))?;

    for external_id in external_ids {
        let is_installed = external_id
            .parse::<u64>()
            .ok()
            .map(|app_id| installed_app_ids.contains(&app_id))
            .unwrap_or(false);

        update
            .execute(params![
                if is_installed { 1 } else { 0 },
                user_id,
                provider,
                external_id
            ])
            .map_err(|error| format!("Failed to update installed flag: {error}"))?;
    }

    Ok(())
}

fn detect_locally_installed_steam_app_ids(
    steam_root_override: Option<&str>,
) -> Result<HashSet<u64>, String> {
    // Fast-path: consult in-memory cache to avoid repeated blocking filesystem scans.
    const LOCAL_INSTALL_DETECTION_CACHE_TTL_SECS: i64 = 300; // 5 minutes
    if let Some(cached) = cache::get_cached("local_installed_app_ids", LOCAL_INSTALL_DETECTION_CACHE_TTL_SECS) {
        if let Ok(vec) = serde_json::from_value::<Vec<u64>>(cached) {
            return Ok(vec.into_iter().collect());
        }
    }
    let steam_roots = resolve_steam_root_paths(steam_root_override);
    if steam_roots.is_empty() {
        return Ok(HashSet::new());
    }
    let mut installed_app_ids = HashSet::new();
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
            if let Err(error) = collect_installed_app_ids_from_steamapps_dir(
                &steamapps_directory,
                &mut installed_app_ids,
            ) {
                eprintln!(
                    "Could not collect installed Steam app IDs from {}: {}",
                    steamapps_directory.display(),
                    error
                );
            }
        }
    }

    // cache the computed installed app ids for a short TTL to avoid repeated filesystem scans
    let _ = serde_json::to_value(&installed_app_ids.iter().cloned().collect::<Vec<u64>>())
        .map(|value| cache::set_cached("local_installed_app_ids", value));
    Ok(installed_app_ids)
}

fn collect_locally_installed_steam_games(
    steam_root_override: Option<&str>,
) -> Result<Vec<LibraryGameInput>, String> {
    let steam_roots = resolve_steam_root_paths(steam_root_override);
    if steam_roots.is_empty() {
        return Ok(Vec::new());
    }

    let mut seen_app_ids = HashSet::new();
    let mut local_games = Vec::new();
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
            if let Err(error) = collect_locally_installed_steam_games_from_steamapps_dir(
                &steamapps_directory,
                &mut seen_app_ids,
                &mut local_games,
            ) {
                eprintln!(
                    "Could not collect local Steam games from {}: {}",
                    steamapps_directory.display(),
                    error
                );
            }
        }
    }

    Ok(local_games)
}

fn collect_locally_installed_steam_games_from_steamapps_dir(
    steamapps_directory: &Path,
    seen_app_ids: &mut HashSet<u64>,
    output: &mut Vec<LibraryGameInput>,
) -> Result<(), String> {
    let directory_entries = match fs::read_dir(steamapps_directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "Failed to read Steam library directory {}: {error}",
                steamapps_directory.display()
            ));
        }
    };

    for directory_entry in directory_entries {
        let entry = match directory_entry {
            Ok(value) => value,
            Err(error) => {
                eprintln!(
                    "Could not read Steam library entry in {}: {}",
                    steamapps_directory.display(),
                    error
                );
                continue;
            }
        };
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let Some(app_id) = parse_steam_manifest_app_id(&file_name) else {
            continue;
        };
        if seen_app_ids.contains(&app_id) {
            continue;
        }

        let manifest_contents = match fs::read_to_string(entry.path()) {
            Ok(contents) => contents,
            Err(error) => {
                eprintln!(
                    "Could not read Steam app manifest {}: {}",
                    entry.path().display(),
                    error
                );
                continue;
            }
        };

        if let Some(state_flags) = parse_steam_manifest_u64_field(&manifest_contents, "StateFlags") {
            if state_flags & STEAM_APP_STATE_FULLY_INSTALLED == 0 {
                continue;
            }
        }

        let install_dir_name = match parse_steam_manifest_install_directory(&manifest_contents) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let install_directory = steamapps_directory.join("common").join(install_dir_name);
        if !install_directory.is_dir() {
            continue;
        }

        let has_install_content = match fs::read_dir(&install_directory) {
            Ok(mut entries) => entries.next().is_some(),
            Err(_) => false,
        };
        if !has_install_content {
            continue;
        }

        let external_id = app_id.to_string();
        let name = parse_steam_manifest_string_field(&manifest_contents, "name")
            .unwrap_or_else(|| format!("Steam App {external_id}"));
        let kind = classify_steam_game_kind(&name).to_owned();
        let artwork_url = parse_steam_manifest_string_field(&manifest_contents, "icon")
            .map(|icon_hash| {
                format!(
                    "https://media.steampowered.com/steamcommunity/public/images/apps/{external_id}/{icon_hash}.jpg"
                )
            });

        output.push(LibraryGameInput {
            external_id,
            name,
            kind,
            playtime_minutes: 0,
            installed: true,
            artwork_url,
            last_synced_at: Utc::now().to_rfc3339(),
            last_played_at: parse_steam_manifest_last_played_at(&manifest_contents),
        });
        seen_app_ids.insert(app_id);
    }

    Ok(())
}

fn parse_steam_manifest_last_played_at(manifest_contents: &str) -> Option<String> {
    let seconds_since_epoch = parse_steam_manifest_u64_field(manifest_contents, "LastPlayed")?;
    parse_rfc3339_from_unix_epoch_seconds(seconds_since_epoch)
}

fn parse_rfc3339_from_unix_epoch_seconds(seconds_since_epoch: u64) -> Option<String> {
    if seconds_since_epoch == 0 {
        return None;
    }

    let seconds_since_epoch = i64::try_from(seconds_since_epoch).ok()?;
    Utc.timestamp_opt(seconds_since_epoch, 0)
        .single()
        .map(|value| value.to_rfc3339())
}

fn collect_steam_app_history_entries_from_localconfig(
    steam_root_override: Option<&str>,
    steam_id: &str,
    include_empty_entries: bool,
) -> Result<Vec<LocalSteamAppHistoryEntry>, String> {
    let localconfig_path = match resolve_localconfig_path_for_linked_or_active_steam_user(
        steam_root_override,
        steam_id,
    ) {
        Some(path) => path,
        None => return Ok(Vec::new()),
    };

    let localconfig_contents = fs::read_to_string(&localconfig_path).map_err(|error| {
        format!(
            "Failed to read Steam localconfig at {}: {error}",
            localconfig_path.display()
        )
    })?;
    let parsed_localconfig =
        crate::infrastructure::runtime_vdf::parse_vdf_document(&localconfig_contents).map_err(
            |error| {
                format!(
                    "Failed to parse Steam localconfig at {}: {error}",
                    localconfig_path.display()
                )
            },
        )?;

    let user_local_config_store = crate::infrastructure::runtime_vdf::vdf_find_object_value(
        &parsed_localconfig,
        "UserLocalConfigStore",
    )
    .unwrap_or(&parsed_localconfig);
    let Some(software_value) =
        crate::infrastructure::runtime_vdf::vdf_find_object_value(user_local_config_store, "Software")
    else {
        return Ok(Vec::new());
    };
    let Some(valve_value) =
        crate::infrastructure::runtime_vdf::vdf_find_object_value(software_value, "Valve")
    else {
        return Ok(Vec::new());
    };
    let Some(steam_value) =
        crate::infrastructure::runtime_vdf::vdf_find_object_value(valve_value, "Steam")
    else {
        return Ok(Vec::new());
    };
    let Some(apps_value) =
        crate::infrastructure::runtime_vdf::vdf_find_object_value(steam_value, "apps")
    else {
        return Ok(Vec::new());
    };

    let crate::infrastructure::runtime_vdf::VdfValue::Object(app_entries) = apps_value else {
        return Ok(Vec::new());
    };

    let mut history_entries = Vec::new();
    let mut seen_app_ids = HashSet::new();
    for (app_id_key, app_value) in app_entries {
        let Some(app_id) = app_id_key.trim().parse::<u64>().ok() else {
            continue;
        };
        if !seen_app_ids.insert(app_id) {
            continue;
        }

        let playtime_minutes = crate::infrastructure::runtime_vdf::vdf_get_text_entry(
            app_value,
            "Playtime",
        )
        .or_else(|| crate::infrastructure::runtime_vdf::vdf_get_text_entry(app_value, "Playtime2wks"))
        .and_then(|value| value.trim().parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);
        let last_played_at = crate::infrastructure::runtime_vdf::vdf_get_text_entry(
            app_value,
            "LastPlayed",
        )
        .and_then(|value| value.trim().parse::<u64>().ok())
        .and_then(parse_rfc3339_from_unix_epoch_seconds);
        let name = crate::infrastructure::runtime_vdf::vdf_get_text_entry(app_value, "name")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);

        if !include_empty_entries
            && playtime_minutes <= 0
            && last_played_at.is_none()
            && name.is_none()
        {
            continue;
        }

        history_entries.push(LocalSteamAppHistoryEntry {
            app_id,
            name,
            playtime_minutes,
            last_played_at,
        });
    }

    Ok(history_entries)
}

fn collect_steam_app_history_entries_from_sharedconfig(
    steam_root_override: Option<&str>,
    steam_id: &str,
    include_empty_entries: bool,
) -> Result<Vec<LocalSteamAppHistoryEntry>, String> {
    let Some(sharedconfig_paths) =
        resolve_sharedconfig_paths_for_linked_or_active_steam_user(steam_root_override, steam_id)
    else {
        return Ok(Vec::new());
    };
    if sharedconfig_paths.is_empty() {
        return Ok(Vec::new());
    }

    let mut history_entries = Vec::new();
    let mut seen_app_ids = HashSet::new();

    for sharedconfig_path in sharedconfig_paths {
        let sharedconfig_contents = fs::read_to_string(&sharedconfig_path).map_err(|error| {
            format!(
                "Failed to read Steam sharedconfig at {}: {error}",
                sharedconfig_path.display()
            )
        })?;
        let parsed_sharedconfig = crate::infrastructure::runtime_vdf::parse_vdf_document(
            &sharedconfig_contents,
        )
        .map_err(|error| {
            format!(
                "Failed to parse Steam sharedconfig at {}: {error}",
                sharedconfig_path.display()
            )
        })?;

        let roaming_root = crate::infrastructure::runtime_vdf::vdf_find_object_value(
            &parsed_sharedconfig,
            "UserRoamingConfigStore",
        )
        .unwrap_or(&parsed_sharedconfig);
        let Some(software_value) = crate::infrastructure::runtime_vdf::vdf_find_object_value(
            roaming_root,
            "Software",
        ) else {
            continue;
        };
        let Some(valve_value) =
            crate::infrastructure::runtime_vdf::vdf_find_object_value(software_value, "Valve")
        else {
            continue;
        };
        let Some(steam_value) =
            crate::infrastructure::runtime_vdf::vdf_find_object_value(valve_value, "Steam")
        else {
            continue;
        };
        let Some(apps_value) =
            crate::infrastructure::runtime_vdf::vdf_find_object_value(steam_value, "apps")
        else {
            continue;
        };
        let crate::infrastructure::runtime_vdf::VdfValue::Object(app_entries) = apps_value else {
            continue;
        };

        for (app_id_key, app_value) in app_entries {
            let Some(app_id) = app_id_key.trim().parse::<u64>().ok() else {
                continue;
            };
            if !seen_app_ids.insert(app_id) {
                continue;
            }

            let playtime_minutes = crate::infrastructure::runtime_vdf::vdf_get_text_entry(
                app_value,
                "Playtime",
            )
            .or_else(|| {
                crate::infrastructure::runtime_vdf::vdf_get_text_entry(app_value, "Playtime2wks")
            })
            .and_then(|value| value.trim().parse::<i64>().ok())
            .unwrap_or(0)
            .max(0);
            let last_played_at = crate::infrastructure::runtime_vdf::vdf_get_text_entry(
                app_value,
                "LastPlayed",
            )
            .and_then(|value| value.trim().parse::<u64>().ok())
            .and_then(parse_rfc3339_from_unix_epoch_seconds);
            let name = crate::infrastructure::runtime_vdf::vdf_get_text_entry(app_value, "name")
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned);

            if !include_empty_entries
                && playtime_minutes <= 0
                && last_played_at.is_none()
                && name.is_none()
            {
                continue;
            }

            history_entries.push(LocalSteamAppHistoryEntry {
                app_id,
                name,
                playtime_minutes,
                last_played_at,
            });
        }
    }

    Ok(history_entries)
}

fn resolve_steam_user_candidates_for_local_discovery(
    steam_root_override: Option<&str>,
    steam_id: &str,
) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    let preferred = steam_id.trim();
    if !preferred.is_empty() && seen.insert(preferred.to_owned()) {
        candidates.push(preferred.to_owned());
    }

    if let Some(active_steam_id) = resolve_active_local_steam_id(steam_root_override) {
        let trimmed_active = active_steam_id.trim();
        if !trimmed_active.is_empty() && seen.insert(trimmed_active.to_owned()) {
            candidates.push(trimmed_active.to_owned());
        }
    }

    candidates
}

fn resolve_localconfig_path_for_linked_or_active_steam_user(
    steam_root_override: Option<&str>,
    steam_id: &str,
) -> Option<PathBuf> {
    for steam_id_candidate in
        resolve_steam_user_candidates_for_local_discovery(steam_root_override, steam_id)
    {
        if let Ok(path) = resolve_steam_localconfig_path(steam_root_override, &steam_id_candidate) {
            return Some(path);
        }
    }

    None
}

fn resolve_localconfig_paths_for_linked_or_active_steam_user(
    steam_root_override: Option<&str>,
    steam_id: &str,
) -> Vec<PathBuf> {
    let steam_id_candidates =
        resolve_steam_user_candidates_for_local_discovery(steam_root_override, steam_id);
    if steam_id_candidates.is_empty() {
        return Vec::new();
    }

    let mut paths = Vec::new();
    let mut seen_paths = HashSet::new();
    for steam_root in resolve_steam_root_paths(steam_root_override) {
        for steam_id_candidate in &steam_id_candidates {
            let Ok(userdata_directory) =
                resolve_steam_userdata_directory(&steam_root, steam_id_candidate)
            else {
                continue;
            };
            let localconfig_path = userdata_directory.join("config").join("localconfig.vdf");
            if !localconfig_path.is_file() {
                continue;
            }
            if seen_paths.insert(localconfig_path.clone()) {
                paths.push(localconfig_path);
            }
        }
    }

    if paths.is_empty() {
        if let Some(localconfig_path) =
            resolve_localconfig_path_for_linked_or_active_steam_user(steam_root_override, steam_id)
        {
            paths.push(localconfig_path);
        }
    }

    if steam_sync_debug_logging_enabled() {
        let rendered_paths = paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
        log_steam_sync_debug(&format!(
            "resolved localconfig paths for steam_id={steam_id}: candidates={:?}, paths={rendered_paths:?}",
            steam_id_candidates
        ));
    }

    paths
}

fn resolve_sharedconfig_paths_for_linked_or_active_steam_user(
    steam_root_override: Option<&str>,
    steam_id: &str,
) -> Option<Vec<PathBuf>> {
    for steam_id_candidate in
        resolve_steam_user_candidates_for_local_discovery(steam_root_override, steam_id)
    {
        if let Ok(paths) = resolve_steam_sharedconfig_paths(steam_root_override, &steam_id_candidate)
        {
            return Some(paths);
        }
    }

    None
}

fn collect_locally_known_steam_games_from_localconfig(
    connection: &Connection,
    user: &UserRow,
    steam_root_override: Option<&str>,
    include_empty_entries: bool,
) -> Result<Vec<LibraryGameInput>, String> {
    let steam_id = user
        .steam_id
        .as_deref()
        .ok_or_else(|| String::from("User is not linked to Steam"))?;
    let history_entries = collect_steam_app_history_entries_from_localconfig(
        steam_root_override,
        steam_id,
        include_empty_entries,
    )?;
    if history_entries.is_empty() {
        return Ok(Vec::new());
    }

    let existing_game_names = load_provider_game_names(connection, &user.id, "steam")?;
    let now = Utc::now().to_rfc3339();
    let mut local_games = Vec::with_capacity(history_entries.len());
    for history_entry in history_entries {
        let external_id = history_entry.app_id.to_string();
        let name = history_entry
            .name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .or_else(|| existing_game_names.get(&external_id).cloned())
            .unwrap_or_else(|| format!("Steam App {external_id}"));
        let kind = classify_steam_game_kind(&name).to_owned();
        let artwork_url = Some(format!(
            "https://cdn.cloudflare.steamstatic.com/steam/apps/{external_id}/capsule_231x87.jpg"
        ));

        local_games.push(LibraryGameInput {
            external_id,
            name,
            kind,
            playtime_minutes: history_entry.playtime_minutes.max(0),
            installed: false,
            artwork_url,
            last_synced_at: now.clone(),
            last_played_at: history_entry.last_played_at,
        });
    }

    Ok(local_games)
}

fn collect_locally_known_steam_games_from_sharedconfig(
    connection: &Connection,
    user: &UserRow,
    steam_root_override: Option<&str>,
    include_empty_entries: bool,
) -> Result<Vec<LibraryGameInput>, String> {
    let steam_id = user
        .steam_id
        .as_deref()
        .ok_or_else(|| String::from("User is not linked to Steam"))?;
    let history_entries = collect_steam_app_history_entries_from_sharedconfig(
        steam_root_override,
        steam_id,
        include_empty_entries,
    )?;
    if history_entries.is_empty() {
        return Ok(Vec::new());
    }

    let existing_game_names = load_provider_game_names(connection, &user.id, "steam")?;
    let now = Utc::now().to_rfc3339();
    let mut local_games = Vec::with_capacity(history_entries.len());
    for history_entry in history_entries {
        let external_id = history_entry.app_id.to_string();
        let name = history_entry
            .name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .or_else(|| existing_game_names.get(&external_id).cloned())
            .unwrap_or_else(|| format!("Steam App {external_id}"));
        let kind = classify_steam_game_kind(&name).to_owned();
        let artwork_url = Some(format!(
            "https://cdn.cloudflare.steamstatic.com/steam/apps/{external_id}/capsule_231x87.jpg"
        ));

        local_games.push(LibraryGameInput {
            external_id,
            name,
            kind,
            playtime_minutes: history_entry.playtime_minutes.max(0),
            installed: false,
            artwork_url,
            last_synced_at: now.clone(),
            last_played_at: history_entry.last_played_at,
        });
    }

    Ok(local_games)
}

fn collect_selected_steam_library_app_ids_from_localconfig(
    steam_root_override: Option<&str>,
    steam_id: &str,
) -> HashSet<u64> {
    let mut selected_app_ids = HashSet::new();
    let localconfig_paths =
        resolve_localconfig_paths_for_linked_or_active_steam_user(steam_root_override, steam_id);
    if localconfig_paths.is_empty() {
        return selected_app_ids;
    }

    for localconfig_path in &localconfig_paths {
        let Ok(localconfig_contents) = fs::read_to_string(&localconfig_path) else {
            if steam_sync_debug_logging_enabled() {
                log_steam_sync_debug(&format!(
                    "failed to read localconfig for selected app ids: {}",
                    localconfig_path.display()
                ));
            }
            continue;
        };
        let Ok(parsed_localconfig) =
            crate::infrastructure::runtime_vdf::parse_vdf_document(&localconfig_contents)
        else {
            if steam_sync_debug_logging_enabled() {
                log_steam_sync_debug(&format!(
                    "failed to parse localconfig for selected app ids: {}",
                    localconfig_path.display()
                ));
            }
            continue;
        };
        let user_local_config_store = crate::infrastructure::runtime_vdf::vdf_find_object_value(
            &parsed_localconfig,
            "UserLocalConfigStore",
        )
        .unwrap_or(&parsed_localconfig);
        let Some(web_storage_value) = crate::infrastructure::runtime_vdf::vdf_find_object_value(
            user_local_config_store,
            "WebStorage",
        ) else {
            continue;
        };
        let Some(ui_state_json_text) = crate::infrastructure::runtime_vdf::vdf_get_text_entry(
            web_storage_value,
            "UIStoreLocalSteamUIState",
        ) else {
            if steam_sync_debug_logging_enabled() {
                log_steam_sync_debug(&format!(
                    "missing UIStoreLocalSteamUIState in {}",
                    localconfig_path.display()
                ));
            }
            continue;
        };
        let Ok(ui_state_value) = serde_json::from_str::<serde_json::Value>(ui_state_json_text)
        else {
            if steam_sync_debug_logging_enabled() {
                log_steam_sync_debug(&format!(
                    "invalid UIStoreLocalSteamUIState JSON in {}",
                    localconfig_path.display()
                ));
            }
            continue;
        };
        let Some(current_selection_value) = ui_state_value.get("currentSelection") else {
            if steam_sync_debug_logging_enabled() {
                log_steam_sync_debug(&format!(
                    "missing currentSelection in {}",
                    localconfig_path.display()
                ));
            }
            continue;
        };

        if let Some(app_id) = current_selection_value
            .get("nAppId")
            .and_then(serde_json::Value::as_u64)
        {
            selected_app_ids.insert(app_id);
            if steam_sync_debug_logging_enabled() {
                log_steam_sync_debug(&format!(
                    "selected app id from {} => {}",
                    localconfig_path.display(),
                    app_id
                ));
            }
        } else if let Some(app_id) = current_selection_value
            .get("nAppId")
            .and_then(serde_json::Value::as_str)
            .and_then(|value| value.trim().parse::<u64>().ok())
        {
            selected_app_ids.insert(app_id);
            if steam_sync_debug_logging_enabled() {
                log_steam_sync_debug(&format!(
                    "selected app id from {} => {}",
                    localconfig_path.display(),
                    app_id
                ));
            }
        } else if steam_sync_debug_logging_enabled() {
            log_steam_sync_debug(&format!(
                "currentSelection in {} did not contain parseable nAppId",
                localconfig_path.display()
            ));
        }
    }

    if steam_sync_debug_logging_enabled() {
        let mut rendered = selected_app_ids.iter().copied().collect::<Vec<_>>();
        rendered.sort_unstable();
        log_steam_sync_debug(&format!(
            "selected app ids for steam_id={steam_id}: {:?}",
            rendered
        ));
    }

    selected_app_ids
}

fn collect_signal_steam_library_app_ids_from_local_state(
    steam_root_override: Option<&str>,
    steam_id: &str,
) -> HashSet<u64> {
    let mut app_ids = HashSet::new();
    let localconfig_paths =
        resolve_localconfig_paths_for_linked_or_active_steam_user(steam_root_override, steam_id);
    let rollup_key_pattern = Regex::new(r"^NewContentRollup_(\d+)$").ok();

    for localconfig_path in &localconfig_paths {
        let Ok(localconfig_contents) = fs::read_to_string(localconfig_path) else {
            continue;
        };
        let Ok(parsed_localconfig) =
            crate::infrastructure::runtime_vdf::parse_vdf_document(&localconfig_contents)
        else {
            continue;
        };
        let user_local_config_store = crate::infrastructure::runtime_vdf::vdf_find_object_value(
            &parsed_localconfig,
            "UserLocalConfigStore",
        )
        .unwrap_or(&parsed_localconfig);
        let Some(web_storage_value) = crate::infrastructure::runtime_vdf::vdf_find_object_value(
            user_local_config_store,
            "WebStorage",
        ) else {
            continue;
        };
        let crate::infrastructure::runtime_vdf::VdfValue::Object(web_storage_entries) = web_storage_value
        else {
            continue;
        };

        for (web_storage_key, web_storage_entry) in web_storage_entries {
            let Some(raw_json_text) = (match web_storage_entry {
                crate::infrastructure::runtime_vdf::VdfValue::Text(text) => Some(text.as_str()),
                crate::infrastructure::runtime_vdf::VdfValue::Object(_) => None,
            }) else {
                continue;
            };
            let Ok(parsed_json_value) = serde_json::from_str::<serde_json::Value>(raw_json_text)
            else {
                continue;
            };

            if web_storage_key.eq_ignore_ascii_case("playnextstore_storage") {
                if let Some(playnext_app_ids) = parsed_json_value
                    .get("cachedPlayNext")
                    .and_then(|value| value.get("appids"))
                    .and_then(serde_json::Value::as_array)
                {
                    for value in playnext_app_ids {
                        if let Some(app_id) = value.as_u64().or_else(|| {
                            value
                                .as_str()
                                .and_then(|raw| raw.trim().parse::<u64>().ok())
                        }) {
                            app_ids.insert(app_id);
                        }
                    }
                }
                continue;
            }

            if web_storage_key.eq_ignore_ascii_case("UIStoreLocalSteamUIState") {
                if let Some(selected_app_id) = parsed_json_value
                    .get("currentSelection")
                    .and_then(|value| value.get("nAppId"))
                    .and_then(steam_app_id_from_json_value)
                {
                    app_ids.insert(selected_app_id);
                }
                continue;
            }

            if web_storage_key.starts_with("user-collections.") {
                if let Some(added_app_ids) = parsed_json_value.get("added").and_then(serde_json::Value::as_array) {
                    for value in added_app_ids {
                        if let Some(app_id) = value.as_u64().or_else(|| {
                            value
                                .as_str()
                                .and_then(|raw| raw.trim().parse::<u64>().ok())
                        }) {
                            app_ids.insert(app_id);
                        }
                    }
                }
                continue;
            }

            if let Some(pattern) = rollup_key_pattern.as_ref() {
                if let Some(captured_id) = pattern
                    .captures(web_storage_key)
                    .and_then(|captures| captures.get(1))
                    .and_then(|value| value.as_str().trim().parse::<u64>().ok())
                {
                    app_ids.insert(captured_id);
                }
            }
        }
    }

    app_ids.extend(collect_signal_steam_library_app_ids_from_cloudstorage(
        steam_root_override,
        steam_id,
    ));

    if steam_sync_debug_logging_enabled() {
        log_steam_sync_debug(&format!(
            "local signal app ids for steam_id={steam_id}: count={}",
            app_ids.len()
        ));
    }

    app_ids
}

fn steam_app_id_from_json_value(value: &serde_json::Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|raw| raw.trim().parse::<u64>().ok()))
}

fn collect_steam_app_ids_from_json_array(values: &[serde_json::Value], app_ids: &mut HashSet<u64>) {
    for value in values {
        if let Some(app_id) = steam_app_id_from_json_value(value) {
            app_ids.insert(app_id);
        }
    }
}

fn collect_signal_steam_library_app_ids_from_cloudstorage(
    steam_root_override: Option<&str>,
    steam_id: &str,
) -> HashSet<u64> {
    let mut app_ids = HashSet::new();
    let local_steam_id_candidates =
        resolve_steam_user_candidates_for_local_discovery(steam_root_override, steam_id);
    if local_steam_id_candidates.is_empty() {
        return app_ids;
    }

    let rollup_key_pattern = Regex::new(r"^NewContentRollup_(\d+)$").ok();
    let mut seen_cloudstorage_files = HashSet::new();
    for steam_root in resolve_steam_root_paths(steam_root_override) {
        for local_steam_id_candidate in &local_steam_id_candidates {
            let Ok(userdata_directory) =
                resolve_steam_userdata_directory(&steam_root, local_steam_id_candidate)
            else {
                continue;
            };
            let cloudstorage_directory = userdata_directory.join("config").join("cloudstorage");
            let Ok(entries) = fs::read_dir(&cloudstorage_directory) else {
                continue;
            };

            for entry in entries.flatten() {
                let file_path = entry.path();
                let Some(file_name) = file_path.file_name().and_then(|value| value.to_str()) else {
                    continue;
                };
                if !file_name.starts_with("cloud-storage-namespace-")
                    || !file_name.ends_with(".json")
                    || file_name.ends_with(".modified.json")
                {
                    continue;
                }
                if !seen_cloudstorage_files.insert(file_path.clone()) {
                    continue;
                }

                let Ok(contents) = fs::read_to_string(&file_path) else {
                    continue;
                };
                let Ok(entries_json) = serde_json::from_str::<serde_json::Value>(&contents) else {
                    continue;
                };
                let Some(entries_array) = entries_json.as_array() else {
                    continue;
                };

                for entry in entries_array {
                    let Some(entry_array) = entry.as_array() else {
                        continue;
                    };
                    if entry_array.len() < 2 {
                        continue;
                    }
                    let Some(entry_key) = entry_array.first().and_then(serde_json::Value::as_str)
                    else {
                        continue;
                    };
                    let Some(entry_metadata) = entry_array.get(1).and_then(serde_json::Value::as_object)
                    else {
                        continue;
                    };

                    if let Some(pattern) = rollup_key_pattern.as_ref() {
                        if let Some(app_id) = pattern
                            .captures(entry_key)
                            .and_then(|captures| captures.get(1))
                            .and_then(|value| value.as_str().trim().parse::<u64>().ok())
                        {
                            app_ids.insert(app_id);
                        }
                    }

                    let parsed_value = entry_metadata
                        .get("value")
                        .and_then(serde_json::Value::as_str)
                        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok());
                    let Some(parsed_value) = parsed_value else {
                        continue;
                    };

                    if entry_key.eq_ignore_ascii_case("GameReleased") {
                        if let Some(released_apps) =
                            parsed_value.get("apps").and_then(serde_json::Value::as_array)
                        {
                            collect_steam_app_ids_from_json_array(released_apps, &mut app_ids);
                        }
                        continue;
                    }

                    if entry_key.eq_ignore_ascii_case("playnextstore_storage") {
                        if let Some(playnext_app_ids) = parsed_value
                            .get("cachedPlayNext")
                            .and_then(|value| value.get("appids"))
                            .and_then(serde_json::Value::as_array)
                        {
                            collect_steam_app_ids_from_json_array(playnext_app_ids, &mut app_ids);
                        }
                        continue;
                    }

                    if entry_key.eq_ignore_ascii_case("UIStoreLocalSteamUIState") {
                        if let Some(selected_app_id) = parsed_value
                            .get("currentSelection")
                            .and_then(|value| value.get("nAppId"))
                            .and_then(steam_app_id_from_json_value)
                        {
                            app_ids.insert(selected_app_id);
                        }
                        continue;
                    }

                    if entry_key.starts_with("user-collections.") {
                        if let Some(added_app_ids) =
                            parsed_value.get("added").and_then(serde_json::Value::as_array)
                        {
                            collect_steam_app_ids_from_json_array(added_app_ids, &mut app_ids);
                        }
                    }
                }
            }
        }
    }

    app_ids
}

fn collect_steam_app_ids_from_librarycache(
    steam_root_override: Option<&str>,
) -> Result<HashSet<u64>, String> {
    let steam_roots = resolve_steam_root_paths(steam_root_override);
    if steam_roots.is_empty() {
        return Ok(HashSet::new());
    }

    let mut app_ids = HashSet::new();
    for steam_root in steam_roots {
        let librarycache_directory = steam_root.join("appcache").join("librarycache");
        let directory_entries = match fs::read_dir(&librarycache_directory) {
            Ok(entries) => entries,
            Err(error) => {
                if error.kind() != std::io::ErrorKind::NotFound {
                    eprintln!(
                        "Could not read Steam librarycache directory at {}: {}",
                        librarycache_directory.display(),
                        error
                    );
                }
                continue;
            }
        };

        for directory_entry in directory_entries.flatten() {
            let Ok(file_type) = directory_entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }

            let directory_name = directory_entry.file_name().to_string_lossy().to_string();
            let Some(app_id) = directory_name.trim().parse::<u64>().ok() else {
                continue;
            };
            app_ids.insert(app_id);
        }
    }

    Ok(app_ids)
}

fn collect_locally_known_steam_games_from_librarycache(
    connection: &Connection,
    user: &UserRow,
    steam_root_override: Option<&str>,
) -> Result<Vec<LibraryGameInput>, String> {
    let app_ids = collect_steam_app_ids_from_librarycache(steam_root_override)?;
    if app_ids.is_empty() {
        return Ok(Vec::new());
    }

    let existing_game_names = load_provider_game_names(connection, &user.id, "steam")?;
    let now = Utc::now().to_rfc3339();
    let mut local_games = Vec::with_capacity(app_ids.len());
    for app_id in app_ids {
        let external_id = app_id.to_string();
        let name = existing_game_names
            .get(&external_id)
            .cloned()
            .unwrap_or_else(|| format!("Steam App {external_id}"));
        let kind = classify_steam_game_kind(&name).to_owned();
        let artwork_url = Some(format!(
            "https://cdn.cloudflare.steamstatic.com/steam/apps/{external_id}/capsule_231x87.jpg"
        ));

        local_games.push(LibraryGameInput {
            external_id,
            name,
            kind,
            playtime_minutes: 0,
            installed: false,
            artwork_url,
            last_synced_at: now.clone(),
            last_played_at: None,
        });
    }

    Ok(local_games)
}

fn is_steam_placeholder_game_name(name: &str, external_id: &str) -> bool {
    let trimmed_name = name.trim();
    if trimmed_name.is_empty() {
        return true;
    }

    if trimmed_name.eq_ignore_ascii_case(&format!("Steam App {external_id}")) {
        return true;
    }

    // Legacy rows sometimes persisted just the app id as the title.
    trimmed_name == external_id
}

fn should_resolve_local_steam_game_name(game: &LibraryGameInput) -> bool {
    is_steam_placeholder_game_name(&game.name, &game.external_id)
}

fn should_replace_placeholder_steam_game_name(
    existing_name: &str,
    existing_external_id: &str,
    candidate_name: &str,
    candidate_external_id: &str,
) -> bool {
    is_steam_placeholder_game_name(existing_name, existing_external_id)
        && !is_steam_placeholder_game_name(candidate_name, candidate_external_id)
}

fn apply_resolved_local_steam_game_name(game: &mut LibraryGameInput, resolved_name: &str) {
    let trimmed_name = resolved_name.trim();
    if trimmed_name.is_empty() || !should_resolve_local_steam_game_name(game) {
        return;
    }

    game.name = trimmed_name.to_owned();
    game.kind = classify_steam_game_kind(trimmed_name).to_owned();
}

fn normalize_unresolved_steam_game_placeholder_name(game: &mut LibraryGameInput) {
    if !should_resolve_local_steam_game_name(game) {
        return;
    }

    game.name = format!("Steam App {}", game.external_id);
}

fn hydrate_local_steam_game_names_from_manifests(
    games_by_external_id: &mut HashMap<String, LibraryGameInput>,
    steam_root_override: Option<&str>,
) -> Result<(), String> {
    let manifest_names_by_external_id = collect_steam_manifest_names(steam_root_override)?;
    if manifest_names_by_external_id.is_empty() {
        return Ok(());
    }

    for (external_id, game) in games_by_external_id.iter_mut() {
        if !should_resolve_local_steam_game_name(game) {
            continue;
        }
        let Some(manifest_name) = manifest_names_by_external_id.get(external_id) else {
            continue;
        };
        apply_resolved_local_steam_game_name(game, manifest_name);
    }

    Ok(())
}

fn hydrate_local_steam_game_names_from_public_catalog(
    connection: &Connection,
    client: &Client,
    games_by_external_id: &mut HashMap<String, LibraryGameInput>,
    steam_root_override: Option<&str>,
) -> Result<(), String> {
    let unresolved_app_ids = games_by_external_id
        .values()
        .filter(|game| should_resolve_local_steam_game_name(game))
        .filter_map(|game| game.external_id.parse::<u64>().ok())
        .collect::<Vec<_>>();
    if unresolved_app_ids.is_empty() {
        return Ok(());
    }

    let resolved_local_names =
        resolve_steam_local_appinfo_names(steam_root_override, &unresolved_app_ids)?;
    for game in games_by_external_id.values_mut() {
        if !should_resolve_local_steam_game_name(game) {
            continue;
        }
        let Some(app_id) = game.external_id.parse::<u64>().ok() else {
            continue;
        };
        let Some(resolved_name) = resolved_local_names.get(&app_id) else {
            continue;
        };
        apply_resolved_local_steam_game_name(game, resolved_name);
    }

    let unresolved_after_local = games_by_external_id
        .values()
        .filter(|game| should_resolve_local_steam_game_name(game))
        .filter_map(|game| game.external_id.parse::<u64>().ok())
        .collect::<Vec<_>>();
    if unresolved_after_local.is_empty() {
        return Ok(());
    }

    let resolved_public_names =
        resolve_steam_public_app_names(connection, client, &unresolved_after_local)?;
    if resolved_public_names.is_empty() {
        return Ok(());
    }

    for game in games_by_external_id.values_mut() {
        if !should_resolve_local_steam_game_name(game) {
            continue;
        }
        let Some(app_id) = game.external_id.parse::<u64>().ok() else {
            continue;
        };
        let Some(resolved_name) = resolved_public_names.get(&app_id) else {
            continue;
        };
        apply_resolved_local_steam_game_name(game, resolved_name);
    }

    for game in games_by_external_id.values_mut() {
        normalize_unresolved_steam_game_placeholder_name(game);
    }

    Ok(())
}

fn resolve_steam_local_appinfo_names(
    steam_root_override: Option<&str>,
    app_ids: &[u64],
) -> Result<HashMap<u64, String>, String> {
    if app_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let unresolved = app_ids.iter().copied().collect::<HashSet<_>>();
    let steam_roots = resolve_steam_root_paths(steam_root_override);
    if steam_roots.is_empty() {
        return Ok(HashMap::new());
    }

    let mut names_by_app_id = HashMap::new();
    for steam_root in steam_roots {
        let appinfo_path = steam_root.join("appcache").join("appinfo.vdf");
        if !appinfo_path.is_file() {
            continue;
        }
        let file_bytes = match fs::read(&appinfo_path) {
            Ok(bytes) => bytes,
            Err(error) => {
                eprintln!(
                    "Could not read Steam appinfo catalog at {}: {}",
                    appinfo_path.display(),
                    error
                );
                continue;
            }
        };
        if file_bytes.len() < 16 {
            continue;
        }

        let mut cursor = 16usize;
        while cursor + 8 <= file_bytes.len() {
            let app_id = u32::from_le_bytes([
                file_bytes[cursor],
                file_bytes[cursor + 1],
                file_bytes[cursor + 2],
                file_bytes[cursor + 3],
            ]) as u64;
            if app_id == 0 {
                break;
            }

            let payload_size = u32::from_le_bytes([
                file_bytes[cursor + 4],
                file_bytes[cursor + 5],
                file_bytes[cursor + 6],
                file_bytes[cursor + 7],
            ]) as usize;
            cursor += 8;

            if cursor + payload_size > file_bytes.len() {
                break;
            }
            let payload = &file_bytes[cursor..cursor + payload_size];
            cursor += payload_size;

            if !unresolved.contains(&app_id) || names_by_app_id.contains_key(&app_id) {
                continue;
            }

            let Some(name) = extract_steam_app_name_from_local_appinfo_payload(payload) else {
                continue;
            };
            names_by_app_id.insert(app_id, name);
        }

        if names_by_app_id.len() == unresolved.len() {
            break;
        }
    }

    Ok(names_by_app_id)
}

fn extract_steam_app_name_from_local_appinfo_payload(payload: &[u8]) -> Option<String> {
    for segment in payload.split(|byte| *byte == 0) {
        if segment.len() < 3 || segment.len() > 96 {
            continue;
        }
        if !segment.iter().all(|byte| (0x20..=0x7e).contains(byte)) {
            continue;
        }

        let candidate = std::str::from_utf8(segment).ok()?.trim();
        if !is_valid_steam_appinfo_name_candidate(candidate) {
            continue;
        }

        return Some(candidate.to_owned());
    }

    None
}

fn is_valid_steam_appinfo_name_candidate(candidate: &str) -> bool {
    let lowered = candidate.to_ascii_lowercase();
    if lowered.starts_with("http://") || lowered.starts_with("https://") {
        return false;
    }
    if lowered.ends_with(".exe")
        || lowered.ends_with(".app")
        || lowered.ends_with(".jpg")
        || lowered.ends_with(".png")
    {
        return false;
    }
    if lowered.contains('\\') || lowered.contains('/') {
        return false;
    }
    if lowered.contains("eula") {
        return false;
    }
    if lowered.contains(',') {
        return false;
    }
    if candidate
        .chars()
        .all(|character| character.is_ascii_hexdigit())
        && candidate.len() >= 24
    {
        return false;
    }
    if lowered == "windows"
        || lowered == "macos"
        || lowered == "linux"
        || lowered == "common"
        || lowered == "config"
        || lowered == "extended"
        || lowered == "default"
        || lowered == "released"
        || lowered == "partial"
        || lowered == "for qa"
    {
        return false;
    }

    candidate.chars().any(|character| character.is_ascii_alphabetic())
}

fn collect_steam_manifest_names(
    steam_root_override: Option<&str>,
) -> Result<HashMap<String, String>, String> {
    let steam_roots = resolve_steam_root_paths(steam_root_override);
    if steam_roots.is_empty() {
        return Ok(HashMap::new());
    }

    let mut names_by_external_id = HashMap::new();
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
            if let Err(error) = collect_steam_manifest_names_from_steamapps_dir(
                &steamapps_directory,
                &mut names_by_external_id,
            ) {
                eprintln!(
                    "Could not collect Steam app names from manifests in {}: {}",
                    steamapps_directory.display(),
                    error
                );
            }
        }
    }

    Ok(names_by_external_id)
}

fn collect_steam_manifest_names_from_steamapps_dir(
    steamapps_directory: &Path,
    names_by_external_id: &mut HashMap<String, String>,
) -> Result<(), String> {
    let directory_entries = match fs::read_dir(steamapps_directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "Failed to read Steam library directory {}: {error}",
                steamapps_directory.display()
            ));
        }
    };

    for directory_entry in directory_entries {
        let entry = match directory_entry {
            Ok(value) => value,
            Err(error) => {
                eprintln!(
                    "Could not read Steam library entry in {}: {}",
                    steamapps_directory.display(),
                    error
                );
                continue;
            }
        };
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let Some(app_id) = parse_steam_manifest_app_id(&file_name) else {
            continue;
        };

        let manifest_contents = match fs::read_to_string(entry.path()) {
            Ok(contents) => contents,
            Err(error) => {
                eprintln!(
                    "Could not read Steam app manifest {}: {}",
                    entry.path().display(),
                    error
                );
                continue;
            }
        };

        let Some(name) = parse_steam_manifest_string_field(&manifest_contents, "name") else {
            continue;
        };
        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            continue;
        }

        names_by_external_id
            .entry(app_id.to_string())
            .or_insert_with(|| trimmed_name.to_owned());
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct ActiveSteamUserCandidate {
    steam_id: String,
    source_priority: u8,
    modified_epoch_secs: u64,
    root_index: usize,
}

fn should_replace_active_steam_user_candidate(
    current: &ActiveSteamUserCandidate,
    candidate: &ActiveSteamUserCandidate,
) -> bool {
    if candidate.source_priority != current.source_priority {
        return candidate.source_priority > current.source_priority;
    }
    if candidate.modified_epoch_secs != current.modified_epoch_secs {
        return candidate.modified_epoch_secs > current.modified_epoch_secs;
    }

    candidate.root_index < current.root_index
}

fn modified_epoch_secs(path: &Path) -> Option<u64> {
    fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

fn steam_id64_from_userdata_directory_name(directory_name: &str) -> Option<String> {
    let trimmed_name = directory_name.trim();
    if trimmed_name.is_empty() || !trimmed_name.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }

    let numeric_value = trimmed_name.parse::<u64>().ok()?;
    if trimmed_name.len() == 17 {
        return Some(trimmed_name.to_owned());
    }

    if numeric_value >= STEAM_ID64_ACCOUNT_ID_BASE {
        return Some(numeric_value.to_string());
    }

    Some((STEAM_ID64_ACCOUNT_ID_BASE + numeric_value).to_string())
}

fn resolve_active_local_steam_id_from_userdata_root(steam_root: &Path) -> Option<(String, u64)> {
    let userdata_directory = steam_root.join("userdata");
    let directory_entries = fs::read_dir(&userdata_directory).ok()?;
    let mut best_candidate: Option<(String, u64)> = None;

    for directory_entry in directory_entries.flatten() {
        let Ok(file_type) = directory_entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }

        let directory_name = directory_entry.file_name().to_string_lossy().to_string();
        let Some(steam_id) = steam_id64_from_userdata_directory_name(&directory_name) else {
            continue;
        };

        let entry_path = directory_entry.path();
        let modified_at = [
            entry_path.join("config").join("localconfig.vdf"),
            entry_path.join("config").join("sharedconfig.vdf"),
            entry_path.join("7").join("remote").join("sharedconfig.vdf"),
            entry_path,
        ]
        .iter()
        .filter_map(|path| modified_epoch_secs(path))
        .max()
        .unwrap_or(0);

        let candidate = (steam_id, modified_at);
        match &best_candidate {
            Some((_, best_modified_at)) if modified_at <= *best_modified_at => {}
            _ => {
                best_candidate = Some(candidate);
            }
        }
    }

    best_candidate
}

fn is_truthy_steam_flag(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn is_steam_id64(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.len() == 17 && trimmed.chars().all(|character| character.is_ascii_digit())
}

fn parse_active_steam_id_from_loginusers(contents: &str) -> Option<String> {
    let parsed_document = crate::infrastructure::runtime_vdf::parse_vdf_document(contents).ok()?;
    let users_value =
        crate::infrastructure::runtime_vdf::vdf_find_object_value(&parsed_document, "users")?;
    let crate::infrastructure::runtime_vdf::VdfValue::Object(user_entries) = users_value else {
        return None;
    };

    let mut most_recent = None;
    let mut auto_login = None;
    let mut first_user = None;

    for (raw_steam_id, user_value) in user_entries {
        let steam_id = raw_steam_id.trim();
        if !is_steam_id64(steam_id) {
            continue;
        }

        let steam_id = steam_id.to_owned();
        if first_user.is_none() {
            first_user = Some(steam_id.clone());
        }

        let is_most_recent = crate::infrastructure::runtime_vdf::vdf_get_text_entry(
            user_value,
            "MostRecent",
        )
        .map(is_truthy_steam_flag)
        .unwrap_or(false);
        if is_most_recent {
            most_recent = Some(steam_id.clone());
            continue;
        }

        let allows_auto_login = crate::infrastructure::runtime_vdf::vdf_get_text_entry(
            user_value,
            "AllowAutoLogin",
        )
        .or_else(|| {
            crate::infrastructure::runtime_vdf::vdf_get_text_entry(user_value, "RememberPassword")
        })
        .map(is_truthy_steam_flag)
        .unwrap_or(false);
        if allows_auto_login && auto_login.is_none() {
            auto_login = Some(steam_id);
        }
    }

    most_recent.or(auto_login).or(first_user)
}

fn resolve_active_local_steam_id(steam_root_override: Option<&str>) -> Option<String> {
    let steam_roots = resolve_steam_root_paths(steam_root_override);
    let mut best_candidate: Option<ActiveSteamUserCandidate> = None;

    for (root_index, steam_root) in steam_roots.into_iter().enumerate() {
        let loginusers_path = steam_root.join("config").join("loginusers.vdf");
        if let Ok(loginusers_contents) = fs::read_to_string(&loginusers_path) {
            if let Some(active_steam_id) = parse_active_steam_id_from_loginusers(&loginusers_contents)
            {
                let candidate = ActiveSteamUserCandidate {
                    steam_id: active_steam_id,
                    source_priority: 2,
                    modified_epoch_secs: modified_epoch_secs(&loginusers_path).unwrap_or(0),
                    root_index,
                };
                match &best_candidate {
                    Some(current) if !should_replace_active_steam_user_candidate(current, &candidate) => {}
                    _ => {
                        best_candidate = Some(candidate);
                    }
                }
                continue;
            }
        }

        if let Some((active_steam_id, modified_epoch_secs)) =
            resolve_active_local_steam_id_from_userdata_root(&steam_root)
        {
            let candidate = ActiveSteamUserCandidate {
                steam_id: active_steam_id,
                source_priority: 1,
                modified_epoch_secs,
                root_index,
            };
            match &best_candidate {
                Some(current) if !should_replace_active_steam_user_candidate(current, &candidate) => {}
                _ => {
                    best_candidate = Some(candidate);
                }
            }
        }
    }

    best_candidate.map(|candidate| candidate.steam_id)
}

fn resolve_steam_root_paths(steam_root_override: Option<&str>) -> Vec<PathBuf> {
    let debug_logging_enabled = steam_detection_debug_logging_enabled();
    if let Some(override_path) = steam_root_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let normalized_override_root = normalize_steam_root_candidate_path(Path::new(override_path));
        let steamapps_directory = normalized_override_root.join("steamapps");
        if steamapps_directory.is_dir() {
            if debug_logging_enabled {
                log_steam_detection_debug(&format!(
                    "override root accepted: {} (steamapps: {})",
                    normalized_override_root.display(),
                    steamapps_directory.display()
                ));
            }
            return vec![normalized_override_root];
        }

        if debug_logging_enabled {
            log_steam_detection_debug(&format!(
                "override root rejected (missing steamapps); falling back to auto-detection: {}",
                normalized_override_root.display()
            ));
        }
    }

    let mut roots = Vec::new();
    let mut seen_roots = HashSet::new();
    for candidate in steam_root_candidates() {
        let normalized_candidate = normalize_steam_root_candidate_path(&candidate);
        let dedupe_path =
            fs::canonicalize(&normalized_candidate).unwrap_or_else(|_| normalized_candidate.clone());
        if !seen_roots.insert(dedupe_path) {
            continue;
        }

        let steamapps_directory = normalized_candidate.join("steamapps");
        if steamapps_directory.is_dir() {
            if debug_logging_enabled {
                log_steam_detection_debug(&format!(
                    "root accepted: {}",
                    normalized_candidate.display()
                ));
            }
            roots.push(normalized_candidate);
        } else if debug_logging_enabled {
            let rejection_reason = if !normalized_candidate.exists() {
                "path does not exist"
            } else if !normalized_candidate.is_dir() {
                "path is not a directory"
            } else {
                "missing steamapps directory"
            };
            log_steam_detection_debug(&format!(
                "root rejected: {} ({rejection_reason})",
                normalized_candidate.display()
            ));
        }
    }

    if debug_logging_enabled && roots.is_empty() {
        log_steam_detection_debug("no candidate Steam root paths were accepted");
    }

    roots
}

fn steam_detection_debug_logging_enabled() -> bool {
    steam_detection_env_flag("STEAM_SETTINGS_DEBUG_LOGGING")
        || steam_detection_env_flag("STEAM_DETECTION_DEBUG_LOGGING")
}

fn steam_sync_debug_logging_enabled() -> bool {
    steam_detection_debug_logging_enabled() || steam_detection_env_flag("STEAM_SYNC_DEBUG_LOGGING")
}

fn steam_sync_debug_target_app_id() -> Option<u64> {
    let Ok(raw_value) = std::env::var("STEAM_SYNC_DEBUG_APP_ID") else {
        return None;
    };

    raw_value.trim().parse::<u64>().ok()
}

fn steam_detection_env_flag(name: &str) -> bool {
    let Ok(raw_value) = std::env::var(name) else {
        return false;
    };

    matches!(
        raw_value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn log_steam_detection_debug(message: &str) {
    static SEEN_DETECTION_MESSAGES: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let seen_messages = SEEN_DETECTION_MESSAGES.get_or_init(|| Mutex::new(HashSet::new()));
    if let Ok(mut seen_messages) = seen_messages.lock() {
        if !seen_messages.insert(message.to_owned()) {
            return;
        }
        if seen_messages.len() > 512 {
            seen_messages.clear();
            seen_messages.insert(message.to_owned());
        }
    }
    eprintln!("[catalyst:steam-detection] {message}");
}

fn log_steam_sync_debug(message: &str) {
    eprintln!("[catalyst:steam-sync] {message}");
}

fn normalize_steam_root_candidate_path(candidate: &Path) -> PathBuf {
    let Some(file_name) = candidate.file_name().and_then(|value| value.to_str()) else {
        return candidate.to_path_buf();
    };

    if file_name.eq_ignore_ascii_case("steamapps")
        || file_name.eq_ignore_ascii_case("config")
        || file_name.eq_ignore_ascii_case("userdata")
        || file_name.eq_ignore_ascii_case("bin")
    {
        if let Some(parent) = candidate.parent() {
            return parent.to_path_buf();
        }
    }

    if file_name.eq_ignore_ascii_case("steam.exe") || file_name.eq_ignore_ascii_case("steam.sh") {
        if let Some(parent) = candidate.parent() {
            return parent.to_path_buf();
        }
    }

    if file_name.eq_ignore_ascii_case("loginusers.vdf") {
        if let Some(config_directory) = candidate.parent() {
            if config_directory
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("config"))
            {
                if let Some(steam_root) = config_directory.parent() {
                    return steam_root.to_path_buf();
                }
            }
        }
    }

    candidate.to_path_buf()
}

fn resolve_steam_root_path(steam_root_override: Option<&str>) -> Option<PathBuf> {
    resolve_steam_root_paths(steam_root_override)
        .into_iter()
        .next()
}

fn resolve_steam_root_path_for_user(
    steam_root_override: Option<&str>,
    steam_id: &str,
) -> Option<PathBuf> {
    let mut best_candidate: Option<(PathBuf, u64, usize)> = None;

    for (root_index, steam_root) in resolve_steam_root_paths(steam_root_override)
        .into_iter()
        .enumerate()
    {
        let Ok(userdata_directory) = resolve_steam_userdata_directory(&steam_root, steam_id) else {
            continue;
        };
        let activity_epoch_secs = [
            userdata_directory.join("config").join("localconfig.vdf"),
            userdata_directory.join("config").join("sharedconfig.vdf"),
            userdata_directory
                .join("7")
                .join("remote")
                .join("sharedconfig.vdf"),
            userdata_directory.join("config").join("shortcuts.vdf"),
            userdata_directory.join("config").join("cloudstorage"),
            userdata_directory.clone(),
        ]
        .iter()
        .filter_map(|path| modified_epoch_secs(path))
        .max()
        .unwrap_or(0);

        if steam_sync_debug_logging_enabled() {
            log_steam_sync_debug(&format!(
                "root candidate for steam_id={steam_id}: root={}, userdata={}, activity_epoch_secs={activity_epoch_secs}, root_index={root_index}",
                steam_root.display(),
                userdata_directory.display()
            ));
        }

        let candidate = (steam_root, activity_epoch_secs, root_index);
        match &best_candidate {
            Some((_, best_activity_epoch_secs, best_root_index))
                if *best_activity_epoch_secs > activity_epoch_secs
                    || (*best_activity_epoch_secs == activity_epoch_secs
                        && *best_root_index <= root_index) => {}
            _ => {
                best_candidate = Some(candidate);
            }
        }
    }

    let resolved = best_candidate.map(|(steam_root, _, _)| steam_root);
    if steam_sync_debug_logging_enabled() {
        let rendered = resolved
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| String::from("<none>"));
        log_steam_sync_debug(&format!(
            "resolved steam root for steam_id={steam_id}: {rendered}"
        ));
    }

    resolved
}

fn resolve_steam_userdata_directory(steam_root: &Path, steam_id: &str) -> Result<PathBuf, String> {
    let userdata_directory = steam_root.join("userdata");
    let candidate_directory_names = steam_userdata_candidate_directory_names(steam_id)?;

    for candidate_directory_name in &candidate_directory_names {
        let candidate_path = userdata_directory.join(candidate_directory_name);
        if candidate_path.is_dir() {
            return Ok(candidate_path);
        }
    }

    Err(format!(
        "Could not find Steam userdata directory for account {steam_id} in {}",
        userdata_directory.display()
    ))
}

fn resolve_steam_localconfig_path(
    steam_root_override: Option<&str>,
    steam_id: &str,
) -> Result<PathBuf, String> {
    let steam_root = resolve_steam_root_path_for_user(steam_root_override, steam_id)
        .or_else(|| resolve_steam_root_path(steam_root_override))
        .ok_or_else(|| String::from("Could not locate local Steam installation"))?;
    let userdata_directory = resolve_steam_userdata_directory(&steam_root, steam_id)?;
    let localconfig_path = userdata_directory.join("config").join("localconfig.vdf");
    if !localconfig_path.is_file() {
        return Err(format!(
            "Could not locate Steam localconfig.vdf at {}",
            localconfig_path.display()
        ));
    }

    Ok(localconfig_path)
}

fn resolve_steam_sharedconfig_paths(
    steam_root_override: Option<&str>,
    steam_id: &str,
) -> Result<Vec<PathBuf>, String> {
    let steam_root = resolve_steam_root_path_for_user(steam_root_override, steam_id)
        .or_else(|| resolve_steam_root_path(steam_root_override))
        .ok_or_else(|| String::from("Could not locate local Steam installation"))?;
    let userdata_directory = resolve_steam_userdata_directory(&steam_root, steam_id)?;
    let candidates = [
        userdata_directory.join("7").join("remote").join("sharedconfig.vdf"),
        userdata_directory.join("config").join("sharedconfig.vdf"),
    ];
    Ok(candidates
        .into_iter()
        .filter(|candidate_path| candidate_path.is_file())
        .collect())
}

fn resolve_steam_cloudstorage_directory(
    steam_root_override: Option<&str>,
    steam_id: &str,
) -> Result<PathBuf, String> {
    let steam_root = resolve_steam_root_path_for_user(steam_root_override, steam_id)
        .or_else(|| resolve_steam_root_path(steam_root_override))
        .ok_or_else(|| String::from("Could not locate local Steam installation"))?;
    let userdata_directory = resolve_steam_userdata_directory(&steam_root, steam_id)?;
    let cloudstorage_directory = userdata_directory.join("config").join("cloudstorage");
    if !cloudstorage_directory.is_dir() {
        return Err(format!(
            "Could not locate Steam cloudstorage directory at {}",
            cloudstorage_directory.display()
        ));
    }
    Ok(cloudstorage_directory)
}

fn empty_game_customization_artwork_response() -> GameCustomizationArtworkResponse {
    GameCustomizationArtworkResponse {
        cover: None,
        background: None,
        logo: None,
        wide_cover: None,
    }
}

fn extension_priority_rank(extension: &str) -> usize {
    match extension {
        "png" => 0,
        "jpg" => 1,
        "jpeg" => 2,
        "webp" => 3,
        _ => usize::MAX,
    }
}

fn find_steam_grid_artwork_path(grid_directory: &Path, stem: &str) -> Option<PathBuf> {
    if stem.trim().is_empty() {
        return None;
    }

    let mut best_match: Option<(usize, PathBuf)> = None;
    let normalized_stem = stem.trim().to_ascii_lowercase();
    let Ok(entries) = fs::read_dir(grid_directory) else {
        return None;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let extension = path
            .extension()
            .map(|value| value.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let rank = extension_priority_rank(&extension);
        if rank == usize::MAX {
            continue;
        }

        let file_stem = path
            .file_stem()
            .map(|value| value.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if file_stem != normalized_stem {
            continue;
        }

        match &best_match {
            Some((best_rank, _)) if rank >= *best_rank => {}
            _ => {
                best_match = Some((rank, path));
            }
        }
    }

    best_match.map(|(_, path)| path)
}

fn resolve_steam_customization_artwork(
    steam_root_override: Option<&str>,
    steam_id: &str,
    app_id: &str,
) -> GameCustomizationArtworkResponse {
    let Some(steam_root) = resolve_steam_root_path(steam_root_override) else {
        return empty_game_customization_artwork_response();
    };
    let Ok(userdata_directory) = resolve_steam_userdata_directory(&steam_root, steam_id) else {
        return empty_game_customization_artwork_response();
    };
    let grid_directory = userdata_directory.join("config").join("grid");
    if !grid_directory.is_dir() {
        return empty_game_customization_artwork_response();
    }

    let to_path_string = |path: Option<PathBuf>| {
        path.map(|resolved| resolved.to_string_lossy().to_string())
    };
    GameCustomizationArtworkResponse {
        cover: to_path_string(find_steam_grid_artwork_path(&grid_directory, &format!("{app_id}p"))),
        background: to_path_string(find_steam_grid_artwork_path(
            &grid_directory,
            &format!("{app_id}_hero"),
        )),
        logo: to_path_string(find_steam_grid_artwork_path(
            &grid_directory,
            &format!("{app_id}_logo"),
        )),
        wide_cover: to_path_string(find_steam_grid_artwork_path(&grid_directory, app_id)),
    }
}

fn steam_userdata_candidate_directory_names(steam_id: &str) -> Result<Vec<String>, String> {
    let trimmed_steam_id = steam_id.trim();
    if trimmed_steam_id.is_empty() {
        return Err(String::from("Steam ID is required"));
    }

    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    if seen.insert(trimmed_steam_id.to_owned()) {
        candidates.push(trimmed_steam_id.to_owned());
    }

    if let Ok(steam_id64) = trimmed_steam_id.parse::<u64>() {
        if steam_id64 > STEAM_ID64_ACCOUNT_ID_BASE {
            let account_id = steam_id64 - STEAM_ID64_ACCOUNT_ID_BASE;
            let account_id_string = account_id.to_string();
            if seen.insert(account_id_string.clone()) {
                candidates.push(account_id_string);
            }
        }
    }

    Ok(candidates)
}

fn steam_root_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    for env_name in [
        "STEAM_ROOT_OVERRIDE",
        "STEAM_ROOT",
        "STEAM_PATH",
        "STEAM_INSTALL_PATH",
        "STEAM_COMPAT_CLIENT_INSTALL_PATH",
        "STEAMROOT",
    ] {
        if let Ok(path) = std::env::var(env_name) {
            let trimmed_path = path.trim();
            if !trimmed_path.is_empty() {
                candidates.push(PathBuf::from(trimmed_path));
            }
        }
    }

    if cfg!(target_os = "windows") {
        if let Ok(path) = std::env::var("PROGRAMFILES(X86)") {
            candidates.push(PathBuf::from(path).join("Steam"));
        }
        if let Ok(path) = std::env::var("PROGRAMFILES") {
            candidates.push(PathBuf::from(path).join("Steam"));
        }
        if let Ok(path) = std::env::var("LOCALAPPDATA") {
            candidates.push(PathBuf::from(path).join("Programs").join("Steam"));
        }
        candidates.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
        candidates.push(PathBuf::from(r"C:\Program Files\Steam"));
        candidates.push(PathBuf::from(r"C:\Steam"));
        candidates.push(PathBuf::from(r"D:\Steam"));
        candidates.push(PathBuf::from(r"E:\Steam"));
        candidates.push(PathBuf::from(r"F:\Steam"));
        candidates.push(PathBuf::from(r"G:\Steam"));
    } else if cfg!(target_os = "macos") {
        if let Ok(home) = std::env::var("HOME") {
            let home_path = PathBuf::from(home);
            candidates.push(home_path.join("Library/Application Support/Steam"));
        }
        candidates.push(PathBuf::from("/Users/Shared/Steam"));
    } else {
        if let Ok(home) = std::env::var("HOME") {
            let home_path = PathBuf::from(home);
            candidates.push(home_path.join(".steam/root"));
            candidates.push(home_path.join(".steam/steam"));
            candidates.push(home_path.join(".steam/debian-installation"));
            candidates.push(home_path.join(".local/share/Steam"));
            candidates.push(home_path.join("Steam"));
            candidates.push(home_path.join("snap/steam/common/.local/share/Steam"));
            candidates.push(home_path.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"));
            candidates.push(home_path.join(".var/app/com.valvesoftware.Steam/data/Steam"));
        }
        if let Ok(xdg_data_home) = std::env::var("XDG_DATA_HOME") {
            let xdg_data_home = xdg_data_home.trim();
            if !xdg_data_home.is_empty() {
                candidates.push(PathBuf::from(xdg_data_home).join("Steam"));
            }
        }
    }

    candidates
}

fn resolve_steamapps_directories(steam_root: &Path) -> Result<Vec<PathBuf>, String> {
    let root_steamapps_directory = steam_root.join("steamapps");
    let mut steamapps_directories = Vec::new();
    let mut seen_directories = HashSet::new();

    if seen_directories.insert(root_steamapps_directory.clone()) {
        steamapps_directories.push(root_steamapps_directory.clone());
    }

    let library_folders_path = root_steamapps_directory.join("libraryfolders.vdf");
    let library_folders_content = match fs::read_to_string(&library_folders_path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(steamapps_directories);
        }
        Err(error) => {
            eprintln!(
                "Could not read Steam library folder file at {}: {}; using root steamapps only.",
                library_folders_path.display(),
                error
            );
            return Ok(steamapps_directories);
        }
    };
    let library_paths = match parse_steam_libraryfolder_paths(&library_folders_content) {
        Ok(paths) => paths,
        Err(error) => {
            eprintln!(
                "Could not parse Steam library folders at {}: {}; using root steamapps only.",
                library_folders_path.display(),
                error
            );
            return Ok(steamapps_directories);
        }
    };

    for library_path in library_paths {
        let steamapps_directory = library_path.join("steamapps");
        if seen_directories.insert(steamapps_directory.clone()) {
            steamapps_directories.push(steamapps_directory);
        }
    }

    Ok(steamapps_directories)
}

fn parse_steam_libraryfolder_paths(contents: &str) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    let mut seen_paths = HashSet::new();

    if let Ok(parsed_document) = crate::infrastructure::runtime_vdf::parse_vdf_document(contents) {
        let root_value =
            crate::infrastructure::runtime_vdf::vdf_find_object_value(&parsed_document, "libraryfolders")
                .unwrap_or(&parsed_document);
        collect_steam_libraryfolder_paths_from_vdf(root_value, &mut paths, &mut seen_paths);
        if !paths.is_empty() {
            return Ok(paths);
        }
    }

    let path_pattern = Regex::new(r#"^\s*"path"\s*"([^"]+)""#)
        .map_err(|error| format!("Failed to compile Steam path pattern: {error}"))?;
    let legacy_pattern = Regex::new(r#"^\s*"[0-9]+"\s*"([^"]+)""#)
        .map_err(|error| format!("Failed to compile legacy Steam path pattern: {error}"))?;

    for line in contents.lines() {
        let Some(captures) = path_pattern.captures(line) else {
            continue;
        };
        let Some(matched_path) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let decoded_path = decode_steam_vdf_value(matched_path);
        let trimmed_path = decoded_path.trim();
        if trimmed_path.is_empty() {
            continue;
        }
        let path = PathBuf::from(trimmed_path);
        if seen_paths.insert(path.clone()) {
            paths.push(path);
        }
    }

    if !paths.is_empty() {
        return Ok(paths);
    }

    for line in contents.lines() {
        let Some(captures) = legacy_pattern.captures(line) else {
            continue;
        };
        let Some(matched_path) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let decoded_path = decode_steam_vdf_value(matched_path);
        let trimmed_path = decoded_path.trim();
        if trimmed_path.is_empty() {
            continue;
        }
        let path = PathBuf::from(trimmed_path);
        if seen_paths.insert(path.clone()) {
            paths.push(path);
        }
    }

    Ok(paths)
}

fn collect_steam_libraryfolder_paths_from_vdf(
    value: &crate::infrastructure::runtime_vdf::VdfValue,
    output: &mut Vec<PathBuf>,
    seen_paths: &mut HashSet<PathBuf>,
) {
    let crate::infrastructure::runtime_vdf::VdfValue::Object(entries) = value else {
        return;
    };

    for (entry_key, entry_value) in entries {
        let is_path_key = entry_key.eq_ignore_ascii_case("path");
        let is_legacy_library_key = entry_key.chars().all(|character| character.is_ascii_digit());

        if (is_path_key || is_legacy_library_key)
            && matches!(entry_value, crate::infrastructure::runtime_vdf::VdfValue::Text(_))
        {
            if let crate::infrastructure::runtime_vdf::VdfValue::Text(raw_path) = entry_value {
                let trimmed_path = raw_path.trim();
                if !trimmed_path.is_empty() {
                    let looks_like_filesystem_path = trimmed_path.contains('/')
                        || trimmed_path.contains('\\')
                        || trimmed_path.starts_with('~')
                        || trimmed_path.starts_with('.')
                        || trimmed_path
                            .as_bytes()
                            .get(1)
                            .is_some_and(|character| *character == b':');
                    if !is_path_key && !looks_like_filesystem_path {
                        continue;
                    }
                    let path = PathBuf::from(trimmed_path);
                    if seen_paths.insert(path.clone()) {
                        output.push(path);
                    }
                }
            }
        }

        collect_steam_libraryfolder_paths_from_vdf(entry_value, output, seen_paths);
    }
}

fn decode_steam_vdf_value(value: &str) -> String {
    let mut decoded = String::with_capacity(value.len());
    let mut characters = value.chars();

    while let Some(character) = characters.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }

        let Some(escaped) = characters.next() else {
            break;
        };

        match escaped {
            '\\' => decoded.push('\\'),
            '"' => decoded.push('"'),
            't' => decoded.push('\t'),
            'n' => decoded.push('\n'),
            'r' => decoded.push('\r'),
            other => decoded.push(other),
        }
    }

    decoded
}

fn collect_installed_app_ids_from_steamapps_dir(
    steamapps_directory: &Path,
    installed_app_ids: &mut HashSet<u64>,
) -> Result<(), String> {
    let directory_entries = match fs::read_dir(steamapps_directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "Failed to read Steam library directory {}: {error}",
                steamapps_directory.display()
            ));
        }
    };

    for directory_entry in directory_entries {
        let entry = match directory_entry {
            Ok(value) => value,
            Err(error) => {
                eprintln!(
                    "Could not read Steam library entry in {}: {}",
                    steamapps_directory.display(),
                    error
                );
                continue;
            }
        };
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let Some(app_id) = parse_steam_manifest_app_id(&file_name) else {
            continue;
        };

        let manifest_contents = match fs::read_to_string(entry.path()) {
            Ok(contents) => contents,
            Err(error) => {
                eprintln!(
                    "Could not read Steam app manifest {}: {}",
                    entry.path().display(),
                    error
                );
                continue;
            }
        };

        // Require a fully installed state when the flag is present.
        if let Some(state_flags) = parse_steam_manifest_u64_field(&manifest_contents, "StateFlags") {
            if state_flags & STEAM_APP_STATE_FULLY_INSTALLED == 0 {
                continue;
            }
        }

        let install_dir_name = match parse_steam_manifest_install_directory(&manifest_contents) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let install_directory = steamapps_directory.join("common").join(install_dir_name);
        if !install_directory.is_dir() {
            continue;
        }

        let has_install_content = match fs::read_dir(&install_directory) {
            Ok(mut entries) => entries.next().is_some(),
            Err(_) => false,
        };
        if !has_install_content {
            continue;
        }

        installed_app_ids.insert(app_id);
    }

    Ok(())
}

fn parse_steam_manifest_app_id(file_name: &str) -> Option<u64> {
    let app_id = file_name
        .strip_prefix("appmanifest_")?
        .strip_suffix(".acf")?;
    app_id.parse::<u64>().ok()
}

fn resolve_steam_manifest_path_for_app_id(
    steam_root_override: Option<&str>,
    app_id: u64,
) -> Result<PathBuf, String> {
    let steam_roots = resolve_steam_root_paths(steam_root_override);
    if steam_roots.is_empty() {
        return Err(String::from("Could not locate local Steam installation"));
    }
    let manifest_file_name = format!("appmanifest_{app_id}.acf");
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
            let manifest_path = steamapps_directory.join(&manifest_file_name);
            if manifest_path.is_file() {
                return Ok(manifest_path);
            }
        }
    }

    Err(format!(
        "Could not find Steam app manifest for app {app_id}. Install the game first."
    ))
}

fn parse_steam_manifest_install_directory(manifest_contents: &str) -> Result<String, String> {
    let install_dir_pattern = Regex::new(r#"^\s*"installdir"\s*"([^"]+)""#)
        .map_err(|error| format!("Failed to compile Steam install directory pattern: {error}"))?;

    for line in manifest_contents.lines() {
        let Some(captures) = install_dir_pattern.captures(line) else {
            continue;
        };
        let Some(raw_install_dir) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let decoded_install_dir = decode_steam_vdf_value(raw_install_dir);
        let trimmed_install_dir = decoded_install_dir.trim();
        if trimmed_install_dir.is_empty() {
            continue;
        }

        return Ok(trimmed_install_dir.to_owned());
    }

    Err(String::from(
        "Could not determine install directory from Steam app manifest.",
    ))
}

fn parse_steam_manifest_size_on_disk_bytes(manifest_contents: &str) -> Option<u64> {
    let size_pattern = Regex::new(r#"^\s*"SizeOnDisk"\s*"([^"]+)""#).ok()?;

    for line in manifest_contents.lines() {
        let Some(captures) = size_pattern.captures(line) else {
            continue;
        };
        let Some(raw_size) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let decoded_size = decode_steam_vdf_value(raw_size);
        let trimmed_size = decoded_size.trim();
        if trimmed_size.is_empty() {
            continue;
        }

        if let Ok(parsed_size) = trimmed_size.parse::<u64>() {
            return Some(parsed_size);
        }
    }

    None
}

fn parse_steam_manifest_string_field(manifest_contents: &str, field_name: &str) -> Option<String> {
    let normalized_field_name = field_name.trim();
    if normalized_field_name.is_empty() {
        return None;
    }

    let line_pattern = Regex::new(r#"^\s*"([^"]+)"\s*"([^"]*)""#).ok()?;
    for line in manifest_contents.lines() {
        let Some(captures) = line_pattern.captures(line) else {
            continue;
        };

        let Some(raw_key) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        if !raw_key.eq_ignore_ascii_case(normalized_field_name) {
            continue;
        }

        let Some(raw_value) = captures.get(2).map(|value| value.as_str()) else {
            continue;
        };
        let decoded_value = decode_steam_vdf_value(raw_value);
        let trimmed_value = decoded_value.trim();
        if trimmed_value.is_empty() {
            return None;
        }

        return Some(trimmed_value.to_owned());
    }

    None
}

fn parse_steam_manifest_u64_field(manifest_contents: &str, field_name: &str) -> Option<u64> {
    parse_steam_manifest_string_field(manifest_contents, field_name)?.parse::<u64>().ok()
}

fn parse_steam_manifest_download_progress(
    manifest_contents: &str,
) -> SteamManifestDownloadProgressSnapshot {
    let bytes_to_download = parse_steam_manifest_u64_field(manifest_contents, "BytesToDownload");
    let bytes_downloaded = [
        parse_steam_manifest_u64_field(manifest_contents, "BytesDownloaded"),
        parse_steam_manifest_u64_field(manifest_contents, "BytesDownloadedOnCurrentRun"),
        parse_steam_manifest_u64_field(manifest_contents, "TotalDownloaded"),
    ]
    .into_iter()
    .flatten()
    .max();
    let bytes_to_stage = parse_steam_manifest_u64_field(manifest_contents, "BytesToStage");
    let bytes_staged = [
        parse_steam_manifest_u64_field(manifest_contents, "BytesStaged"),
        parse_steam_manifest_u64_field(manifest_contents, "BytesStagedOnCurrentRun"),
    ]
    .into_iter()
    .flatten()
    .max();

    SteamManifestDownloadProgressSnapshot {
        state_flags: parse_steam_manifest_u64_field(manifest_contents, "StateFlags"),
        bytes_downloaded,
        bytes_to_download,
        bytes_staged,
        bytes_to_stage,
    }
}

fn steam_manifest_is_stale(manifest_path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(manifest_path) else {
        return false;
    };
    let Ok(last_modified_at) = metadata.modified() else {
        return false;
    };
    let Ok(age) = SystemTime::now().duration_since(last_modified_at) else {
        return false;
    };
    age.as_secs() >= STEAM_DIRECTORY_PROGRESS_MANIFEST_STALE_SECONDS
}

fn resolve_steam_manifest_download_progress(
    manifest_path: &Path,
    manifest_contents: &str,
    active_download_directory: &Path,
    active_temp_directory: &Path,
) -> ResolvedSteamDownloadProgressSnapshot {
    let progress_snapshot = parse_steam_manifest_download_progress(manifest_contents);
    let download_total = progress_snapshot.bytes_to_download.filter(|value| *value > 0);
    let stage_total = progress_snapshot.bytes_to_stage.filter(|value| *value > 0);
    let mut bytes_total = download_total.or(stage_total);
    let mut bytes_downloaded = if download_total.is_some() {
        match (progress_snapshot.bytes_downloaded, bytes_total) {
            (Some(downloaded), _) => Some(downloaded),
            (None, Some(_)) => Some(0),
            (None, None) => None,
        }
    } else {
        match (progress_snapshot.bytes_staged, bytes_total) {
            (Some(staged), _) => Some(staged),
            (None, Some(_)) => Some(0),
            (None, None) => None,
        }
    };
    let manifest_is_stale = steam_manifest_is_stale(manifest_path);
    let has_active_download_directory =
        active_download_directory.is_dir() || active_temp_directory.is_dir();
    let mut progress_source = String::from("manifest");

    if has_active_download_directory && (matches!(bytes_downloaded, Some(0)) || manifest_is_stale) {
        let measured_downloaded_bytes = directory_size_bytes(active_download_directory)
            .or_else(|| directory_size_bytes(active_temp_directory));
        if let Some(measured_downloaded_bytes) = measured_downloaded_bytes {
            if let Some(stage_total_bytes) = stage_total {
                let manifest_staged_bytes = progress_snapshot
                    .bytes_staged
                    .unwrap_or(0)
                    .min(stage_total_bytes);
                let staged_bytes = measured_downloaded_bytes
                    .min(stage_total_bytes)
                    .max(manifest_staged_bytes);

                if let Some(download_total_bytes) = download_total {
                    let stage_ratio =
                        (staged_bytes as f64 / stage_total_bytes as f64).clamp(0.0, 1.0);
                    let scaled_download_bytes =
                        (stage_ratio * download_total_bytes as f64).round() as u64;
                    let manifest_downloaded_bytes = bytes_downloaded.unwrap_or(0);
                    let scaled_download_bytes = scaled_download_bytes.max(manifest_downloaded_bytes);
                    let delta_bytes =
                        scaled_download_bytes.saturating_sub(manifest_downloaded_bytes);
                    let should_use_directory_estimate = manifest_downloaded_bytes == 0
                        || (manifest_is_stale
                            && delta_bytes >= STEAM_DIRECTORY_PROGRESS_MIN_DELTA_BYTES);

                    if should_use_directory_estimate && scaled_download_bytes > manifest_downloaded_bytes
                    {
                        let estimated_downloaded_bytes = if manifest_downloaded_bytes == 0 {
                            scaled_download_bytes
                        } else {
                            let blended = manifest_downloaded_bytes as f64
                                + (scaled_download_bytes as f64 - manifest_downloaded_bytes as f64)
                                    * STEAM_DIRECTORY_PROGRESS_BLEND_FACTOR;
                            blended.round() as u64
                        };
                        bytes_total = Some(download_total_bytes);
                        bytes_downloaded = Some(estimated_downloaded_bytes.min(download_total_bytes));
                        progress_source = String::from("directory-estimate");
                    }
                } else if staged_bytes > bytes_downloaded.unwrap_or(0) {
                    bytes_total = Some(stage_total_bytes);
                    bytes_downloaded = Some(staged_bytes);
                    progress_source = String::from("directory-estimate");
                }
            } else if let Some(download_total_bytes) = download_total {
                let estimated_downloaded_bytes = measured_downloaded_bytes.min(download_total_bytes);
                if estimated_downloaded_bytes > bytes_downloaded.unwrap_or(0) {
                    bytes_total = Some(download_total_bytes);
                    bytes_downloaded = Some(estimated_downloaded_bytes);
                    progress_source = String::from("directory-estimate");
                }
            }
        }
    }

    let bytes_downloaded = match (bytes_downloaded, bytes_total) {
        (Some(downloaded), Some(total)) => Some(downloaded.min(total)),
        (value, _) => value,
    };

    ResolvedSteamDownloadProgressSnapshot {
        state_flags: progress_snapshot.state_flags,
        bytes_downloaded,
        bytes_total,
        progress_source,
    }
}

fn infer_steam_download_state(
    state_flags: u64,
    has_progress: bool,
    has_active_download_directory: bool,
) -> Option<&'static str> {
    if state_flags & STEAM_APP_STATE_UPDATE_PAUSED != 0 {
        return Some("Paused");
    }

    if state_flags & STEAM_APP_STATE_PREALLOCATING != 0 {
        return Some("Preallocating");
    }

    if state_flags & STEAM_APP_STATE_DOWNLOADING != 0 {
        return Some("Downloading");
    }

    if state_flags & STEAM_APP_STATE_UPDATE_RUNNING != 0
        || state_flags & STEAM_APP_STATE_UPDATE_STARTED != 0
    {
        if has_progress || has_active_download_directory {
            return Some("Downloading");
        }
        return Some("Updating");
    }

    if state_flags & STEAM_APP_STATE_STAGING != 0 {
        return Some("Staging");
    }

    if state_flags & STEAM_APP_STATE_COMMITTING != 0 || state_flags & STEAM_APP_STATE_ADDING_FILES != 0 {
        return Some("Installing");
    }

    if state_flags & STEAM_APP_STATE_VALIDATING != 0 {
        return Some("Verifying");
    }

    if has_progress || has_active_download_directory {
        return Some("Queued");
    }

    if state_flags & STEAM_APP_STATE_UPDATE_REQUIRED != 0
        && state_flags & STEAM_APP_STATE_FULLY_INSTALLED == 0
    {
        return Some("Queued");
    }

    None
}

fn collect_steam_download_progress_from_steamapps_dir(
    steamapps_directory: &Path,
    owned_games_by_app_id: &HashMap<u64, OwnedSteamGameMetadata>,
    seen_external_ids: &mut HashSet<String>,
    output: &mut Vec<SteamDownloadProgressResponse>,
) -> Result<(), String> {
    let allow_unknown_games = owned_games_by_app_id.is_empty();
    let directory_entries = match fs::read_dir(steamapps_directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "Failed to read Steam library directory {}: {error}",
                steamapps_directory.display()
            ));
        }
    };

    for directory_entry in directory_entries {
        let entry = directory_entry
            .map_err(|error| format!("Failed to read Steam library entry: {error}"))?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let Some(app_id) = parse_steam_manifest_app_id(&file_name) else {
            continue;
        };

        let manifest_contents = match fs::read_to_string(entry.path()) {
            Ok(contents) => contents,
            Err(error) => {
                eprintln!(
                    "Could not read Steam app manifest {}: {}",
                    entry.path().display(),
                    error
                );
                continue;
            }
        };

        let app_id_path_segment = app_id.to_string();
        let active_download_directory = steamapps_directory
            .join("downloading")
            .join(&app_id_path_segment);
        let active_temp_directory = steamapps_directory.join("temp").join(&app_id_path_segment);
        let progress_snapshot = resolve_steam_manifest_download_progress(
            &entry.path(),
            &manifest_contents,
            &active_download_directory,
            &active_temp_directory,
        );
        let bytes_total = progress_snapshot.bytes_total;
        let has_active_download_directory =
            active_download_directory.is_dir() || active_temp_directory.is_dir();
        let bytes_downloaded = progress_snapshot.bytes_downloaded;
        let progress_source = progress_snapshot.progress_source;

        let has_progress = match (bytes_downloaded, bytes_total) {
            (Some(downloaded), Some(total)) => downloaded < total,
            _ => false,
        };
        let state_flags = progress_snapshot.state_flags.unwrap_or(0);
        let Some(state_label) =
            infer_steam_download_state(state_flags, has_progress, has_active_download_directory)
        else {
            continue;
        };
        let is_actively_transferring = has_active_download_directory
            || state_flags & STEAM_APP_STATE_DOWNLOADING != 0
            || state_flags & STEAM_APP_STATE_PREALLOCATING != 0;
        let game_metadata = owned_games_by_app_id.get(&app_id);
        if !allow_unknown_games && game_metadata.is_none() {
            continue;
        }
        if !state_label.eq_ignore_ascii_case("Downloading") || !is_actively_transferring {
            continue;
        }
        let external_id = game_metadata
            .map(|game| game.external_id.clone())
            .unwrap_or_else(|| app_id.to_string());
        if !seen_external_ids.insert(external_id.clone()) {
            continue;
        }

        let progress_percent = match (bytes_downloaded, bytes_total) {
            (Some(downloaded), Some(total)) if total > 0 => Some(
                ((downloaded.min(total)) as f64 / total as f64 * 100.0).clamp(0.0, 100.0),
            ),
            _ => None,
        };
        let name = game_metadata
            .map(|game| game.name.clone())
            .or_else(|| parse_steam_manifest_string_field(&manifest_contents, "name"))
            .unwrap_or_else(|| format!("Steam App {app_id}"));
        let game_id = game_metadata
            .map(|game| game.game_id.clone())
            .unwrap_or_else(|| format!("steam:{app_id}"));

        output.push(SteamDownloadProgressResponse {
            game_id,
            provider: String::from("steam"),
            external_id,
            name,
            state: String::from(state_label),
            bytes_downloaded,
            bytes_total,
            progress_percent,
            progress_source: Some(progress_source),
        });
    }

    for download_subdirectory in ["downloading", "temp"] {
        let active_downloads_directory = steamapps_directory.join(download_subdirectory);
        let directory_entries = match fs::read_dir(&active_downloads_directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                eprintln!(
                    "Could not read Steam active download directory {}: {}",
                    active_downloads_directory.display(),
                    error
                );
                continue;
            }
        };

        for directory_entry in directory_entries {
            let entry = match directory_entry {
                Ok(value) => value,
                Err(error) => {
                    eprintln!(
                        "Could not read Steam active download entry in {}: {}",
                        active_downloads_directory.display(),
                        error
                    );
                    continue;
                }
            };
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }
            let raw_file_name = entry.file_name();
            let Some(file_name) = raw_file_name.to_str().map(str::trim) else {
                continue;
            };
            let Some(app_id) = file_name.parse::<u64>().ok() else {
                continue;
            };

            let game_metadata = owned_games_by_app_id.get(&app_id);
            if !allow_unknown_games && game_metadata.is_none() {
                continue;
            }
            let external_id = game_metadata
                .map(|game| game.external_id.clone())
                .unwrap_or_else(|| app_id.to_string());
            if !seen_external_ids.insert(external_id.clone()) {
                continue;
            }

            let name = game_metadata
                .map(|game| game.name.clone())
                .unwrap_or_else(|| format!("Steam App {app_id}"));
            let game_id = game_metadata
                .map(|game| game.game_id.clone())
                .unwrap_or_else(|| format!("steam:{app_id}"));

            let manifest_path = steamapps_directory.join(format!("appmanifest_{app_id}.acf"));
            let (bytes_downloaded, bytes_total, progress_percent, progress_source) =
                if let Ok(manifest_contents) = fs::read_to_string(&manifest_path) {
                    let progress_snapshot = resolve_steam_manifest_download_progress(
                        &manifest_path,
                        &manifest_contents,
                        &steamapps_directory.join("downloading").join(file_name),
                        &steamapps_directory.join("temp").join(file_name),
                    );
                    let bytes_downloaded = progress_snapshot.bytes_downloaded;
                    let bytes_total = progress_snapshot.bytes_total;
                    let progress_source = progress_snapshot.progress_source;
                    let progress_percent = match (bytes_downloaded, bytes_total) {
                        (Some(downloaded), Some(total)) if total > 0 => Some(
                            ((downloaded.min(total)) as f64 / total as f64 * 100.0).clamp(0.0, 100.0),
                        ),
                        _ => None,
                    };
                    (bytes_downloaded, bytes_total, progress_percent, Some(progress_source))
                } else {
                    (None, None, None, None)
                };

            output.push(SteamDownloadProgressResponse {
                game_id,
                provider: String::from("steam"),
                external_id,
                name,
                state: String::from("Downloading"),
                bytes_downloaded,
                bytes_total,
                progress_percent,
                progress_source,
            });
        }
    }

    Ok(())
}

fn directory_size_bytes(path: &Path) -> Option<u64> {
    if !path.is_dir() {
        return None;
    }

    if cfg!(target_os = "linux") {
        let output = Command::new("du").arg("-sb").arg(path).output().ok()?;
        if output.status.success() {
            let stdout = String::from_utf8(output.stdout).ok()?;
            let first_token = stdout.split_whitespace().next()?;
            if let Ok(size_bytes) = first_token.parse::<u64>() {
                return Some(size_bytes);
            }
        }
    }

    None
}

fn detect_available_disk_space_bytes(path: &Path) -> Option<u64> {
    if cfg!(target_os = "windows") {
        return None;
    }

    let output = Command::new("df")
        .arg("-Pk")
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    let data_row = stdout.lines().nth(1)?;
    let available_kib = data_row.split_whitespace().nth(3)?.parse::<u64>().ok()?;
    Some(available_kib.saturating_mul(1024))
}

fn resolve_steam_install_directory_for_app_id(
    steam_root_override: Option<&str>,
    app_id: u64,
) -> Result<PathBuf, String> {
    let manifest_path = resolve_steam_manifest_path_for_app_id(steam_root_override, app_id)?;
    let manifest_contents = fs::read_to_string(&manifest_path).map_err(|error| {
        format!(
            "Failed to read Steam app manifest at {}: {error}",
            manifest_path.display()
        )
    })?;
    let install_dir_name = parse_steam_manifest_install_directory(&manifest_contents)?;
    let steamapps_directory = manifest_path.parent().ok_or_else(|| {
        format!(
            "Failed to resolve Steam library directory for manifest {}",
            manifest_path.display()
        )
    })?;

    Ok(steamapps_directory.join("common").join(install_dir_name))
}

fn resolve_steam_game_kinds(
    connection: &Connection,
    client: &Client,
    games: &[SteamOwnedGame],
) -> Result<HashMap<u64, String>, String> {
    let app_ids = games.iter().map(|game| game.appid).collect::<Vec<_>>();
    resolve_steam_app_kinds_for_app_ids(connection, client, &app_ids)
}

fn resolve_steam_app_kinds_for_app_ids(
    connection: &Connection,
    client: &Client,
    app_ids: &[u64],
) -> Result<HashMap<u64, String>, String> {
    let stale_before = Utc::now() - ChronoDuration::hours(STEAM_APP_METADATA_CACHE_TTL_HOURS);
    let mut kinds_by_app_id = HashMap::new();
    let mut uncached_app_ids = Vec::new();
    let mut seen_app_ids = HashSet::new();

    for app_id in app_ids {
        if !seen_app_ids.insert(*app_id) {
            continue;
        }

        if let Some(cached_type) = find_cached_steam_app_type(connection, *app_id, stale_before)?
        {
            kinds_by_app_id.insert(
                *app_id,
                steam_kind_from_app_type(&cached_type).to_owned(),
            );
        } else {
            uncached_app_ids.push(*app_id);
        }
    }

    for app_id_batch in uncached_app_ids.chunks(STEAM_APP_DETAILS_BATCH_SIZE) {
        let fetched_types = match fetch_steam_app_types_batch(client, app_id_batch) {
            Ok(types) => types,
            Err(_) => continue,
        };

        for (app_id, app_type) in fetched_types {
            cache_steam_app_type(connection, app_id, &app_type)?;
            kinds_by_app_id.insert(app_id, steam_kind_from_app_type(&app_type).to_owned());
        }
    }

    Ok(kinds_by_app_id)
}

fn find_cached_steam_app_type(
    connection: &Connection,
    app_id: u64,
    stale_before: chrono::DateTime<Utc>,
) -> Result<Option<String>, String> {
    let cached = connection
        .query_row(
            "SELECT app_type, fetched_at FROM steam_app_metadata WHERE app_id = ?1",
            params![app_id.to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| format!("Failed to query cached Steam app metadata: {error}"))?;

    let Some((app_type, fetched_at)) = cached else {
        return Ok(None);
    };

    let is_fresh = chrono::DateTime::parse_from_rfc3339(&fetched_at)
        .map(|timestamp| timestamp.with_timezone(&Utc) >= stale_before)
        .unwrap_or(false);
    if !is_fresh {
        return Ok(None);
    }

    let normalized_type = normalize_steam_app_type(&app_type);
    if normalized_type.is_empty() {
        return Ok(None);
    }

    Ok(Some(normalized_type))
}

fn cache_steam_app_type(
    connection: &Connection,
    app_id: u64,
    app_type: &str,
) -> Result<(), String> {
    let normalized_type = normalize_steam_app_type(app_type);
    if normalized_type.is_empty() {
        return Ok(());
    }

    connection
        .execute(
            "
            INSERT INTO steam_app_metadata (app_id, app_type, fetched_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(app_id) DO UPDATE SET
              app_type = excluded.app_type,
              fetched_at = excluded.fetched_at
            ",
            params![app_id.to_string(), normalized_type, Utc::now().to_rfc3339()],
        )
        .map_err(|error| format!("Failed to cache Steam app metadata: {error}"))?;

    Ok(())
}

fn fetch_steam_app_types_batch(
    client: &Client,
    app_id_batch: &[u64],
) -> Result<HashMap<u64, String>, String> {
    if app_id_batch.is_empty() {
        return Ok(HashMap::new());
    }

    let app_ids = app_id_batch
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let mut request_url = Url::parse(STEAM_APP_DETAILS_ENDPOINT)
        .map_err(|error| format!("Failed to parse Steam app details endpoint: {error}"))?;
    request_url
        .query_pairs_mut()
        .append_pair("appids", &app_ids);

    let response = client
        .get(request_url)
        .send()
        .map_err(|error| format!("Steam app details request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam app details request failed with status {}",
            response.status()
        ));
    }

    let payload = response
        .json::<serde_json::Value>()
        .map_err(|error| format!("Failed to decode Steam app details response: {error}"))?;

    let mut app_types = HashMap::new();
    for app_id in app_id_batch {
        let key = app_id.to_string();
        let Some(entry) = payload.get(&key) else {
            continue;
        };
        let Some(true) = entry.get("success").and_then(serde_json::Value::as_bool) else {
            continue;
        };

        let app_type = entry
            .get("data")
            .and_then(|value| value.get("type"))
            .and_then(serde_json::Value::as_str)
            .map(normalize_steam_app_type)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| String::from("unknown"));

        app_types.insert(*app_id, app_type);
    }

    Ok(app_types)
}

fn resolve_steam_public_app_names(
    connection: &Connection,
    client: &Client,
    app_ids: &[u64],
) -> Result<HashMap<u64, String>, String> {
    if app_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let stale_before = Utc::now() - ChronoDuration::hours(STEAM_PUBLIC_APP_NAME_CACHE_TTL_HOURS);
    let mut resolved_names_by_app_id = HashMap::new();
    let mut uncached_app_ids = Vec::new();
    let mut seen_app_ids = HashSet::new();

    for app_id in app_ids {
        if !seen_app_ids.insert(*app_id) {
            continue;
        }
        if let Some(cached_name) =
            find_cached_steam_public_app_name(connection, *app_id, stale_before)?
        {
            resolved_names_by_app_id.insert(*app_id, cached_name);
        } else {
            uncached_app_ids.push(*app_id);
        }
    }

    if !uncached_app_ids.is_empty() {
        let lookup_limit = uncached_app_ids
            .len()
            .min(STEAM_PUBLIC_APP_NAME_LOOKUP_MAX_REQUESTS_PER_SYNC);
        let lookup_started_at = Instant::now();
        let mut consecutive_errors = 0usize;
        for app_id in uncached_app_ids.into_iter().take(lookup_limit) {
            if lookup_started_at.elapsed()
                >= Duration::from_secs(STEAM_PUBLIC_APP_NAME_LOOKUP_TIME_BUDGET_SECS)
            {
                break;
            }
            let fetched_name = match fetch_steam_app_name(client, app_id) {
                Ok(name) => name,
                Err(error) => {
                    consecutive_errors += 1;
                    eprintln!(
                        "Could not fetch Steam public app name from app details for app {app_id}: {error}"
                    );
                    if consecutive_errors >= STEAM_PUBLIC_APP_NAME_LOOKUP_MAX_CONSECUTIVE_ERRORS {
                        eprintln!(
                            "Stopping Steam app details name lookups after {} consecutive failures; continuing with app-list fallback.",
                            STEAM_PUBLIC_APP_NAME_LOOKUP_MAX_CONSECUTIVE_ERRORS
                        );
                        break;
                    }
                    continue;
                }
            };
            consecutive_errors = 0;

            let Some(name) = fetched_name else {
                continue;
            };
            cache_steam_public_app_name(connection, app_id, &name)?;
            resolved_names_by_app_id.insert(app_id, name);
        }
    }

    let mut unresolved_app_ids = HashSet::new();
    for app_id in &seen_app_ids {
        if resolved_names_by_app_id.contains_key(&app_id) {
            continue;
        }
        unresolved_app_ids.insert(*app_id);
    }
    if unresolved_app_ids.is_empty() {
        return Ok(resolved_names_by_app_id);
    }

    match fetch_steam_public_app_names_from_app_list(client, &unresolved_app_ids) {
        Ok(app_list_names) => {
            for (app_id, name) in app_list_names {
                cache_steam_public_app_name(connection, app_id, &name)?;
                resolved_names_by_app_id.insert(app_id, name);
            }
        }
        Err(error) => {
            eprintln!("Could not fetch Steam public app names from app list: {error}");
        }
    }

    let mut unresolved_after_app_list = seen_app_ids
        .into_iter()
        .filter(|app_id| !resolved_names_by_app_id.contains_key(app_id))
        .collect::<Vec<_>>();
    if !unresolved_after_app_list.is_empty() {
        unresolved_after_app_list.sort_unstable();
        let lookup_started_at = Instant::now();
        let mut consecutive_errors = 0usize;
        for app_id in unresolved_after_app_list {
            if lookup_started_at.elapsed()
                >= Duration::from_secs(STEAM_PUBLIC_APP_NAME_LOOKUP_TIME_BUDGET_SECS)
            {
                break;
            }

            let fetched_name = match fetch_steam_app_name_from_store_page(client, app_id) {
                Ok(name) => name,
                Err(error) => {
                    consecutive_errors += 1;
                    eprintln!(
                        "Could not fetch Steam public app name from store page for app {app_id}: {error}"
                    );
                    if consecutive_errors >= STEAM_PUBLIC_APP_NAME_LOOKUP_MAX_CONSECUTIVE_ERRORS {
                        break;
                    }
                    continue;
                }
            };
            consecutive_errors = 0;

            let Some(name) = fetched_name else {
                continue;
            };
            cache_steam_public_app_name(connection, app_id, &name)?;
            resolved_names_by_app_id.insert(app_id, name);
        }
    }

    Ok(resolved_names_by_app_id)
}

fn fetch_steam_app_name(
    client: &Client,
    app_id: u64,
) -> Result<Option<String>, String> {
    let mut request_url = Url::parse(STEAM_APP_DETAILS_ENDPOINT)
        .map_err(|error| format!("Failed to parse Steam app details endpoint: {error}"))?;
    request_url
        .query_pairs_mut()
        .append_pair("appids", &app_id.to_string())
        .append_pair("l", "english");

    let response = client
        .get(request_url)
        .send()
        .map_err(|error| format!("Steam app details request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam app details request failed with status {}",
            response.status()
        ));
    }

    let payload = response
        .json::<serde_json::Value>()
        .map_err(|error| format!("Failed to decode Steam app details response: {error}"))?;

    let key = app_id.to_string();
    let Some(entry) = payload.get(&key) else {
        return Ok(None);
    };
    let Some(true) = entry.get("success").and_then(serde_json::Value::as_bool) else {
        return Ok(None);
    };
    let Some(name) = entry
        .get("data")
        .and_then(|value| value.get("name"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    Ok(Some(name.to_owned()))
}

fn fetch_steam_app_name_from_store_page(
    client: &Client,
    app_id: u64,
) -> Result<Option<String>, String> {
    let mut request_url = Url::parse(&format!("{STEAM_STORE_APP_ENDPOINT}/{app_id}/"))
        .map_err(|error| format!("Failed to parse Steam store app endpoint: {error}"))?;
    request_url.query_pairs_mut().append_pair("l", "english");

    let response = client
        .get(request_url)
        .send()
        .map_err(|error| format!("Steam store app page request failed: {error}"))?;
    if !response.status().is_success() {
        if response.status().as_u16() == 404 {
            return Ok(None);
        }
        return Err(format!(
            "Steam store app page request failed with status {}",
            response.status()
        ));
    }

    let page_html = response
        .text()
        .map_err(|error| format!("Failed to read Steam store app page response: {error}"))?;
    Ok(extract_steam_app_name_from_store_page_html(&page_html))
}

fn extract_steam_app_name_from_store_page_html(page_html: &str) -> Option<String> {
    let app_name_regex = Regex::new(
        r#"(?is)<div[^>]*id\s*=\s*["']appHubAppName["'][^>]*>\s*([^<]+?)\s*</div>"#,
    )
    .ok()?;
    if let Some(captures) = app_name_regex.captures(page_html) {
        let name = captures.get(1)?.as_str().trim();
        if !name.is_empty() {
            return Some(decode_basic_html_entities(name));
        }
    }

    let og_title_regex =
        Regex::new(r#"(?is)<meta[^>]*property\s*=\s*["']og:title["'][^>]*content\s*=\s*["']([^"']+)["']"#)
            .ok()?;
    let captures = og_title_regex.captures(page_html)?;
    let raw_title = captures.get(1)?.as_str().trim();
    if raw_title.is_empty() {
        return None;
    }

    let decoded_title = decode_basic_html_entities(raw_title);
    let normalized = decoded_title
        .strip_suffix(" on Steam")
        .unwrap_or(decoded_title.as_str())
        .trim()
        .to_owned();
    if normalized.is_empty() {
        return None;
    }

    Some(normalized)
}

fn fetch_steam_public_app_names_from_app_list(
    client: &Client,
    unresolved_app_ids: &HashSet<u64>,
) -> Result<HashMap<u64, String>, String> {
    if unresolved_app_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut endpoint_errors = Vec::new();
    let mut saw_404_status = false;
    let mut saw_non_404_failure = false;
    for endpoint in STEAM_PUBLIC_APP_LIST_ENDPOINTS {
        let request_url = match Url::parse(endpoint) {
            Ok(url) => url,
            Err(error) => {
                endpoint_errors.push(format!("{endpoint} parse failed: {error}"));
                saw_non_404_failure = true;
                continue;
            }
        };
        let response = match client.get(request_url).send() {
            Ok(value) => value,
            Err(error) => {
                endpoint_errors.push(format!("{endpoint} request failed: {error}"));
                saw_non_404_failure = true;
                continue;
            }
        };
        if !response.status().is_success() {
            if response.status().as_u16() == 404 {
                saw_404_status = true;
            } else {
                saw_non_404_failure = true;
            }
            endpoint_errors.push(format!("{endpoint} status {}", response.status()));
            continue;
        }

        let payload = match response.json::<serde_json::Value>() {
            Ok(value) => value,
            Err(error) => {
                endpoint_errors.push(format!("{endpoint} decode failed: {error}"));
                saw_non_404_failure = true;
                continue;
            }
        };
        let Some(apps) = payload
            .get("applist")
            .and_then(|value| value.get("apps"))
            .and_then(serde_json::Value::as_array)
        else {
            endpoint_errors.push(format!("{endpoint} missing applist.apps"));
            saw_non_404_failure = true;
            continue;
        };

        let mut names_by_app_id = HashMap::new();
        for app_entry in apps {
            let Some(app_id) = app_entry.get("appid").and_then(serde_json::Value::as_u64) else {
                continue;
            };
            if !unresolved_app_ids.contains(&app_id) {
                continue;
            }
            let Some(name) = app_entry
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };

            names_by_app_id.insert(app_id, name.to_owned());
            if names_by_app_id.len() == unresolved_app_ids.len() {
                break;
            }
        }

        return Ok(names_by_app_id);
    }

    // When all known endpoints return 404, treat this fallback as unavailable
    // rather than an error so local syncs remain quiet and functional.
    if saw_404_status && !saw_non_404_failure {
        return Ok(HashMap::new());
    }

    Err(format!(
        "All Steam public app list endpoints failed: {}",
        endpoint_errors.join(" | ")
    ))
}

fn find_cached_steam_public_app_name(
    connection: &Connection,
    app_id: u64,
    stale_before: chrono::DateTime<Utc>,
) -> Result<Option<String>, String> {
    let cached = connection
        .query_row(
            "SELECT name, fetched_at FROM steam_public_app_names WHERE app_id = ?1",
            params![app_id.to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| format!("Failed to query cached Steam public app name: {error}"))?;

    let Some((name, fetched_at)) = cached else {
        return Ok(None);
    };
    let trimmed_name = name.trim();
    if trimmed_name.is_empty() {
        return Ok(None);
    }

    let is_fresh = chrono::DateTime::parse_from_rfc3339(&fetched_at)
        .map(|timestamp| timestamp.with_timezone(&Utc) >= stale_before)
        .unwrap_or(false);
    if !is_fresh {
        return Ok(None);
    }

    Ok(Some(trimmed_name.to_owned()))
}

fn cache_steam_public_app_name(
    connection: &Connection,
    app_id: u64,
    name: &str,
) -> Result<(), String> {
    let trimmed_name = name.trim();
    if trimmed_name.is_empty() {
        return Ok(());
    }

    connection
        .execute(
            "
            INSERT INTO steam_public_app_names (app_id, name, fetched_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(app_id) DO UPDATE SET
              name = excluded.name,
              fetched_at = excluded.fetched_at
            ",
            params![app_id.to_string(), trimmed_name, Utc::now().to_rfc3339()],
        )
        .map_err(|error| format!("Failed to cache Steam public app name: {error}"))?;

    Ok(())
}

fn refresh_steam_store_tags_cache(
    connection: &Connection,
    client: &Client,
    app_ids: &[u64],
) -> Result<(), String> {
    let stale_before = Utc::now() - ChronoDuration::hours(STEAM_APP_STORE_TAGS_CACHE_TTL_HOURS);
    let mut seen_app_ids = HashSet::new();
    let started_at = Instant::now();
    let mut attempted_fetches = 0usize;

    for app_id in app_ids {
        if !seen_app_ids.insert(*app_id) {
            continue;
        }

        if find_cached_steam_store_tags(connection, *app_id, stale_before)?.is_some() {
            continue;
        }

        if attempted_fetches >= STEAM_STORE_TAGS_SYNC_MAX_REQUESTS
            || started_at.elapsed() >= Duration::from_secs(STEAM_STORE_TAGS_SYNC_TIME_BUDGET_SECS)
        {
            break;
        }

        let fetched_tags = match fetch_steam_store_user_tags(client, *app_id) {
            Ok(tags) => tags,
            Err(error) => {
                eprintln!("Could not fetch Steam Store tags for app {app_id}: {error}");
                Vec::new()
            }
        };
        attempted_fetches += 1;
        cache_steam_store_tags(connection, *app_id, &fetched_tags)?;
    }

    Ok(())
}

fn find_cached_steam_store_tags(
    connection: &Connection,
    app_id: u64,
    stale_before: chrono::DateTime<Utc>,
) -> Result<Option<Vec<String>>, String> {
    let cached = connection
        .query_row(
            "SELECT tags_json, fetched_at FROM steam_app_store_tags WHERE app_id = ?1",
            params![app_id.to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| format!("Failed to query cached Steam Store tags: {error}"))?;

    let Some((tags_json, fetched_at)) = cached else {
        return Ok(None);
    };

    let is_fresh = chrono::DateTime::parse_from_rfc3339(&fetched_at)
        .map(|timestamp| timestamp.with_timezone(&Utc) >= stale_before)
        .unwrap_or(false);
    if !is_fresh {
        return Ok(None);
    }

    let parsed_tags = serde_json::from_str::<Vec<String>>(&tags_json).unwrap_or_default();
    Ok(Some(normalize_steam_store_tags(&parsed_tags)))
}

fn cache_steam_store_tags(
    connection: &Connection,
    app_id: u64,
    tags: &[String],
) -> Result<(), String> {
    let normalized_tags = normalize_steam_store_tags(tags);
    let tags_json = serde_json::to_string(&normalized_tags)
        .map_err(|error| format!("Failed to encode Steam Store tags cache entry: {error}"))?;

    connection
        .execute(
            "
            INSERT INTO steam_app_store_tags (app_id, tags_json, fetched_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(app_id) DO UPDATE SET
              tags_json = excluded.tags_json,
              fetched_at = excluded.fetched_at
            ",
            params![app_id.to_string(), tags_json, Utc::now().to_rfc3339()],
        )
        .map_err(|error| format!("Failed to cache Steam Store tags: {error}"))?;

    Ok(())
}

fn find_cached_steam_app_details(
    connection: &Connection,
    app_id: u64,
    stale_before: chrono::DateTime<Utc>,
) -> Result<Option<serde_json::Value>, String> {
    let cached = connection
        .query_row(
            "SELECT details_json, fetched_at FROM steam_app_details WHERE app_id = ?1",
            params![app_id.to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| format!("Failed to query cached Steam app details: {error}"))?;

    let Some((details_json, fetched_at)) = cached else {
        return Ok(None);
    };

    let is_fresh = chrono::DateTime::parse_from_rfc3339(&fetched_at)
        .map(|timestamp| timestamp.with_timezone(&Utc) >= stale_before)
        .unwrap_or(false);
    if !is_fresh {
        return Ok(None);
    }

    let parsed = serde_json::from_str::<serde_json::Value>(&details_json)
        .map_err(|error| format!("Failed to parse cached Steam app details JSON: {error}"))?;
    Ok(Some(parsed))
}

fn cache_steam_app_details(
    connection: &Connection,
    app_id: u64,
    details: &serde_json::Value,
) -> Result<(), String> {
    let details_json = serde_json::to_string(details)
        .map_err(|error| format!("Failed to encode Steam app details for cache: {error}"))?;

    connection
        .execute(
            "INSERT INTO steam_app_details (app_id, details_json, fetched_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(app_id) DO UPDATE SET
              details_json = excluded.details_json,
              fetched_at = excluded.fetched_at",
            params![app_id.to_string(), details_json, Utc::now().to_rfc3339()],
        )
        .map_err(|error| format!("Failed to cache Steam app details: {error}"))?;

    // Also attempt to infer and cache common features (best-effort)
    if let Some(data) = details.get("data") {
        // achievements: presence of `achievements` object
        let has_achievements = data.get("achievements").is_some();
        // cloud saves: presence of `cloud` object or `cloud` enabled flag
        let has_cloud = data
            .get("cloud")
            .and_then(|v| v.get("enabled").and_then(serde_json::Value::as_bool))
            .unwrap_or_else(|| data.get("cloud").is_some());

        // controller support: look in `categories` for controller descriptions, fallback to `controller_support` fields
        let mut controller_support: Option<String> = None;
        if let Some(categories) = data.get("categories").and_then(serde_json::Value::as_array) {
            for cat in categories {
                if let Some(desc) = cat.get("description").and_then(serde_json::Value::as_str) {
                    let lowered = desc.to_ascii_lowercase();
                    if lowered.contains("full controller") || lowered.contains("full controller support") {
                        controller_support = Some(String::from("Full"));
                        break;
                    }
                    if lowered.contains("partial controller") || lowered.contains("partial controller support") {
                        controller_support = Some(String::from("Partial"));
                        break;
                    }
                }
            }
        }
        if controller_support.is_none() {
            if let Some(cs) = data.get("controller_support").and_then(serde_json::Value::as_str) {
                controller_support = Some(cs.to_owned());
            } else if let Some(cs) = data.get("controller_supports").and_then(serde_json::Value::as_str) {
                controller_support = Some(cs.to_owned());
            }
        }

        // best-effort persist features (achievements_count & cloud_details not inferred here)
        let _ = cache_steam_app_features(connection, app_id, has_achievements, None, has_cloud, None, controller_support.as_deref());
    }

    Ok(())
}

fn cache_steam_app_features(
    connection: &Connection,
    app_id: u64,
    has_achievements: bool,
    achievements_count: Option<u64>,
    has_cloud_saves: bool,
    cloud_details: Option<&str>,
    controller_support: Option<&str>,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO steam_app_features (app_id, has_achievements, achievements_count, has_cloud_saves, cloud_details, controller_support, fetched_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(app_id) DO UPDATE SET
              has_achievements = excluded.has_achievements,
              achievements_count = excluded.achievements_count,
              has_cloud_saves = excluded.has_cloud_saves,
              cloud_details = excluded.cloud_details,
              controller_support = excluded.controller_support,
              fetched_at = excluded.fetched_at",
            params![
                app_id.to_string(),
                if has_achievements { 1 } else { 0 },
                achievements_count.map(|v| v.to_string()),
                if has_cloud_saves { 1 } else { 0 },
                cloud_details,
                controller_support,
                Utc::now().to_rfc3339()
            ],
        )
        .map_err(|error| format!("Failed to cache Steam app features: {error}"))?;

    Ok(())
}

fn find_cached_steam_app_features(
    connection: &Connection,
    app_id: u64,
    stale_before: chrono::DateTime<Utc>,
) -> Result<Option<(bool, Option<i64>, bool, Option<String>, Option<String>)>, String> {
    let cached = connection
        .query_row(
            "SELECT has_achievements, achievements_count, has_cloud_saves, cloud_details, controller_support, fetched_at FROM steam_app_features WHERE app_id = ?1",
            params![app_id.to_string()],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, i64>(2)?, row.get::<_, Option<String>>(3)?, row.get::<_, Option<String>>(4)?, row.get::<_, String>(5)?)),
        )
        .optional()
        .map_err(|error| format!("Failed to query cached Steam app features: {error}"))?;

    let Some((ach_raw, ach_count_opt, cloud_raw, cloud_details_opt, controller_opt, fetched_at)) = cached else {
        return Ok(None);
    };

    let is_fresh = chrono::DateTime::parse_from_rfc3339(&fetched_at)
        .map(|timestamp| timestamp.with_timezone(&Utc) >= stale_before)
        .unwrap_or(false);
    if !is_fresh {
        return Ok(None);
    }

    let achievements_count = ach_count_opt.and_then(|s| s.parse::<i64>().ok());
    Ok(Some((ach_raw > 0, achievements_count, cloud_raw > 0, cloud_details_opt, controller_opt)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn cache_and_find_steam_app_details_roundtrip() {
        let connection = Connection::open_in_memory().expect("open in-memory");

        // create minimal steam_app_details table used by helpers
        connection
            .execute(
                "CREATE TABLE IF NOT EXISTS steam_app_details (
                    app_id TEXT PRIMARY KEY,
                    details_json TEXT NOT NULL,
                    fetched_at TEXT NOT NULL
                )",
                (),
            )
            .expect("create table");

        let app_id: u64 = 12345;
        let entry = serde_json::json!({
            "success": true,
            "data": { "name": "Test Game" }
        });

        // cache entry
        cache_steam_app_details(&connection, app_id, &entry).expect("cache ok");

        let stale_before = Utc::now() - ChronoDuration::hours(24);
        let cached = find_cached_steam_app_details(&connection, app_id, stale_before)
            .expect("query ok");
        assert!(cached.is_some(), "expected cached entry to be present");
        let cached = cached.unwrap();
        assert_eq!(cached.get("success").and_then(|v| v.as_bool()), Some(true));
        assert!(cached.get("data").is_some());
    }

    #[test]
    fn cache_and_find_steam_public_app_name_roundtrip() {
        let connection = Connection::open_in_memory().expect("open in-memory");
        connection
            .execute(
                "CREATE TABLE IF NOT EXISTS steam_public_app_names (
                    app_id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    fetched_at TEXT NOT NULL
                )",
                (),
            )
            .expect("create table");

        let app_id: u64 = 570;
        cache_steam_public_app_name(&connection, app_id, "Dota 2").expect("cache app name");

        let stale_before = Utc::now() - ChronoDuration::hours(24);
        let cached_name = find_cached_steam_public_app_name(&connection, app_id, stale_before)
            .expect("query app name");
        assert_eq!(cached_name.as_deref(), Some("Dota 2"));
    }

    #[test]
    fn parse_active_steam_id_prefers_most_recent_loginuser() {
        let contents = r#"
            "users"
            {
                "76561198000000001"
                {
                    "AccountName" "older"
                    "AllowAutoLogin" "1"
                    "MostRecent" "0"
                }
                "76561198000000002"
                {
                    "AccountName" "active"
                    "AllowAutoLogin" "0"
                    "MostRecent" "1"
                }
            }
        "#;

        let active_steam_id = parse_active_steam_id_from_loginusers(contents);
        assert_eq!(active_steam_id.as_deref(), Some("76561198000000002"));
    }

    #[test]
    fn parse_active_steam_id_falls_back_to_auto_login_user() {
        let contents = r#"
            "users"
            {
                "76561198000000003"
                {
                    "AccountName" "candidate"
                    "AllowAutoLogin" "1"
                }
                "76561198000000004"
                {
                    "AccountName" "secondary"
                    "AllowAutoLogin" "0"
                }
            }
        "#;

        let active_steam_id = parse_active_steam_id_from_loginusers(contents);
        assert_eq!(active_steam_id.as_deref(), Some("76561198000000003"));
    }

    #[test]
    fn resolve_active_local_steam_id_falls_back_to_userdata_when_loginusers_missing() {
        let steam_root = tempdir().expect("create temp steam root");
        let account_id = 12_345_678_u64;
        let expected_steam_id = (STEAM_ID64_ACCOUNT_ID_BASE + account_id).to_string();
        fs::create_dir_all(steam_root.path().join("steamapps")).expect("create steamapps directory");

        fs::create_dir_all(
            steam_root
                .path()
                .join("userdata")
                .join(account_id.to_string())
                .join("config"),
        )
        .expect("create userdata config directory");
        fs::write(
            steam_root
                .path()
                .join("userdata")
                .join(account_id.to_string())
                .join("config")
                .join("localconfig.vdf"),
            "\"UserLocalConfigStore\" {}",
        )
        .expect("write localconfig");

        let active_steam_id =
            resolve_active_local_steam_id(Some(steam_root.path().to_string_lossy().as_ref()));
        assert_eq!(active_steam_id.as_deref(), Some(expected_steam_id.as_str()));
    }

    #[test]
    fn resolve_active_local_steam_id_falls_back_to_userdata_when_loginusers_is_invalid() {
        let steam_root = tempdir().expect("create temp steam root");
        let account_id = 87_654_321_u64;
        let expected_steam_id = (STEAM_ID64_ACCOUNT_ID_BASE + account_id).to_string();
        fs::create_dir_all(steam_root.path().join("steamapps")).expect("create steamapps directory");

        fs::create_dir_all(steam_root.path().join("config")).expect("create config directory");
        fs::write(steam_root.path().join("config").join("loginusers.vdf"), "not-vdf")
            .expect("write invalid loginusers");

        fs::create_dir_all(
            steam_root
                .path()
                .join("userdata")
                .join(account_id.to_string())
                .join("config"),
        )
        .expect("create userdata config directory");
        fs::write(
            steam_root
                .path()
                .join("userdata")
                .join(account_id.to_string())
                .join("config")
                .join("localconfig.vdf"),
            "\"UserLocalConfigStore\" {}",
        )
        .expect("write localconfig");

        let active_steam_id =
            resolve_active_local_steam_id(Some(steam_root.path().to_string_lossy().as_ref()));
        assert_eq!(active_steam_id.as_deref(), Some(expected_steam_id.as_str()));
    }

    #[test]
    fn resolve_active_local_steam_id_prefers_loginusers_over_userdata_fallback() {
        let steam_root = tempdir().expect("create temp steam root");
        let loginusers_steam_id = "76561198000000055";
        let account_id = 99_888_777_u64;
        fs::create_dir_all(steam_root.path().join("steamapps")).expect("create steamapps directory");

        fs::create_dir_all(steam_root.path().join("config")).expect("create config directory");
        fs::write(
            steam_root.path().join("config").join("loginusers.vdf"),
            format!(
                "\"users\"\n{{\n    \"{loginusers_steam_id}\"\n    {{\n        \"MostRecent\"\t\"1\"\n    }}\n}}\n"
            ),
        )
        .expect("write loginusers");

        fs::create_dir_all(
            steam_root
                .path()
                .join("userdata")
                .join(account_id.to_string())
                .join("config"),
        )
        .expect("create userdata config directory");

        let active_steam_id =
            resolve_active_local_steam_id(Some(steam_root.path().to_string_lossy().as_ref()));
        assert_eq!(active_steam_id.as_deref(), Some(loginusers_steam_id));
    }

    #[test]
    fn collect_steam_app_history_entries_from_localconfig_reads_playtime_and_last_played() {
        let steam_root = tempdir().expect("create temp steam root");
        let steam_id = "76561198000000042";
        fs::create_dir_all(steam_root.path().join("steamapps")).expect("create steamapps directory");
        let localconfig_directory = steam_root
            .path()
            .join("userdata")
            .join(steam_id)
            .join("config");
        fs::create_dir_all(&localconfig_directory).expect("create localconfig directory");
        fs::write(
            localconfig_directory.join("localconfig.vdf"),
            r#"
            "UserLocalConfigStore"
            {
                "Software"
                {
                    "Valve"
                    {
                        "Steam"
                        {
                            "apps"
                            {
                                "7"
                                {
                                    "cloud"
                                    {
                                        "last_sync_state" "synchronized"
                                    }
                                }
                                "570"
                                {
                                    "Playtime" "120"
                                    "LastPlayed" "1710000000"
                                }
                                "730"
                                {
                                    "Playtime2wks" "45"
                                }
                                "invalid"
                                {
                                    "Playtime" "900"
                                }
                            }
                        }
                    }
                }
            }
            "#,
        )
        .expect("write localconfig");

        let entries = collect_steam_app_history_entries_from_localconfig(
            Some(steam_root.path().to_string_lossy().as_ref()),
            steam_id,
            false,
        )
        .expect("collect localconfig history");

        assert_eq!(entries.len(), 2);
        let app_570 = entries
            .iter()
            .find(|entry| entry.app_id == 570)
            .expect("570 should be present");
        assert_eq!(app_570.playtime_minutes, 120);
        assert!(app_570.last_played_at.is_some());

        let app_730 = entries
            .iter()
            .find(|entry| entry.app_id == 730)
            .expect("730 should be present");
        assert_eq!(app_730.playtime_minutes, 45);
    }

    #[test]
    fn collect_steam_app_history_entries_from_localconfig_can_include_empty_entries() {
        let steam_root = tempdir().expect("create temp steam root");
        let steam_id = "76561198000000042";
        fs::create_dir_all(steam_root.path().join("steamapps")).expect("create steamapps directory");
        let localconfig_directory = steam_root
            .path()
            .join("userdata")
            .join(steam_id)
            .join("config");
        fs::create_dir_all(&localconfig_directory).expect("create localconfig directory");
        fs::write(
            localconfig_directory.join("localconfig.vdf"),
            r#"
            "UserLocalConfigStore"
            {
                "Software"
                {
                    "Valve"
                    {
                        "Steam"
                        {
                            "apps"
                            {
                                "4520130"
                                {
                                    "cloud"
                                    {
                                        "last_sync_state" "synchronized"
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "#,
        )
        .expect("write localconfig");

        let entries = collect_steam_app_history_entries_from_localconfig(
            Some(steam_root.path().to_string_lossy().as_ref()),
            steam_id,
            true,
        )
        .expect("collect localconfig history including empty entries");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].app_id, 4_520_130);
        assert_eq!(entries[0].playtime_minutes, 0);
        assert!(entries[0].last_played_at.is_none());
    }

    #[test]
    fn collect_selected_steam_library_app_ids_from_localconfig_reads_current_selection() {
        let steam_root = tempdir().expect("create temp steam root");
        let steam_id = "76561198000000042";
        fs::create_dir_all(steam_root.path().join("steamapps")).expect("create steamapps directory");
        let localconfig_directory = steam_root
            .path()
            .join("userdata")
            .join(steam_id)
            .join("config");
        fs::create_dir_all(&localconfig_directory).expect("create localconfig directory");
        fs::write(
            localconfig_directory.join("localconfig.vdf"),
            r#"
            "UserLocalConfigStore"
            {
                "WebStorage"
                {
                    "UIStoreLocalSteamUIState" "{\"currentSelection\":{\"strCollectionId\":\"uncategorized\",\"nAppId\":4520130}}"
                }
            }
            "#,
        )
        .expect("write localconfig");

        let selected_app_ids = collect_selected_steam_library_app_ids_from_localconfig(
            Some(steam_root.path().to_string_lossy().as_ref()),
            steam_id,
        );
        assert!(selected_app_ids.contains(&4_520_130));
    }

    #[test]
    fn collect_signal_steam_library_app_ids_from_local_state_supports_string_app_ids() {
        let steam_root = tempdir().expect("create temp steam root");
        let steam_id = "76561198000000042";
        fs::create_dir_all(steam_root.path().join("steamapps")).expect("create steamapps directory");

        let localconfig_directory = steam_root
            .path()
            .join("userdata")
            .join(steam_id)
            .join("config");
        fs::create_dir_all(&localconfig_directory).expect("create localconfig directory");
        fs::write(
            localconfig_directory.join("localconfig.vdf"),
            r#"
            "UserLocalConfigStore"
            {
                "WebStorage"
                {
                    "UIStoreLocalSteamUIState" "{\"currentSelection\":{\"nAppId\":\"4520130\"}}"
                    "playnextstore_storage" "{\"cachedPlayNext\":{\"appids\":[\"620\",10]}}"
                    "user-collections.favorite" "{\"added\":[730,\"440\"]}"
                }
            }
            "#,
        )
        .expect("write localconfig");

        let signal_app_ids = collect_signal_steam_library_app_ids_from_local_state(
            Some(steam_root.path().to_string_lossy().as_ref()),
            steam_id,
        );
        assert!(signal_app_ids.contains(&4_520_130));
        assert!(signal_app_ids.contains(&620));
        assert!(signal_app_ids.contains(&10));
        assert!(signal_app_ids.contains(&730));
        assert!(signal_app_ids.contains(&440));
    }

    #[test]
    fn collect_selected_steam_library_app_ids_from_localconfig_uses_active_user_candidate() {
        let steam_root = tempdir().expect("create temp steam root");
        fs::create_dir_all(steam_root.path().join("steamapps")).expect("create steamapps directory");
        fs::create_dir_all(steam_root.path().join("config")).expect("create config directory");

        let linked_steam_id = "76561198000000042";
        let active_steam_id = "76561198000000099";
        fs::write(
            steam_root.path().join("config").join("loginusers.vdf"),
            format!(
                "\"users\"\n{{\n    \"{active_steam_id}\"\n    {{\n        \"MostRecent\"\t\"1\"\n    }}\n}}\n"
            ),
        )
        .expect("write loginusers");

        let linked_localconfig_directory = steam_root
            .path()
            .join("userdata")
            .join(linked_steam_id)
            .join("config");
        fs::create_dir_all(&linked_localconfig_directory).expect("create linked localconfig directory");
        fs::write(
            linked_localconfig_directory.join("localconfig.vdf"),
            r#"
            "UserLocalConfigStore"
            {
                "WebStorage"
                {
                    "UIStoreLocalSteamUIState" "{\"currentSelection\":{}}"
                }
            }
            "#,
        )
        .expect("write linked localconfig");

        let active_localconfig_directory = steam_root
            .path()
            .join("userdata")
            .join(active_steam_id)
            .join("config");
        fs::create_dir_all(&active_localconfig_directory).expect("create active localconfig directory");
        fs::write(
            active_localconfig_directory.join("localconfig.vdf"),
            r#"
            "UserLocalConfigStore"
            {
                "WebStorage"
                {
                    "UIStoreLocalSteamUIState" "{\"currentSelection\":{\"nAppId\":4520130}}"
                }
            }
            "#,
        )
        .expect("write active localconfig");

        let selected_app_ids = collect_selected_steam_library_app_ids_from_localconfig(
            Some(steam_root.path().to_string_lossy().as_ref()),
            linked_steam_id,
        );
        assert!(selected_app_ids.contains(&4_520_130));
    }

    #[test]
    fn collect_steam_app_history_entries_from_sharedconfig_can_include_empty_entries() {
        let steam_root = tempdir().expect("create temp steam root");
        let steam_id = "76561198000000042";
        fs::create_dir_all(steam_root.path().join("steamapps")).expect("create steamapps directory");
        let sharedconfig_directory = steam_root
            .path()
            .join("userdata")
            .join(steam_id)
            .join("7")
            .join("remote");
        fs::create_dir_all(&sharedconfig_directory).expect("create sharedconfig directory");
        fs::write(
            sharedconfig_directory.join("sharedconfig.vdf"),
            r#"
            "UserRoamingConfigStore"
            {
                "Software"
                {
                    "Valve"
                    {
                        "Steam"
                        {
                            "apps"
                            {
                                "4520130"
                                {
                                    "cloud"
                                    {
                                        "last_sync_state" "synchronized"
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "#,
        )
        .expect("write sharedconfig");

        let entries = collect_steam_app_history_entries_from_sharedconfig(
            Some(steam_root.path().to_string_lossy().as_ref()),
            steam_id,
            true,
        )
        .expect("collect sharedconfig history including empty entries");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].app_id, 4_520_130);
        assert_eq!(entries[0].playtime_minutes, 0);
        assert!(entries[0].last_played_at.is_none());
    }

    #[test]
    fn collect_signal_steam_library_app_ids_from_cloudstorage_reads_rollups_collections_and_selection() {
        let steam_root = tempdir().expect("create temp steam root");
        let steam_id = "76561198000000042";
        fs::create_dir_all(steam_root.path().join("steamapps")).expect("create steamapps directory");
        let cloudstorage_directory = steam_root
            .path()
            .join("userdata")
            .join(steam_id)
            .join("config")
            .join("cloudstorage");
        fs::create_dir_all(&cloudstorage_directory).expect("create cloudstorage directory");
        fs::write(
            cloudstorage_directory.join("cloud-storage-namespace-1.json"),
            r#"
            [
                ["NewContentRollup_570", {"key":"NewContentRollup_570","value":"{\"rtOld\":0,\"rtNew\":0,\"rtStart\":0}"}],
                ["user-collections.favorite", {"key":"user-collections.favorite","value":"{\"added\":[730,\"440\"],\"removed\":[]}"}],
                ["playnextstore_storage", {"key":"playnextstore_storage","value":"{\"cachedPlayNext\":{\"appids\":[\"620\",10]}}"}],
                ["UIStoreLocalSteamUIState", {"key":"UIStoreLocalSteamUIState","value":"{\"currentSelection\":{\"nAppId\":\"304930\"}}"}],
                ["GameReleased", {"key":"GameReleased","value":"{\"apps\":[\"550\",570]}"}]
            ]
            "#,
        )
        .expect("write cloudstorage namespace");

        let signal_app_ids = collect_signal_steam_library_app_ids_from_cloudstorage(
            Some(steam_root.path().to_string_lossy().as_ref()),
            steam_id,
        );
        assert!(signal_app_ids.contains(&570));
        assert!(signal_app_ids.contains(&730));
        assert!(signal_app_ids.contains(&440));
        assert!(signal_app_ids.contains(&620));
        assert!(signal_app_ids.contains(&10));
        assert!(signal_app_ids.contains(&304_930));
        assert!(signal_app_ids.contains(&550));
    }

    #[test]
    fn collect_locally_known_steam_games_from_localconfig_reuses_existing_name() {
        let db_directory = tempdir().expect("create temp database directory");
        let db_path = db_directory.path().join("catalyst.db");
        initialize_database(&db_path).expect("initialize db");
        let connection = open_connection(&db_path).expect("open db");
        let user = crate::infrastructure::runtime_auth::create_user(
            &connection,
            "fallback-test@example.com",
            "$2b$12$testhash",
            Some("76561198000000042"),
        )
        .expect("create user");
        connection
            .execute(
                "INSERT INTO games (user_id, provider, external_id, name, kind, playtime_minutes, installed, artwork_url, last_synced_at, last_played_at)
                 VALUES (?1, 'steam', '570', 'Dota 2', 'game', 0, 0, NULL, ?2, NULL)",
                params![user.id, Utc::now().to_rfc3339()],
            )
            .expect("insert existing game");

        let steam_root = tempdir().expect("create temp steam root");
        fs::create_dir_all(steam_root.path().join("steamapps")).expect("create steamapps directory");
        let localconfig_directory = steam_root
            .path()
            .join("userdata")
            .join("76561198000000042")
            .join("config");
        fs::create_dir_all(&localconfig_directory).expect("create localconfig directory");
        fs::write(
            localconfig_directory.join("localconfig.vdf"),
            r#"
            "UserLocalConfigStore"
            {
                "Software"
                {
                    "Valve"
                    {
                        "Steam"
                        {
                            "apps"
                            {
                                "570"
                                {
                                    "Playtime" "120"
                                }
                                "730"
                                {
                                    "Playtime" "45"
                                }
                            }
                        }
                    }
                }
            }
            "#,
        )
        .expect("write localconfig");

        let local_games = collect_locally_known_steam_games_from_localconfig(
            &connection,
            &user,
            Some(steam_root.path().to_string_lossy().as_ref()),
            false,
        )
        .expect("collect local fallback games");

        assert_eq!(local_games.len(), 2);
        let app_570 = local_games
            .iter()
            .find(|game| game.external_id == "570")
            .expect("570 should be present");
        assert_eq!(app_570.name, "Dota 2");
        let app_730 = local_games
            .iter()
            .find(|game| game.external_id == "730")
            .expect("730 should be present");
        assert!(app_730.name.starts_with("Steam App 730"));
    }

    #[test]
    fn collect_steam_app_ids_from_librarycache_reads_numeric_directories() {
        let steam_root = tempdir().expect("create temp steam root");
        fs::create_dir_all(steam_root.path().join("steamapps")).expect("create steamapps directory");
        let librarycache_directory = steam_root.path().join("appcache").join("librarycache");
        fs::create_dir_all(librarycache_directory.join("570"))
            .expect("create app 570 directory");
        fs::create_dir_all(librarycache_directory.join("730"))
            .expect("create app 730 directory");
        fs::create_dir_all(librarycache_directory.join("not-an-app"))
            .expect("create non-app directory");

        let app_ids = collect_steam_app_ids_from_librarycache(
            Some(steam_root.path().to_string_lossy().as_ref()),
        )
        .expect("collect librarycache app ids");

        assert_eq!(app_ids.len(), 2);
        assert!(app_ids.contains(&570));
        assert!(app_ids.contains(&730));
    }

    #[test]
    fn collect_locally_known_steam_games_from_librarycache_reuses_existing_name() {
        let db_directory = tempdir().expect("create temp database directory");
        let db_path = db_directory.path().join("catalyst.db");
        initialize_database(&db_path).expect("initialize db");
        let connection = open_connection(&db_path).expect("open db");
        let user = crate::infrastructure::runtime_auth::create_user(
            &connection,
            "librarycache-fallback-test@example.com",
            "$2b$12$testhash",
            Some("76561198000000042"),
        )
        .expect("create user");
        connection
            .execute(
                "INSERT INTO games (user_id, provider, external_id, name, kind, playtime_minutes, installed, artwork_url, last_synced_at, last_played_at)
                 VALUES (?1, 'steam', '570', 'Dota 2', 'game', 0, 0, NULL, ?2, NULL)",
                params![user.id, Utc::now().to_rfc3339()],
            )
            .expect("insert existing game");

        let steam_root = tempdir().expect("create temp steam root");
        fs::create_dir_all(steam_root.path().join("steamapps")).expect("create steamapps directory");
        let librarycache_directory = steam_root.path().join("appcache").join("librarycache");
        fs::create_dir_all(librarycache_directory.join("570"))
            .expect("create app 570 directory");
        fs::create_dir_all(librarycache_directory.join("730"))
            .expect("create app 730 directory");

        let local_games = collect_locally_known_steam_games_from_librarycache(
            &connection,
            &user,
            Some(steam_root.path().to_string_lossy().as_ref()),
        )
        .expect("collect local librarycache games");

        assert_eq!(local_games.len(), 2);
        let app_570 = local_games
            .iter()
            .find(|game| game.external_id == "570")
            .expect("570 should be present");
        assert_eq!(app_570.name, "Dota 2");
        let app_730 = local_games
            .iter()
            .find(|game| game.external_id == "730")
            .expect("730 should be present");
        assert!(app_730.name.starts_with("Steam App 730"));
    }

    #[test]
    fn collect_steam_manifest_names_reads_appmanifest_names() {
        let steam_root = tempdir().expect("create temp steam root");
        let steamapps_directory = steam_root.path().join("steamapps");
        fs::create_dir_all(&steamapps_directory).expect("create steamapps directory");
        fs::write(
            steamapps_directory.join("appmanifest_570.acf"),
            r#"
            "AppState"
            {
                "appid" "570"
                "name" "Dota 2"
            }
            "#,
        )
        .expect("write appmanifest");

        let names = collect_steam_manifest_names(Some(steam_root.path().to_string_lossy().as_ref()))
            .expect("collect manifest names");
        assert_eq!(names.get("570").map(String::as_str), Some("Dota 2"));
    }

    #[test]
    fn hydrate_local_steam_game_names_from_manifests_updates_placeholders_only() {
        let steam_root = tempdir().expect("create temp steam root");
        let steamapps_directory = steam_root.path().join("steamapps");
        fs::create_dir_all(&steamapps_directory).expect("create steamapps directory");
        fs::write(
            steamapps_directory.join("appmanifest_570.acf"),
            r#"
            "AppState"
            {
                "appid" "570"
                "name" "Dota 2"
            }
            "#,
        )
        .expect("write appmanifest");

        let mut games_by_external_id = HashMap::new();
        games_by_external_id.insert(
            String::from("570"),
            LibraryGameInput {
                external_id: String::from("570"),
                name: String::from("570"),
                kind: String::from("game"),
                playtime_minutes: 0,
                installed: false,
                artwork_url: None,
                last_synced_at: Utc::now().to_rfc3339(),
                last_played_at: None,
            },
        );
        games_by_external_id.insert(
            String::from("730"),
            LibraryGameInput {
                external_id: String::from("730"),
                name: String::from("Counter-Strike 2"),
                kind: String::from("game"),
                playtime_minutes: 0,
                installed: false,
                artwork_url: None,
                last_synced_at: Utc::now().to_rfc3339(),
                last_played_at: None,
            },
        );

        hydrate_local_steam_game_names_from_manifests(
            &mut games_by_external_id,
            Some(steam_root.path().to_string_lossy().as_ref()),
        )
        .expect("hydrate from manifests");

        assert_eq!(
            games_by_external_id
                .get("570")
                .map(|game| game.name.as_str()),
            Some("Dota 2")
        );
        assert_eq!(
            games_by_external_id
                .get("730")
                .map(|game| game.name.as_str()),
            Some("Counter-Strike 2")
        );
    }

    #[test]
    fn should_resolve_local_steam_game_name_treats_numeric_titles_as_placeholders() {
        let numeric_placeholder = LibraryGameInput {
            external_id: String::from("570"),
            name: String::from("570"),
            kind: String::from("unknown"),
            playtime_minutes: 0,
            installed: false,
            artwork_url: None,
            last_synced_at: Utc::now().to_rfc3339(),
            last_played_at: None,
        };
        let canonical_placeholder = LibraryGameInput {
            external_id: String::from("730"),
            name: String::from("Steam App 730"),
            kind: String::from("unknown"),
            playtime_minutes: 0,
            installed: false,
            artwork_url: None,
            last_synced_at: Utc::now().to_rfc3339(),
            last_played_at: None,
        };
        let real_name = LibraryGameInput {
            external_id: String::from("440"),
            name: String::from("Team Fortress 2"),
            kind: String::from("game"),
            playtime_minutes: 0,
            installed: false,
            artwork_url: None,
            last_synced_at: Utc::now().to_rfc3339(),
            last_played_at: None,
        };

        assert!(should_resolve_local_steam_game_name(&numeric_placeholder));
        assert!(should_resolve_local_steam_game_name(&canonical_placeholder));
        assert!(!should_resolve_local_steam_game_name(&real_name));
    }

    #[test]
    fn extract_steam_app_name_from_store_page_html_prefers_app_hub_name() {
        let html = r#"
            <html>
                <body>
                    <div id="appHubAppName">Counter-Strike 2 Demo</div>
                    <meta property="og:title" content="Wrong Name on Steam" />
                </body>
            </html>
        "#;

        let extracted = extract_steam_app_name_from_store_page_html(html);
        assert_eq!(extracted.as_deref(), Some("Counter-Strike 2 Demo"));
    }

    #[test]
    fn extract_steam_app_name_from_store_page_html_uses_og_title_fallback() {
        let html = r#"
            <html>
                <head>
                    <meta property="og:title" content="Half-Life Demo on Steam" />
                </head>
            </html>
        "#;

        let extracted = extract_steam_app_name_from_store_page_html(html);
        assert_eq!(extracted.as_deref(), Some("Half-Life Demo"));
    }

    #[test]
    fn extract_steam_app_name_from_local_appinfo_payload_returns_first_valid_title() {
        let mut payload = Vec::new();
        payload.extend_from_slice(b"common\0");
        payload.extend_from_slice(b"https://example.com\0");
        payload.extend_from_slice(b"2081110_eula_0\0");
        payload.extend_from_slice(b"Undecember Demo\0");
        payload.extend_from_slice(b"UndecemberDemo.exe\0");

        let extracted = extract_steam_app_name_from_local_appinfo_payload(&payload);
        assert_eq!(extracted.as_deref(), Some("Undecember Demo"));
    }

    #[test]
    fn extract_steam_app_name_from_local_appinfo_payload_rejects_non_titles() {
        let mut payload = Vec::new();
        payload.extend_from_slice(b"https://example.com\0");
        payload.extend_from_slice(b"e531df1abbd1e67792b5a9bad228801e2119b3da\0");
        payload.extend_from_slice(b"windows\0");
        payload.extend_from_slice(b"for qa\0");

        let extracted = extract_steam_app_name_from_local_appinfo_payload(&payload);
        assert!(extracted.is_none());
    }

    #[test]
    fn normalize_unresolved_steam_game_placeholder_name_rewrites_numeric_ids() {
        let mut unresolved = LibraryGameInput {
            external_id: String::from("3035190"),
            name: String::from("3035190"),
            kind: String::from("unknown"),
            playtime_minutes: 0,
            installed: false,
            artwork_url: None,
            last_synced_at: Utc::now().to_rfc3339(),
            last_played_at: None,
        };

        normalize_unresolved_steam_game_placeholder_name(&mut unresolved);
        assert_eq!(unresolved.name, "Steam App 3035190");
    }

    #[test]
    fn upsert_provider_games_does_not_prune_existing_rows() {
        let db_directory = tempdir().expect("create temp database directory");
        let db_path = db_directory.path().join("catalyst.db");
        initialize_database(&db_path).expect("initialize db");
        let connection = open_connection(&db_path).expect("open db");
        let user = crate::infrastructure::runtime_auth::create_user(
            &connection,
            "upsert-provider-games-test@example.com",
            "$2b$12$testhash",
            Some("76561198000000042"),
        )
        .expect("create user");

        let now = Utc::now().to_rfc3339();
        connection
            .execute(
                "INSERT INTO games (user_id, provider, external_id, name, kind, playtime_minutes, installed, artwork_url, last_synced_at, last_played_at)
                 VALUES (?1, 'steam', '570', 'Dota 2', 'game', 10, 0, NULL, ?2, NULL)",
                params![user.id, now.clone()],
            )
            .expect("insert app 570");
        connection
            .execute(
                "INSERT INTO games (user_id, provider, external_id, name, kind, playtime_minutes, installed, artwork_url, last_synced_at, last_played_at)
                 VALUES (?1, 'steam', '730', 'Counter-Strike 2', 'game', 20, 0, NULL, ?2, NULL)",
                params![user.id, now],
            )
            .expect("insert app 730");

        upsert_provider_games(
            &connection,
            &user.id,
            "steam",
            &[LibraryGameInput {
                external_id: String::from("570"),
                name: String::from("Dota 2"),
                kind: String::from("game"),
                playtime_minutes: 123,
                installed: true,
                artwork_url: None,
                last_synced_at: Utc::now().to_rfc3339(),
                last_played_at: None,
            }],
        )
        .expect("upsert provider games");

        let total_rows: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM games WHERE user_id = ?1 AND provider = 'steam'",
                params![user.id],
                |row| row.get(0),
            )
            .expect("count rows");
        assert_eq!(total_rows, 2);

        let app_730_exists: Option<String> = connection
            .query_row(
                "SELECT name FROM games WHERE user_id = ?1 AND provider = 'steam' AND external_id = '730'",
                params![user.id],
                |row| row.get(0),
            )
            .optional()
            .expect("query app 730");
        assert_eq!(app_730_exists.as_deref(), Some("Counter-Strike 2"));
    }

    #[test]
    fn prune_untrusted_placeholder_steam_games_removes_librarycache_like_overflow() {
        let db_directory = tempdir().expect("create temp database directory");
        let db_path = db_directory.path().join("catalyst.db");
        initialize_database(&db_path).expect("initialize db");
        let connection = open_connection(&db_path).expect("open db");
        let user = crate::infrastructure::runtime_auth::create_user(
            &connection,
            "prune-untrusted-placeholder-games-test@example.com",
            "$2b$12$testhash",
            Some("76561198000000042"),
        )
        .expect("create user");

        let now = Utc::now().to_rfc3339();
        for app_id in 1..=30_u64 {
            let name = format!("Steam App {}", 100_000 + app_id);
            connection
                .execute(
                    "INSERT INTO games (user_id, provider, external_id, name, kind, playtime_minutes, installed, artwork_url, last_synced_at, last_played_at)
                     VALUES (?1, 'steam', ?2, ?3, 'game', 0, 0, NULL, ?4, NULL)",
                    params![user.id, (100_000 + app_id).to_string(), name, now.clone()],
                )
                .expect("insert placeholder game");
        }
        connection
            .execute(
                "INSERT INTO games (user_id, provider, external_id, name, kind, playtime_minutes, installed, artwork_url, last_synced_at, last_played_at)
                 VALUES (?1, 'steam', '570', 'Dota 2', 'game', 120, 1, NULL, ?2, NULL)",
                params![user.id, now],
            )
            .expect("insert trusted game");

        let trusted = HashSet::from([String::from("570"), String::from("100001")]);
        let deleted_count =
            prune_untrusted_placeholder_steam_games(&connection, &user.id, &trusted)
                .expect("prune untrusted placeholders");
        assert_eq!(deleted_count, 29);

        let remaining_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM games WHERE user_id = ?1 AND provider = 'steam'",
                params![user.id],
                |row| row.get(0),
            )
            .expect("count remaining games");
        assert_eq!(remaining_count, 2);
    }

    #[test]
    fn resolve_steam_root_paths_normalizes_steamapps_override_path() {
        let steam_root = tempdir().expect("create temp steam root");
        let steamapps_directory = steam_root.path().join("steamapps");
        fs::create_dir_all(&steamapps_directory).expect("create steamapps directory");

        let override_path = steamapps_directory.to_string_lossy().to_string();
        let roots = resolve_steam_root_paths(Some(override_path.as_str()));
        assert_eq!(roots, vec![steam_root.path().to_path_buf()]);
    }

    #[test]
    fn resolve_steam_root_paths_normalizes_loginusers_override_path() {
        let steam_root = tempdir().expect("create temp steam root");
        fs::create_dir_all(steam_root.path().join("config")).expect("create config directory");
        fs::create_dir_all(steam_root.path().join("steamapps")).expect("create steamapps directory");
        fs::write(
            steam_root.path().join("config").join("loginusers.vdf"),
            "\"users\" {}",
        )
        .expect("write loginusers");

        let override_path = steam_root
            .path()
            .join("config")
            .join("loginusers.vdf")
            .to_string_lossy()
            .to_string();
        let roots = resolve_steam_root_paths(Some(override_path.as_str()));
        assert_eq!(roots, vec![steam_root.path().to_path_buf()]);
    }

    #[test]
    fn parse_steam_libraryfolder_paths_ignores_numeric_app_entries() {
        let contents = r#"
            "libraryfolders"
            {
                "0"
                {
                    "path"      "/mnt/games/SteamLibrary"
                    "apps"
                    {
                        "570" "1"
                        "730" "1"
                    }
                }
            }
        "#;

        let paths = parse_steam_libraryfolder_paths(contents).expect("parse libraryfolders");
        assert_eq!(paths, vec![PathBuf::from("/mnt/games/SteamLibrary")]);
    }
}

fn fetch_steam_store_user_tags(client: &Client, app_id: u64) -> Result<Vec<String>, String> {
    let mut request_url = Url::parse(&format!("{STEAM_STORE_APP_ENDPOINT}/{app_id}/"))
        .map_err(|error| format!("Failed to parse Steam Store endpoint: {error}"))?;
    request_url
        .query_pairs_mut()
        .append_pair("l", "english")
        .append_pair("cc", "us");

    let response = client
        .get(request_url)
        .send()
        .map_err(|error| format!("Steam Store tags request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam Store tags request failed with status {}",
            response.status()
        ));
    }

    let html = response
        .text()
        .map_err(|error| format!("Failed to decode Steam Store tags response: {error}"))?;
    Ok(parse_steam_store_user_tags_from_html(&html))
}

fn parse_steam_store_user_tags_from_html(html: &str) -> Vec<String> {
    let tag_regex = match Regex::new(
        r#"(?is)<a[^>]*\bclass\s*=\s*"[^"]*\bapp_tag\b[^"]*"[^>]*>(.*?)</a>"#,
    ) {
        Ok(regex) => regex,
        Err(_) => return Vec::new(),
    };
    let strip_markup_regex = Regex::new(r"(?is)<[^>]+>").ok();
    let mut tags = Vec::new();
    let mut seen = HashSet::new();

    for captures in tag_regex.captures_iter(html) {
        let Some(raw_text) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };

        let without_markup = if let Some(strip_regex) = strip_markup_regex.as_ref() {
            strip_regex.replace_all(raw_text, " ").into_owned()
        } else {
            raw_text.to_owned()
        };
        let decoded = decode_basic_html_entities(&without_markup);
        let compact = decoded.split_whitespace().collect::<Vec<_>>().join(" ");
        let normalized = compact.trim();
        if normalized.is_empty() || normalized == "+" {
            continue;
        }

        let dedupe_key = normalized.to_ascii_lowercase();
        if seen.insert(dedupe_key) {
            tags.push(normalized.to_owned());
        }
    }

    tags
}

fn normalize_steam_store_tags(raw_tags: &[String]) -> Vec<String> {
    let mut normalized_tags = Vec::new();
    let mut seen = HashSet::new();

    for tag in raw_tags {
        let normalized = tag.trim();
        if normalized.is_empty() || normalized == "+" {
            continue;
        }

        let dedupe_key = normalized.to_ascii_lowercase();
        if seen.insert(dedupe_key) {
            normalized_tags.push(normalized.to_owned());
        }
    }

    normalized_tags
}

fn map_steam_tags_to_genres(tags: &[String]) -> Vec<String> {
    use std::collections::HashSet;
    let mut genres: HashSet<String> = HashSet::new();

    for tag in tags {
        let key = tag.to_ascii_lowercase();

        if key.contains("action") {
            genres.insert(String::from("action"));
        }
        if key.contains("adventure") {
            genres.insert(String::from("adventure"));
        }
        if key.contains("casual") {
            genres.insert(String::from("casual"));
        }
        if key.contains("indie") {
            genres.insert(String::from("indie"));
        }
        if key.contains("massively multiplayer") || key.contains("mmorpg") || key == "mmo" {
            genres.insert(String::from("massively-multiplayer"));
        }
        if key.contains("racing") {
            genres.insert(String::from("racing"));
        }
        if key.contains("rpg") || key.contains("role-playing") {
            genres.insert(String::from("rpg"));
        }
        if key.contains("simulation") || key.contains("simulator") {
            genres.insert(String::from("simulation"));
        }
        if key.contains("sports") {
            genres.insert(String::from("sports"));
        }
        if key.contains("strategy") || key.contains("tactics") || key.contains("turn-based") || key.contains("real time strategy") || key.contains("real-time strategy") {
            genres.insert(String::from("strategy"));
        }
    }

    let mut result: Vec<String> = genres.into_iter().collect();
    result.sort();
    result
}

fn fetch_steam_supported_languages(
    connection: &Connection,
    client: &Client,
    app_id: u64,
) -> Result<Vec<String>, String> {
    // Check DB cache first
    let stale_before = Utc::now() - ChronoDuration::hours(STEAM_APP_DETAILS_CACHE_TTL_HOURS);
    if let Ok(Some(cached)) = find_cached_steam_app_details(connection, app_id, stale_before) {
        if let Some(data) = cached.get("data") {
            if let Some(raw_languages) = data.get("supported_languages").and_then(serde_json::Value::as_str) {
                return Ok(parse_steam_supported_languages(raw_languages));
            }
        }
    }

    // Fetch from store
    let mut request_url = Url::parse(STEAM_APP_DETAILS_ENDPOINT)
        .map_err(|error| format!("Failed to parse Steam app details endpoint: {error}"))?;
    request_url
        .query_pairs_mut()
        .append_pair("appids", &app_id.to_string())
        .append_pair("l", "english");

    let response = client
        .get(request_url)
        .send()
        .map_err(|error| format!("Steam app details request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam app details request failed with status {}",
            response.status()
        ));
    }

    let payload = response
        .json::<serde_json::Value>()
        .map_err(|error| format!("Failed to decode Steam app details response: {error}"))?;

    let key = app_id.to_string();
    let Some(entry) = payload.get(&key) else {
        return Ok(Vec::new());
    };
    let Some(true) = entry.get("success").and_then(serde_json::Value::as_bool) else {
        return Ok(Vec::new());
    };

    // Best-effort cache of the entry object
    let _ = cache_steam_app_details(connection, app_id, entry);

    let raw_languages = entry
        .get("data")
        .and_then(|value| value.get("supported_languages"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();

    Ok(parse_steam_supported_languages(raw_languages))
}

fn fetch_steam_install_size_estimate_from_store(
    connection: &Connection,
    client: &Client,
    app_id: u64,
) -> Result<Option<u64>, String> {
    // Check cached appdetails first
    let stale_before = Utc::now() - ChronoDuration::hours(STEAM_APP_DETAILS_CACHE_TTL_HOURS);
    if let Ok(Some(cached)) = find_cached_steam_app_details(connection, app_id, stale_before) {
        if let Some(data) = cached.get("data").and_then(|v| v.as_object()) {
            let mut max_size_bytes: Option<u64> = None;
            for requirements_field in ["pc_requirements", "mac_requirements", "linux_requirements"] {
                if let Some(requirements_value) = data.get(requirements_field) {
                    if let Some(size_bytes) = parse_steam_install_size_from_requirements_value(requirements_value) {
                        max_size_bytes = Some(match max_size_bytes {
                            Some(existing) => existing.max(size_bytes),
                            None => size_bytes,
                        });
                    }

                    // infer achievements count and cloud details from details payload when present
                    let mut inferred_achievements_count: Option<u64> = None;
                    if let Some(ach) = data.get("achievements") {
                        if let Some(total) = ach.get("total").and_then(serde_json::Value::as_u64) {
                            inferred_achievements_count = Some(total);
                        } else if let Some(arr) = ach.as_array() {
                            inferred_achievements_count = Some(arr.len() as u64);
                        }
                    }

                    let mut inferred_cloud_details: Option<String> = None;
                    let mut inferred_has_cloud = false;
                    if let Some(cloud) = data.get("cloud") {
                        inferred_has_cloud = cloud.get("enabled").and_then(serde_json::Value::as_bool).unwrap_or(true);
                        if let Some(note) = cloud.get("note").and_then(serde_json::Value::as_str) {
                            inferred_cloud_details = Some(note.to_owned());
                        } else if let Some(desc) = cloud.get("description").and_then(serde_json::Value::as_str) {
                            inferred_cloud_details = Some(desc.to_owned());
                        }
                    }

                    // also attempt to infer cloud support from depots/platforms (best-effort)
                    if inferred_cloud_details.is_none() {
                        if let Some(pc_req) = data.get("pc_requirements") {
                            if pc_req.is_object() {
                                inferred_cloud_details = Some(String::from("PC requirements available"));
                            }
                        }
                    }

                    // persist inferred features to features cache (best-effort)
                    // controller support not available in this scope; pass None
                    let _ = cache_steam_app_features(
                        connection,
                        app_id,
                        data.get("achievements").is_some(),
                        inferred_achievements_count,
                        inferred_has_cloud,
                        inferred_cloud_details.as_deref(),
                        None,
                    );
                }
            }
            return Ok(max_size_bytes);
        }
    }

    let mut request_url = Url::parse(STEAM_APP_DETAILS_ENDPOINT)
        .map_err(|error| format!("Failed to parse Steam app details endpoint: {error}"))?;
    request_url
        .query_pairs_mut()
        .append_pair("appids", &app_id.to_string())
        .append_pair("l", "english")
        .append_pair("cc", "us");

    let response = client
        .get(request_url)
        .send()
        .map_err(|error| format!("Steam app details request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam app details request failed with status {}",
            response.status()
        ));
    }

    let payload = response
        .json::<serde_json::Value>()
        .map_err(|error| format!("Failed to decode Steam app details response: {error}"))?;

    let app_id_key = app_id.to_string();
    let Some(entry) = payload.get(&app_id_key) else {
        return Ok(None);
    };
    let Some(true) = entry.get("success").and_then(serde_json::Value::as_bool) else {
        return Ok(None);
    };
    let Some(data) = entry.get("data").and_then(serde_json::Value::as_object) else {
        return Ok(None);
    };

    // Best-effort cache
    let _ = cache_steam_app_details(connection, app_id, entry);

    let mut max_size_bytes: Option<u64> = None;
    for requirements_field in ["pc_requirements", "mac_requirements", "linux_requirements"] {
        let Some(requirements_value) = data.get(requirements_field) else {
            continue;
        };
        if let Some(size_bytes) = parse_steam_install_size_from_requirements_value(requirements_value)
        {
            max_size_bytes = match max_size_bytes {
                Some(existing_max) => Some(existing_max.max(size_bytes)),
                None => Some(size_bytes),
            };
        }
    }

    Ok(max_size_bytes)
}

fn fetch_steam_app_linux_platform_support_from_store(
    connection: &Connection,
    client: &Client,
    app_id: u64,
) -> Result<Option<bool>, String> {
    // Consult cached appdetails first
    let stale_before = Utc::now() - ChronoDuration::hours(STEAM_APP_DETAILS_CACHE_TTL_HOURS);
    if let Ok(Some(cached)) = find_cached_steam_app_details(connection, app_id, stale_before) {
        if let Some(data) = cached.get("data") {
            if let Some(platforms) = data.get("platforms").and_then(serde_json::Value::as_object) {
                return Ok(platforms.get("linux").and_then(serde_json::Value::as_bool));
            }
        }
    }

    let mut request_url = Url::parse(STEAM_APP_DETAILS_ENDPOINT)
        .map_err(|error| format!("Failed to parse Steam app details endpoint: {error}"))?;
    request_url
        .query_pairs_mut()
        .append_pair("appids", &app_id.to_string())
        .append_pair("l", "english")
        .append_pair("cc", "us");

    let response = client
        .get(request_url)
        .send()
        .map_err(|error| format!("Steam app details request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam app details request failed with status {}",
            response.status()
        ));
    }

    let payload = response
        .json::<serde_json::Value>()
        .map_err(|error| format!("Failed to decode Steam app details response: {error}"))?;

    let app_id_key = app_id.to_string();
    let Some(entry) = payload.get(&app_id_key) else {
        return Ok(None);
    };
    let Some(true) = entry.get("success").and_then(serde_json::Value::as_bool) else {
        return Ok(None);
    };
    // Best-effort cache
    let _ = cache_steam_app_details(connection, app_id, entry);

    let Some(data) = entry.get("data").and_then(serde_json::Value::as_object) else {
        return Ok(None);
    };
    let Some(platforms) = data.get("platforms").and_then(serde_json::Value::as_object) else {
        return Ok(None);
    };

    Ok(platforms.get("linux").and_then(serde_json::Value::as_bool))
}

fn parse_steam_install_size_from_requirements_value(value: &serde_json::Value) -> Option<u64> {
    let mut candidate_texts = Vec::new();
    collect_steam_requirement_text_candidates(value, &mut candidate_texts);

    let mut max_size_bytes: Option<u64> = None;
    for candidate_text in &candidate_texts {
        if let Some(parsed_size) = parse_steam_install_size_from_requirement_text(candidate_text) {
            max_size_bytes = match max_size_bytes {
                Some(existing_max) => Some(existing_max.max(parsed_size)),
                None => Some(parsed_size),
            };
        }
    }

    max_size_bytes
}

fn collect_steam_requirement_text_candidates(value: &serde_json::Value, output: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) => {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                output.push(trimmed.to_owned());
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_steam_requirement_text_candidates(item, output);
            }
        }
        serde_json::Value::Object(object) => {
            for key in ["minimum", "recommended"] {
                if let Some(candidate) = object.get(key).and_then(serde_json::Value::as_str) {
                    let trimmed = candidate.trim();
                    if !trimmed.is_empty() {
                        output.push(trimmed.to_owned());
                    }
                }
            }

            for value in object.values() {
                if let Some(candidate) = value.as_str() {
                    let trimmed = candidate.trim();
                    if !trimmed.is_empty() {
                        output.push(trimmed.to_owned());
                    }
                }
            }
        }
        _ => {}
    }
}

fn parse_steam_install_size_from_requirement_text(raw_text: &str) -> Option<u64> {
    if raw_text.trim().is_empty() {
        return None;
    }

    let with_breaks_replaced = raw_text
        .replace("<br />", "\n")
        .replace("<br/>", "\n")
        .replace("<br>", "\n");
    let without_tags = match Regex::new(r"(?is)<[^>]+>") {
        Ok(tag_regex) => tag_regex.replace_all(&with_breaks_replaced, "").into_owned(),
        Err(_) => with_breaks_replaced,
    };
    let decoded = decode_basic_html_entities(&without_tags);
    let size_pattern = match Regex::new(r"(?i)([0-9]+(?:[.,][0-9]+)?)\s*(tb|gb|mb|kb)") {
        Ok(regex) => regex,
        Err(_) => return None,
    };

    let mut max_size_bytes: Option<u64> = None;
    for line in decoded.lines() {
        let normalized_line = line.trim();
        if normalized_line.is_empty() {
            continue;
        }

        let lowercased_line = normalized_line.to_ascii_lowercase();
        let looks_like_storage_requirement = lowercased_line.contains("storage")
            || lowercased_line.contains("disk space")
            || lowercased_line.contains("available space")
            || lowercased_line.contains("space required");
        if !looks_like_storage_requirement {
            continue;
        }

        for captures in size_pattern.captures_iter(normalized_line) {
            let Some(amount_raw) = captures.get(1).map(|value| value.as_str()) else {
                continue;
            };
            let Some(unit_raw) = captures.get(2).map(|value| value.as_str()) else {
                continue;
            };

            let normalized_amount = amount_raw.replace(',', ".");
            let Ok(amount) = normalized_amount.parse::<f64>() else {
                continue;
            };
            if !(amount.is_finite() && amount > 0.0) {
                continue;
            }

            let multiplier = match unit_raw.to_ascii_uppercase().as_str() {
                "TB" => 1024_f64 * 1024_f64 * 1024_f64 * 1024_f64,
                "GB" => 1024_f64 * 1024_f64 * 1024_f64,
                "MB" => 1024_f64 * 1024_f64,
                "KB" => 1024_f64,
                _ => continue,
            };
            let estimated_bytes = (amount * multiplier).round();
            if !(estimated_bytes.is_finite() && estimated_bytes > 0.0) {
                continue;
            }

            let estimated_bytes = estimated_bytes as u64;
            max_size_bytes = match max_size_bytes {
                Some(existing_max) => Some(existing_max.max(estimated_bytes)),
                None => Some(estimated_bytes),
            };
        }
    }

    max_size_bytes
}

fn default_game_version_beta_options() -> Vec<GameVersionBetaOptionResponse> {
    vec![GameVersionBetaOptionResponse {
        id: String::from("public"),
        name: String::from("Default Public Version"),
        description: String::from("Most common version of the game"),
        last_updated: String::from("Unavailable"),
        build_id: None,
        requires_access_code: false,
        is_default: true,
    }]
}

fn normalize_game_version_beta_options(
    options: &[GameVersionBetaOptionResponse],
) -> Vec<GameVersionBetaOptionResponse> {
    let mut normalized_options = Vec::new();
    let mut seen = HashSet::new();

    for option in options {
        let normalized_id = option.id.trim();
        if normalized_id.is_empty() {
            continue;
        }

        let dedupe_key = normalized_id.to_ascii_lowercase();
        if !seen.insert(dedupe_key) {
            continue;
        }

        let normalized_name = option.name.trim();
        let normalized_description = option.description.trim();
        let normalized_last_updated = option.last_updated.trim();
        let normalized_build_id = option
            .build_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let normalized_is_default = option.is_default || normalized_id.eq_ignore_ascii_case("public");

        normalized_options.push(GameVersionBetaOptionResponse {
            id: normalized_id.to_owned(),
            name: if normalized_name.is_empty() {
                normalized_id.to_owned()
            } else {
                normalized_name.to_owned()
            },
            description: if normalized_description.is_empty() {
                if normalized_is_default {
                    String::from("Most common version of the game")
                } else if option.requires_access_code {
                    String::from("Requires access code")
                } else {
                    String::from("No description available")
                }
            } else {
                normalized_description.to_owned()
            },
            last_updated: if normalized_last_updated.is_empty() {
                String::from("Unavailable")
            } else {
                normalized_last_updated.to_owned()
            },
            build_id: normalized_build_id,
            requires_access_code: option.requires_access_code,
            is_default: normalized_is_default,
        });
    }

    normalized_options.sort_by(|left, right| {
        if left.is_default != right.is_default {
            if left.is_default {
                return std::cmp::Ordering::Less;
            }
            return std::cmp::Ordering::Greater;
        }

        left.name
            .to_ascii_lowercase()
            .cmp(&right.name.to_ascii_lowercase())
    });

    normalized_options
}

fn normalize_backend_warning_message(message: &str) -> String {
    let compact = message
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if compact.is_empty() {
        return String::from("Could not load beta branch data from Steam.");
    }

    if compact.chars().count() <= 220 {
        return compact;
    }

    let mut shortened = compact.chars().take(217).collect::<String>();
    shortened.push_str("...");
    shortened
}

fn is_forbidden_http_error(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("status 403") || normalized.contains("forbidden")
}

fn fetch_steam_game_version_betas(
    client: &Client,
    app_id: u64,
    api_key: &str,
) -> Result<Vec<GameVersionBetaOptionResponse>, String> {
    let mut request_url = Url::parse(STEAM_APP_BETAS_ENDPOINT)
        .map_err(|error| format!("Failed to parse Steam beta endpoint: {error}"))?;
    request_url
        .query_pairs_mut()
        .append_pair("key", api_key)
        .append_pair("appid", &app_id.to_string());

    let response = client
        .get(request_url)
        .send()
        .map_err(|error| format!("Steam betas request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam betas request failed with status {}",
            response.status()
        ));
    }

    let payload = response
        .json::<serde_json::Value>()
        .map_err(|error| format!("Failed to decode Steam betas response: {error}"))?;

    Ok(parse_steam_game_version_betas_payload(&payload, app_id))
}

fn fetch_steam_game_version_betas_from_store(
    client: &Client,
    app_id: u64,
) -> Result<Vec<GameVersionBetaOptionResponse>, String> {
    let mut request_url = Url::parse(STEAM_APP_DETAILS_ENDPOINT)
        .map_err(|error| format!("Failed to parse Steam app details endpoint: {error}"))?;
    request_url
        .query_pairs_mut()
        .append_pair("appids", &app_id.to_string())
        .append_pair("l", "english");

    let response = client
        .get(request_url)
        .send()
        .map_err(|error| format!("Steam app details request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam app details request failed with status {}",
            response.status()
        ));
    }

    let payload = response
        .json::<serde_json::Value>()
        .map_err(|error| format!("Failed to decode Steam app details response: {error}"))?;

    Ok(parse_steam_game_version_betas_payload(&payload, app_id))
}

fn fetch_steam_beta_access_code_validation(
    client: &Client,
    app_id: u64,
    api_key: &str,
    access_code: &str,
) -> Result<GameBetaAccessCodeValidationResponse, String> {
    let mut request_url = Url::parse(STEAM_APP_BETA_CODE_CHECK_ENDPOINT)
        .map_err(|error| format!("Failed to parse Steam beta code check endpoint: {error}"))?;
    request_url
        .query_pairs_mut()
        .append_pair("key", api_key)
        .append_pair("appid", &app_id.to_string())
        .append_pair("betapassword", access_code);

    let response = client
        .get(request_url)
        .send()
        .map_err(|error| format!("Steam beta code check failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam beta code check failed with status {}",
            response.status()
        ));
    }

    let payload = response
        .json::<serde_json::Value>()
        .map_err(|error| format!("Failed to decode Steam beta code check response: {error}"))?;

    Ok(parse_steam_beta_access_code_validation_payload(&payload))
}

fn parse_steam_game_version_betas_payload(
    payload: &serde_json::Value,
    app_id: u64,
) -> Vec<GameVersionBetaOptionResponse> {
    let app_id_key = app_id.to_string();
    let maybe_branch_map = payload
        .get("response")
        .and_then(|response| response.get("betas"))
        .and_then(serde_json::Value::as_object)
        .or_else(|| payload.get("betas").and_then(serde_json::Value::as_object))
        .or_else(|| {
            payload
                .get(&app_id_key)
                .and_then(|entry| entry.get("data"))
                .and_then(|data| data.get("depots"))
                .and_then(|depots| depots.get("branches"))
                .and_then(serde_json::Value::as_object)
        })
        .or_else(|| {
            payload
                .get("data")
                .and_then(|data| data.get("depots"))
                .and_then(|depots| depots.get("branches"))
                .and_then(serde_json::Value::as_object)
        });

    let mut options = Vec::new();
    if let Some(branch_map) = maybe_branch_map {
        for (branch_id_raw, branch_data) in branch_map {
            let branch_id = branch_id_raw.trim();
            if branch_id.is_empty() {
                continue;
            }

            let Some(branch_object) = branch_data.as_object() else {
                continue;
            };

            let is_default = branch_id.eq_ignore_ascii_case("public");
            let requires_access_code = parse_json_bool(
                get_json_value_by_keys_case_insensitive(
                    branch_object,
                    &["pwdrequired", "password_required", "requires_password"],
                ),
            );
            let build_id = get_json_value_by_keys_case_insensitive(
                branch_object,
                &["buildid", "build_id", "build"],
            )
            .and_then(parse_json_text_value);
            let last_updated = format_steam_beta_last_updated(
                get_json_value_by_keys_case_insensitive(
                    branch_object,
                    &["timeupdated", "lastupdated", "updated_at", "last_update"],
                ),
            );
            let description = get_json_value_by_keys_case_insensitive(
                branch_object,
                &["description", "desc", "notes"],
            )
            .and_then(parse_json_text_value)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| {
                if is_default {
                    String::from("Most common version of the game")
                } else if requires_access_code {
                    String::from("Requires access code")
                } else {
                    String::from("No description available")
                }
            });

            options.push(GameVersionBetaOptionResponse {
                id: branch_id.to_owned(),
                name: if is_default {
                    String::from("Default Public Version")
                } else {
                    branch_id.to_owned()
                },
                description,
                last_updated,
                build_id,
                requires_access_code,
                is_default,
            });
        }
    }

    normalize_game_version_beta_options(&options)
}

fn parse_steam_beta_access_code_validation_payload(
    payload: &serde_json::Value,
) -> GameBetaAccessCodeValidationResponse {
    let response_object = payload
        .get("response")
        .and_then(serde_json::Value::as_object)
        .or_else(|| payload.as_object());

    let Some(response_object) = response_object else {
        return GameBetaAccessCodeValidationResponse {
            valid: false,
            message: String::from("Could not parse Steam beta code check response."),
            branch_id: None,
            branch_name: None,
        };
    };

    let branch_id = get_json_value_by_keys_case_insensitive(
        response_object,
        &["betaname", "beta_name", "branch", "branch_name"],
    )
    .and_then(parse_json_text_value)
    .map(|value| value.trim().to_owned())
    .filter(|value| !value.is_empty());

    let explicit_valid = parse_json_bool(get_json_value_by_keys_case_insensitive(
        response_object,
        &["result", "success", "valid", "is_valid", "matched"],
    ));
    let valid = explicit_valid || branch_id.is_some();

    if !valid {
        return GameBetaAccessCodeValidationResponse {
            valid: false,
            message: String::from("Code is invalid or no beta branch is associated with it."),
            branch_id: None,
            branch_name: None,
        };
    }

    let branch_name = branch_id.clone();
    GameBetaAccessCodeValidationResponse {
        valid: true,
        message: if let Some(branch) = branch_name.as_deref() {
            format!("Code accepted. Branch unlocked: {branch}.")
        } else {
            String::from("Code accepted.")
        },
        branch_id,
        branch_name,
    }
}

fn get_json_value_by_keys_case_insensitive<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<&'a serde_json::Value> {
    for key in keys {
        if let Some(value) = object.get(*key) {
            return Some(value);
        }
    }

    let normalized_keys = keys
        .iter()
        .map(|key| key.to_ascii_lowercase())
        .collect::<Vec<_>>();
    object.iter().find_map(|(key, value)| {
        let normalized_key = key.to_ascii_lowercase();
        if normalized_keys.iter().any(|candidate| candidate == &normalized_key) {
            Some(value)
        } else {
            None
        }
    })
}

fn parse_json_text_value(value: &serde_json::Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }

        return Some(trimmed.to_owned());
    }

    if let Some(number) = value.as_i64() {
        return Some(number.to_string());
    }

    if let Some(number) = value.as_u64() {
        return Some(number.to_string());
    }

    None
}

fn parse_json_bool(value: Option<&serde_json::Value>) -> bool {
    let Some(value) = value else {
        return false;
    };

    if let Some(as_bool) = value.as_bool() {
        return as_bool;
    }

    if let Some(as_number) = value.as_i64() {
        return as_number > 0;
    }

    if let Some(as_number) = value.as_u64() {
        return as_number > 0;
    }

    if let Some(as_text) = value.as_str() {
        let normalized = as_text.trim().to_ascii_lowercase();
        return normalized == "1" || normalized == "true" || normalized == "yes" || normalized == "ok";
    }

    false
}

fn format_steam_beta_last_updated(raw_value: Option<&serde_json::Value>) -> String {
    let Some(raw_value) = raw_value else {
        return String::from("Unavailable");
    };

    if let Some(timestamp) = raw_value.as_i64() {
        if let Some(parsed_timestamp) = Utc.timestamp_opt(timestamp, 0).single() {
            return parsed_timestamp.format("%b %d, %Y").to_string();
        }
    }

    if let Some(timestamp_text) = raw_value.as_str() {
        let trimmed = timestamp_text.trim();
        if trimmed.is_empty() {
            return String::from("Unavailable");
        }

        if let Ok(parsed_timestamp) = trimmed.parse::<i64>() {
            if let Some(utc_timestamp) = Utc.timestamp_opt(parsed_timestamp, 0).single() {
                return utc_timestamp.format("%b %d, %Y").to_string();
            }
        }

        if let Ok(parsed_timestamp) = chrono::DateTime::parse_from_rfc3339(trimmed) {
            return parsed_timestamp
                .with_timezone(&Utc)
                .format("%b %d, %Y")
                .to_string();
        }

        return trimmed.to_owned();
    }

    String::from("Unavailable")
}

fn find_cached_steam_app_betas(
    connection: &Connection,
    app_id: u64,
) -> Result<Option<(Vec<GameVersionBetaOptionResponse>, chrono::DateTime<Utc>)>, String> {
    let cached = connection
        .query_row(
            "SELECT betas_json, fetched_at FROM steam_app_betas WHERE app_id = ?1",
            params![app_id.to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| format!("Failed to query cached Steam app betas: {error}"))?;

    let Some((betas_json, fetched_at)) = cached else {
        return Ok(None);
    };

    let fetched_at = match chrono::DateTime::parse_from_rfc3339(&fetched_at) {
        Ok(timestamp) => timestamp.with_timezone(&Utc),
        Err(_) => return Ok(None),
    };
    let parsed_options = serde_json::from_str::<Vec<GameVersionBetaOptionResponse>>(&betas_json)
        .map_err(|error| format!("Failed to decode cached Steam app betas: {error}"))?;
    let normalized_options = normalize_game_version_beta_options(&parsed_options);

    Ok(Some((normalized_options, fetched_at)))
}

fn cache_steam_app_betas(
    connection: &Connection,
    app_id: u64,
    options: &[GameVersionBetaOptionResponse],
) -> Result<(), String> {
    let normalized_options = normalize_game_version_beta_options(options);
    let serialized_options = serde_json::to_string(&normalized_options)
        .map_err(|error| format!("Failed to encode Steam app betas cache entry: {error}"))?;

    connection
        .execute(
            "
            INSERT INTO steam_app_betas (app_id, betas_json, fetched_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(app_id) DO UPDATE SET
              betas_json = excluded.betas_json,
              fetched_at = excluded.fetched_at
            ",
            params![
                app_id.to_string(),
                serialized_options,
                Utc::now().to_rfc3339()
            ],
        )
        .map_err(|error| format!("Failed to cache Steam app betas: {error}"))?;

    Ok(())
}

fn find_cached_steam_app_languages(
    connection: &Connection,
    app_id: u64,
) -> Result<Option<(Vec<String>, chrono::DateTime<Utc>)>, String> {
    let cached = connection
        .query_row(
            "SELECT languages_json, fetched_at FROM steam_app_languages WHERE app_id = ?1",
            params![app_id.to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| format!("Failed to query cached Steam app languages: {error}"))?;

    let Some((languages_json, fetched_at)) = cached else {
        return Ok(None);
    };

    let fetched_at = match chrono::DateTime::parse_from_rfc3339(&fetched_at) {
        Ok(timestamp) => timestamp.with_timezone(&Utc),
        Err(_) => return Ok(None),
    };
    let parsed_languages = serde_json::from_str::<Vec<String>>(&languages_json)
        .map_err(|error| format!("Failed to decode cached Steam app languages: {error}"))?;
    let normalized_languages = normalize_language_list(&parsed_languages);

    Ok(Some((normalized_languages, fetched_at)))
}

fn cache_steam_app_languages(
    connection: &Connection,
    app_id: u64,
    languages: &[String],
) -> Result<(), String> {
    let normalized_languages = normalize_language_list(languages);
    let serialized_languages = serde_json::to_string(&normalized_languages)
        .map_err(|error| format!("Failed to encode Steam app languages cache entry: {error}"))?;

    connection
        .execute(
            "
            INSERT INTO steam_app_languages (app_id, languages_json, fetched_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(app_id) DO UPDATE SET
              languages_json = excluded.languages_json,
              fetched_at = excluded.fetched_at
            ",
            params![
                app_id.to_string(),
                serialized_languages,
                Utc::now().to_rfc3339()
            ],
        )
        .map_err(|error| format!("Failed to cache Steam app languages: {error}"))?;

    Ok(())
}

fn parse_steam_supported_languages(raw_value: &str) -> Vec<String> {
    if raw_value.trim().is_empty() {
        return Vec::new();
    }

    let with_breaks_replaced = raw_value
        .replace("<br />", ",")
        .replace("<br/>", ",")
        .replace("<br>", ",");
    let without_tags = match Regex::new(r"(?is)<[^>]+>") {
        Ok(tag_regex) => tag_regex.replace_all(&with_breaks_replaced, "").into_owned(),
        Err(_) => with_breaks_replaced,
    };
    let decoded = decode_basic_html_entities(&without_tags);

    let mut languages = Vec::new();
    let mut seen = HashSet::new();

    for token in decoded.split([',', ';', '\n']) {
        let compact = token
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .trim_matches(|character: char| {
                character == '*'
                    || character == ':'
                    || character == '.'
                    || character == '-'
                    || character == '('
                    || character == ')'
            })
            .trim()
            .to_owned();

        if compact.is_empty() {
            continue;
        }

        let normalized = compact.to_ascii_lowercase();
        if normalized.contains("full audio support")
            || normalized.contains("languages supported")
            || normalized == "supported languages"
            || normalized == "not supported"
            || normalized == "none"
        {
            continue;
        }

        if seen.insert(normalized) {
            languages.push(compact);
        }
    }

    normalize_language_list(&languages)
}

fn normalize_language_list(raw_languages: &[String]) -> Vec<String> {
    let mut normalized_languages = Vec::new();
    let mut seen = HashSet::new();

    for language in raw_languages {
        let trimmed = language.trim();
        if trimmed.is_empty() {
            continue;
        }

        let dedupe_key = trimmed.to_ascii_lowercase();
        if seen.insert(dedupe_key) {
            normalized_languages.push(trimmed.to_owned());
        }
    }

    normalized_languages
}

fn decode_basic_html_entities(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn normalize_steam_app_type(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn steam_kind_from_app_type(app_type: &str) -> &'static str {
    match normalize_steam_app_type(app_type).as_str() {
        "game" => "game",
        "demo" => "demo",
        "dlc" => "dlc",
        _ => "unknown",
    }
}

fn map_steam_game(
    game: SteamOwnedGame,
    resolved_kind: Option<&str>,
    installed: bool,
) -> LibraryGameInput {
    let external_id = game.appid.to_string();
    let normalized_name = game
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let name = normalized_name
        .map(str::to_owned)
        .unwrap_or_else(|| format!("Steam App {external_id}"));
    let fallback_kind = normalized_name
        .map(classify_steam_game_kind)
        .unwrap_or("unknown");
    let kind = resolved_kind
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "unknown")
        .unwrap_or(fallback_kind)
        .to_owned();
    let artwork_url = game
        .img_logo_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|logo_hash| {
            format!(
                "https://media.steampowered.com/steamcommunity/public/images/apps/{external_id}/{logo_hash}.jpg"
            )
        })
        .or_else(|| {
            game.img_icon_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|icon_hash| {
                    format!(
                        "https://media.steampowered.com/steamcommunity/public/images/apps/{external_id}/{icon_hash}.jpg"
                    )
                })
        });

    LibraryGameInput {
        external_id,
        name,
        kind,
        playtime_minutes: game.playtime_forever.unwrap_or(0),
        installed,
        artwork_url,
        last_synced_at: Utc::now().to_rfc3339(),
        last_played_at: match game.rtime_last_played {
            Some(secs) if secs > 0 => {
                match Utc.timestamp_opt(secs, 0).single() {
                    Some(dt) => Some(dt.to_rfc3339()),
                    None => None,
                }
            }
            _ => None,
        },
    }
}

fn classify_steam_game_kind(name: &str) -> &'static str {
    let normalized = name.to_ascii_lowercase();
    let contains_word = |needle: &str| {
        normalized
            .split(|character: char| !character.is_ascii_alphanumeric())
            .any(|token| token == needle)
    };

    if contains_word("demo") {
        return "demo";
    }

    if contains_word("dlc")
        || normalized.contains("season pass")
        || normalized.contains("expansion pass")
        || normalized.contains("add-on")
        || normalized.contains("add on")
        || normalized.contains("soundtrack")
    {
        return "dlc";
    }

    "game"
}

fn replace_provider_games(
    connection: &Connection,
    user_id: &str,
    provider: &str,
    games: &[LibraryGameInput],
) -> Result<(), String> {
    let incoming_external_ids = games
        .iter()
        .map(|game| game.external_id.clone())
        .collect::<HashSet<_>>();
    let mut existing_statement = connection
        .prepare("SELECT external_id FROM games WHERE user_id = ?1 AND provider = ?2")
        .map_err(|error| format!("Failed to prepare existing provider game query: {error}"))?;
    let existing_external_ids = existing_statement
        .query_map(params![user_id, provider], |row| row.get::<_, String>(0))
        .map_err(|error| format!("Failed to query existing provider games: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to decode existing provider games: {error}"))?;
    let mut delete = connection
        .prepare("DELETE FROM games WHERE user_id = ?1 AND provider = ?2 AND external_id = ?3")
        .map_err(|error| format!("Failed to prepare stale game cleanup statement: {error}"))?;
    for existing_external_id in existing_external_ids {
        if incoming_external_ids.contains(&existing_external_id) {
            continue;
        }

        delete
            .execute(params![user_id, provider, existing_external_id])
            .map_err(|error| format!("Failed to delete stale provider game: {error}"))?;
    }

    upsert_provider_games(connection, user_id, provider, games)
}

fn upsert_provider_games(
    connection: &Connection,
    user_id: &str,
    provider: &str,
    games: &[LibraryGameInput],
) -> Result<(), String> {
    let mut insert = connection
        .prepare(
            "
                        INSERT INTO games (user_id, provider, external_id, name, kind, playtime_minutes, installed, artwork_url, last_synced_at, last_played_at)
                        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                        ON CONFLICT(user_id, provider, external_id) DO UPDATE SET
                            name = excluded.name,
                            kind = excluded.kind,
                            playtime_minutes = excluded.playtime_minutes,
                            installed = excluded.installed,
                            artwork_url = excluded.artwork_url,
                            last_synced_at = excluded.last_synced_at,
                            last_played_at = excluded.last_played_at
            ",
        )
        .map_err(|error| format!("Failed to prepare game insert statement: {error}"))?;

    for game in games {
        insert
            .execute(params![
                user_id,
                provider,
                game.external_id,
                game.name,
                game.kind,
                game.playtime_minutes,
                if game.installed { 1 } else { 0 },
                game.artwork_url,
                game.last_synced_at,
                game.last_played_at
            ])
            .map_err(|error| format!("Failed to persist synced game: {error}"))?;
        // Persist derived genres for this game from cached Steam store tags (if any).
        // Delete existing genre rows for freshness, then insert new ones.
        let mut delete_stmt = connection
            .prepare(
                "DELETE FROM game_genres WHERE user_id = ?1 AND provider = ?2 AND external_id = ?3",
            )
            .map_err(|error| format!("Failed to prepare genre delete statement: {error}"))?;
        delete_stmt
            .execute(params![user_id, provider, game.external_id])
            .map_err(|error| format!("Failed to delete old genres: {error}"))?;

        // Look up cached Steam tags (if provider is steam) and map to genres.
        if provider.eq_ignore_ascii_case("steam") {
            let mut tags_stmt = connection
                .prepare("SELECT tags_json FROM steam_app_store_tags WHERE app_id = ?1")
                .map_err(|error| format!("Failed to prepare steam tags lookup: {error}"))?;
            let tag_row = tags_stmt
                .query_row(params![game.external_id], |row| row.get::<_, String>(0))
                .optional()
                .map_err(|error| format!("Failed to query steam tags: {error}"))?;
            if let Some(tags_json) = tag_row {
                let parsed_tags = serde_json::from_str::<Vec<String>>(&tags_json).unwrap_or_default();
                let normalized_tags = normalize_steam_store_tags(&parsed_tags);
                let mapped_genres = map_steam_tags_to_genres(&normalized_tags);
                if !mapped_genres.is_empty() {
                    let mut insert_genre = connection
                        .prepare(
                            "INSERT INTO game_genres (user_id, provider, external_id, genre) VALUES (?1, ?2, ?3, ?4)",
                        )
                        .map_err(|error| format!("Failed to prepare genre insert statement: {error}"))?;
                    for genre in mapped_genres {
                        insert_genre
                            .execute(params![user_id, provider, game.external_id, genre])
                            .map_err(|error| format!("Failed to persist genre: {error}"))?;
                    }
                }
            }
        }
    }

    Ok(())
}

fn prune_untrusted_placeholder_steam_games(
    connection: &Connection,
    user_id: &str,
    trusted_external_ids: &HashSet<String>,
) -> Result<usize, String> {
    if trusted_external_ids.is_empty() {
        return Ok(0);
    }

    let existing_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM games WHERE user_id = ?1 AND provider = 'steam'",
            params![user_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("Failed to count existing Steam games: {error}"))?;
    let trusted_count = trusted_external_ids.len() as i64;
    if existing_count <= (trusted_count * 2) {
        return Ok(0);
    }

    let mut statement = connection
        .prepare(
            "SELECT external_id, name, playtime_minutes, installed
             FROM games
             WHERE user_id = ?1 AND provider = 'steam'",
        )
        .map_err(|error| format!("Failed to prepare Steam cleanup query: {error}"))?;
    let rows = statement
        .query_map(params![user_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(|error| format!("Failed to query Steam cleanup candidates: {error}"))?;

    let mut delete = connection
        .prepare(
            "DELETE FROM games WHERE user_id = ?1 AND provider = 'steam' AND external_id = ?2",
        )
        .map_err(|error| format!("Failed to prepare Steam cleanup delete statement: {error}"))?;
    let mut deleted_count = 0usize;
    for row in rows {
        let (external_id, name, playtime_minutes, installed_raw) =
            row.map_err(|error| format!("Failed to decode Steam cleanup row: {error}"))?;
        if trusted_external_ids.contains(&external_id) {
            continue;
        }

        let installed = installed_raw > 0;
        if installed || playtime_minutes > 0 {
            continue;
        }
        if !is_steam_placeholder_game_name(&name, &external_id) {
            continue;
        }

        delete
            .execute(params![user_id, external_id])
            .map_err(|error| format!("Failed to delete untrusted placeholder Steam game: {error}"))?;
        deleted_count += 1;
    }

    Ok(deleted_count)
}

fn list_games_by_user(connection: &Connection, user_id: &str) -> Result<Vec<GameResponse>, String> {
    let collections_by_game = load_collection_names_by_game(connection, user_id)?;
    let steam_tags_by_game = load_steam_tags_by_game(connection, user_id)?;
    let game_genres_by_game = load_game_genres_by_game(connection, user_id)?;
    let mut statement = connection
        .prepare(
            "
            SELECT
              g.provider,
              g.external_id,
              g.name,
              g.kind,
              g.playtime_minutes,
              g.installed,
              g.artwork_url,
                            g.last_synced_at,
                            g.last_played_at,
                            EXISTS(
                SELECT 1
                FROM game_favorites favorite
                WHERE favorite.user_id = g.user_id
                  AND favorite.provider = g.provider
                  AND favorite.external_id = g.external_id
              ) AS favorite,
              COALESCE(privacy.hide_in_library, 0) AS hide_in_library
            FROM games g
            LEFT JOIN game_privacy_settings privacy
              ON privacy.user_id = g.user_id
              AND privacy.provider = g.provider
              AND privacy.external_id = g.external_id
            WHERE g.user_id = ?1
            ORDER BY g.name COLLATE NOCASE ASC
            ",
        )
        .map_err(|error| format!("Failed to prepare library query: {error}"))?;

    let rows = statement
        .query_map(params![user_id], |row| {
            let provider: String = row.get(0)?;
            let external_id: String = row.get(1)?;
            let installed_raw: i64 = row.get(5)?;
            let last_played: Option<String> = row.get(8)?;
            let favorite_raw: i64 = row.get(9)?;
            let hide_in_library_raw: i64 = row.get(10)?;
            let steam_tags = if provider.eq_ignore_ascii_case("steam") {
                steam_tags_by_game
                    .get(&external_id)
                    .cloned()
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            let game_key = game_membership_key(&provider, &external_id);
            let genres = game_genres_by_game
                .get(&game_key)
                .cloned()
                .unwrap_or_else(|| map_steam_tags_to_genres(&steam_tags));
            let collections = collections_by_game
                .get(&game_key)
                .cloned()
                .unwrap_or_default();
            Ok(GameResponse {
                id: format!("{provider}:{external_id}"),
                provider,
                external_id,
                name: row.get(2)?,
                kind: row.get(3)?,
                playtime_minutes: row.get(4)?,
                installed: installed_raw > 0,
                artwork_url: row.get(6)?,
                last_synced_at: row.get(7)?,
                last_played_at: last_played,
                favorite: favorite_raw > 0,
                steam_tags,
                genres,
                collections,
                hide_in_library: hide_in_library_raw > 0,
                developers: Vec::new(),
                publishers: Vec::new(),
                franchise: None,
                release_date: None,
                short_description: None,
                header_image: None,
                has_achievements: false,
                has_cloud_saves: false,
                controller_support: None,
                achievements_count: None,
                cloud_details: None,
                features: Vec::new(),
            })
        })
        .map_err(|error| format!("Failed to query library rows: {error}"))?;

    let mut games = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to decode library rows: {error}"))?;

    // Enrich steam games with cached Steam Store details (best-effort).
    // To avoid N+1 queries, prefetch cached details and features for all Steam app ids present in the library.
    let stale_before = Utc::now() - ChronoDuration::hours(STEAM_APP_DETAILS_CACHE_TTL_HOURS);
    let mut steam_app_ids: Vec<u64> = Vec::new();
    for g in games.iter() {
        if g.provider.eq_ignore_ascii_case("steam") {
            if let Ok(app_id) = g.external_id.parse::<u64>() {
                steam_app_ids.push(app_id);
            }
        }
    }

    use std::collections::HashMap as StdHashMap;
    let mut prefetched_details: StdHashMap<u64, serde_json::Value> = StdHashMap::new();
    let mut prefetched_features: StdHashMap<u64, (bool, Option<i64>, bool, Option<String>, Option<String>)> = StdHashMap::new();

    if !steam_app_ids.is_empty() {
        // Prefetch steam_app_details for these app ids in a single query (numeric literal list)
        let id_list = steam_app_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT app_id, details_json, fetched_at FROM steam_app_details WHERE app_id IN ({})",
            id_list
        );
        if let Ok(mut stmt) = connection.prepare(&sql) {
            if let Ok(rows) = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))) {
                for r in rows {
                    if let Ok((app_id_s, details_json, fetched_at)) = r {
                        if let Ok(app_id) = app_id_s.parse::<u64>() {
                            let is_fresh = chrono::DateTime::parse_from_rfc3339(&fetched_at)
                                .map(|timestamp| timestamp.with_timezone(&Utc) >= stale_before)
                                .unwrap_or(false);
                            if !is_fresh {
                                continue;
                            }
                            match serde_json::from_str::<serde_json::Value>(&details_json) {
                                Ok(parsed) => {
                                    prefetched_details.insert(app_id, parsed);
                                }
                                Err(err) => {
                                    eprintln!("Failed to parse cached steam_app_details for {}: {}", app_id, err);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Prefetch steam_app_features similarly
        let sqlf = format!(
            "SELECT app_id, has_achievements, achievements_count, has_cloud_saves, cloud_details, controller_support, fetched_at FROM steam_app_features WHERE app_id IN ({})",
            id_list
        );
        if let Ok(mut stmt) = connection.prepare(&sqlf) {
            if let Ok(rows) = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, Option<String>>(2)?, row.get::<_, i64>(3)?, row.get::<_, Option<String>>(4)?, row.get::<_, Option<String>>(5)?, row.get::<_, String>(6)?))) {
                for r in rows {
                    if let Ok((app_id_s, has_ach_raw, ach_count_opt_s, has_cloud_raw, cloud_details_opt, controller_opt, fetched_at)) = r {
                        if let Ok(app_id) = app_id_s.parse::<u64>() {
                            let is_fresh = chrono::DateTime::parse_from_rfc3339(&fetched_at)
                                .map(|timestamp| timestamp.with_timezone(&Utc) >= stale_before)
                                .unwrap_or(false);
                            if !is_fresh {
                                continue;
                            }
                            let achievements_count = ach_count_opt_s.and_then(|s| s.parse::<i64>().ok());
                            prefetched_features.insert(app_id, (has_ach_raw > 0, achievements_count, has_cloud_raw > 0, cloud_details_opt, controller_opt));
                        }
                    }
                }
            }
        }
    }

    // Apply prefetched data to games
    for game in games.iter_mut() {
        if !game.provider.eq_ignore_ascii_case("steam") {
            continue;
        }
        if let Ok(app_id) = game.external_id.parse::<u64>() {
                let mut maybe_data: Option<serde_json::Value> = None;
                if let Some(cached) = prefetched_details.get(&app_id) {
                    if let Some(data) = cached.get("data") {
                        maybe_data = Some(data.clone());
                    if let Some(devs) = data.get("developers").and_then(|v| v.as_array()) {
                        game.developers = devs
                            .iter()
                            .filter_map(|s| s.as_str().map(|s| s.to_string()))
                            .collect();
                    }
                    if let Some(pubs) = data.get("publishers").and_then(|v| v.as_array()) {
                        game.publishers = pubs
                            .iter()
                            .filter_map(|s| s.as_str().map(|s| s.to_string()))
                            .collect();
                    }
                    // franchise: prefer `franchise`, fall back to `series` array joined
                    game.franchise = data
                        .get("franchise")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .or_else(|| {
                            data.get("series").and_then(|v| v.as_array()).map(|arr| {
                                arr.iter()
                                    .filter_map(|s| s.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                        });

                    // release_date: try nested `release_date.date`, then plain string fallback
                    game.release_date = data
                        .get("release_date")
                        .and_then(|v| v.get("date"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .or_else(|| data.get("release_date").and_then(|v| v.as_str()).map(|s| s.to_string()));
                    game.short_description = data.get("short_description").and_then(|v| v.as_str()).map(|s| s.to_string());
                    game.header_image = data.get("header_image").and_then(|v| v.as_str()).map(|s| s.to_string());
                }
            }

            if let Some((has_ach, ach_count_opt, has_cloud, cloud_details_opt, controller_opt)) = prefetched_features.get(&app_id) {
                game.has_achievements = *has_ach;
                game.achievements_count = *ach_count_opt;
                game.has_cloud_saves = *has_cloud;
                game.cloud_details = cloud_details_opt.clone();
                game.controller_support = controller_opt.clone();
            }

            // Build normalized features for the game based on cached details and features
            let mut features: Vec<FeatureResponse> = Vec::new();
            if let Some(data) = maybe_data {
                if let Some(categories) = data.get("categories").and_then(serde_json::Value::as_array) {
                    let mut seen_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
                    // helper to canonicalize description to a preferred feature key/label
                    let canonical_from_desc = |desc: &str| -> Option<(String, String)> {
                        let lowered = desc.to_ascii_lowercase();
                        if lowered.contains("remote play together") || lowered.contains("remote play") {
                            // prefer showing Family Sharing instead of Remote Play Together per UX preference
                            return Some(("family-sharing".to_string(), "Family Sharing".to_string()));
                        }
                        if lowered.contains("steam cloud") || lowered.contains("steam cloud saves") || lowered.contains("cloud saves") || lowered == "cloud" {
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
                            return Some(("controller-full".to_string(), "Full Controller Support".to_string()));
                        }
                        if lowered.contains("partial controller") {
                            return Some(("controller-partial".to_string(), "Partial Controller Support".to_string()));
                        }
                        if lowered.contains("workshop") {
                            return Some(("workshop".to_string(), "Steam Workshop".to_string()));
                        }
                        if lowered.contains("family sharing") || lowered.contains("family-share") || lowered.contains("family_share") {
                            return Some(("family-sharing".to_string(), "Family Sharing".to_string()));
                        }
                        if lowered.contains("trading card") || lowered.contains("trading cards") {
                            return Some(("__skip__".to_string(), "".to_string()));
                        }
                        None
                    };
                    for cat in categories {
                        let id_opt = cat.get("id").and_then(|v| v.as_u64());
                        let desc_opt = cat.get("description").and_then(serde_json::Value::as_str).map(|s| s.to_string());
                        if let Some(desc) = desc_opt.as_deref() {
                            if let Some((key, label)) = canonical_from_desc(desc) {
                                if seen_keys.insert(key.clone()) {
                                    features.push(FeatureResponse { key: key.clone(), label: label.clone(), icon: None, tooltip: None });
                                }
                                // don't also add generic category-<id> when a canonical mapping applies
                                continue;
                            }
                        }
                        // no canonical mapping: include category id-based feature so raw ids are available in UI
                        let label = desc_opt.clone().or_else(|| id_opt.map(|id| format!("Category {}", id))).unwrap_or_else(|| "Category".to_string());
                        let key = if let Some(id) = id_opt { format!("category-{}", id) } else { label.to_ascii_lowercase().replace(' ', "-") };
                        if seen_keys.insert(key.clone()) {
                            features.push(FeatureResponse { key: key.clone(), label: label.clone(), icon: None, tooltip: None });
                        }
                    }
                }

                // Controller-specific strings (DualShock / DualSense) and workshop/family sharing may appear anywhere in the store data
                let as_string = data.to_string().to_ascii_lowercase();
                if as_string.contains("dualshock") {
                    features.push(FeatureResponse { key: "controller-dualshock".to_string(), label: "DualShock Support".to_string(), icon: Some("dualshock".to_string()), tooltip: None });
                }
                if as_string.contains("dualsense") {
                    features.push(FeatureResponse { key: "controller-dualsense".to_string(), label: "DualSense Support".to_string(), icon: Some("dualsense".to_string()), tooltip: None });
                }
                // Steam Workshop
                if as_string.contains("workshop") || as_string.contains("steam workshop") {
                    if !as_string.contains("trading card") && !as_string.contains("trading cards") {
                        features.push(FeatureResponse { key: "workshop".to_string(), label: "Steam Workshop".to_string(), icon: Some("workshop".to_string()), tooltip: None });
                    }
                }
                // Family Sharing eligibility
                if as_string.contains("family sharing") || as_string.contains("family-share") || as_string.contains("family_share") {
                    if !as_string.contains("trading card") && !as_string.contains("trading cards") {
                        features.push(FeatureResponse { key: "family-sharing".to_string(), label: "Family Sharing".to_string(), icon: Some("family".to_string()), tooltip: None });
                    }
                }
            }

            if game.has_achievements {
                let tooltip = game.achievements_count.map(|c| format!("{} achievements", c));
                features.push(FeatureResponse { key: "achievements".to_string(), label: "Achievements".to_string(), icon: Some("trophy".to_string()), tooltip });
            }
            if game.has_cloud_saves {
                features.push(FeatureResponse { key: "cloud-saves".to_string(), label: "Cloud Saves".to_string(), icon: Some("cloud".to_string()), tooltip: game.cloud_details.clone() });
            }
            if let Some(ref ctrl) = game.controller_support {
                features.push(FeatureResponse { key: "controller-support".to_string(), label: format!("Controller: {}", ctrl), icon: Some("gamepad".to_string()), tooltip: None });
            }

            if !features.is_empty() {
                game.features = features;
            }
        }
    }

    Ok(games)
}

fn game_membership_key(provider: &str, external_id: &str) -> String {
    format!(
        "{}:{}",
        provider.trim().to_ascii_lowercase(),
        external_id.trim()
    )
}

fn load_collection_names_by_game(
    connection: &Connection,
    user_id: &str,
) -> Result<HashMap<String, Vec<String>>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              membership.provider,
              membership.external_id,
              c.name
            FROM collection_games membership
            JOIN collections c
              ON c.id = membership.collection_id
             AND c.user_id = membership.user_id
            WHERE membership.user_id = ?1
            ORDER BY c.name COLLATE NOCASE ASC
            ",
        )
        .map_err(|error| format!("Failed to prepare collection membership query: {error}"))?;

    let rows = statement
        .query_map(params![user_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| format!("Failed to query collection memberships: {error}"))?;

    let mut collections_by_game: HashMap<String, Vec<String>> = HashMap::new();
    let mut seen_names_by_game: HashMap<String, HashSet<String>> = HashMap::new();

    for row in rows {
        let (provider, external_id, raw_collection_name) = row
            .map_err(|error| format!("Failed to decode collection membership row: {error}"))?;
        let collection_name = raw_collection_name.trim();
        if collection_name.is_empty() {
            continue;
        }

        let key = game_membership_key(&provider, &external_id);
        let dedupe_key = collection_name.to_ascii_lowercase();
        let seen_names = seen_names_by_game
            .entry(key.clone())
            .or_insert_with(HashSet::new);
        if !seen_names.insert(dedupe_key) {
            continue;
        }

        collections_by_game
            .entry(key)
            .or_insert_with(Vec::new)
            .push(collection_name.to_owned());
    }

    Ok(collections_by_game)
}

fn load_steam_tags_by_game(
    connection: &Connection,
    user_id: &str,
) -> Result<HashMap<String, Vec<String>>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              g.external_id,
              t.tags_json
            FROM games g
            LEFT JOIN steam_app_store_tags t
              ON t.app_id = g.external_id
            WHERE g.user_id = ?1
              AND g.provider = 'steam'
            ",
        )
        .map_err(|error| format!("Failed to prepare Steam Store tag query: {error}"))?;

    let rows = statement
        .query_map(params![user_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .map_err(|error| format!("Failed to query Steam Store tags: {error}"))?;

    let mut steam_tags_by_game: HashMap<String, Vec<String>> = HashMap::new();

    for row in rows {
        let (external_id, tags_json) =
            row.map_err(|error| format!("Failed to decode Steam Store tag row: {error}"))?;
        let Some(tags_json) = tags_json else {
            continue;
        };
        let parsed_tags = serde_json::from_str::<Vec<String>>(&tags_json).unwrap_or_default();
        let normalized_tags = normalize_steam_store_tags(&parsed_tags);
        if normalized_tags.is_empty() {
            continue;
        }

        steam_tags_by_game.insert(external_id, normalized_tags);
    }

    Ok(steam_tags_by_game)
}

fn load_game_genres_by_game(
    connection: &Connection,
    user_id: &str,
) -> Result<HashMap<String, Vec<String>>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              provider,
              external_id,
              genre
            FROM game_genres
            WHERE user_id = ?1
            ORDER BY genre ASC
            ",
        )
        .map_err(|error| format!("Failed to prepare game genres query: {error}"))?;

    let rows = statement
        .query_map(params![user_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })
        .map_err(|error| format!("Failed to query game genres: {error}"))?;

    let mut genres_by_game: HashMap<String, Vec<String>> = HashMap::new();

    for row in rows {
        let (provider, external_id, genre) = row
            .map_err(|error| format!("Failed to decode game genres row: {error}"))?;
        let key = game_membership_key(&provider, &external_id);
        genres_by_game.entry(key).or_insert_with(Vec::new).push(genre);
    }

    Ok(genres_by_game)
}

fn normalize_game_identity_input(
    provider: &str,
    external_id: &str,
) -> crate::application::error::AppResult<(String, String)> {
    Ok(crate::domain::game::GameIdentity::parse(provider, external_id)?.into_parts())
}

fn ensure_owned_game_exists(
    connection: &Connection,
    user_id: &str,
    provider: &str,
    external_id: &str,
) -> Result<(), String> {
    let exists = connection
        .query_row(
            "SELECT 1 FROM games WHERE user_id = ?1 AND provider = ?2 AND external_id = ?3",
            params![user_id, provider, external_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| format!("Failed to validate game ownership: {error}"))?;

    if exists.is_none() {
        return Err(String::from("Game not found for current user"));
    }

    Ok(())
}

fn upsert_game_favorite(
    connection: &Connection,
    user_id: &str,
    provider: &str,
    external_id: &str,
) -> Result<(), String> {
    connection
        .execute(
            "
            INSERT INTO game_favorites (user_id, provider, external_id, created_at)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(user_id, provider, external_id) DO NOTHING
            ",
            params![user_id, provider, external_id, Utc::now().to_rfc3339()],
        )
        .map_err(|error| format!("Failed to update game favorite: {error}"))?;

    Ok(())
}

fn remove_game_favorite(
    connection: &Connection,
    user_id: &str,
    provider: &str,
    external_id: &str,
) -> Result<(), String> {
    connection
        .execute(
            "DELETE FROM game_favorites WHERE user_id = ?1 AND provider = ?2 AND external_id = ?3",
            params![user_id, provider, external_id],
        )
        .map_err(|error| format!("Failed to remove game favorite: {error}"))?;
    Ok(())
}

fn load_owned_steam_games_by_app_id(
    connection: &Connection,
    user_id: &str,
) -> Result<HashMap<u64, OwnedSteamGameMetadata>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT external_id, name
            FROM games
            WHERE user_id = ?1 AND provider = 'steam'
            ",
        )
        .map_err(|error| format!("Failed to prepare owned Steam game query: {error}"))?;
    let rows = statement
        .query_map(params![user_id], |row| {
            let external_id = row.get::<_, String>(0)?;
            Ok(OwnedSteamGameMetadata {
                game_id: format!("steam:{external_id}"),
                external_id,
                name: row.get::<_, String>(1)?,
            })
        })
        .map_err(|error| format!("Failed to query owned Steam games: {error}"))?;

    let mut games_by_app_id = HashMap::new();
    for row in rows {
        let game = row.map_err(|error| format!("Failed to decode owned Steam game row: {error}"))?;
        let Some(app_id) = game.external_id.parse::<u64>().ok() else {
            continue;
        };
        games_by_app_id.insert(app_id, game);
    }

    Ok(games_by_app_id)
}

fn default_game_customization_settings_payload() -> GameCustomizationSettingsPayload {
    GameCustomizationSettingsPayload {
        custom_sort_name: String::new(),
    }
}

fn default_game_properties_settings_payload() -> GamePropertiesSettingsPayload {
    GamePropertiesSettingsPayload {
        general: GameGeneralSettingsPayload {
            language: String::from("English"),
            launch_options: String::new(),
            steam_overlay_enabled: true,
        },
        compatibility: GameCompatibilitySettingsPayload {
            force_steam_play_compatibility_tool: false,
            steam_play_compatibility_tool: String::from("Proton Experimental"),
        },
        updates: GameUpdatesSettingsPayload {
            automatic_updates_mode: String::from("use-global-setting"),
            background_downloads_mode: String::from("pause-while-playing-global"),
        },
        controller: GameControllerSettingsPayload {
            steam_input_override: String::from("use-default-settings"),
        },
        customization: default_game_customization_settings_payload(),
        game_versions_betas: GameVersionsBetasSettingsPayload {
            private_access_code: String::new(),
            selected_version_id: String::from("public"),
        },
    }
}

fn normalize_game_properties_settings_payload(
    settings: GamePropertiesSettingsPayload,
) -> GamePropertiesSettingsPayload {
    crate::infrastructure::runtime_steam_settings::normalize_game_properties_settings_payload(
        settings,
    )
}

fn load_game_properties_settings(
    connection: &Connection,
    user_id: &str,
    provider: &str,
    external_id: &str,
) -> Result<GamePropertiesSettingsPayload, String> {
    crate::infrastructure::runtime_steam_settings::load_game_properties_settings(
        connection,
        user_id,
        provider,
        external_id,
    )
}

fn save_game_properties_settings(
    connection: &Connection,
    user_id: &str,
    provider: &str,
    external_id: &str,
    settings: &GamePropertiesSettingsPayload,
) -> Result<(), String> {
    crate::infrastructure::runtime_steam_settings::save_game_properties_settings(
        connection,
        user_id,
        provider,
        external_id,
        settings,
    )
}

fn resolve_steam_compatibility_tools(
    steam_root_override: Option<&str>,
    include_linux_runtime_tools: bool,
) -> Result<Vec<GameCompatibilityToolResponse>, String> {
    crate::infrastructure::runtime_steam_settings::resolve_steam_compatibility_tools(
        steam_root_override,
        include_linux_runtime_tools,
    )
}

fn clear_steam_game_overlay_data(state: &AppState, user: &UserRow, app_id: u64) -> Result<(), String> {
    crate::infrastructure::runtime_steam_settings::clear_steam_game_overlay_data(state, user, app_id)
}

fn apply_steam_game_privacy_settings(
    state: &AppState,
    user: &UserRow,
    app_id: u64,
    settings: &GamePrivacySettingsResponse,
) -> Result<(), String> {
    crate::infrastructure::runtime_steam_settings::apply_steam_game_privacy_settings(
        state,
        user,
        app_id,
        settings,
    )
}

fn apply_steam_game_properties_settings(
    state: &AppState,
    user: &UserRow,
    app_id: u64,
    settings: &GamePropertiesSettingsPayload,
) -> Result<(), String> {
    crate::infrastructure::runtime_steam_settings::apply_steam_game_properties_settings(
        state,
        user,
        app_id,
        settings,
    )
}

fn load_game_privacy_settings(
    connection: &Connection,
    user_id: &str,
    provider: &str,
    external_id: &str,
) -> Result<GamePrivacySettingsResponse, String> {
    crate::infrastructure::runtime_steam_settings::load_game_privacy_settings(
        connection,
        user_id,
        provider,
        external_id,
    )
}

fn save_game_privacy_settings(
    connection: &Connection,
    user_id: &str,
    provider: &str,
    external_id: &str,
    settings: GamePrivacySettingsResponse,
) -> Result<(), String> {
    crate::infrastructure::runtime_steam_settings::save_game_privacy_settings(
        connection,
        user_id,
        provider,
        external_id,
        settings,
    )
}
fn get_authenticated_user(state: &AppState, connection: &Connection) -> Result<UserRow, String> {
    crate::infrastructure::runtime_auth::get_authenticated_user(state, connection)
}

// `find_auth_user_by_email` removed: local credential flows were deleted.

fn find_user_by_session_token(
    connection: &Connection,
    session_token: &str,
) -> Result<Option<UserRow>, String> {
    crate::infrastructure::runtime_auth::find_user_by_session_token(connection, session_token)
}

fn invalidate_session_by_token(connection: &Connection, session_token: &str) -> Result<(), String> {
    crate::infrastructure::runtime_auth::invalidate_session_by_token(connection, session_token)
}

fn cleanup_expired_sessions(connection: &Connection) -> Result<(), String> {
    crate::infrastructure::runtime_auth::cleanup_expired_sessions(connection)
}

fn get_state_session_token(state: &AppState) -> Result<Option<String>, String> {
    crate::infrastructure::runtime_session_state::get_state_session_token(state)
}

fn persist_active_session(state: &AppState, session_token: &str) -> Result<(), String> {
    crate::infrastructure::runtime_session_state::persist_active_session(state, session_token)
}

fn clear_active_session(state: &AppState) -> Result<(), String> {
    crate::infrastructure::runtime_session_state::clear_active_session(state)
}

fn restore_persisted_session(state: &AppState) -> Result<(), String> {
    crate::infrastructure::runtime_auth::restore_persisted_session(state)
}

fn bootstrap_local_session(state: &AppState) -> Result<Option<UserRow>, String> {
    crate::infrastructure::runtime_auth::bootstrap_local_session(state)
}

fn build_http_client() -> Result<Client, String> {
    crate::infrastructure::runtime_http::build_http_client()
}

// Email/password validation helpers removed as local auth is no longer supported.

fn public_user_from_row(user: &UserRow) -> PublicUser {
    PublicUser {
        id: user.id.clone(),
        email: user.email.clone(),
        steam_linked: user.steam_id.is_some(),
        steam_id: user.steam_id.clone(),
    }
}

fn open_connection(db_path: &Path) -> Result<Connection, String> {
    crate::infrastructure::runtime_database::open_connection(db_path)
}

fn initialize_database(db_path: &Path) -> Result<(), String> {
    crate::infrastructure::runtime_database::initialize_database(db_path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            application::bootstrap::setup_app(app)?;
            configure_linux_webview_acceleration(app);
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            // `register` and `login` (local credentials) are intentionally
            // not exposed over the IPC surface. Authentication is primarily
            // performed via Steam SSO (`start_steam_auth`) so these older
            // endpoints are omitted from the generated handler to reduce
            // attack surface and surface area for dead code.
            interface::tauri::commands::auth::logout,
            interface::tauri::commands::auth::get_session,
            interface::tauri::commands::auth::start_steam_auth,
            interface::tauri::commands::library::get_library,
            interface::tauri::commands::library::get_game_store_metadata,
            interface::tauri::commands::library::get_game_friends_activity,
            interface::tauri::commands::library::get_game_activity_timeline,
            interface::tauri::commands::library::get_game_achievements,
            interface::tauri::commands::library::get_game_trading_cards,
            interface::tauri::commands::library::get_game_dlc,
            interface::tauri::commands::library::get_game_review,
            // `get_steam_status` is a server-side helper (not exposed to the
            // frontend) and is intentionally not registered here.
            interface::tauri::commands::library::start_local_steam_scan,
            interface::tauri::commands::library::sync_steam_library,
            interface::tauri::commands::library::set_game_favorite,
            interface::tauri::commands::collections::list_collections,
            interface::tauri::commands::game_settings::list_game_languages,
            interface::tauri::commands::game_settings::list_game_compatibility_tools,
            interface::tauri::commands::game_settings::get_game_privacy_settings,
            interface::tauri::commands::game_settings::set_game_privacy_settings,
            interface::tauri::commands::game_settings::clear_game_overlay_data,
            interface::tauri::commands::game_settings::get_game_properties_settings,
            interface::tauri::commands::game_settings::set_game_properties_settings,
            interface::tauri::commands::game_settings::get_game_customization_artwork,
            interface::tauri::commands::game_settings::get_game_installation_details,
            interface::tauri::commands::game_settings::get_game_screenshots,
            interface::tauri::commands::game_settings::get_game_install_size_estimate,
            interface::tauri::commands::game_settings::list_game_install_locations,
            interface::tauri::commands::library::list_steam_downloads,
            interface::tauri::commands::steam::list_game_versions_betas,
            interface::tauri::commands::steam::validate_game_beta_access_code,
            interface::tauri::commands::collections::create_collection,
            interface::tauri::commands::collections::rename_collection,
            interface::tauri::commands::collections::delete_collection,
            interface::tauri::commands::collections::add_game_to_collection,
            interface::tauri::commands::game_actions::play_game,
            interface::tauri::commands::game_actions::install_game,
            interface::tauri::commands::game_actions::uninstall_game,
            interface::tauri::commands::game_actions::browse_game_installed_files,
            interface::tauri::commands::game_actions::backup_game_files,
            interface::tauri::commands::game_actions::verify_game_files,
            interface::tauri::commands::game_actions::add_game_desktop_shortcut,
            interface::tauri::commands::game_actions::open_game_recording_settings,
            interface::tauri::commands::steam::import_steam_collections
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(target_os = "linux")]
fn configure_linux_webview_acceleration(app: &mut tauri::App) {
    let Some(main_window) = app.get_webview_window("main") else {
        eprintln!("[catalyst] linux webview 'main' not found; skipping acceleration policy setup");
        return;
    };

    let apply_result = main_window.with_webview(|platform_webview| {
        let webview = platform_webview.inner();
        let Some(settings) = webview.settings() else {
            eprintln!("[catalyst] webkit settings unavailable; could not set acceleration policy");
            return;
        };

        settings.set_hardware_acceleration_policy(HardwareAccelerationPolicy::Always);
        let effective_policy = settings.hardware_acceleration_policy();
        eprintln!(
            "[catalyst] WebKitGTK hardware acceleration policy requested=Always effective={effective_policy:?}"
        );
    });

    if let Err(error) = apply_result {
        eprintln!("[catalyst] failed to configure WebKitGTK acceleration policy: {error}");
    }
}

#[cfg(not(target_os = "linux"))]
fn configure_linux_webview_acceleration(_app: &mut tauri::App) {}
