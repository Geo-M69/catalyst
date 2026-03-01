use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{Duration as ChronoDuration, TimeZone, Utc};
use regex::Regex;
use reqwest::blocking::Client;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{Manager, State};
use url::Url;
use uuid::Uuid;

const STEAM_OPENID_ENDPOINT: &str = "https://steamcommunity.com/openid/login";
const STEAM_WEB_API_ENDPOINT: &str =
    "https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/";
const STEAM_APP_DETAILS_ENDPOINT: &str = "https://store.steampowered.com/api/appdetails";
const STEAM_STORE_APP_ENDPOINT: &str = "https://store.steampowered.com/app";
const STEAM_CALLBACK_PUBLIC_HOST: &str = "catalyst";
const STEAM_APP_BETAS_ENDPOINT: &str = "https://api.steampowered.com/ISteamApps/GetAppBetas/v1/";
const STEAM_APP_BETA_CODE_CHECK_ENDPOINT: &str =
    "https://api.steampowered.com/ISteamApps/CheckAppBetaPassword/v1/";
const STEAM_CALLBACK_TIMEOUT: Duration = Duration::from_secs(180);
const STEAM_APP_DETAILS_BATCH_SIZE: usize = 75;
const STEAM_APP_METADATA_CACHE_TTL_HOURS: i64 = 24 * 7;
const STEAM_APP_LANGUAGES_CACHE_TTL_HOURS: i64 = 24 * 7;
const STEAM_APP_BETAS_CACHE_TTL_HOURS: i64 = 24 * 7;
const STEAM_APP_STORE_TAGS_CACHE_TTL_HOURS: i64 = 24 * 7;
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

struct AppState {
    db_path: PathBuf,
    session_token_path: PathBuf,
    steam_api_key: Option<String>,
    steam_local_install_detection: bool,
    steam_settings_debug_logging: bool,
    steam_root_override: Option<String>,
    current_session_token: Mutex<Option<String>>,
}

impl AppState {
    fn new(
        db_path: PathBuf,
        session_token_path: PathBuf,
        steam_api_key: Option<String>,
        steam_local_install_detection: bool,
        steam_settings_debug_logging: bool,
        steam_root_override: Option<String>,
    ) -> Self {
        Self {
            db_path,
            session_token_path,
            steam_api_key,
            steam_local_install_detection,
            steam_settings_debug_logging,
            steam_root_override,
            current_session_token: Mutex::new(None),
        }
    }
}

#[derive(Debug, Clone)]
struct UserRow {
    id: String,
    email: String,
    steam_id: Option<String>,
}

#[derive(Debug)]
struct AuthUserRow {
    user: UserRow,
    password_hash: String,
}

#[derive(Debug)]
struct LibraryGameInput {
    external_id: String,
    name: String,
    kind: String,
    playtime_minutes: i64,
    installed: bool,
    artwork_url: Option<String>,
    last_synced_at: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PublicUser {
    id: String,
    email: String,
    steam_linked: bool,
    steam_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthResponse {
    user: PublicUser,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SteamAuthResponse {
    user: PublicUser,
    synced_games: usize,
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
    favorite: bool,
    steam_tags: Vec<String>,
    collections: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LibraryResponse {
    user_id: String,
    total: usize,
    games: Vec<GameResponse>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SteamStatusResponse {
    user_id: String,
    provider: String,
    linked: bool,
    steam_id: Option<String>,
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
struct SteamSyncResponse {
    user_id: String,
    provider: String,
    synced_games: usize,
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
struct GameInstallationDetailsResponse {
    install_path: Option<String>,
    size_on_disk_bytes: Option<u64>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GameInstallLocationResponse {
    path: String,
    free_space_bytes: Option<u64>,
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
    bytes_total: Option<u64>,
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
struct GamePropertiesSettingsPayload {
    general: GameGeneralSettingsPayload,
    compatibility: GameCompatibilitySettingsPayload,
    updates: GameUpdatesSettingsPayload,
    controller: GameControllerSettingsPayload,
    game_versions_betas: GameVersionsBetasSettingsPayload,
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
struct GameVersionBetasResponse {
    options: Vec<GameVersionBetaOptionResponse>,
    warning: Option<String>,
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
}

#[tauri::command]
fn register(
    email: String,
    password: String,
    state: State<'_, AppState>,
) -> Result<AuthResponse, String> {
    let normalized_email = normalize_email(&email)?;
    validate_password(&password)?;

    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;

    if find_auth_user_by_email(&connection, &normalized_email)?.is_some() {
        return Err(String::from("Email is already in use"));
    }

    let password_hash = hash(password, DEFAULT_COST)
        .map_err(|error| format!("Failed to hash password: {error}"))?;
    let user = create_user(&connection, &normalized_email, &password_hash, None)?;
    let session_token = create_session(&connection, &user.id)?;
    persist_active_session(state.inner(), &session_token)?;

    Ok(AuthResponse {
        user: public_user_from_row(&user),
    })
}

#[tauri::command]
fn login(
    email: String,
    password: String,
    state: State<'_, AppState>,
) -> Result<AuthResponse, String> {
    let normalized_email = normalize_email(&email)?;
    validate_password(&password)?;

    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;

    let auth_user = find_auth_user_by_email(&connection, &normalized_email)?
        .ok_or_else(|| String::from("Invalid email or password"))?;
    let valid_password = verify(password, auth_user.password_hash.as_str())
        .map_err(|error| format!("Failed to verify password: {error}"))?;
    if !valid_password {
        return Err(String::from("Invalid email or password"));
    }

    let session_token = create_session(&connection, &auth_user.user.id)?;
    persist_active_session(state.inner(), &session_token)?;

    Ok(AuthResponse {
        user: public_user_from_row(&auth_user.user),
    })
}

#[tauri::command]
fn logout(state: State<'_, AppState>) -> Result<(), String> {
    let session_token = get_state_session_token(state.inner())?;
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;

    if let Some(token) = session_token {
        invalidate_session_by_token(&connection, &token)?;
    }

    clear_active_session(state.inner())
}

#[tauri::command]
fn get_session(state: State<'_, AppState>) -> Result<Option<PublicUser>, String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;

    let Some(session_token) = get_state_session_token(state.inner())? else {
        return Ok(None);
    };

    let user = find_user_by_session_token(&connection, &session_token)?;
    if user.is_none() {
        clear_active_session(state.inner())?;
    }

    Ok(user.map(|row| public_user_from_row(&row)))
}

#[tauri::command]
async fn start_steam_auth(state: State<'_, AppState>) -> Result<SteamAuthResponse, String> {
    let db_path = state.db_path.clone();
    let steam_api_key = state.steam_api_key.clone();
    let steam_local_install_detection = state.steam_local_install_detection;
    let steam_root_override = state.steam_root_override.clone();
    let current_session_token = get_state_session_token(state.inner())?;

    let outcome = tauri::async_runtime::spawn_blocking(move || {
        complete_steam_auth_flow(
            &db_path,
            steam_api_key,
            steam_local_install_detection,
            steam_root_override,
            current_session_token,
        )
    })
    .await
    .map_err(|error| format!("Steam auth task failed: {error}"))??;

    persist_active_session(state.inner(), &outcome.session_token)?;

    Ok(SteamAuthResponse {
        user: public_user_from_row(&outcome.user),
        synced_games: outcome.synced_games,
    })
}

#[tauri::command]
fn get_library(state: State<'_, AppState>) -> Result<LibraryResponse, String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let games = list_games_by_user(&connection, &user.id)?;

    Ok(LibraryResponse {
        user_id: user.id,
        total: games.len(),
        games,
    })
}

#[tauri::command]
fn get_steam_status(state: State<'_, AppState>) -> Result<SteamStatusResponse, String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;

    Ok(SteamStatusResponse {
        user_id: user.id,
        provider: String::from("steam"),
        linked: user.steam_id.is_some(),
        steam_id: user.steam_id,
    })
}

#[tauri::command]
fn sync_steam_library(state: State<'_, AppState>) -> Result<SteamSyncResponse, String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
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

#[tauri::command]
fn set_game_favorite(
    provider: String,
    external_id: String,
    favorite: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;

    if favorite {
        upsert_game_favorite(&connection, &user.id, &provider, &external_id)?;
    } else {
        remove_game_favorite(&connection, &user.id, &provider, &external_id)?;
    }

    Ok(())
}

#[tauri::command]
fn list_collections(
    provider: Option<String>,
    external_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<CollectionResponse>, String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;

    let target = match (provider.as_deref(), external_id.as_deref()) {
        (None, None) => None,
        (Some(target_provider), Some(target_external_id)) => {
            let (normalized_provider, normalized_external_id) =
                normalize_game_identity_input(target_provider, target_external_id)?;
            ensure_owned_game_exists(
                &connection,
                &user.id,
                &normalized_provider,
                &normalized_external_id,
            )?;
            Some((normalized_provider, normalized_external_id))
        }
        _ => {
            return Err(String::from(
                "provider and external_id must be supplied together",
            ))
        }
    };

    let list = if let Some((target_provider, target_external_id)) = target {
        list_collections_by_user(
            &connection,
            &user.id,
            Some(target_provider.as_str()),
            Some(target_external_id.as_str()),
        )?
    } else {
        list_collections_by_user(&connection, &user.id, None, None)?
    };

    Ok(list)
}

#[tauri::command]
fn list_game_languages(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (normalized_provider, normalized_external_id) =
        normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )?;

    if normalized_provider != "steam" {
        return Ok(Vec::new());
    }

    let app_id = match normalized_external_id.parse::<u64>() {
        Ok(parsed) => parsed,
        Err(_) => return Ok(Vec::new()),
    };

    let stale_before = Utc::now() - ChronoDuration::hours(STEAM_APP_LANGUAGES_CACHE_TTL_HOURS);
    let cached_languages_entry = find_cached_steam_app_languages(&connection, app_id)?;
    if let Some((cached_languages, fetched_at)) = cached_languages_entry.as_ref() {
        if *fetched_at >= stale_before {
            return Ok(cached_languages.clone());
        }
    }

    let client = build_http_client()?;
    match fetch_steam_supported_languages(&client, app_id) {
        Ok(fetched_languages) => {
            cache_steam_app_languages(&connection, app_id, &fetched_languages)?;
            Ok(fetched_languages)
        }
        Err(fetch_error) => {
            if let Some((cached_languages, _)) = cached_languages_entry {
                return Ok(cached_languages);
            }

            Err(fetch_error)
        }
    }
}

#[tauri::command]
fn list_game_compatibility_tools(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<GameCompatibilityToolResponse>, String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (normalized_provider, normalized_external_id) =
        normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )?;

    if normalized_provider != "steam" {
        return Ok(Vec::new());
    }

    let app_id = match normalized_external_id.parse::<u64>() {
        Ok(parsed) => parsed,
        Err(_) => return Ok(Vec::new()),
    };
    let include_linux_runtime_tools = match build_http_client()
        .and_then(|client| fetch_steam_app_linux_platform_support_from_store(&client, app_id))
    {
        Ok(Some(supported)) => supported,
        Ok(None) => false,
        Err(error) => {
            eprintln!(
                "Could not resolve Linux platform support for app {} while building compatibility tool list: {}",
                app_id, error
            );
            false
        }
    };

    resolve_steam_compatibility_tools(
        state.steam_root_override.as_deref(),
        include_linux_runtime_tools,
    )
}

#[tauri::command]
fn get_game_privacy_settings(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> Result<GamePrivacySettingsResponse, String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (normalized_provider, normalized_external_id) =
        normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )?;

    load_game_privacy_settings(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )
}

#[tauri::command]
fn set_game_privacy_settings(
    provider: String,
    external_id: String,
    hide_in_library: bool,
    mark_as_private: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (normalized_provider, normalized_external_id) =
        normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )?;

    let mut settings = load_game_privacy_settings(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )?;
    settings.hide_in_library = hide_in_library;
    settings.mark_as_private = mark_as_private;
    save_game_privacy_settings(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
        settings,
    )
}

#[tauri::command]
fn clear_game_overlay_data(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (normalized_provider, normalized_external_id) =
        normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )?;

    let mut settings = load_game_privacy_settings(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )?;
    settings.overlay_data_deleted = true;
    save_game_privacy_settings(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
        settings,
    )
}

#[tauri::command]
fn get_game_properties_settings(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> Result<GamePropertiesSettingsPayload, String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (normalized_provider, normalized_external_id) =
        normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )?;

    load_game_properties_settings(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )
}

#[tauri::command]
fn set_game_properties_settings(
    provider: String,
    external_id: String,
    settings: GamePropertiesSettingsPayload,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (normalized_provider, normalized_external_id) =
        normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )?;

    let normalized_settings = normalize_game_properties_settings_payload(settings);
    save_game_properties_settings(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
        &normalized_settings,
    )?;

    if normalized_provider == "steam" {
        let app_id = normalized_external_id
            .parse::<u64>()
            .map_err(|_| String::from("Steam external_id must be a numeric app ID"))?;
        if let Err(error) = apply_steam_game_properties_settings(
            state.inner(),
            &user,
            app_id,
            &normalized_settings,
        ) {
            eprintln!(
                "Could not apply Steam game properties for app {}: {}",
                app_id, error
            );
        }
    }

    Ok(())
}

#[tauri::command]
fn get_game_installation_details(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> Result<GameInstallationDetailsResponse, String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (normalized_provider, normalized_external_id) =
        normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )?;

    if normalized_provider != "steam" {
        return Ok(GameInstallationDetailsResponse {
            install_path: None,
            size_on_disk_bytes: None,
        });
    }

    let app_id = match normalized_external_id.parse::<u64>() {
        Ok(parsed) => parsed,
        Err(_) => {
            return Ok(GameInstallationDetailsResponse {
                install_path: None,
                size_on_disk_bytes: None,
            });
        }
    };

    let manifest_path =
        match resolve_steam_manifest_path_for_app_id(state.steam_root_override.as_deref(), app_id)
        {
            Ok(path) => path,
            Err(_) => {
                return Ok(GameInstallationDetailsResponse {
                    install_path: None,
                    size_on_disk_bytes: None,
                });
            }
        };

    let manifest_contents = fs::read_to_string(&manifest_path).map_err(|error| {
        format!(
            "Failed to read Steam app manifest at {}: {error}",
            manifest_path.display()
        )
    })?;
    let install_path = manifest_path
        .parent()
        .and_then(Path::parent)
        .map(|steam_library_path| steam_library_path.display().to_string());
    let size_on_disk_bytes = parse_steam_manifest_size_on_disk_bytes(&manifest_contents);

    Ok(GameInstallationDetailsResponse {
        install_path,
        size_on_disk_bytes,
    })
}

#[tauri::command]
fn get_game_install_size_estimate(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> Result<Option<u64>, String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (normalized_provider, normalized_external_id) =
        normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )?;

    if normalized_provider != "steam" {
        return Ok(None);
    }

    let app_id = match normalized_external_id.parse::<u64>() {
        Ok(parsed) => parsed,
        Err(_) => return Ok(None),
    };

    if let Ok(manifest_path) =
        resolve_steam_manifest_path_for_app_id(state.steam_root_override.as_deref(), app_id)
    {
        if let Ok(manifest_contents) = fs::read_to_string(&manifest_path) {
            if let Some(size_on_disk_bytes) = parse_steam_manifest_size_on_disk_bytes(&manifest_contents)
            {
                return Ok(Some(size_on_disk_bytes));
            }
        }
    }

    let client = build_http_client()?;
    fetch_steam_install_size_estimate_from_store(&client, app_id)
}

#[tauri::command]
fn list_game_install_locations(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<GameInstallLocationResponse>, String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (normalized_provider, normalized_external_id) =
        normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )?;

    if normalized_provider != "steam" {
        return Ok(Vec::new());
    }

    let Some(steam_root) = resolve_steam_root_path(state.steam_root_override.as_deref()) else {
        return Ok(Vec::new());
    };
    let steamapps_directories = resolve_steamapps_directories(&steam_root)?;

    let mut locations = Vec::new();
    let mut seen_paths = HashSet::new();
    for steamapps_directory in steamapps_directories {
        let library_path = steamapps_directory
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or(steamapps_directory);
        let path_label = library_path.display().to_string();
        let normalized_key = path_label.to_ascii_lowercase();
        if !seen_paths.insert(normalized_key) {
            continue;
        }

        locations.push(GameInstallLocationResponse {
            free_space_bytes: detect_available_disk_space_bytes(&library_path),
            path: path_label,
        });
    }

    if locations.is_empty() {
        let path_label = steam_root.display().to_string();
        locations.push(GameInstallLocationResponse {
            free_space_bytes: detect_available_disk_space_bytes(&steam_root),
            path: path_label,
        });
    }

    Ok(locations)
}

#[tauri::command]
fn list_steam_downloads(state: State<'_, AppState>) -> Result<Vec<SteamDownloadProgressResponse>, String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let owned_games_by_app_id = load_owned_steam_games_by_app_id(&connection, &user.id)?;
    if owned_games_by_app_id.is_empty() {
        return Ok(Vec::new());
    }

    let Some(steam_root) = resolve_steam_root_path(state.steam_root_override.as_deref()) else {
        return Ok(Vec::new());
    };
    let steamapps_directories = resolve_steamapps_directories(&steam_root)?;
    let mut downloads = Vec::new();
    let mut seen_external_ids = HashSet::new();

    for steamapps_directory in steamapps_directories {
        collect_steam_download_progress_from_steamapps_dir(
            &steamapps_directory,
            &owned_games_by_app_id,
            &mut seen_external_ids,
            &mut downloads,
        )?;
    }

    downloads.sort_by(|left, right| {
        left.name
            .to_ascii_lowercase()
            .cmp(&right.name.to_ascii_lowercase())
    });
    Ok(downloads)
}

#[tauri::command]
fn list_game_versions_betas(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> Result<GameVersionBetasResponse, String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (normalized_provider, normalized_external_id) =
        normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )?;

    if normalized_provider != "steam" {
        return Ok(GameVersionBetasResponse {
            options: default_game_version_beta_options(),
            warning: None,
        });
    }

    let app_id = match normalized_external_id.parse::<u64>() {
        Ok(parsed) => parsed,
        Err(_) => {
            return Ok(GameVersionBetasResponse {
                options: default_game_version_beta_options(),
                warning: Some(String::from("This Steam app ID is invalid.")),
            });
        }
    };

    let stale_before = Utc::now() - ChronoDuration::hours(STEAM_APP_BETAS_CACHE_TTL_HOURS);
    let cached_options_entry = find_cached_steam_app_betas(&connection, app_id)?;
    if let Some((cached_options, fetched_at)) = cached_options_entry.as_ref() {
        if *fetched_at >= stale_before {
            return Ok(GameVersionBetasResponse {
                options: cached_options.clone(),
                warning: None,
            });
        }
    }

    let Some(api_key) = state
        .steam_api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        if let Some((cached_options, _)) = cached_options_entry.as_ref() {
            return Ok(GameVersionBetasResponse {
                options: cached_options.clone(),
                warning: Some(String::from(
                    "Using cached beta branch data because STEAM_API_KEY is not configured.",
                )),
            });
        }

        return Ok(GameVersionBetasResponse {
            options: default_game_version_beta_options(),
            warning: Some(String::from(
                "Live beta branch data is unavailable because STEAM_API_KEY is not configured.",
            )),
        });
    };

    let client = build_http_client()?;
    match fetch_steam_game_version_betas(&client, app_id, api_key) {
        Ok(options) => {
            if !options.is_empty() {
                cache_steam_app_betas(&connection, app_id, &options)?;
                return Ok(GameVersionBetasResponse {
                    options,
                    warning: None,
                });
            }

            if let Some((cached_options, _)) = cached_options_entry.as_ref() {
                return Ok(GameVersionBetasResponse {
                    options: cached_options.clone(),
                    warning: Some(String::from(
                        "Steam returned no beta branch data. Showing cached data.",
                    )),
                });
            }

            Ok(GameVersionBetasResponse {
                options: default_game_version_beta_options(),
                warning: Some(String::from(
                    "Steam returned no beta branch data for this app.",
                )),
            })
        }
        Err(fetch_error) => {
            if is_forbidden_http_error(&fetch_error) {
                match fetch_steam_game_version_betas_from_store(&client, app_id) {
                    Ok(fallback_options) => {
                        if !fallback_options.is_empty() {
                            cache_steam_app_betas(&connection, app_id, &fallback_options)?;
                            return Ok(GameVersionBetasResponse {
                                options: fallback_options,
                                warning: Some(String::from(
                                    "Using public Steam branch metadata (partner betas API returned 403). Private branch visibility may be limited.",
                                )),
                            });
                        }
                    }
                    Err(fallback_error) => {
                        eprintln!(
                            "Steam betas partner API and store fallback both failed for app {app_id}: {fallback_error}"
                        );
                    }
                }
            }

            eprintln!("Failed to fetch Steam beta branches for app {app_id}: {fetch_error}");
            if let Some((cached_options, _)) = cached_options_entry.as_ref() {
                return Ok(GameVersionBetasResponse {
                    options: cached_options.clone(),
                    warning: Some(format!(
                        "Could not refresh beta branch data: {} Using cached data.",
                        normalize_backend_warning_message(&fetch_error)
                    )),
                });
            }
            Ok(GameVersionBetasResponse {
                options: default_game_version_beta_options(),
                warning: Some(normalize_backend_warning_message(&fetch_error)),
            })
        }
    }
}

#[tauri::command]
fn validate_game_beta_access_code(
    provider: String,
    external_id: String,
    access_code: String,
    state: State<'_, AppState>,
) -> Result<GameBetaAccessCodeValidationResponse, String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (normalized_provider, normalized_external_id) =
        normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )?;

    if normalized_provider != "steam" {
        return Ok(GameBetaAccessCodeValidationResponse {
            valid: false,
            message: String::from("Beta access code validation is only available for Steam games."),
            branch_id: None,
            branch_name: None,
        });
    }

    let trimmed_access_code = access_code.trim();
    if trimmed_access_code.is_empty() {
        return Ok(GameBetaAccessCodeValidationResponse {
            valid: false,
            message: String::from("Enter an access code before checking."),
            branch_id: None,
            branch_name: None,
        });
    }

    let app_id = match normalized_external_id.parse::<u64>() {
        Ok(parsed) => parsed,
        Err(_) => {
            return Ok(GameBetaAccessCodeValidationResponse {
                valid: false,
                message: String::from("This Steam app ID is invalid."),
                branch_id: None,
                branch_name: None,
            });
        }
    };

    let Some(api_key) = state
        .steam_api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(GameBetaAccessCodeValidationResponse {
            valid: false,
            message: String::from(
                "Beta access code validation is unavailable because STEAM_API_KEY is not configured.",
            ),
            branch_id: None,
            branch_name: None,
        });
    };

    let client = build_http_client()?;
    match fetch_steam_beta_access_code_validation(&client, app_id, api_key, trimmed_access_code) {
        Ok(validation) => Ok(validation),
        Err(fetch_error) => Ok(GameBetaAccessCodeValidationResponse {
            valid: false,
            message: if is_forbidden_http_error(&fetch_error) {
                String::from(
                    "Steam returned 403 for beta code validation. This usually requires publisher-level API access.",
                )
            } else if fetch_error.trim().is_empty() {
                String::from("Could not validate this code right now.")
            } else {
                normalize_backend_warning_message(&fetch_error)
            },
            branch_id: None,
            branch_name: None,
        }),
    }
}

#[tauri::command]
fn create_collection(name: String, state: State<'_, AppState>) -> Result<CollectionResponse, String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    create_user_collection(&connection, &user.id, &name)
}

#[tauri::command]
fn rename_collection(
    collection_id: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<CollectionResponse, String> {
    let trimmed_collection_id = collection_id.trim();
    if trimmed_collection_id.is_empty() {
        return Err(String::from("Collection ID is required"));
    }

    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    rename_user_collection(&connection, &user.id, trimmed_collection_id, &name)
}

#[tauri::command]
fn delete_collection(collection_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let trimmed_collection_id = collection_id.trim();
    if trimmed_collection_id.is_empty() {
        return Err(String::from("Collection ID is required"));
    }

    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    delete_user_collection(&connection, &user.id, trimmed_collection_id)
}

#[tauri::command]
fn add_game_to_collection(
    provider: String,
    external_id: String,
    collection_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let trimmed_collection_id = collection_id.trim();
    if trimmed_collection_id.is_empty() {
        return Err(String::from("Collection ID is required"));
    }

    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;
    ensure_owned_collection_exists(&connection, &user.id, trimmed_collection_id)?;
    add_game_to_collection_membership(
        &connection,
        &user.id,
        trimmed_collection_id,
        &provider,
        &external_id,
    )?;
    Ok(())
}

#[tauri::command]
fn play_game(
    provider: String,
    external_id: String,
    launch_options: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;
    let resolved_launch_options = match launch_options
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => Some(value.to_owned()),
        None => load_game_properties_settings(&connection, &user.id, &provider, &external_id)
            .ok()
            .and_then(|settings| {
                let trimmed_value = settings.general.launch_options.trim();
                if trimmed_value.is_empty() {
                    None
                } else {
                    Some(trimmed_value.to_owned())
                }
            }),
    };
    open_provider_game_uri(
        &provider,
        &external_id,
        "play",
        resolved_launch_options.as_deref(),
    )
}

#[tauri::command]
fn install_game(
    provider: String,
    external_id: String,
    install_path: Option<String>,
    create_desktop_shortcut: Option<bool>,
    create_application_shortcut: Option<bool>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;
    // Steam currently controls install destination and shortcut behavior from its own flow.
    // Keep receiving these values so the UI can evolve without breaking command contracts.
    let _ = (
        install_path,
        create_desktop_shortcut,
        create_application_shortcut,
    );
    open_provider_game_uri(&provider, &external_id, "install", None)
}

#[tauri::command]
fn browse_game_installed_files(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;

    if provider != "steam" {
        return Err(String::from(
            "Browsing installed files is only supported for Steam games.",
        ));
    }

    let app_id = external_id
        .parse::<u64>()
        .map_err(|_| String::from("Steam external_id must be a numeric app ID"))?;
    let install_directory =
        resolve_steam_install_directory_for_app_id(state.steam_root_override.as_deref(), app_id)?;
    if !install_directory.is_dir() {
        return Err(format!(
            "Install directory is unavailable: {}",
            install_directory.display()
        ));
    }

    open_path_in_file_manager(&install_directory)
}

#[tauri::command]
fn backup_game_files(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;
    open_provider_game_uri(&provider, &external_id, "backup", None)
}

#[tauri::command]
fn verify_game_files(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;
    open_provider_game_uri(&provider, &external_id, "validate", None)
}

#[tauri::command]
fn import_steam_collections(state: State<'_, AppState>) -> Result<SteamCollectionsImportResponse, String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let steam_id = user
        .steam_id
        .as_deref()
        .ok_or_else(|| String::from("Steam is not linked for this account"))?;
    let steam_root = resolve_steam_root_path(state.steam_root_override.as_deref())
        .ok_or_else(|| String::from("Could not locate local Steam installation"))?;
    let userdata_directory = resolve_steam_userdata_directory(&steam_root, steam_id)?;
    let config_paths = [
        userdata_directory.join("7").join("remote").join("sharedconfig.vdf"),
        userdata_directory.join("config").join("sharedconfig.vdf"),
        userdata_directory.join("config").join("localconfig.vdf"),
    ];

    let mut combined_collections_by_app_id: HashMap<String, HashSet<String>> = HashMap::new();
    let mut loaded_any_config_file = false;
    let mut loaded_config_paths = Vec::new();
    for config_path in config_paths {
        if !config_path.is_file() {
            continue;
        }

        let config_contents = fs::read_to_string(&config_path).map_err(|error| {
            format!(
                "Failed to read Steam config at {}: {error}",
                config_path.display()
            )
        })?;
        let parsed_collections = parse_steam_collections_from_vdf(&config_contents)?;
        merge_collections_by_app_id(&mut combined_collections_by_app_id, parsed_collections);
        loaded_any_config_file = true;
        loaded_config_paths.push(config_path.display().to_string());
    }

    if !loaded_any_config_file {
        return Err(format!(
            "Could not locate Steam collection config files for account {steam_id} in {}",
            userdata_directory.display()
        ));
    }

    if combined_collections_by_app_id.is_empty() {
        let files_label = if loaded_config_paths.is_empty() {
            String::from("none")
        } else {
            loaded_config_paths.join(", ")
        };
        return Err(format!(
            "No Steam collections were found in local Steam configuration. Checked files: {files_label}"
        ));
    }

    import_steam_collections_for_user(&connection, &user.id, combined_collections_by_app_id)
}

fn complete_steam_auth_flow(
    db_path: &Path,
    steam_api_key: Option<String>,
    steam_local_install_detection: bool,
    steam_root_override: Option<String>,
    current_session_token: Option<String>,
) -> Result<SteamAuthOutcome, String> {
    let connection = open_connection(db_path)?;
    cleanup_expired_sessions(&connection)?;
    let client = build_http_client()?;

    let current_user = match current_session_token {
        Some(token) => find_user_by_session_token(&connection, &token)?,
        None => None,
    };

    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|error| format!("Failed to bind Steam callback listener: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("Failed to read callback listener address: {error}"))?
        .port();
    let callback_public_host = resolve_steam_callback_public_host();

    let state_token = Uuid::new_v4().to_string();
    let callback_url = format!(
        "http://{callback_public_host}:{port}/auth/steam/callback?state={state_token}"
    );
    let realm = format!("http://{callback_public_host}:{port}");
    let authorization_url = build_steam_authorization_url(&callback_url, &realm)?;

    webbrowser::open(&authorization_url)
        .map_err(|error| format!("Failed to open Steam login in browser: {error}"))?;

    let callback_params = wait_for_steam_callback(
        listener,
        &state_token,
        STEAM_CALLBACK_TIMEOUT,
        &callback_public_host,
    )?;
    let verified = verify_steam_openid_response(&client, &callback_params)?;
    if !verified {
        return Err(String::from("Steam login verification failed"));
    }

    let claimed_id = callback_params
        .get("openid.claimed_id")
        .ok_or_else(|| String::from("Steam callback missing claimed ID"))?;

    let steam_id_pattern = Regex::new(r"/openid/id/(\d{17})$")
        .map_err(|error| format!("Failed to compile Steam ID regex: {error}"))?;
    let steam_id = steam_id_pattern
        .captures(claimed_id)
        .and_then(|capture| capture.get(1))
        .map(|matched| matched.as_str().to_owned())
        .ok_or_else(|| String::from("Steam callback returned an invalid claimed ID"))?;

    let user = resolve_user_for_steam_auth(&connection, current_user.as_ref(), &steam_id)?;
    let synced_games = sync_steam_games_for_user(
        &connection,
        &user,
        steam_api_key.as_deref(),
        steam_local_install_detection,
        steam_root_override.as_deref(),
        &client,
    )?;
    let session_token = create_session(&connection, &user.id)?;

    Ok(SteamAuthOutcome {
        user,
        synced_games,
        session_token,
    })
}

fn resolve_user_for_steam_auth(
    connection: &Connection,
    current_user: Option<&UserRow>,
    steam_id: &str,
) -> Result<UserRow, String> {
    if let Some(authenticated_user) = current_user {
        if let Some(existing_linked_user) = find_user_by_steam_id(connection, steam_id)? {
            if existing_linked_user.id != authenticated_user.id {
                return Err(String::from(
                    "Steam account is already linked to another user",
                ));
            }
            return Ok(existing_linked_user);
        }

        return set_user_steam_id(connection, &authenticated_user.id, steam_id);
    }

    if let Some(existing_linked_user) = find_user_by_steam_id(connection, steam_id)? {
        return Ok(existing_linked_user);
    }

    create_steam_user(connection, steam_id)
}

fn resolve_steam_callback_public_host() -> String {
    let preferred_host = STEAM_CALLBACK_PUBLIC_HOST.trim();
    if preferred_host.is_empty() {
        return String::from(STEAM_CALLBACK_FALLBACK_HOST);
    }

    let can_resolve_preferred_host = (preferred_host, 0).to_socket_addrs().is_ok();
    if can_resolve_preferred_host {
        return preferred_host.to_owned();
    }

    eprintln!(
        "Steam callback host '{preferred_host}' could not be resolved. Falling back to {STEAM_CALLBACK_FALLBACK_HOST}."
    );
    String::from(STEAM_CALLBACK_FALLBACK_HOST)
}

fn wait_for_steam_callback(
    listener: TcpListener,
    expected_state: &str,
    timeout: Duration,
    callback_public_host: &str,
) -> Result<HashMap<String, String>, String> {
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("Failed to configure callback listener: {error}"))?;

    let deadline = Instant::now() + timeout;
    loop {
        if Instant::now() >= deadline {
            return Err(String::from(
                "Timed out waiting for Steam callback. Complete Steam sign-in in your browser and if Windows Firewall prompts for Catalyst, allow local/private access.",
            ));
        }

        match listener.accept() {
            Ok((mut stream, _)) => {
                let request_target = read_http_request_target(&mut stream)?;
                let callback_url =
                    Url::parse(&format!("http://{callback_public_host}{request_target}"))
                    .map_err(|error| format!("Failed to parse callback URL: {error}"))?;
                let callback_params = callback_url
                    .query_pairs()
                    .map(|(key, value)| (key.to_string(), value.to_string()))
                    .collect::<HashMap<_, _>>();

                if callback_params.get("state").map(|value| value.as_str()) != Some(expected_state)
                {
                    let body = "<html><body><h2>Steam login failed</h2><p>State mismatch. Return to Catalyst and try again.</p></body></html>";
                    let _ = write_http_response(&mut stream, "400 Bad Request", body);
                    return Err(String::from("Steam callback state mismatch"));
                }

                let body = "<html><body><h2>Steam login complete</h2><p>You can close this tab and return to Catalyst.</p></body></html>";
                let _ = write_http_response(&mut stream, "200 OK", body);
                return Ok(callback_params);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(error) => return Err(format!("Failed while waiting for Steam callback: {error}")),
        }
    }
}

fn read_http_request_target(stream: &mut TcpStream) -> Result<String, String> {
    let mut buffer = [0u8; 8192];
    let bytes_read = stream
        .read(&mut buffer)
        .map_err(|error| format!("Failed to read callback request: {error}"))?;
    if bytes_read == 0 {
        return Err(String::from("Steam callback request was empty"));
    }

    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let request_line = request
        .lines()
        .next()
        .ok_or_else(|| String::from("Steam callback request line missing"))?;

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or_default();

    if method != "GET" {
        return Err(format!("Steam callback used unsupported method: {method}"));
    }
    if target.is_empty() {
        return Err(String::from("Steam callback request target missing"));
    }

    Ok(target.to_owned())
}

fn write_http_response(stream: &mut TcpStream, status: &str, body: &str) -> Result<(), String> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.as_bytes().len()
    );

    stream
        .write_all(response.as_bytes())
        .map_err(|error| format!("Failed to write callback response: {error}"))?;
    stream
        .flush()
        .map_err(|error| format!("Failed to flush callback response: {error}"))
}

fn build_steam_authorization_url(return_to: &str, realm: &str) -> Result<String, String> {
    let mut url = Url::parse(STEAM_OPENID_ENDPOINT)
        .map_err(|error| format!("Failed to parse Steam OpenID endpoint: {error}"))?;

    url.query_pairs_mut()
        .append_pair("openid.ns", "http://specs.openid.net/auth/2.0")
        .append_pair("openid.mode", "checkid_setup")
        .append_pair("openid.return_to", return_to)
        .append_pair("openid.realm", realm)
        .append_pair(
            "openid.identity",
            "http://specs.openid.net/auth/2.0/identifier_select",
        )
        .append_pair(
            "openid.claimed_id",
            "http://specs.openid.net/auth/2.0/identifier_select",
        );

    Ok(url.to_string())
}

fn verify_steam_openid_response(
    client: &Client,
    callback_params: &HashMap<String, String>,
) -> Result<bool, String> {
    let mut verification_form = callback_params
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Vec<_>>();
    verification_form.retain(|(key, _)| key != "openid.mode");
    verification_form.push((
        String::from("openid.mode"),
        String::from("check_authentication"),
    ));

    let response = client
        .post(STEAM_OPENID_ENDPOINT)
        .form(&verification_form)
        .send()
        .map_err(|error| format!("Steam OpenID verification request failed: {error}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Steam OpenID verification failed with status {}",
            response.status()
        ));
    }

    let body = response
        .text()
        .map_err(|error| format!("Failed to read Steam OpenID verification response: {error}"))?;
    Ok(body.contains("is_valid:true"))
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

    let locally_installed_app_ids = if steam_local_install_detection {
        match detect_locally_installed_steam_app_ids(steam_root_override) {
            Ok(app_ids) => Some(app_ids),
            Err(error) => {
                eprintln!("Local Steam install detection failed: {error}");
                None
            }
        }
    } else {
        Some(HashSet::new())
    };

    let Some(api_key) = steam_api_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        if let Some(app_ids) = locally_installed_app_ids.as_ref() {
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

    if let Err(error) = refresh_steam_store_tags_cache(connection, client, &steam_owned_app_ids) {
        eprintln!("Steam Store tag sync failed: {error}");
    }

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
    let Some(steam_root) = resolve_steam_root_path(steam_root_override) else {
        return Ok(HashSet::new());
    };

    let steamapps_directories = resolve_steamapps_directories(&steam_root)?;
    let mut installed_app_ids = HashSet::new();
    for steamapps_directory in steamapps_directories {
        collect_installed_app_ids_from_steamapps_dir(&steamapps_directory, &mut installed_app_ids)?;
    }

    Ok(installed_app_ids)
}

fn resolve_steam_root_path(steam_root_override: Option<&str>) -> Option<PathBuf> {
    if let Some(override_path) = steam_root_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(PathBuf::from(override_path));
    }

    steam_root_candidates()
        .into_iter()
        .find(|candidate| candidate.join("steamapps").is_dir())
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
    let steam_root = resolve_steam_root_path(steam_root_override)
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

    if cfg!(target_os = "windows") {
        if let Ok(path) = std::env::var("PROGRAMFILES(X86)") {
            candidates.push(PathBuf::from(path).join("Steam"));
        }
        if let Ok(path) = std::env::var("PROGRAMFILES") {
            candidates.push(PathBuf::from(path).join("Steam"));
        }
        candidates.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
        candidates.push(PathBuf::from(r"C:\Program Files\Steam"));
    } else if cfg!(target_os = "macos") {
        if let Ok(home) = std::env::var("HOME") {
            let home_path = PathBuf::from(home);
            candidates.push(home_path.join("Library/Application Support/Steam"));
        }
    } else {
        if let Ok(home) = std::env::var("HOME") {
            let home_path = PathBuf::from(home);
            candidates.push(home_path.join(".steam/root"));
            candidates.push(home_path.join(".steam/steam"));
            candidates.push(home_path.join(".local/share/Steam"));
            candidates.push(home_path.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"));
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
            return Err(format!(
                "Failed to read Steam library folder file at {}: {error}",
                library_folders_path.display()
            ));
        }
    };
    let library_paths = parse_steam_libraryfolder_paths(&library_folders_content)?;

    for library_path in library_paths {
        let steamapps_directory = library_path.join("steamapps");
        if seen_directories.insert(steamapps_directory.clone()) {
            steamapps_directories.push(steamapps_directory);
        }
    }

    Ok(steamapps_directories)
}

fn parse_steam_libraryfolder_paths(contents: &str) -> Result<Vec<PathBuf>, String> {
    let path_pattern = Regex::new(r#"^\s*"path"\s*"([^"]+)""#)
        .map_err(|error| format!("Failed to compile Steam path pattern: {error}"))?;
    let legacy_pattern = Regex::new(r#"^\s*"[0-9]+"\s*"([^"]+)""#)
        .map_err(|error| format!("Failed to compile legacy Steam path pattern: {error}"))?;

    let mut paths = Vec::new();
    let mut seen_paths = HashSet::new();

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
        let entry = directory_entry
            .map_err(|error| format!("Failed to read Steam library entry: {error}"))?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let Some(app_id) = parse_steam_manifest_app_id(&file_name) else {
            continue;
        };
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
    let Some(steam_root) = resolve_steam_root_path(steam_root_override) else {
        return Err(String::from("Could not locate local Steam installation"));
    };

    let steamapps_directories = resolve_steamapps_directories(&steam_root)?;
    let manifest_file_name = format!("appmanifest_{app_id}.acf");
    for steamapps_directory in steamapps_directories {
        let manifest_path = steamapps_directory.join(&manifest_file_name);
        if manifest_path.is_file() {
            return Ok(manifest_path);
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
    let bytes_total = parse_steam_manifest_u64_field(manifest_contents, "BytesToDownload")
        .or_else(|| parse_steam_manifest_u64_field(manifest_contents, "TotalDownloaded"));
    let bytes_downloaded = parse_steam_manifest_u64_field(manifest_contents, "BytesDownloaded")
        .or_else(|| parse_steam_manifest_u64_field(manifest_contents, "BytesDownloadedOnCurrentRun"));

    SteamManifestDownloadProgressSnapshot {
        state_flags: parse_steam_manifest_u64_field(manifest_contents, "StateFlags"),
        bytes_downloaded,
        bytes_total,
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

        let Some(game) = owned_games_by_app_id.get(&app_id) else {
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

        let progress_snapshot = parse_steam_manifest_download_progress(&manifest_contents);
        let bytes_total = progress_snapshot.bytes_total.filter(|value| *value > 0);
        let bytes_downloaded = match (progress_snapshot.bytes_downloaded, bytes_total) {
            (Some(downloaded), _) => Some(downloaded),
            (None, Some(_)) => Some(0),
            (None, None) => None,
        };
        let has_progress = match (bytes_downloaded, bytes_total) {
            (Some(downloaded), Some(total)) => downloaded < total,
            _ => false,
        };
        let app_id_path_segment = app_id.to_string();
        let has_active_download_directory = steamapps_directory
            .join("downloading")
            .join(&app_id_path_segment)
            .is_dir()
            || steamapps_directory
                .join("temp")
                .join(&app_id_path_segment)
                .is_dir();
        let state_flags = progress_snapshot.state_flags.unwrap_or(0);
        let Some(state_label) =
            infer_steam_download_state(state_flags, has_progress, has_active_download_directory)
        else {
            continue;
        };
        if !seen_external_ids.insert(game.external_id.clone()) {
            continue;
        }

        let progress_percent = match (bytes_downloaded, bytes_total) {
            (Some(downloaded), Some(total)) if total > 0 => Some(
                ((downloaded.min(total)) as f64 / total as f64 * 100.0).clamp(0.0, 100.0),
            ),
            _ => None,
        };

        output.push(SteamDownloadProgressResponse {
            game_id: game.game_id.clone(),
            provider: String::from("steam"),
            external_id: game.external_id.clone(),
            name: game.name.clone(),
            state: String::from(state_label),
            bytes_downloaded,
            bytes_total,
            progress_percent,
        });
    }

    Ok(())
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

fn open_path_in_file_manager(path: &Path) -> Result<(), String> {
    let open_result = if cfg!(target_os = "windows") {
        Command::new("explorer").arg(path).spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(path).spawn()
    } else {
        Command::new("xdg-open").arg(path).spawn()
    };

    open_result
        .map(|_| ())
        .map_err(|error| format!("Failed to open path {}: {error}", path.display()))
}

fn resolve_steam_game_kinds(
    connection: &Connection,
    client: &Client,
    games: &[SteamOwnedGame],
) -> Result<HashMap<u64, String>, String> {
    let stale_before = Utc::now() - ChronoDuration::hours(STEAM_APP_METADATA_CACHE_TTL_HOURS);
    let mut kinds_by_app_id = HashMap::new();
    let mut uncached_app_ids = Vec::new();
    let mut seen_app_ids = HashSet::new();

    for game in games {
        if !seen_app_ids.insert(game.appid) {
            continue;
        }

        if let Some(cached_type) = find_cached_steam_app_type(connection, game.appid, stale_before)?
        {
            kinds_by_app_id.insert(
                game.appid,
                steam_kind_from_app_type(&cached_type).to_owned(),
            );
        } else {
            uncached_app_ids.push(game.appid);
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

fn refresh_steam_store_tags_cache(
    connection: &Connection,
    client: &Client,
    app_ids: &[u64],
) -> Result<(), String> {
    let stale_before = Utc::now() - ChronoDuration::hours(STEAM_APP_STORE_TAGS_CACHE_TTL_HOURS);
    let mut seen_app_ids = HashSet::new();

    for app_id in app_ids {
        if !seen_app_ids.insert(*app_id) {
            continue;
        }

        if find_cached_steam_store_tags(connection, *app_id, stale_before)?.is_some() {
            continue;
        }

        let fetched_tags = match fetch_steam_store_user_tags(client, *app_id) {
            Ok(tags) => tags,
            Err(error) => {
                eprintln!("Could not fetch Steam Store tags for app {app_id}: {error}");
                Vec::new()
            }
        };
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

fn fetch_steam_supported_languages(client: &Client, app_id: u64) -> Result<Vec<String>, String> {
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

    let raw_languages = entry
        .get("data")
        .and_then(|value| value.get("supported_languages"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();

    Ok(parse_steam_supported_languages(raw_languages))
}

fn fetch_steam_install_size_estimate_from_store(
    client: &Client,
    app_id: u64,
) -> Result<Option<u64>, String> {
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
    client: &Client,
    app_id: u64,
) -> Result<Option<bool>, String> {
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

    let mut insert = connection
        .prepare(
            "
            INSERT INTO games (user_id, provider, external_id, name, kind, playtime_minutes, installed, artwork_url, last_synced_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(user_id, provider, external_id) DO UPDATE SET
              name = excluded.name,
              kind = excluded.kind,
              playtime_minutes = excluded.playtime_minutes,
              installed = excluded.installed,
              artwork_url = excluded.artwork_url,
              last_synced_at = excluded.last_synced_at
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
                game.last_synced_at
            ])
            .map_err(|error| format!("Failed to persist synced game: {error}"))?;
    }

    Ok(())
}

fn list_games_by_user(connection: &Connection, user_id: &str) -> Result<Vec<GameResponse>, String> {
    let collections_by_game = load_collection_names_by_game(connection, user_id)?;
    let steam_tags_by_game = load_steam_tags_by_game(connection, user_id)?;
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
              EXISTS(
                SELECT 1
                FROM game_favorites favorite
                WHERE favorite.user_id = g.user_id
                  AND favorite.provider = g.provider
                  AND favorite.external_id = g.external_id
              ) AS favorite
            FROM games g
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
            let favorite_raw: i64 = row.get(8)?;
            let game_key = game_membership_key(&provider, &external_id);
            let steam_tags = if provider.eq_ignore_ascii_case("steam") {
                steam_tags_by_game
                    .get(&external_id)
                    .cloned()
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
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
                favorite: favorite_raw > 0,
                steam_tags,
                collections,
            })
        })
        .map_err(|error| format!("Failed to query library rows: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to decode library rows: {error}"))
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

fn normalize_game_identity_input(
    provider: &str,
    external_id: &str,
) -> Result<(String, String), String> {
    let normalized_provider = provider.trim().to_ascii_lowercase();
    if normalized_provider.is_empty() {
        return Err(String::from("Game provider is required"));
    }

    let normalized_external_id = external_id.trim().to_owned();
    if normalized_external_id.is_empty() {
        return Err(String::from("Game external ID is required"));
    }

    Ok((normalized_provider, normalized_external_id))
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

fn normalize_collection_name(name: &str) -> Result<String, String> {
    let normalized_name = name.trim();
    if normalized_name.is_empty() {
        return Err(String::from("Collection name is required"));
    }

    if normalized_name.chars().count() > 80 {
        return Err(String::from("Collection name must be 80 characters or fewer"));
    }

    Ok(normalized_name.to_owned())
}

fn create_user_collection(
    connection: &Connection,
    user_id: &str,
    name: &str,
) -> Result<CollectionResponse, String> {
    let normalized_name = normalize_collection_name(name)?;
    let collection_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let insert_result = connection.execute(
        "
        INSERT INTO collections (id, user_id, name, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5)
        ",
        params![collection_id, user_id, normalized_name, now, now],
    );

    match insert_result {
        Ok(_) => Ok(CollectionResponse {
            id: collection_id,
            name: normalized_name,
            game_count: 0,
            contains_game: false,
        }),
        Err(error) if error.to_string().contains("UNIQUE constraint failed: collections.user_id, collections.name") => {
            Err(String::from("Collection name already exists"))
        }
        Err(error) => Err(format!("Failed to create collection: {error}")),
    }
}

fn rename_user_collection(
    connection: &Connection,
    user_id: &str,
    collection_id: &str,
    name: &str,
) -> Result<CollectionResponse, String> {
    ensure_owned_collection_exists(connection, user_id, collection_id)?;
    let normalized_name = normalize_collection_name(name)?;
    let now = Utc::now().to_rfc3339();
    let update_result = connection.execute(
        "
        UPDATE collections
        SET name = ?1, updated_at = ?2
        WHERE id = ?3 AND user_id = ?4
        ",
        params![normalized_name, now, collection_id, user_id],
    );
    match update_result {
        Ok(updated_rows) => {
            if updated_rows == 0 {
                return Err(String::from("Collection not found for current user"));
            }
        }
        Err(error)
            if error
                .to_string()
                .contains("UNIQUE constraint failed: collections.user_id, collections.name") =>
        {
            return Err(String::from("Collection name already exists"));
        }
        Err(error) => return Err(format!("Failed to rename collection: {error}")),
    }

    let game_count_raw = connection
        .query_row(
            "
            SELECT COUNT(*)
            FROM collection_games
            WHERE user_id = ?1 AND collection_id = ?2
            ",
            params![user_id, collection_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("Failed to query renamed collection size: {error}"))?;

    Ok(CollectionResponse {
        id: collection_id.to_owned(),
        name: normalized_name,
        game_count: usize::try_from(game_count_raw).unwrap_or_default(),
        contains_game: false,
    })
}

fn delete_user_collection(
    connection: &Connection,
    user_id: &str,
    collection_id: &str,
) -> Result<(), String> {
    ensure_owned_collection_exists(connection, user_id, collection_id)?;
    let deleted_rows = connection
        .execute(
            "DELETE FROM collections WHERE id = ?1 AND user_id = ?2",
            params![collection_id, user_id],
        )
        .map_err(|error| format!("Failed to delete collection: {error}"))?;
    if deleted_rows == 0 {
        return Err(String::from("Collection not found for current user"));
    }

    Ok(())
}

fn ensure_owned_collection_exists(
    connection: &Connection,
    user_id: &str,
    collection_id: &str,
) -> Result<(), String> {
    let exists = connection
        .query_row(
            "SELECT 1 FROM collections WHERE id = ?1 AND user_id = ?2",
            params![collection_id, user_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| format!("Failed to validate collection ownership: {error}"))?;
    if exists.is_none() {
        return Err(String::from("Collection not found for current user"));
    }

    Ok(())
}

fn add_game_to_collection_membership(
    connection: &Connection,
    user_id: &str,
    collection_id: &str,
    provider: &str,
    external_id: &str,
) -> Result<bool, String> {
    let inserted_rows = connection
        .execute(
            "
            INSERT OR IGNORE INTO collection_games (user_id, collection_id, provider, external_id, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            params![user_id, collection_id, provider, external_id, Utc::now().to_rfc3339()],
        )
        .map_err(|error| format!("Failed to add game to collection: {error}"))?;
    Ok(inserted_rows > 0)
}

fn list_collections_by_user(
    connection: &Connection,
    user_id: &str,
    provider: Option<&str>,
    external_id: Option<&str>,
) -> Result<Vec<CollectionResponse>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              c.id,
              c.name,
              (
                SELECT COUNT(*)
                FROM collection_games membership
                WHERE membership.user_id = c.user_id
                  AND membership.collection_id = c.id
              ) AS game_count
            FROM collections c
            WHERE c.user_id = ?1
            ORDER BY c.name COLLATE NOCASE ASC
            ",
        )
        .map_err(|error| format!("Failed to prepare collections query: {error}"))?;

    let rows = statement
        .query_map(params![user_id], |row| {
            let game_count_raw: i64 = row.get(2)?;
            Ok(CollectionResponse {
                id: row.get(0)?,
                name: row.get(1)?,
                game_count: usize::try_from(game_count_raw).unwrap_or_default(),
                contains_game: false,
            })
        })
        .map_err(|error| format!("Failed to query collections: {error}"))?;
    let mut collections = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to decode collections: {error}"))?;

    let membership_ids = if let (Some(target_provider), Some(target_external_id)) = (provider, external_id)
    {
        let mut membership_statement = connection
            .prepare(
                "
                SELECT collection_id
                FROM collection_games
                WHERE user_id = ?1 AND provider = ?2 AND external_id = ?3
                ",
            )
            .map_err(|error| format!("Failed to prepare collection membership query: {error}"))?;
        let membership_rows = membership_statement
            .query_map(params![user_id, target_provider, target_external_id], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|error| format!("Failed to query collection membership: {error}"))?;
        membership_rows
            .collect::<Result<HashSet<_>, _>>()
            .map_err(|error| format!("Failed to decode collection membership: {error}"))?
    } else {
        HashSet::new()
    };

    for collection in &mut collections {
        collection.contains_game = membership_ids.contains(&collection.id);
    }

    Ok(collections)
}

fn find_collection_id_by_name(
    connection: &Connection,
    user_id: &str,
    name: &str,
) -> Result<Option<String>, String> {
    connection
        .query_row(
            "SELECT id FROM collections WHERE user_id = ?1 AND name = ?2 COLLATE NOCASE",
            params![user_id, name],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("Failed to query collection by name: {error}"))
}

fn get_or_create_collection_id_by_name(
    connection: &Connection,
    user_id: &str,
    name: &str,
) -> Result<(String, bool), String> {
    let normalized_name = normalize_collection_name(name)?;
    if let Some(existing_id) = find_collection_id_by_name(connection, user_id, &normalized_name)? {
        return Ok((existing_id, false));
    }

    let collection_id = Uuid::new_v4().to_string();
    let timestamp = Utc::now().to_rfc3339();
    let inserted_rows = connection
        .execute(
            "
            INSERT OR IGNORE INTO collections (id, user_id, name, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            params![collection_id, user_id, normalized_name, timestamp, timestamp],
        )
        .map_err(|error| format!("Failed to create collection during Steam import: {error}"))?;
    if inserted_rows > 0 {
        return Ok((collection_id, true));
    }

    let existing_id = find_collection_id_by_name(connection, user_id, &normalized_name)?
        .ok_or_else(|| String::from("Failed to resolve collection created during Steam import"))?;
    Ok((existing_id, false))
}

fn load_provider_game_external_ids(
    connection: &Connection,
    user_id: &str,
    provider: &str,
) -> Result<HashSet<String>, String> {
    let mut statement = connection
        .prepare("SELECT external_id FROM games WHERE user_id = ?1 AND provider = ?2")
        .map_err(|error| format!("Failed to prepare provider game list query: {error}"))?;
    let rows = statement
        .query_map(params![user_id, provider], |row| row.get::<_, String>(0))
        .map_err(|error| format!("Failed to query provider game list: {error}"))?;

    rows.collect::<Result<HashSet<_>, _>>()
        .map_err(|error| format!("Failed to decode provider game list: {error}"))
}

fn load_owned_steam_games_by_app_id(
    connection: &Connection,
    user_id: &str,
) -> Result<HashMap<u64, OwnedSteamGameMetadata>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT id, external_id, name
            FROM games
            WHERE user_id = ?1 AND provider = 'steam'
            ",
        )
        .map_err(|error| format!("Failed to prepare owned Steam game query: {error}"))?;
    let rows = statement
        .query_map(params![user_id], |row| {
            Ok(OwnedSteamGameMetadata {
                game_id: row.get::<_, String>(0)?,
                external_id: row.get::<_, String>(1)?,
                name: row.get::<_, String>(2)?,
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

#[derive(Debug, Clone)]
enum VdfToken {
    OpenBrace,
    CloseBrace,
    Text(String),
}

#[derive(Debug, Clone)]
enum VdfValue {
    Object(Vec<(String, VdfValue)>),
    Text(String),
}

fn tokenize_vdf(contents: &str) -> Vec<VdfToken> {
    let normalized_contents = contents.trim_start_matches('\u{feff}');
    let mut characters = normalized_contents.chars().peekable();
    let mut tokens = Vec::new();

    while let Some(character) = characters.next() {
        match character {
            '{' => tokens.push(VdfToken::OpenBrace),
            '}' => tokens.push(VdfToken::CloseBrace),
            '"' => {
                let mut value = String::new();
                while let Some(inner_character) = characters.next() {
                    if inner_character == '"' {
                        break;
                    }

                    if inner_character == '\0' {
                        continue;
                    }

                    if inner_character == '\\' {
                        if let Some(escaped_character) = characters.next() {
                            match escaped_character {
                                '\\' => value.push('\\'),
                                '"' => value.push('"'),
                                'n' => value.push('\n'),
                                'r' => value.push('\r'),
                                't' => value.push('\t'),
                                '\0' => {}
                                other => value.push(other),
                            }
                        }
                        continue;
                    }

                    value.push(inner_character);
                }
                tokens.push(VdfToken::Text(value));
            }
            '/' => {
                if matches!(characters.peek(), Some('/')) {
                    let _ = characters.next();
                    while let Some(comment_character) = characters.next() {
                        if comment_character == '\n' {
                            break;
                        }
                    }
                    continue;
                }

                let mut bare_token = String::from("/");
                while let Some(peeked_character) = characters.peek().copied() {
                    if peeked_character == '\0' {
                        let _ = characters.next();
                        continue;
                    }
                    if peeked_character.is_whitespace()
                        || peeked_character == '{'
                        || peeked_character == '}'
                    {
                        break;
                    }
                    bare_token.push(peeked_character);
                    let _ = characters.next();
                }
                tokens.push(VdfToken::Text(bare_token));
            }
            value if value.is_whitespace() || value == '\0' => {}
            value => {
                let mut bare_token = String::new();
                bare_token.push(value);
                while let Some(peeked_character) = characters.peek().copied() {
                    if peeked_character == '\0' {
                        let _ = characters.next();
                        continue;
                    }
                    if peeked_character.is_whitespace()
                        || peeked_character == '{'
                        || peeked_character == '}'
                    {
                        break;
                    }
                    bare_token.push(peeked_character);
                    let _ = characters.next();
                }
                tokens.push(VdfToken::Text(bare_token));
            }
        }
    }

    tokens
}

fn parse_vdf_tokens(tokens: &[VdfToken], cursor: &mut usize) -> Result<Vec<(String, VdfValue)>, String> {
    let mut entries = Vec::new();

    while *cursor < tokens.len() {
        let Some(token) = tokens.get(*cursor) else {
            break;
        };

        match token {
            VdfToken::CloseBrace => {
                *cursor += 1;
                break;
            }
            VdfToken::OpenBrace => {
                return Err(String::from("Invalid VDF format: unexpected '{'"));
            }
            VdfToken::Text(key) => {
                let key = key.clone();
                *cursor += 1;
                let Some(value_token) = tokens.get(*cursor) else {
                    return Err(format!(
                        "Invalid VDF format: missing value for key '{key}'"
                    ));
                };

                match value_token {
                    VdfToken::Text(value) => {
                        entries.push((key, VdfValue::Text(value.clone())));
                        *cursor += 1;
                    }
                    VdfToken::OpenBrace => {
                        *cursor += 1;
                        let object_value = parse_vdf_tokens(tokens, cursor)?;
                        entries.push((key, VdfValue::Object(object_value)));
                    }
                    VdfToken::CloseBrace => {
                        return Err(format!(
                            "Invalid VDF format: missing value for key '{key}'"
                        ));
                    }
                }
            }
        }
    }

    Ok(entries)
}

fn parse_vdf_document(contents: &str) -> Result<VdfValue, String> {
    let tokens = tokenize_vdf(contents);
    let mut cursor = 0;
    let entries = parse_vdf_tokens(&tokens, &mut cursor)?;
    if cursor < tokens.len() {
        return Err(String::from("Invalid VDF format: trailing tokens"));
    }
    Ok(VdfValue::Object(entries))
}

fn vdf_find_object_value<'a>(value: &'a VdfValue, key: &str) -> Option<&'a VdfValue> {
    let VdfValue::Object(entries) = value else {
        return None;
    };

    entries
        .iter()
        .find(|(entry_key, _)| entry_key.eq_ignore_ascii_case(key))
        .map(|(_, entry_value)| entry_value)
}

fn vdf_collect_objects_by_key<'a>(value: &'a VdfValue, key: &str, output: &mut Vec<&'a VdfValue>) {
    let VdfValue::Object(entries) = value else {
        return;
    };

    for (entry_key, entry_value) in entries {
        if entry_key.eq_ignore_ascii_case(key) && matches!(entry_value, VdfValue::Object(_)) {
            output.push(entry_value);
        }
        vdf_collect_objects_by_key(entry_value, key, output);
    }
}

fn vdf_get_or_insert_object_mut<'a>(value: &'a mut VdfValue, key: &str) -> &'a mut VdfValue {
    if matches!(value, VdfValue::Text(_)) {
        *value = VdfValue::Object(Vec::new());
    }

    let VdfValue::Object(entries) = value else {
        unreachable!()
    };

    if let Some(entry_index) = entries
        .iter()
        .position(|(entry_key, _)| entry_key.eq_ignore_ascii_case(key))
    {
        if !matches!(entries[entry_index].1, VdfValue::Object(_)) {
            entries[entry_index].1 = VdfValue::Object(Vec::new());
        }
        return &mut entries[entry_index].1;
    }

    entries.push((key.to_owned(), VdfValue::Object(Vec::new())));
    let last_index = entries.len() - 1;
    &mut entries[last_index].1
}

fn vdf_ensure_object_path_mut<'a>(value: &'a mut VdfValue, path: &[&str]) -> &'a mut VdfValue {
    if path.is_empty() {
        return value;
    }

    let child = vdf_get_or_insert_object_mut(value, path[0]);
    vdf_ensure_object_path_mut(child, &path[1..])
}

fn vdf_set_text_entry(value: &mut VdfValue, key: &str, text: &str) {
    if matches!(value, VdfValue::Text(_)) {
        *value = VdfValue::Object(Vec::new());
    }

    let VdfValue::Object(entries) = value else {
        unreachable!()
    };
    if let Some(entry_index) = entries
        .iter()
        .position(|(entry_key, _)| entry_key.eq_ignore_ascii_case(key))
    {
        entries[entry_index].1 = VdfValue::Text(text.to_owned());
        return;
    }

    entries.push((key.to_owned(), VdfValue::Text(text.to_owned())));
}

fn vdf_remove_entry(value: &mut VdfValue, key: &str) {
    let VdfValue::Object(entries) = value else {
        return;
    };
    entries.retain(|(entry_key, _)| !entry_key.eq_ignore_ascii_case(key));
}

fn escape_vdf_text(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }

    escaped
}

fn serialize_vdf_entry(key: &str, value: &VdfValue, depth: usize, output: &mut String) {
    let indent = "\t".repeat(depth);
    output.push_str(&indent);
    output.push('"');
    output.push_str(&escape_vdf_text(key));
    output.push('"');

    match value {
        VdfValue::Text(text) => {
            output.push('\t');
            output.push('"');
            output.push_str(&escape_vdf_text(text));
            output.push('"');
            output.push('\n');
        }
        VdfValue::Object(entries) => {
            output.push('\n');
            output.push_str(&indent);
            output.push_str("{\n");
            for (entry_key, entry_value) in entries {
                serialize_vdf_entry(entry_key, entry_value, depth + 1, output);
            }
            output.push_str(&indent);
            output.push_str("}\n");
        }
    }
}

fn serialize_vdf_document(value: &VdfValue) -> String {
    let mut output = String::new();
    match value {
        VdfValue::Object(entries) => {
            for (entry_key, entry_value) in entries {
                serialize_vdf_entry(entry_key, entry_value, 0, &mut output);
            }
        }
        VdfValue::Text(text) => {
            output.push('"');
            output.push_str(&escape_vdf_text(text));
            output.push('"');
            output.push('\n');
        }
    }

    output
}

fn parse_collection_name_candidate(raw_value: &str) -> Option<String> {
    let normalized = raw_value.replace('\0', "");
    let normalized = normalized.trim();
    if normalized.is_empty() {
        return None;
    }

    let lowered = normalized.to_ascii_lowercase();
    if matches!(lowered.as_str(), "0" | "1" | "true" | "false") {
        return None;
    }
    if normalized.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }

    Some(normalized.to_owned())
}

fn vdf_collect_text_leaves(value: &VdfValue, output: &mut Vec<String>) {
    match value {
        VdfValue::Text(text) => output.push(text.clone()),
        VdfValue::Object(entries) => {
            for (_, entry_value) in entries {
                vdf_collect_text_leaves(entry_value, output);
            }
        }
    }
}

fn parse_steam_collections_from_vdf(
    contents: &str,
) -> Result<HashMap<String, HashSet<String>>, String> {
    let root_value = parse_vdf_document(contents)?;
    let mut collections_by_app_id = HashMap::new();
    let mut apps_objects = Vec::new();
    vdf_collect_objects_by_key(&root_value, "apps", &mut apps_objects);

    for apps_value in apps_objects {
        let VdfValue::Object(app_entries) = apps_value else {
            continue;
        };

        for (app_id, app_value) in app_entries {
            let normalized_app_id = app_id.trim_matches(|character: char| {
                character.is_whitespace() || character == '\0'
            });
            if normalized_app_id.is_empty()
                || !normalized_app_id
                    .chars()
                    .all(|character| character.is_ascii_digit())
            {
                continue;
            }

            let Some(VdfValue::Object(tag_entries)) = vdf_find_object_value(app_value, "tags") else {
                continue;
            };
            let mut collection_names = HashSet::new();
            for (tag_key, tag_value) in tag_entries {
                if let Some(collection_name) = parse_collection_name_candidate(tag_key) {
                    collection_names.insert(collection_name);
                }
                let mut tag_value_text_candidates = Vec::new();
                vdf_collect_text_leaves(tag_value, &mut tag_value_text_candidates);
                for candidate in tag_value_text_candidates {
                    if let Some(collection_name) = parse_collection_name_candidate(&candidate) {
                        collection_names.insert(collection_name);
                    }
                }
            }

            if !collection_names.is_empty() {
                collections_by_app_id
                    .entry(normalized_app_id.to_owned())
                    .or_insert_with(HashSet::new)
                    .extend(collection_names);
            }
        }
    }

    Ok(collections_by_app_id)
}

fn merge_collections_by_app_id(
    target: &mut HashMap<String, HashSet<String>>,
    source: HashMap<String, HashSet<String>>,
) {
    for (app_id, collections) in source {
        target
            .entry(app_id)
            .or_insert_with(HashSet::new)
            .extend(collections);
    }
}

fn import_steam_collections_for_user(
    connection: &Connection,
    user_id: &str,
    collections_by_app_id: HashMap<String, HashSet<String>>,
) -> Result<SteamCollectionsImportResponse, String> {
    let owned_steam_game_external_ids = load_provider_game_external_ids(connection, user_id, "steam")?;
    let mut collection_ids_by_name: HashMap<String, String> = HashMap::new();
    let mut apps_tagged = 0usize;
    let mut collections_created = 0usize;
    let mut memberships_added = 0usize;
    let mut skipped_games = 0usize;
    let mut tags_discovered = 0usize;

    for (external_id, collection_names) in collections_by_app_id {
        apps_tagged += 1;
        for collection_name in collection_names {
            tags_discovered += 1;

            if !owned_steam_game_external_ids.contains(&external_id) {
                skipped_games += 1;
                continue;
            }

            let normalized_key = collection_name.trim().to_ascii_lowercase();
            if normalized_key.is_empty() {
                continue;
            }

            let collection_id = if let Some(existing_collection_id) =
                collection_ids_by_name.get(&normalized_key)
            {
                existing_collection_id.clone()
            } else {
                let (collection_id, created) =
                    get_or_create_collection_id_by_name(connection, user_id, &collection_name)?;
                if created {
                    collections_created += 1;
                }
                collection_ids_by_name.insert(normalized_key, collection_id.clone());
                collection_id
            };

            if add_game_to_collection_membership(
                connection,
                user_id,
                &collection_id,
                "steam",
                &external_id,
            )? {
                memberships_added += 1;
            }
        }
    }

    Ok(SteamCollectionsImportResponse {
        apps_tagged,
        collections_created,
        memberships_added,
        skipped_games,
        tags_discovered,
    })
}

fn encode_steam_launch_options(launch_options: &str) -> String {
    url::form_urlencoded::byte_serialize(launch_options.as_bytes()).collect::<String>()
}

fn try_spawn_command(command: &str, args: &[&str]) -> Result<(), String> {
    Command::new(command)
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|error| {
            let rendered_args = if args.is_empty() {
                String::new()
            } else {
                format!(" {}", args.join(" "))
            };
            format!("{command}{rendered_args}: {error}")
        })
}

fn launch_steam_uri(uri: &str, action: &str) -> Result<(), String> {
    let install_action = action.eq_ignore_ascii_case("install");

    if cfg!(target_os = "windows") {
        let mut errors = Vec::new();

        if install_action {
            match try_spawn_command("cmd", &["/C", "start", "", "/MIN", "steam", "-silent", uri]) {
                Ok(()) => return Ok(()),
                Err(error) => errors.push(error),
            }
            let _ = try_spawn_command("cmd", &["/C", "start", "", "/MIN", "steam", "-silent"]);
            match try_spawn_command("cmd", &["/C", "start", "", "/MIN", uri]) {
                Ok(()) => return Ok(()),
                Err(error) => errors.push(error),
            }
        } else {
            match try_spawn_command("cmd", &["/C", "start", "", uri]) {
                Ok(()) => return Ok(()),
                Err(error) => errors.push(error),
            }
        }

        return Err(format!(
            "Failed to launch Steam URI '{uri}' on Windows. Attempts: {}",
            errors.join("; ")
        ));
    }

    if cfg!(target_os = "macos") {
        let mut errors = Vec::new();

        if install_action {
            let _ = try_spawn_command("open", &["-g", "-j", "-a", "Steam"]);
            match try_spawn_command("open", &["-g", uri]) {
                Ok(()) => return Ok(()),
                Err(error) => errors.push(error),
            }
        }

        match try_spawn_command("open", &[uri]) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
        }

        return Err(format!(
            "Failed to launch Steam URI '{uri}' on macOS. Attempts: {}",
            errors.join("; ")
        ));
    }

    if cfg!(target_os = "linux") {
        let mut errors = Vec::new();

        if install_action {
            match try_spawn_command("steam", &["-silent", uri]) {
                Ok(()) => return Ok(()),
                Err(error) => errors.push(error),
            }

            match try_spawn_command("steam-runtime", &["-silent", uri]) {
                Ok(()) => return Ok(()),
                Err(error) => errors.push(error),
            }

            match try_spawn_command("flatpak", &["run", "com.valvesoftware.Steam", "-silent", uri]) {
                Ok(()) => return Ok(()),
                Err(error) => errors.push(error),
            }

            let _ = try_spawn_command("steam", &["-silent"]);
            let _ = try_spawn_command("steam-runtime", &["-silent"]);
            let _ = try_spawn_command("flatpak", &["run", "com.valvesoftware.Steam", "-silent"]);
        }

        match try_spawn_command("steam", &[uri]) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
        }

        match try_spawn_command("steam-runtime", &[uri]) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
        }

        match try_spawn_command("flatpak", &["run", "com.valvesoftware.Steam", uri]) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
        }

        return Err(format!(
            "Could not open Steam URI '{uri}'. Make sure Steam is installed and available in PATH. Attempts: {}",
            errors.join("; ")
        ));
    }

    webbrowser::open(uri)
        .map(|_| ())
        .map_err(|error| format!("Failed to open Steam URI '{uri}': {error}"))
}

fn open_provider_game_uri(
    provider: &str,
    external_id: &str,
    action: &str,
    launch_options: Option<&str>,
) -> Result<(), String> {
    match provider {
        "steam" => {
            let app_id = external_id
                .parse::<u64>()
                .map_err(|_| String::from("Steam external_id must be a numeric app ID"))?;
            let uri = match action {
                "play" => match launch_options {
                    Some(value) => {
                        let encoded_options = encode_steam_launch_options(value);
                        format!("steam://run/{app_id}//{encoded_options}/")
                    }
                    None => format!("steam://run/{app_id}"),
                },
                "install" => format!("steam://install/{app_id}"),
                "validate" => format!("steam://validate/{app_id}"),
                "backup" => format!("steam://backup/{app_id}"),
                _ => return Err(String::from("Unsupported Steam action")),
            };

            launch_steam_uri(&uri, action)
        }
        _ => Err(format!(
            "Provider '{provider}' is not supported for action '{action}'"
        )),
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
        game_versions_betas: GameVersionsBetasSettingsPayload {
            private_access_code: String::new(),
            selected_version_id: String::from("public"),
        },
    }
}

fn normalize_game_properties_mode(value: String, allowed_modes: &[&str], fallback_mode: &str) -> String {
    let trimmed_value = value.trim();
    if trimmed_value.is_empty() {
        return fallback_mode.to_owned();
    }

    for allowed_mode in allowed_modes {
        if allowed_mode.eq_ignore_ascii_case(trimmed_value) {
            return (*allowed_mode).to_owned();
        }
    }

    fallback_mode.to_owned()
}

fn normalize_game_properties_settings_payload(
    settings: GamePropertiesSettingsPayload,
) -> GamePropertiesSettingsPayload {
    let defaults = default_game_properties_settings_payload();

    let language = settings.general.language.trim();
    let compatibility_tool = settings.compatibility.steam_play_compatibility_tool.trim();
    let private_access_code = settings.game_versions_betas.private_access_code.trim();
    let selected_version_id = settings.game_versions_betas.selected_version_id.trim();
    GamePropertiesSettingsPayload {
        general: GameGeneralSettingsPayload {
            language: if language.is_empty() {
                defaults.general.language
            } else {
                language.to_owned()
            },
            launch_options: settings.general.launch_options.trim().to_owned(),
            steam_overlay_enabled: settings.general.steam_overlay_enabled,
        },
        compatibility: GameCompatibilitySettingsPayload {
            force_steam_play_compatibility_tool: settings
                .compatibility
                .force_steam_play_compatibility_tool,
            steam_play_compatibility_tool: if compatibility_tool.is_empty() {
                defaults.compatibility.steam_play_compatibility_tool
            } else {
                compatibility_tool.to_owned()
            },
        },
        updates: GameUpdatesSettingsPayload {
            automatic_updates_mode: normalize_game_properties_mode(
                settings.updates.automatic_updates_mode,
                &[
                    "use-global-setting",
                    "wait-until-launch",
                    "let-steam-decide",
                    "immediately-download",
                ],
                &defaults.updates.automatic_updates_mode,
            ),
            background_downloads_mode: normalize_game_properties_mode(
                settings.updates.background_downloads_mode,
                &[
                    "pause-while-playing-global",
                    "always-allow",
                    "never-allow",
                ],
                &defaults.updates.background_downloads_mode,
            ),
        },
        controller: GameControllerSettingsPayload {
            steam_input_override: normalize_game_properties_mode(
                settings.controller.steam_input_override,
                &[
                    "use-default-settings",
                    "disable-steam-input",
                    "enable-steam-input",
                ],
                &defaults.controller.steam_input_override,
            ),
        },
        game_versions_betas: GameVersionsBetasSettingsPayload {
            private_access_code: private_access_code.to_owned(),
            selected_version_id: if selected_version_id.is_empty() {
                defaults.game_versions_betas.selected_version_id
            } else {
                selected_version_id.to_owned()
            },
        },
    }
}

fn load_game_properties_settings(
    connection: &Connection,
    user_id: &str,
    provider: &str,
    external_id: &str,
) -> Result<GamePropertiesSettingsPayload, String> {
    let row = connection
        .query_row(
            "
            SELECT settings_json
            FROM game_properties_settings
            WHERE user_id = ?1 AND provider = ?2 AND external_id = ?3
            ",
            params![user_id, provider, external_id],
            |record| record.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("Failed to query game properties settings: {error}"))?;

    let Some(settings_json) = row else {
        return Ok(default_game_properties_settings_payload());
    };
    let parsed_settings = serde_json::from_str::<GamePropertiesSettingsPayload>(&settings_json)
        .unwrap_or_else(|_| default_game_properties_settings_payload());
    Ok(normalize_game_properties_settings_payload(parsed_settings))
}

fn save_game_properties_settings(
    connection: &Connection,
    user_id: &str,
    provider: &str,
    external_id: &str,
    settings: &GamePropertiesSettingsPayload,
) -> Result<(), String> {
    let serialized_settings = serde_json::to_string(settings)
        .map_err(|error| format!("Failed to serialize game properties settings: {error}"))?;
    connection
        .execute(
            "
            INSERT INTO game_properties_settings (
              user_id,
              provider,
              external_id,
              settings_json,
              updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(user_id, provider, external_id) DO UPDATE SET
              settings_json = excluded.settings_json,
              updated_at = excluded.updated_at
            ",
            params![
                user_id,
                provider,
                external_id,
                serialized_settings,
                Utc::now().to_rfc3339(),
            ],
        )
        .map_err(|error| format!("Failed to persist game properties settings: {error}"))?;

    Ok(())
}

fn map_compatibility_tool_label_to_steam_name(label: &str) -> String {
    let trimmed_label = label.trim();
    if trimmed_label.is_empty() {
        return String::new();
    }

    let normalized = trimmed_label.to_ascii_lowercase();
    for (tool_id, display_label) in STEAM_BUILTIN_COMPATIBILITY_TOOLS {
        if normalized == tool_id.to_ascii_lowercase()
            || normalized == display_label.to_ascii_lowercase()
        {
            return tool_id.to_owned();
        }
    }

    trimmed_label.to_owned()
}

fn default_steam_compatibility_tools() -> Vec<GameCompatibilityToolResponse> {
    STEAM_BUILTIN_COMPATIBILITY_TOOLS
        .iter()
        .map(|(id, label)| GameCompatibilityToolResponse {
            id: (*id).to_owned(),
            label: (*label).to_owned(),
        })
        .collect::<Vec<_>>()
}

fn is_linux_runtime_compatibility_tool(tool: &GameCompatibilityToolResponse) -> bool {
    let normalized_id = tool.id.trim().to_ascii_lowercase();
    if normalized_id == "sniper" || normalized_id == "soldier" {
        return true;
    }

    let normalized_label = tool.label.trim().to_ascii_lowercase();
    normalized_label.starts_with("steam linux runtime")
}

fn add_compatibility_tool_option(
    tools: &mut Vec<GameCompatibilityToolResponse>,
    seen_ids: &mut HashSet<String>,
    id: &str,
    label: &str,
) {
    let normalized_id = id.trim();
    if normalized_id.is_empty() {
        return;
    }

    let normalized_label = if label.trim().is_empty() {
        normalized_id
    } else {
        label.trim()
    };
    let dedupe_key = normalized_id.to_ascii_lowercase();
    if seen_ids.insert(dedupe_key) {
        tools.push(GameCompatibilityToolResponse {
            id: normalized_id.to_owned(),
            label: normalized_label.to_owned(),
        });
    }
}

fn compatibility_tool_from_common_directory_name(
    directory_name: &str,
) -> Option<GameCompatibilityToolResponse> {
    let trimmed_name = directory_name.trim();
    if trimmed_name.is_empty() {
        return None;
    }

    let normalized_name = trimmed_name.to_ascii_lowercase();
    if !normalized_name.starts_with("proton")
        && !normalized_name.starts_with("steam linux runtime")
    {
        return None;
    }

    Some(GameCompatibilityToolResponse {
        id: map_compatibility_tool_label_to_steam_name(trimmed_name),
        label: trimmed_name.to_owned(),
    })
}

fn parse_steam_custom_compatibility_tools_from_vdf(
    contents: &str,
) -> Result<Vec<GameCompatibilityToolResponse>, String> {
    let root_value = parse_vdf_document(contents)?;
    let compat_tools_value = vdf_find_object_value(&root_value, "compatibilitytools")
        .and_then(|compatibility_tools| vdf_find_object_value(compatibility_tools, "compat_tools"))
        .or_else(|| vdf_find_object_value(&root_value, "compat_tools"));
    let Some(VdfValue::Object(tool_entries)) = compat_tools_value else {
        return Ok(Vec::new());
    };

    let mut parsed_tools = Vec::new();
    let mut seen_ids = HashSet::new();
    for (tool_key, tool_value) in tool_entries {
        let tool_id = tool_key.trim();
        if tool_id.is_empty() {
            continue;
        }

        let display_label = vdf_find_object_value(tool_value, "display_name")
            .and_then(|display_name_value| match display_name_value {
                VdfValue::Text(display_name_text) => {
                    let trimmed_display_name = display_name_text.trim();
                    if trimmed_display_name.is_empty() {
                        None
                    } else {
                        Some(trimmed_display_name.to_owned())
                    }
                }
                VdfValue::Object(_) => None,
            })
            .unwrap_or_else(|| tool_id.to_owned());

        add_compatibility_tool_option(
            &mut parsed_tools,
            &mut seen_ids,
            tool_id,
            &display_label,
        );
    }

    Ok(parsed_tools)
}

fn resolve_steam_compatibility_tools(
    steam_root_override: Option<&str>,
    include_linux_runtime_tools: bool,
) -> Result<Vec<GameCompatibilityToolResponse>, String> {
    let mut tools = Vec::new();
    let mut seen_ids = HashSet::new();
    for builtin_tool in default_steam_compatibility_tools() {
        if !include_linux_runtime_tools && is_linux_runtime_compatibility_tool(&builtin_tool) {
            continue;
        }
        add_compatibility_tool_option(
            &mut tools,
            &mut seen_ids,
            &builtin_tool.id,
            &builtin_tool.label,
        );
    }

    let Some(steam_root) = resolve_steam_root_path(steam_root_override) else {
        return Ok(tools);
    };

    let common_path = steam_root.join("steamapps").join("common");
    if let Ok(common_entries) = fs::read_dir(&common_path) {
        for common_entry in common_entries.flatten() {
            let Ok(file_type) = common_entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }

            let directory_name = common_entry.file_name().to_string_lossy().trim().to_owned();
            let Some(parsed_tool) = compatibility_tool_from_common_directory_name(&directory_name)
            else {
                continue;
            };
            if !include_linux_runtime_tools && is_linux_runtime_compatibility_tool(&parsed_tool) {
                continue;
            }
            add_compatibility_tool_option(
                &mut tools,
                &mut seen_ids,
                &parsed_tool.id,
                &parsed_tool.label,
            );
        }
    }

    let custom_tools_path = steam_root.join("compatibilitytools.d");
    if let Ok(custom_tool_entries) = fs::read_dir(&custom_tools_path) {
        for custom_tool_entry in custom_tool_entries.flatten() {
            let Ok(file_type) = custom_tool_entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }

            let entry_path = custom_tool_entry.path();
            let compatibility_tool_vdf_path = entry_path.join("compatibilitytool.vdf");
            let mut discovered_any_tool_from_vdf = false;
            if compatibility_tool_vdf_path.is_file() {
                if let Ok(contents) = fs::read_to_string(&compatibility_tool_vdf_path) {
                    if let Ok(parsed_tools) =
                        parse_steam_custom_compatibility_tools_from_vdf(&contents)
                    {
                        for parsed_tool in parsed_tools {
                            if !include_linux_runtime_tools
                                && is_linux_runtime_compatibility_tool(&parsed_tool)
                            {
                                continue;
                            }
                            add_compatibility_tool_option(
                                &mut tools,
                                &mut seen_ids,
                                &parsed_tool.id,
                                &parsed_tool.label,
                            );
                            discovered_any_tool_from_vdf = true;
                        }
                    }
                }
            }

            if discovered_any_tool_from_vdf {
                continue;
            }

            let fallback_name = custom_tool_entry.file_name().to_string_lossy().trim().to_owned();
            if fallback_name.is_empty() {
                continue;
            }
            let fallback_tool = GameCompatibilityToolResponse {
                id: fallback_name.clone(),
                label: fallback_name.clone(),
            };
            if !include_linux_runtime_tools && is_linux_runtime_compatibility_tool(&fallback_tool) {
                continue;
            }

            add_compatibility_tool_option(
                &mut tools,
                &mut seen_ids,
                &fallback_name,
                &fallback_name,
            );
        }
    }

    Ok(tools)
}

fn log_steam_settings_debug(state: &AppState, message: &str) {
    if state.steam_settings_debug_logging {
        eprintln!("[catalyst:steam-settings] {message}");
    }
}

fn apply_steam_game_properties_settings(
    state: &AppState,
    user: &UserRow,
    app_id: u64,
    settings: &GamePropertiesSettingsPayload,
) -> Result<(), String> {
    let steam_id = user
        .steam_id
        .as_deref()
        .ok_or_else(|| String::from("Steam is not linked for this account"))?;
    let localconfig_path = resolve_steam_localconfig_path(state.steam_root_override.as_deref(), steam_id)?;
    log_steam_settings_debug(
        state,
        &format!(
            "Applying settings for app {} using localconfig {}",
            app_id,
            localconfig_path.display()
        ),
    );
    let localconfig_contents = fs::read_to_string(&localconfig_path).map_err(|error| {
        format!(
            "Failed to read Steam localconfig at {}: {error}",
            localconfig_path.display()
        )
    })?;
    let mut localconfig_value = parse_vdf_document(&localconfig_contents)?;

    let app_id_key = app_id.to_string();
    let apps_object = vdf_ensure_object_path_mut(
        &mut localconfig_value,
        &["UserLocalConfigStore", "Software", "Valve", "Steam", "apps"],
    );
    let app_settings_object = vdf_ensure_object_path_mut(apps_object, &[app_id_key.as_str()]);

    let launch_options = settings.general.launch_options.trim();
    if launch_options.is_empty() {
        vdf_remove_entry(app_settings_object, "LaunchOptions");
        log_steam_settings_debug(state, &format!("app {}: cleared LaunchOptions", app_id));
    } else {
        vdf_set_text_entry(app_settings_object, "LaunchOptions", launch_options);
        log_steam_settings_debug(
            state,
            &format!("app {}: set LaunchOptions to {:?}", app_id, launch_options),
        );
    }

    match settings.updates.automatic_updates_mode.as_str() {
        "use-global-setting" => {
            vdf_remove_entry(app_settings_object, "AutoUpdateBehavior");
            log_steam_settings_debug(state, &format!("app {}: cleared AutoUpdateBehavior", app_id));
        }
        "wait-until-launch" => {
            vdf_set_text_entry(app_settings_object, "AutoUpdateBehavior", "1");
            log_steam_settings_debug(state, &format!("app {}: set AutoUpdateBehavior=1", app_id));
        }
        "let-steam-decide" => {
            vdf_set_text_entry(app_settings_object, "AutoUpdateBehavior", "0");
            log_steam_settings_debug(state, &format!("app {}: set AutoUpdateBehavior=0", app_id));
        }
        "immediately-download" => {
            vdf_set_text_entry(app_settings_object, "AutoUpdateBehavior", "2");
            log_steam_settings_debug(state, &format!("app {}: set AutoUpdateBehavior=2", app_id));
        }
        _ => {}
    }

    match settings.updates.background_downloads_mode.as_str() {
        "pause-while-playing-global" => {
            vdf_remove_entry(app_settings_object, "AllowDownloadsWhileRunning");
            log_steam_settings_debug(
                state,
                &format!("app {}: cleared AllowDownloadsWhileRunning", app_id),
            );
        }
        "always-allow" => {
            vdf_set_text_entry(app_settings_object, "AllowDownloadsWhileRunning", "1");
            log_steam_settings_debug(
                state,
                &format!("app {}: set AllowDownloadsWhileRunning=1", app_id),
            );
        }
        "never-allow" => {
            vdf_set_text_entry(app_settings_object, "AllowDownloadsWhileRunning", "0");
            log_steam_settings_debug(
                state,
                &format!("app {}: set AllowDownloadsWhileRunning=0", app_id),
            );
        }
        _ => {}
    }

    match settings.controller.steam_input_override.as_str() {
        "use-default-settings" => {
            vdf_remove_entry(app_settings_object, "SteamInput");
            log_steam_settings_debug(state, &format!("app {}: cleared SteamInput", app_id));
        }
        "disable-steam-input" => {
            vdf_set_text_entry(app_settings_object, "SteamInput", "0");
            log_steam_settings_debug(state, &format!("app {}: set SteamInput=0", app_id));
        }
        "enable-steam-input" => {
            vdf_set_text_entry(app_settings_object, "SteamInput", "1");
            log_steam_settings_debug(state, &format!("app {}: set SteamInput=1", app_id));
        }
        _ => {}
    }

    let compat_mapping_object = vdf_ensure_object_path_mut(
        &mut localconfig_value,
        &[
            "UserLocalConfigStore",
            "Software",
            "Valve",
            "Steam",
            "CompatToolMapping",
        ],
    );
    if settings.compatibility.force_steam_play_compatibility_tool {
        let compat_mapping_entry = vdf_ensure_object_path_mut(compat_mapping_object, &[app_id_key.as_str()]);
        let compat_name = map_compatibility_tool_label_to_steam_name(
            &settings.compatibility.steam_play_compatibility_tool,
        );
        if compat_name.is_empty() {
            vdf_remove_entry(compat_mapping_object, &app_id_key);
            log_steam_settings_debug(
                state,
                &format!("app {}: cleared CompatToolMapping entry (empty compat name)", app_id),
            );
        } else {
            vdf_set_text_entry(compat_mapping_entry, "name", &compat_name);
            vdf_set_text_entry(compat_mapping_entry, "config", "");
            vdf_set_text_entry(compat_mapping_entry, "priority", "250");
            log_steam_settings_debug(
                state,
                &format!(
                    "app {}: set CompatToolMapping name={:?}, priority=250",
                    app_id, compat_name
                ),
            );
        }
    } else {
        vdf_remove_entry(compat_mapping_object, &app_id_key);
        log_steam_settings_debug(
            state,
            &format!("app {}: removed CompatToolMapping override", app_id),
        );
    }

    let serialized_localconfig = serialize_vdf_document(&localconfig_value);
    fs::write(&localconfig_path, serialized_localconfig).map_err(|error| {
        format!(
            "Failed to write Steam localconfig at {}: {error}",
            localconfig_path.display()
        )
    })?;
    log_steam_settings_debug(
        state,
        &format!("app {}: wrote Steam localconfig successfully", app_id),
    );
    Ok(())
}

fn load_game_privacy_settings(
    connection: &Connection,
    user_id: &str,
    provider: &str,
    external_id: &str,
) -> Result<GamePrivacySettingsResponse, String> {
    let row = connection
        .query_row(
            "
            SELECT hide_in_library, mark_as_private, overlay_data_deleted
            FROM game_privacy_settings
            WHERE user_id = ?1 AND provider = ?2 AND external_id = ?3
            ",
            params![user_id, provider, external_id],
            |record| {
                Ok(GamePrivacySettingsResponse {
                    hide_in_library: record.get::<_, i64>(0)? != 0,
                    mark_as_private: record.get::<_, i64>(1)? != 0,
                    overlay_data_deleted: record.get::<_, i64>(2)? != 0,
                })
            },
        )
        .optional()
        .map_err(|error| format!("Failed to query game privacy settings: {error}"))?;

    Ok(row.unwrap_or(GamePrivacySettingsResponse {
        hide_in_library: false,
        mark_as_private: false,
        overlay_data_deleted: false,
    }))
}

fn save_game_privacy_settings(
    connection: &Connection,
    user_id: &str,
    provider: &str,
    external_id: &str,
    settings: GamePrivacySettingsResponse,
) -> Result<(), String> {
    connection
        .execute(
            "
            INSERT INTO game_privacy_settings (
              user_id,
              provider,
              external_id,
              hide_in_library,
              mark_as_private,
              overlay_data_deleted,
              updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(user_id, provider, external_id) DO UPDATE SET
              hide_in_library = excluded.hide_in_library,
              mark_as_private = excluded.mark_as_private,
              overlay_data_deleted = excluded.overlay_data_deleted,
              updated_at = excluded.updated_at
            ",
            params![
                user_id,
                provider,
                external_id,
                if settings.hide_in_library { 1 } else { 0 },
                if settings.mark_as_private { 1 } else { 0 },
                if settings.overlay_data_deleted { 1 } else { 0 },
                Utc::now().to_rfc3339(),
            ],
        )
        .map_err(|error| format!("Failed to persist game privacy settings: {error}"))?;

    Ok(())
}

fn get_authenticated_user(state: &AppState, connection: &Connection) -> Result<UserRow, String> {
    let session_token =
        get_state_session_token(state)?.ok_or_else(|| String::from("Not authenticated"))?;
    let user = find_user_by_session_token(connection, &session_token)?;

    match user {
        Some(user_row) => Ok(user_row),
        None => {
            clear_active_session(state)?;
            Err(String::from("Session expired or invalid"))
        }
    }
}

fn find_auth_user_by_email(
    connection: &Connection,
    email: &str,
) -> Result<Option<AuthUserRow>, String> {
    connection
        .query_row(
            "SELECT id, email, password_hash, steam_id FROM users WHERE email = ?1",
            params![email],
            |row| {
                Ok(AuthUserRow {
                    user: UserRow {
                        id: row.get(0)?,
                        email: row.get(1)?,
                        steam_id: row.get(3)?,
                    },
                    password_hash: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(|error| format!("Failed to query user by email: {error}"))
}

fn find_user_by_id(connection: &Connection, user_id: &str) -> Result<Option<UserRow>, String> {
    connection
        .query_row(
            "SELECT id, email, steam_id FROM users WHERE id = ?1",
            params![user_id],
            |row| {
                Ok(UserRow {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    steam_id: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(|error| format!("Failed to query user by ID: {error}"))
}

fn find_user_by_steam_id(
    connection: &Connection,
    steam_id: &str,
) -> Result<Option<UserRow>, String> {
    connection
        .query_row(
            "SELECT id, email, steam_id FROM users WHERE steam_id = ?1",
            params![steam_id],
            |row| {
                Ok(UserRow {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    steam_id: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(|error| format!("Failed to query user by Steam ID: {error}"))
}

fn create_user(
    connection: &Connection,
    email: &str,
    password_hash: &str,
    steam_id: Option<&str>,
) -> Result<UserRow, String> {
    let user_id = Uuid::new_v4().to_string();
    let timestamp = Utc::now().to_rfc3339();

    connection
        .execute(
            "INSERT INTO users (id, email, password_hash, steam_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![user_id, email, password_hash, steam_id, timestamp, timestamp],
        )
        .map_err(|error| format!("Failed to create user: {error}"))?;

    find_user_by_id(connection, &user_id)?
        .ok_or_else(|| String::from("Failed to load newly created user"))
}

fn create_steam_user(connection: &Connection, steam_id: &str) -> Result<UserRow, String> {
    let placeholder_email = format!("steam_{}@steam.local", Uuid::new_v4().simple());
    let placeholder_password_hash = hash(Uuid::new_v4().to_string(), DEFAULT_COST)
        .map_err(|error| format!("Failed to hash placeholder Steam password: {error}"))?;
    create_user(
        connection,
        &placeholder_email,
        &placeholder_password_hash,
        Some(steam_id),
    )
}

fn set_user_steam_id(
    connection: &Connection,
    user_id: &str,
    steam_id: &str,
) -> Result<UserRow, String> {
    if let Some(existing_user) = find_user_by_steam_id(connection, steam_id)? {
        if existing_user.id != user_id {
            return Err(String::from(
                "Steam account is already linked to another user",
            ));
        }
        return Ok(existing_user);
    }

    let updated_at = Utc::now().to_rfc3339();
    let changed = connection
        .execute(
            "UPDATE users SET steam_id = ?1, updated_at = ?2 WHERE id = ?3",
            params![steam_id, updated_at, user_id],
        )
        .map_err(|error| format!("Failed to update Steam link for user: {error}"))?;

    if changed == 0 {
        return Err(String::from("User not found"));
    }

    find_user_by_id(connection, user_id)?.ok_or_else(|| String::from("Failed to load updated user"))
}

fn create_session(connection: &Connection, user_id: &str) -> Result<String, String> {
    let now = Utc::now();
    let expires_at = now + ChronoDuration::days(SESSION_TTL_DAYS);
    let session_token = format!("{}.{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let token_hash = hash_session_token(&session_token);

    connection
        .execute(
            "INSERT INTO sessions (token_hash, user_id, created_at, expires_at, last_seen_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                token_hash,
                user_id,
                now.to_rfc3339(),
                expires_at.to_rfc3339(),
                now.to_rfc3339()
            ],
        )
        .map_err(|error| format!("Failed to create session: {error}"))?;

    Ok(session_token)
}

fn find_user_by_session_token(
    connection: &Connection,
    session_token: &str,
) -> Result<Option<UserRow>, String> {
    let token_hash = hash_session_token(session_token);
    let now = Utc::now().to_rfc3339();

    let user = connection
        .query_row(
            "SELECT u.id, u.email, u.steam_id FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.token_hash = ?1 AND s.expires_at > ?2",
            params![token_hash, now],
            |row| {
                Ok(UserRow {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    steam_id: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(|error| format!("Failed to query session user: {error}"))?;

    if user.is_some() {
        connection
            .execute(
                "UPDATE sessions SET last_seen_at = ?1 WHERE token_hash = ?2",
                params![Utc::now().to_rfc3339(), token_hash],
            )
            .map_err(|error| format!("Failed to touch session: {error}"))?;
    }

    Ok(user)
}

fn invalidate_session_by_token(connection: &Connection, session_token: &str) -> Result<(), String> {
    let token_hash = hash_session_token(session_token);
    connection
        .execute(
            "DELETE FROM sessions WHERE token_hash = ?1",
            params![token_hash],
        )
        .map_err(|error| format!("Failed to invalidate session: {error}"))?;
    Ok(())
}

fn cleanup_expired_sessions(connection: &Connection) -> Result<(), String> {
    connection
        .execute(
            "DELETE FROM sessions WHERE expires_at <= ?1",
            params![Utc::now().to_rfc3339()],
        )
        .map_err(|error| format!("Failed to cleanup expired sessions: {error}"))?;
    Ok(())
}

fn hash_session_token(session_token: &str) -> String {
    let digest = Sha256::digest(session_token.as_bytes());
    format!("{digest:x}")
}

fn get_state_session_token(state: &AppState) -> Result<Option<String>, String> {
    let guard = state
        .current_session_token
        .lock()
        .map_err(|_| String::from("Failed to acquire session token lock"))?;
    Ok(guard.clone())
}

fn set_state_session_token(state: &AppState, session_token: Option<String>) -> Result<(), String> {
    let mut guard = state
        .current_session_token
        .lock()
        .map_err(|_| String::from("Failed to acquire session token lock"))?;
    *guard = session_token;
    Ok(())
}

fn persist_active_session(state: &AppState, session_token: &str) -> Result<(), String> {
    persist_session_token(&state.session_token_path, session_token)?;
    set_state_session_token(state, Some(session_token.to_owned()))
}

fn clear_active_session(state: &AppState) -> Result<(), String> {
    clear_session_token_file(&state.session_token_path)?;
    set_state_session_token(state, None)
}

fn restore_persisted_session(state: &AppState) -> Result<(), String> {
    let Some(session_token) = read_session_token(&state.session_token_path)? else {
        return Ok(());
    };

    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;

    if find_user_by_session_token(&connection, &session_token)?.is_some() {
        set_state_session_token(state, Some(session_token))
    } else {
        clear_active_session(state)
    }
}

fn read_session_token(session_path: &Path) -> Result<Option<String>, String> {
    let content = match fs::read_to_string(session_path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Failed to read session token file: {error}")),
    };

    let token = content.trim().to_owned();
    if token.is_empty() {
        clear_session_token_file(session_path)?;
        return Ok(None);
    }

    Ok(Some(token))
}

fn persist_session_token(session_path: &Path, session_token: &str) -> Result<(), String> {
    if let Some(parent) = session_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create session directory: {error}"))?;
    }

    fs::write(session_path, session_token)
        .map_err(|error| format!("Failed to write session token file: {error}"))
}

fn clear_session_token_file(session_path: &Path) -> Result<(), String> {
    match fs::remove_file(session_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Failed to clear session token file: {error}")),
    }
}

fn build_http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| format!("Failed to initialize HTTP client: {error}"))
}

fn normalize_email(email: &str) -> Result<String, String> {
    let normalized = email.trim().to_lowercase();
    if !is_email_like(&normalized) {
        return Err(String::from("Invalid email format"));
    }
    Ok(normalized)
}

fn validate_password(password: &str) -> Result<(), String> {
    let length = password.chars().count();
    if !(8..=128).contains(&length) {
        return Err(String::from(
            "Password must be between 8 and 128 characters",
        ));
    }
    Ok(())
}

fn is_email_like(value: &str) -> bool {
    if value.is_empty() || value.contains(char::is_whitespace) {
        return false;
    }

    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };

    if local.is_empty()
        || domain.is_empty()
        || domain.starts_with('.')
        || domain.ends_with('.')
        || !domain.contains('.')
    {
        return false;
    }

    true
}

fn public_user_from_row(user: &UserRow) -> PublicUser {
    PublicUser {
        id: user.id.clone(),
        email: user.email.clone(),
        steam_linked: user.steam_id.is_some(),
        steam_id: user.steam_id.clone(),
    }
}

fn open_connection(db_path: &Path) -> Result<Connection, String> {
    let connection = Connection::open(db_path)
        .map_err(|error| format!("Failed to open SQLite database: {error}"))?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|error| format!("Failed to configure SQLite connection: {error}"))?;
    Ok(connection)
}

fn initialize_database(db_path: &Path) -> Result<(), String> {
    if let Some(parent_dir) = db_path.parent() {
        fs::create_dir_all(parent_dir)
            .map_err(|error| format!("Failed to create app data directory: {error}"))?;
    }

    let connection = open_connection(db_path)?;
    connection
        .execute_batch(
            "
            PRAGMA journal_mode = WAL;

            CREATE TABLE IF NOT EXISTS users (
              id TEXT PRIMARY KEY,
              email TEXT NOT NULL UNIQUE,
              password_hash TEXT NOT NULL,
              steam_id TEXT UNIQUE,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS sessions (
              token_hash TEXT PRIMARY KEY,
              user_id TEXT NOT NULL,
              created_at TEXT NOT NULL,
              expires_at TEXT NOT NULL,
              last_seen_at TEXT NOT NULL,
              FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id);
            CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions(expires_at);

            CREATE TABLE IF NOT EXISTS games (
              user_id TEXT NOT NULL,
              provider TEXT NOT NULL,
              external_id TEXT NOT NULL,
              name TEXT NOT NULL,
              kind TEXT NOT NULL DEFAULT 'unknown',
              playtime_minutes INTEGER NOT NULL,
              installed INTEGER NOT NULL DEFAULT 0,
              artwork_url TEXT,
              last_synced_at TEXT NOT NULL,
              PRIMARY KEY (user_id, provider, external_id),
              FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_games_user_id ON games(user_id);
            CREATE INDEX IF NOT EXISTS idx_games_provider ON games(provider);

            CREATE TABLE IF NOT EXISTS game_favorites (
              user_id TEXT NOT NULL,
              provider TEXT NOT NULL,
              external_id TEXT NOT NULL,
              created_at TEXT NOT NULL,
              PRIMARY KEY (user_id, provider, external_id),
              FOREIGN KEY (user_id, provider, external_id) REFERENCES games(user_id, provider, external_id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_game_favorites_user_id ON game_favorites(user_id);

            CREATE TABLE IF NOT EXISTS collections (
              id TEXT PRIMARY KEY,
              user_id TEXT NOT NULL,
              name TEXT NOT NULL COLLATE NOCASE,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL,
              UNIQUE (user_id, name),
              UNIQUE (id, user_id),
              FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_collections_user_id ON collections(user_id);

            CREATE TABLE IF NOT EXISTS collection_games (
              user_id TEXT NOT NULL,
              collection_id TEXT NOT NULL,
              provider TEXT NOT NULL,
              external_id TEXT NOT NULL,
              created_at TEXT NOT NULL,
              PRIMARY KEY (user_id, collection_id, provider, external_id),
              FOREIGN KEY (user_id, provider, external_id) REFERENCES games(user_id, provider, external_id) ON DELETE CASCADE,
              FOREIGN KEY (collection_id, user_id) REFERENCES collections(id, user_id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_collection_games_user_game
              ON collection_games(user_id, provider, external_id);
            CREATE INDEX IF NOT EXISTS idx_collection_games_collection_id
              ON collection_games(collection_id);

            CREATE TABLE IF NOT EXISTS game_privacy_settings (
              user_id TEXT NOT NULL,
              provider TEXT NOT NULL,
              external_id TEXT NOT NULL,
              hide_in_library INTEGER NOT NULL DEFAULT 0,
              mark_as_private INTEGER NOT NULL DEFAULT 0,
              overlay_data_deleted INTEGER NOT NULL DEFAULT 0,
              updated_at TEXT NOT NULL,
              PRIMARY KEY (user_id, provider, external_id),
              FOREIGN KEY (user_id, provider, external_id) REFERENCES games(user_id, provider, external_id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_game_privacy_settings_user_id ON game_privacy_settings(user_id);

            CREATE TABLE IF NOT EXISTS game_properties_settings (
              user_id TEXT NOT NULL,
              provider TEXT NOT NULL,
              external_id TEXT NOT NULL,
              settings_json TEXT NOT NULL,
              updated_at TEXT NOT NULL,
              PRIMARY KEY (user_id, provider, external_id),
              FOREIGN KEY (user_id, provider, external_id) REFERENCES games(user_id, provider, external_id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_game_properties_settings_user_id ON game_properties_settings(user_id);

            CREATE TABLE IF NOT EXISTS steam_app_metadata (
              app_id TEXT PRIMARY KEY,
              app_type TEXT NOT NULL,
              fetched_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_steam_app_metadata_fetched_at ON steam_app_metadata(fetched_at);

            CREATE TABLE IF NOT EXISTS steam_app_languages (
              app_id TEXT PRIMARY KEY,
              languages_json TEXT NOT NULL,
              fetched_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_steam_app_languages_fetched_at ON steam_app_languages(fetched_at);

            CREATE TABLE IF NOT EXISTS steam_app_betas (
              app_id TEXT PRIMARY KEY,
              betas_json TEXT NOT NULL,
              fetched_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_steam_app_betas_fetched_at ON steam_app_betas(fetched_at);

            CREATE TABLE IF NOT EXISTS steam_app_store_tags (
              app_id TEXT PRIMARY KEY,
              tags_json TEXT NOT NULL,
              fetched_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_steam_app_store_tags_fetched_at ON steam_app_store_tags(fetched_at);
            ",
        )
        .map_err(|error| format!("Failed to run SQLite migrations: {error}"))?;
    migrate_games_table(&connection)?;

    Ok(())
}

fn migrate_games_table(connection: &Connection) -> Result<(), String> {
    if !games_table_has_column(connection, "kind")? {
        connection
            .execute(
                "ALTER TABLE games ADD COLUMN kind TEXT NOT NULL DEFAULT 'unknown'",
                [],
            )
            .map_err(|error| format!("Failed to migrate games table with kind column: {error}"))?;
    }

    if !games_table_has_column(connection, "installed")? {
        connection
            .execute(
                "ALTER TABLE games ADD COLUMN installed INTEGER NOT NULL DEFAULT 0",
                [],
            )
            .map_err(|error| {
                format!("Failed to migrate games table with installed column: {error}")
            })?;
    }

    Ok(())
}

fn games_table_has_column(connection: &Connection, expected_column: &str) -> Result<bool, String> {
    let mut statement = connection
        .prepare("PRAGMA table_info(games)")
        .map_err(|error| format!("Failed to inspect games table schema: {error}"))?;

    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| format!("Failed to query games table schema: {error}"))?;

    for row in rows {
        let column_name =
            row.map_err(|error| format!("Failed to decode games table schema row: {error}"))?;
        if column_name == expected_column {
            return Ok(true);
        }
    }

    Ok(false)
}

fn env_flag(name: &str, default_value: bool) -> bool {
    let Ok(raw_value) = std::env::var(name) else {
        return default_value;
    };

    match raw_value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => default_value,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| format!("Failed to resolve app data directory: {error}"))?;
            let db_path = app_data_dir.join("catalyst.db");
            let session_token_path = app_data_dir.join("session.token");
            initialize_database(&db_path)?;

            let steam_api_key = std::env::var("STEAM_API_KEY")
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty());
            let steam_local_install_detection = env_flag("STEAM_LOCAL_INSTALL_DETECTION", true);
            let steam_settings_debug_logging = env_flag("STEAM_SETTINGS_DEBUG_LOGGING", false);
            let steam_root_override = std::env::var("STEAM_ROOT_OVERRIDE")
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty());

            let state = AppState::new(
                db_path,
                session_token_path,
                steam_api_key,
                steam_local_install_detection,
                steam_settings_debug_logging,
                steam_root_override,
            );
            restore_persisted_session(&state)?;
            app.manage(state);
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            register,
            login,
            logout,
            get_session,
            start_steam_auth,
            get_library,
            get_steam_status,
            sync_steam_library,
            set_game_favorite,
            list_collections,
            list_game_languages,
            list_game_compatibility_tools,
            get_game_privacy_settings,
            set_game_privacy_settings,
            clear_game_overlay_data,
            get_game_properties_settings,
            set_game_properties_settings,
            get_game_installation_details,
            get_game_install_size_estimate,
            list_game_install_locations,
            list_steam_downloads,
            list_game_versions_betas,
            validate_game_beta_access_code,
            create_collection,
            rename_collection,
            delete_collection,
            add_game_to_collection,
            play_game,
            install_game,
            browse_game_installed_files,
            backup_game_files,
            verify_game_files,
            import_steam_collections
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
