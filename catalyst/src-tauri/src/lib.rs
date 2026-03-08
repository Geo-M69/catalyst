use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
    thread,
    time::{Duration, Instant, SystemTime},
};

use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{Duration as ChronoDuration, TimeZone, Utc};
use regex::Regex;
use reqwest::blocking::Client;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::Manager;
use url::Url;
use uuid::Uuid;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

mod application;
mod interface;
mod cache;

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
const STEAM_APP_DETAILS_CACHE_TTL_HOURS: i64 = 24 * 7; // 1 week
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
const STEAM_DIRECTORY_PROGRESS_MANIFEST_STALE_SECONDS: u64 = 20;
const STEAM_DIRECTORY_PROGRESS_MIN_DELTA_BYTES: u64 = 256 * 1024 * 1024;
const STEAM_DIRECTORY_PROGRESS_BLEND_FACTOR: f64 = 0.5;

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
struct LibraryResponse {
    user_id: String,
    total: usize,
    games: Vec<GameResponse>,
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
    rtime_last_played: Option<i64>,
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

    // Store result in cache for subsequent calls
    // (we can't return earlier because we need a HashSet to be returned, so cache after computing)

fn resolve_steam_root_paths(steam_root_override: Option<&str>) -> Vec<PathBuf> {
    if let Some(override_path) = steam_root_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return vec![PathBuf::from(override_path)];
    }

    let mut roots = Vec::new();
    let mut seen_paths = HashSet::new();
    for candidate in steam_root_candidates() {
        if !candidate.join("steamapps").is_dir() {
            continue;
        }

        let dedupe_path = fs::canonicalize(&candidate).unwrap_or_else(|_| candidate.clone());
        if !seen_paths.insert(dedupe_path) {
            continue;
        }
        roots.push(candidate);
    }

    roots
}

fn resolve_steam_root_path(steam_root_override: Option<&str>) -> Option<PathBuf> {
    resolve_steam_root_paths(steam_root_override)
        .into_iter()
        .next()
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

fn resolve_steam_sharedconfig_paths(
    steam_root_override: Option<&str>,
    steam_id: &str,
) -> Result<Vec<PathBuf>, String> {
    let steam_root = resolve_steam_root_path(steam_root_override)
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
    let steam_root = resolve_steam_root_path(steam_root_override)
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
            candidates.push(home_path.join(".var/app/com.valvesoftware.Steam/data/Steam"));
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
    use rusqlite::Connection;

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
                            return Some(("remote-play-together".to_string(), "Remote Play Together".to_string()));
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
                    features.push(FeatureResponse { key: "workshop".to_string(), label: "Steam Workshop".to_string(), icon: Some("workshop".to_string()), tooltip: None });
                }
                // Family Sharing eligibility
                if as_string.contains("family sharing") || as_string.contains("family-share") || as_string.contains("family_share") {
                    features.push(FeatureResponse { key: "family-sharing".to_string(), label: "Family Sharing".to_string(), icon: Some("family".to_string()), tooltip: None });
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

fn vdf_get_text_entry<'a>(value: &'a VdfValue, key: &str) -> Option<&'a str> {
    let VdfValue::Object(entries) = value else {
        return None;
    };
    entries
        .iter()
        .find(|(entry_key, _)| entry_key.eq_ignore_ascii_case(key))
        .and_then(|(_, entry_value)| match entry_value {
            VdfValue::Text(text) => Some(text.as_str()),
            VdfValue::Object(_) => None,
        })
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

fn sanitize_desktop_shortcut_name(name: &str) -> String {
    let mut sanitized = String::new();
    for character in name.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, ' ' | '-' | '_') {
            sanitized.push(character);
        }
    }

    let trimmed = sanitized.trim();
    if trimmed.is_empty() {
        return String::from("Steam Game");
    }

    trimmed.to_owned()
}

fn resolve_desktop_shortcuts_directory() -> Result<PathBuf, String> {
    let home_directory = std::env::var("HOME")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("USERPROFILE")
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        })
        .ok_or_else(|| String::from("Could not resolve user home directory for desktop shortcut"))?;

    let desktop_directory = home_directory.join("Desktop");
    if desktop_directory.is_dir() {
        return Ok(desktop_directory);
    }
    if fs::create_dir_all(&desktop_directory).is_ok() {
        return Ok(desktop_directory);
    }

    let fallback_directory = if cfg!(target_os = "windows") {
        home_directory
    } else if cfg!(target_os = "macos") {
        home_directory.join("Applications")
    } else {
        home_directory.join(".local").join("share").join("applications")
    };
    fs::create_dir_all(&fallback_directory).map_err(|error| {
        format!(
            "Could not create fallback shortcut directory at {}: {error}",
            fallback_directory.display()
        )
    })?;
    Ok(fallback_directory)
}

fn create_provider_game_desktop_shortcut(
    provider: &str,
    external_id: &str,
    game_name: &str,
) -> Result<(), String> {
    match provider {
        "steam" => create_steam_game_desktop_shortcut(external_id, game_name),
        _ => Err(format!(
            "Provider '{provider}' is not supported for desktop shortcut creation"
        )),
    }
}

fn create_steam_game_desktop_shortcut(external_id: &str, game_name: &str) -> Result<(), String> {
    let app_id = external_id
        .parse::<u64>()
        .map_err(|_| String::from("Steam external_id must be a numeric app ID"))?;
    let shortcuts_directory = resolve_desktop_shortcuts_directory()?;
    let shortcut_name = sanitize_desktop_shortcut_name(game_name);

    #[cfg(target_os = "windows")]
    {
        let shortcut_path = shortcuts_directory.join(format!("{shortcut_name}.url"));
        let content = format!("[InternetShortcut]\r\nURL=steam://run/{app_id}\r\n");
        fs::write(&shortcut_path, content).map_err(|error| {
            format!(
                "Could not write desktop shortcut at {}: {error}",
                shortcut_path.display()
            )
        })?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        let shortcut_path = shortcuts_directory.join(format!("{shortcut_name}.webloc"));
        let content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>URL</key>
  <string>steam://run/{app_id}</string>
</dict>
</plist>
"#
        );
        fs::write(&shortcut_path, content).map_err(|error| {
            format!(
                "Could not write desktop shortcut at {}: {error}",
                shortcut_path.display()
            )
        })?;
        return Ok(());
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let shortcut_path = shortcuts_directory.join(format!("{shortcut_name}.desktop"));
        let content = format!(
            "[Desktop Entry]\nType=Application\nVersion=1.0\nName={shortcut_name}\nExec=xdg-open steam://run/{app_id}\nIcon=steam\nTerminal=false\nCategories=Game;\nStartupNotify=true\n"
        );
        fs::write(&shortcut_path, content).map_err(|error| {
            format!(
                "Could not write desktop shortcut at {}: {error}",
                shortcut_path.display()
            )
        })?;

        let metadata = fs::metadata(&shortcut_path).map_err(|error| {
            format!(
                "Could not read desktop shortcut metadata at {}: {error}",
                shortcut_path.display()
            )
        })?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&shortcut_path, permissions).map_err(|error| {
            format!(
                "Could not set executable permissions on desktop shortcut at {}: {error}",
                shortcut_path.display()
            )
        })?;

        return Ok(());
    }

    #[allow(unreachable_code)]
    Err(String::from(
        "Desktop shortcut creation is unsupported on this platform",
    ))
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
            // Warm Steam in the background, then dispatch the URI via open commands.
            let _ = try_spawn_command("steam", &["-silent"]);
            let _ = try_spawn_command("steam-runtime", &["-silent"]);
            let _ = try_spawn_command("flatpak", &["run", "com.valvesoftware.Steam", "-silent"]);
        }

        match try_spawn_command("xdg-open", &[uri]) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
        }

        match try_spawn_command("gio", &["open", uri]) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
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

        match webbrowser::open(uri) {
            Ok(_) => return Ok(()),
            Err(error) => errors.push(format!("webbrowser::open {uri}: {error}")),
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

fn open_steam_game_recording_settings() -> Result<(), String> {
    let candidate_uris = [
        "steam://open/settings/gamerecording",
        "steam://settings/gamerecording",
        "steam://open/settings",
        "steam://settings",
    ];
    let mut errors = Vec::new();
    for uri in candidate_uris {
        match launch_steam_uri(uri, "open-settings") {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
        }
    }

    let help_url = "https://help.steampowered.com/en/";
    match webbrowser::open(help_url) {
        Ok(_) => Ok(()),
        Err(error) => {
            errors.push(format!("webbrowser::open {help_url}: {error}"));
            Err(format!(
                "Could not open Steam game recording settings. Attempts: {}",
                errors.join("; ")
            ))
        }
    }
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
                "uninstall" => format!("steam://uninstall/{app_id}"),
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
        customization: GameCustomizationSettingsPayload {
            custom_sort_name: settings.customization.custom_sort_name.trim().to_owned(),
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

fn normalize_steam_manifest_language(language: &str) -> Option<String> {
    let normalized = language.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }

    let mapped = match normalized.as_str() {
        "arabic" => "arabic",
        "bulgarian" => "bulgarian",
        "brazilian portuguese" => "brazilian",
        "chinese (simplified)" => "schinese",
        "chinese (traditional)" => "tchinese",
        "croatian" => "croatian",
        "czech" => "czech",
        "danish" => "danish",
        "dutch" => "dutch",
        "english" => "english",
        "estonian" => "estonian",
        "finnish" => "finnish",
        "french" => "french",
        "german" => "german",
        "greek" => "greek",
        "hungarian" => "hungarian",
        "indonesian" => "indonesian",
        "italian" => "italian",
        "japanese" => "japanese",
        "korean" => "koreana",
        "latam" => "latam",
        "latin american spanish" => "latam",
        "norwegian" => "norwegian",
        "polish" => "polish",
        "portuguese" => "portuguese",
        "romanian" => "romanian",
        "russian" => "russian",
        "simplified chinese" => "schinese",
        "spanish" => "spanish",
        "spanish - latin america" => "latam",
        "swedish" => "swedish",
        "thai" => "thai",
        "traditional chinese" => "tchinese",
        "turkish" => "turkish",
        "ukrainian" => "ukrainian",
        "vietnamese" => "vietnamese",
        _ => {
            if normalized.contains("simplified") && normalized.contains("chinese") {
                "schinese"
            } else if normalized.contains("traditional") && normalized.contains("chinese") {
                "tchinese"
            } else if normalized.contains("latin") && normalized.contains("spanish") {
                "latam"
            } else if normalized.contains("brazil") && normalized.contains("portuguese") {
                "brazilian"
            } else if normalized.contains("korean") {
                "koreana"
            } else {
                let compact = normalized.replace([' ', '-', '_'], "");
                if compact.is_empty() {
                    return None;
                }
                return Some(compact);
            }
        }
    };

    Some(mapped.to_owned())
}

fn apply_steam_manifest_game_properties_settings(
    state: &AppState,
    app_id: u64,
    settings: &GamePropertiesSettingsPayload,
) -> Result<(), String> {
    let manifest_path = match resolve_steam_manifest_path_for_app_id(state.steam_root_override.as_deref(), app_id) {
        Ok(path) => path,
        Err(error) => {
            log_steam_settings_debug(
                state,
                &format!(
                    "app {}: skipping manifest settings write because no manifest was found ({})",
                    app_id, error
                ),
            );
            return Ok(());
        }
    };
    let manifest_contents = fs::read_to_string(&manifest_path).map_err(|error| {
        format!(
            "Failed to read Steam app manifest at {}: {error}",
            manifest_path.display()
        )
    })?;
    let mut manifest_value = parse_vdf_document(&manifest_contents)?;
    let app_state_object = vdf_ensure_object_path_mut(&mut manifest_value, &["AppState"]);

    match settings.updates.automatic_updates_mode.as_str() {
        "use-global-setting" => vdf_remove_entry(app_state_object, "AutoUpdateBehavior"),
        "wait-until-launch" => vdf_set_text_entry(app_state_object, "AutoUpdateBehavior", "1"),
        "let-steam-decide" => vdf_set_text_entry(app_state_object, "AutoUpdateBehavior", "0"),
        "immediately-download" => vdf_set_text_entry(app_state_object, "AutoUpdateBehavior", "2"),
        _ => {}
    }

    match settings.updates.background_downloads_mode.as_str() {
        "pause-while-playing-global" => vdf_remove_entry(app_state_object, "AllowOtherDownloadsWhileRunning"),
        "always-allow" => vdf_set_text_entry(app_state_object, "AllowOtherDownloadsWhileRunning", "1"),
        "never-allow" => vdf_set_text_entry(app_state_object, "AllowOtherDownloadsWhileRunning", "0"),
        _ => {}
    }

    let user_config_object = vdf_ensure_object_path_mut(app_state_object, &["UserConfig"]);
    if let Some(language) = normalize_steam_manifest_language(&settings.general.language) {
        vdf_set_text_entry(user_config_object, "language", &language);
    }

    let selected_beta_branch = settings.game_versions_betas.selected_version_id.trim();
    if selected_beta_branch.is_empty() || selected_beta_branch.eq_ignore_ascii_case("public") {
        vdf_remove_entry(user_config_object, "betakey");
        vdf_remove_entry(user_config_object, "BetaKey");
    } else {
        vdf_set_text_entry(user_config_object, "betakey", selected_beta_branch);
    }

    let private_access_code = settings.game_versions_betas.private_access_code.trim();
    if private_access_code.is_empty() {
        vdf_remove_entry(user_config_object, "betapassword");
    } else {
        vdf_set_text_entry(user_config_object, "betapassword", private_access_code);
    }

    let serialized_manifest = serialize_vdf_document(&manifest_value);
    fs::write(&manifest_path, serialized_manifest).map_err(|error| {
        format!(
            "Failed to write Steam app manifest at {}: {error}",
            manifest_path.display()
        )
    })?;
    log_steam_settings_debug(
        state,
        &format!("app {}: wrote Steam app manifest successfully", app_id),
    );
    Ok(())
}

fn vdf_remove_entries_with_case_insensitive_prefixes(
    value: &mut VdfValue,
    prefixes: &[&str],
) -> usize {
    let VdfValue::Object(entries) = value else {
        return 0;
    };
    let normalized_prefixes = prefixes
        .iter()
        .map(|prefix| prefix.to_ascii_lowercase())
        .collect::<Vec<_>>();
    if normalized_prefixes.is_empty() {
        return 0;
    }

    let original_len = entries.len();
    entries.retain(|(entry_key, _)| {
        let normalized_key = entry_key.to_ascii_lowercase();
        !normalized_prefixes
            .iter()
            .any(|prefix| normalized_key.starts_with(prefix))
    });
    original_len.saturating_sub(entries.len())
}

fn clear_steam_game_overlay_data(state: &AppState, user: &UserRow, app_id: u64) -> Result<(), String> {
    let steam_id = user
        .steam_id
        .as_deref()
        .ok_or_else(|| String::from("Steam is not linked for this account"))?;
    let localconfig_path = resolve_steam_localconfig_path(state.steam_root_override.as_deref(), steam_id)?;
    let localconfig_contents = fs::read_to_string(&localconfig_path).map_err(|error| {
        format!(
            "Failed to read Steam localconfig at {}: {error}",
            localconfig_path.display()
        )
    })?;
    let mut localconfig_value = parse_vdf_document(&localconfig_contents)?;
    let steam_settings_object = vdf_ensure_object_path_mut(
        &mut localconfig_value,
        &["UserLocalConfigStore", "Software", "Valve", "Steam"],
    );
    let overlay_prefix = format!("OverlaySavedDataV2_{app_id}_");
    let legacy_overlay_prefix = format!("OverlaySavedData_{app_id}_");
    let removed_entries = vdf_remove_entries_with_case_insensitive_prefixes(
        steam_settings_object,
        &[
            overlay_prefix.as_str(),
            legacy_overlay_prefix.as_str(),
            &format!("OverlaySavedDataV2_{app_id}"),
        ],
    );
    if removed_entries == 0 {
        log_steam_settings_debug(
            state,
            &format!("app {}: no overlay entries found to remove", app_id),
        );
        return Ok(());
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
        &format!("app {}: removed {} overlay entries", app_id, removed_entries),
    );
    Ok(())
}

fn log_steam_settings_debug(state: &AppState, message: &str) {
    if state.steam_settings_debug_logging {
        eprintln!("[catalyst:steam-settings] {message}");
    }
}

fn json_value_matches_app_id(value: &serde_json::Value, app_id: u64) -> bool {
    if let Some(value_number) = value.as_u64() {
        return value_number == app_id;
    }
    value
        .as_str()
        .and_then(|text| text.trim().parse::<u64>().ok())
        .is_some_and(|value_number| value_number == app_id)
}

fn json_array_contains_app_id(values: &[serde_json::Value], app_id: u64) -> bool {
    values
        .iter()
        .any(|entry_value| json_value_matches_app_id(entry_value, app_id))
}

fn json_array_remove_app_id(values: &mut Vec<serde_json::Value>, app_id: u64) {
    values.retain(|entry_value| !json_value_matches_app_id(entry_value, app_id));
}

fn update_hidden_collection_membership(
    hidden_collection_object: &mut serde_json::Map<String, serde_json::Value>,
    app_id: u64,
    hide_in_library: bool,
) {
    let mut added_values = hidden_collection_object
        .get("added")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut removed_values = hidden_collection_object
        .get("removed")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    json_array_remove_app_id(&mut added_values, app_id);
    json_array_remove_app_id(&mut removed_values, app_id);
    if hide_in_library {
        if !json_array_contains_app_id(&added_values, app_id) {
            added_values.push(serde_json::Value::from(app_id));
        }
    } else if !json_array_contains_app_id(&removed_values, app_id) {
        removed_values.push(serde_json::Value::from(app_id));
    }
    hidden_collection_object.insert(String::from("added"), serde_json::Value::Array(added_values));
    hidden_collection_object.insert(String::from("removed"), serde_json::Value::Array(removed_values));
}

fn update_steam_user_collections_hidden_state(
    steam_settings_object: &mut VdfValue,
    app_id: u64,
    hide_in_library: bool,
) -> Result<(), String> {
    let mut user_collections_value = vdf_get_text_entry(steam_settings_object, "user-collections")
        .and_then(|json_text| serde_json::from_str::<serde_json::Value>(json_text).ok())
        .filter(serde_json::Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}));
    let Some(user_collections_object) = user_collections_value.as_object_mut() else {
        return Err(String::from("Steam user-collections value must be a JSON object"));
    };
    let hidden_collection_value = user_collections_object
        .entry(String::from("hidden"))
        .or_insert_with(|| serde_json::json!({}));
    let hidden_collection_name = hidden_collection_value
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Hidden")
        .to_owned();
    let mut hidden_collection_object = hidden_collection_value
        .as_object()
        .cloned()
        .unwrap_or_default();
    hidden_collection_object.insert(
        String::from("name"),
        serde_json::Value::String(hidden_collection_name),
    );
    update_hidden_collection_membership(&mut hidden_collection_object, app_id, hide_in_library);
    user_collections_object.insert(
        String::from("hidden"),
        serde_json::Value::Object(hidden_collection_object),
    );
    let serialized_user_collections = serde_json::to_string(&user_collections_value)
        .map_err(|error| format!("Failed to serialize Steam user-collections JSON: {error}"))?;
    vdf_remove_entry(steam_settings_object, "user-collections");
    vdf_set_text_entry(
        steam_settings_object,
        "user-collections",
        &serialized_user_collections,
    );
    Ok(())
}

fn vdf_for_each_object_path_mut<F>(
    value: &mut VdfValue,
    path: &[&str],
    on_match: &mut F,
) -> Result<usize, String>
where
    F: FnMut(&mut VdfValue) -> Result<(), String>,
{
    if path.is_empty() {
        on_match(value)?;
        return Ok(1);
    }

    if matches!(value, VdfValue::Text(_)) {
        *value = VdfValue::Object(Vec::new());
    }
    let VdfValue::Object(entries) = value else {
        return Ok(0);
    };
    let mut matched_count = 0usize;

    for (entry_key, entry_value) in entries.iter_mut() {
        if !entry_key.eq_ignore_ascii_case(path[0]) {
            continue;
        }
        matched_count += vdf_for_each_object_path_mut(entry_value, &path[1..], on_match)?;
    }

    Ok(matched_count)
}

fn vdf_for_each_matching_app_entry_in_apps_sections_mut<F>(
    value: &mut VdfValue,
    app_id_key: &str,
    on_match: &mut F,
) where
    F: FnMut(&mut VdfValue),
{
    let VdfValue::Object(entries) = value else {
        return;
    };

    for (entry_key, entry_value) in entries.iter_mut() {
        if entry_key.eq_ignore_ascii_case("apps") {
            if let VdfValue::Object(app_entries) = entry_value {
                for (app_entry_key, app_entry_value) in app_entries.iter_mut() {
                    if !app_entry_key.eq_ignore_ascii_case(app_id_key) {
                        continue;
                    }
                    if matches!(app_entry_value, VdfValue::Text(_)) {
                        *app_entry_value = VdfValue::Object(Vec::new());
                    }
                    on_match(app_entry_value);
                }
            }
        }
        vdf_for_each_matching_app_entry_in_apps_sections_mut(entry_value, app_id_key, on_match);
    }
}

fn apply_steam_game_privacy_settings_to_steam_root_object(
    steam_settings_object: &mut VdfValue,
    app_id: u64,
    settings: &GamePrivacySettingsResponse,
) -> Result<(), String> {
    let app_id_key = app_id.to_string();
    let update_app_settings_object = |app_settings_object: &mut VdfValue| {
        if settings.hide_in_library {
            vdf_set_text_entry(app_settings_object, "Hidden", "1");
            vdf_set_text_entry(app_settings_object, "hidden", "1");
        } else {
            vdf_remove_entry(app_settings_object, "Hidden");
            vdf_remove_entry(app_settings_object, "hidden");
        }

        if settings.mark_as_private {
            vdf_set_text_entry(app_settings_object, "Private", "1");
            vdf_set_text_entry(app_settings_object, "private", "1");
        } else {
            vdf_remove_entry(app_settings_object, "Private");
            vdf_remove_entry(app_settings_object, "private");
        }
    };
    let mut matched_any_app_entry = false;
    let mut update_existing_app_settings = |app_settings_object: &mut VdfValue| {
        matched_any_app_entry = true;
        update_app_settings_object(app_settings_object);
    };
    vdf_for_each_matching_app_entry_in_apps_sections_mut(
        steam_settings_object,
        &app_id_key,
        &mut update_existing_app_settings,
    );
    if !matched_any_app_entry {
        let apps_object = vdf_ensure_object_path_mut(steam_settings_object, &["apps"]);
        let app_settings_object = vdf_ensure_object_path_mut(apps_object, &[app_id_key.as_str()]);
        update_app_settings_object(app_settings_object);
    }

    update_steam_user_collections_hidden_state(
        steam_settings_object,
        app_id,
        settings.hide_in_library,
    )?;
    Ok(())
}

fn apply_steam_game_privacy_settings_to_vdf_document(
    vdf_document: &mut VdfValue,
    steam_store_root_path: &[&str],
    app_id: u64,
    settings: &GamePrivacySettingsResponse,
) -> Result<(), String> {
    let mut apply_to_steam_root = |steam_settings_object: &mut VdfValue| {
        apply_steam_game_privacy_settings_to_steam_root_object(
            steam_settings_object,
            app_id,
            settings,
        )
    };

    let matched_count =
        vdf_for_each_object_path_mut(vdf_document, steam_store_root_path, &mut apply_to_steam_root)?;
    if matched_count > 0 {
        return Ok(());
    }

    let steam_settings_object = vdf_ensure_object_path_mut(vdf_document, steam_store_root_path);
    apply_to_steam_root(steam_settings_object)
}

fn apply_steam_user_collections_hidden_state_to_vdf_document(
    vdf_document: &mut VdfValue,
    steam_store_root_path: &[&str],
    app_id: u64,
    hide_in_library: bool,
) -> Result<(), String> {
    let mut apply_to_steam_root = |steam_settings_object: &mut VdfValue| {
        update_steam_user_collections_hidden_state(steam_settings_object, app_id, hide_in_library)
    };
    let matched_count =
        vdf_for_each_object_path_mut(vdf_document, steam_store_root_path, &mut apply_to_steam_root)?;
    if matched_count > 0 {
        return Ok(());
    }

    let steam_settings_object = vdf_ensure_object_path_mut(vdf_document, steam_store_root_path);
    apply_to_steam_root(steam_settings_object)
}

fn current_unix_timestamp_seconds() -> i64 {
    SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn serialize_steam_hidden_collection_cloudstorage_value(
    existing_value_text: Option<&str>,
    app_id: u64,
    hide_in_library: bool,
) -> Result<String, String> {
    let mut hidden_collection_object = existing_value_text
        .and_then(|value_text| serde_json::from_str::<serde_json::Value>(value_text).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    let hidden_collection_id = hidden_collection_object
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("hidden")
        .to_owned();
    let hidden_collection_name = hidden_collection_object
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Hidden")
        .to_owned();
    hidden_collection_object.insert(
        String::from("id"),
        serde_json::Value::String(hidden_collection_id),
    );
    hidden_collection_object.insert(
        String::from("name"),
        serde_json::Value::String(hidden_collection_name),
    );
    update_hidden_collection_membership(&mut hidden_collection_object, app_id, hide_in_library);
    serde_json::to_string(&serde_json::Value::Object(hidden_collection_object))
        .map_err(|error| format!("Failed to serialize Steam hidden cloudstorage JSON: {error}"))
}

fn update_steam_cloudstorage_hidden_collection_namespace(
    namespace_path: &Path,
    app_id: u64,
    hide_in_library: bool,
) -> Result<String, String> {
    let namespace_contents = fs::read_to_string(namespace_path).map_err(|error| {
        format!(
            "Failed to read Steam cloudstorage namespace file at {}: {error}",
            namespace_path.display()
        )
    })?;
    let mut namespace_value = serde_json::from_str::<serde_json::Value>(&namespace_contents).map_err(
        |error| {
            format!(
                "Failed to parse Steam cloudstorage namespace JSON at {}: {error}",
                namespace_path.display()
            )
        },
    )?;
    let Some(namespace_entries) = namespace_value.as_array_mut() else {
        return Err(format!(
            "Steam cloudstorage namespace data at {} must be a JSON array",
            namespace_path.display()
        ));
    };

    let mut updated_namespace_version: Option<String> = None;
    for namespace_entry in namespace_entries.iter_mut() {
        let Some(entry_parts) = namespace_entry.as_array_mut() else {
            continue;
        };
        if entry_parts
            .first()
            .and_then(serde_json::Value::as_str)
            != Some("user-collections.hidden")
        {
            continue;
        }

        if entry_parts.len() < 2 {
            entry_parts.resize(2, serde_json::json!({}));
        }
        if !entry_parts[1].is_object() {
            entry_parts[1] = serde_json::json!({});
        }
        let Some(hidden_collection_metadata) = entry_parts[1].as_object_mut() else {
            continue;
        };
        let serialized_hidden_collection = serialize_steam_hidden_collection_cloudstorage_value(
            hidden_collection_metadata
                .get("value")
                .and_then(serde_json::Value::as_str),
            app_id,
            hide_in_library,
        )?;
        let current_version = hidden_collection_metadata
            .get("version")
            .and_then(serde_json::Value::as_str)
            .and_then(|text| text.parse::<u64>().ok())
            .unwrap_or(0);
        let next_version = current_version.saturating_add(1);
        let next_version_text = next_version.to_string();
        hidden_collection_metadata.insert(
            String::from("key"),
            serde_json::Value::String(String::from("user-collections.hidden")),
        );
        hidden_collection_metadata.insert(
            String::from("timestamp"),
            serde_json::Value::from(current_unix_timestamp_seconds()),
        );
        hidden_collection_metadata.insert(
            String::from("value"),
            serde_json::Value::String(serialized_hidden_collection),
        );
        hidden_collection_metadata.insert(
            String::from("version"),
            serde_json::Value::String(next_version_text.clone()),
        );
        hidden_collection_metadata.insert(
            String::from("conflictResolutionMethod"),
            serde_json::Value::String(String::from("custom")),
        );
        hidden_collection_metadata.insert(
            String::from("strMethodId"),
            serde_json::Value::String(String::from("union-collections")),
        );
        updated_namespace_version = Some(next_version_text);
        break;
    }

    if updated_namespace_version.is_none() {
        let serialized_hidden_collection =
            serialize_steam_hidden_collection_cloudstorage_value(None, app_id, hide_in_library)?;
        namespace_entries.push(serde_json::json!([
            "user-collections.hidden",
            {
                "key": "user-collections.hidden",
                "timestamp": current_unix_timestamp_seconds(),
                "value": serialized_hidden_collection,
                "version": "1",
                "conflictResolutionMethod": "custom",
                "strMethodId": "union-collections"
            }
        ]));
        updated_namespace_version = Some(String::from("1"));
    }

    let serialized_namespace = serde_json::to_string(&namespace_value).map_err(|error| {
        format!(
            "Failed to serialize Steam cloudstorage namespace data at {}: {error}",
            namespace_path.display()
        )
    })?;
    fs::write(namespace_path, serialized_namespace).map_err(|error| {
        format!(
            "Failed to write Steam cloudstorage namespace file at {}: {error}",
            namespace_path.display()
        )
    })?;
    Ok(updated_namespace_version.unwrap_or_else(|| String::from("1")))
}

fn update_steam_cloudstorage_namespaces_version(
    namespaces_path: &Path,
    namespace_id: i64,
    namespace_version: &str,
) -> Result<(), String> {
    let namespaces_contents = fs::read_to_string(namespaces_path).map_err(|error| {
        format!(
            "Failed to read Steam cloudstorage namespaces file at {}: {error}",
            namespaces_path.display()
        )
    })?;
    let mut namespaces_value = serde_json::from_str::<serde_json::Value>(&namespaces_contents).map_err(
        |error| {
            format!(
                "Failed to parse Steam cloudstorage namespaces JSON at {}: {error}",
                namespaces_path.display()
            )
        },
    )?;
    let Some(namespace_entries) = namespaces_value.as_array_mut() else {
        return Err(format!(
            "Steam cloudstorage namespaces data at {} must be a JSON array",
            namespaces_path.display()
        ));
    };

    let mut updated_existing_entry = false;
    for namespace_entry in namespace_entries.iter_mut() {
        let Some(entry_parts) = namespace_entry.as_array_mut() else {
            continue;
        };
        let Some(entry_namespace_id) = entry_parts.first().and_then(serde_json::Value::as_i64) else {
            continue;
        };
        if entry_namespace_id != namespace_id {
            continue;
        }
        if entry_parts.len() < 2 {
            entry_parts.resize(2, serde_json::Value::Null);
        }
        entry_parts[1] = serde_json::Value::String(namespace_version.to_owned());
        updated_existing_entry = true;
        break;
    }

    if !updated_existing_entry {
        namespace_entries.push(serde_json::json!([namespace_id, namespace_version]));
    }

    let serialized_namespaces = serde_json::to_string(&namespaces_value).map_err(|error| {
        format!(
            "Failed to serialize Steam cloudstorage namespaces JSON at {}: {error}",
            namespaces_path.display()
        )
    })?;
    fs::write(namespaces_path, serialized_namespaces).map_err(|error| {
        format!(
            "Failed to write Steam cloudstorage namespaces file at {}: {error}",
            namespaces_path.display()
        )
    })?;
    Ok(())
}

fn apply_steam_cloudstorage_hidden_collection_state(
    state: &AppState,
    steam_id: &str,
    app_id: u64,
    hide_in_library: bool,
) -> Result<(), String> {
    let cloudstorage_directory =
        resolve_steam_cloudstorage_directory(state.steam_root_override.as_deref(), steam_id)?;
    let namespace_path = cloudstorage_directory.join("cloud-storage-namespace-1.json");
    if !namespace_path.is_file() {
        return Ok(());
    }
    let namespace_version =
        update_steam_cloudstorage_hidden_collection_namespace(&namespace_path, app_id, hide_in_library)?;
    let namespaces_path = cloudstorage_directory.join("cloud-storage-namespaces.json");
    if namespaces_path.is_file() {
        update_steam_cloudstorage_namespaces_version(&namespaces_path, 1, &namespace_version)?;
    }
    log_steam_settings_debug(
        state,
        &format!(
            "app {}: wrote Steam cloudstorage hidden collection state at {}",
            app_id,
            namespace_path.display()
        ),
    );
    Ok(())
}

fn apply_steam_game_privacy_settings(
    state: &AppState,
    user: &UserRow,
    app_id: u64,
    settings: &GamePrivacySettingsResponse,
) -> Result<(), String> {
    let steam_id = user
        .steam_id
        .as_deref()
        .ok_or_else(|| String::from("Steam is not linked for this account"))?;
    let localconfig_path = resolve_steam_localconfig_path(state.steam_root_override.as_deref(), steam_id)?;
    log_steam_settings_debug(
        state,
        &format!(
            "Applying privacy settings for app {} using localconfig {}",
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
    apply_steam_game_privacy_settings_to_vdf_document(
        &mut localconfig_value,
        &["UserLocalConfigStore", "Software", "Valve", "Steam"],
        app_id,
        settings,
    )?;
    apply_steam_user_collections_hidden_state_to_vdf_document(
        &mut localconfig_value,
        &["UserLocalConfigStore", "WebStorage"],
        app_id,
        settings.hide_in_library,
    )?;

    let serialized_localconfig = serialize_vdf_document(&localconfig_value);
    fs::write(&localconfig_path, serialized_localconfig).map_err(|error| {
        format!(
            "Failed to write Steam localconfig at {}: {error}",
            localconfig_path.display()
        )
    })?;
    log_steam_settings_debug(
        state,
        &format!("app {}: wrote Steam localconfig privacy settings successfully", app_id),
    );

    let sharedconfig_paths =
        resolve_steam_sharedconfig_paths(state.steam_root_override.as_deref(), steam_id)?;
    for sharedconfig_path in sharedconfig_paths {
        let sharedconfig_contents = fs::read_to_string(&sharedconfig_path).map_err(|error| {
            format!(
                "Failed to read Steam sharedconfig at {}: {error}",
                sharedconfig_path.display()
            )
        })?;
        let mut sharedconfig_value = parse_vdf_document(&sharedconfig_contents)?;
        apply_steam_game_privacy_settings_to_vdf_document(
            &mut sharedconfig_value,
            &["UserRoamingConfigStore", "Software", "Valve", "Steam"],
            app_id,
            settings,
        )?;
        let serialized_sharedconfig = serialize_vdf_document(&sharedconfig_value);
        fs::write(&sharedconfig_path, serialized_sharedconfig).map_err(|error| {
            format!(
                "Failed to write Steam sharedconfig at {}: {error}",
                sharedconfig_path.display()
            )
        })?;
        log_steam_settings_debug(
            state,
            &format!(
                "app {}: wrote Steam sharedconfig privacy settings at {}",
                app_id,
                sharedconfig_path.display()
            ),
        );
    }

    if let Err(error) =
        apply_steam_cloudstorage_hidden_collection_state(state, steam_id, app_id, settings.hide_in_library)
    {
        log_steam_settings_debug(
            state,
            &format!(
                "app {}: skipped cloudstorage hidden collection update ({})",
                app_id, error
            ),
        );
    }

    Ok(())
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

    if settings.general.steam_overlay_enabled {
        vdf_remove_entry(app_settings_object, "EnableGameOverlay");
        vdf_remove_entry(app_settings_object, "DisableOverlay");
        log_steam_settings_debug(
            state,
            &format!("app {}: restored default Steam Overlay behavior", app_id),
        );
    } else {
        vdf_set_text_entry(app_settings_object, "EnableGameOverlay", "0");
        vdf_set_text_entry(app_settings_object, "DisableOverlay", "1");
        log_steam_settings_debug(state, &format!("app {}: disabled Steam Overlay", app_id));
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
            vdf_remove_entry(app_settings_object, "AllowOtherDownloadsWhileRunning");
            log_steam_settings_debug(
                state,
                &format!(
                    "app {}: cleared AllowDownloadsWhileRunning and AllowOtherDownloadsWhileRunning",
                    app_id
                ),
            );
        }
        "always-allow" => {
            vdf_set_text_entry(app_settings_object, "AllowDownloadsWhileRunning", "1");
            vdf_set_text_entry(app_settings_object, "AllowOtherDownloadsWhileRunning", "1");
            log_steam_settings_debug(
                state,
                &format!(
                    "app {}: set AllowDownloadsWhileRunning=1 and AllowOtherDownloadsWhileRunning=1",
                    app_id
                ),
            );
        }
        "never-allow" => {
            vdf_set_text_entry(app_settings_object, "AllowDownloadsWhileRunning", "0");
            vdf_set_text_entry(app_settings_object, "AllowOtherDownloadsWhileRunning", "0");
            log_steam_settings_debug(
                state,
                &format!(
                    "app {}: set AllowDownloadsWhileRunning=0 and AllowOtherDownloadsWhileRunning=0",
                    app_id
                ),
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
    apply_steam_manifest_game_properties_settings(state, app_id, settings)?;
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

// `find_auth_user_by_email` removed: local credential flows were deleted.

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
    // On Unix platforms, prefer creating the file with restrictive permissions (rw-------).
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut options = fs::OpenOptions::new();
        options.create(true).write(true).truncate(true).mode(0o600);
        let mut file = options
            .open(session_path)
            .map_err(|error| format!("Failed to open session token file: {error}"))?;
        use std::io::Write;
        file.write_all(session_token.as_bytes())
            .map_err(|error| format!("Failed to write session token file: {error}"))?;
        Ok(())
    }

    #[cfg(not(unix))]
    {
        fs::write(session_path, session_token)
            .map_err(|error| format!("Failed to write session token file: {error}"))
    }
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
                            last_played_at TEXT,
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

                        CREATE TABLE IF NOT EXISTS game_genres (
                            user_id TEXT NOT NULL,
                            provider TEXT NOT NULL,
                            external_id TEXT NOT NULL,
                            genre TEXT NOT NULL,
                            PRIMARY KEY (user_id, provider, external_id, genre),
                            FOREIGN KEY (user_id, provider, external_id) REFERENCES games(user_id, provider, external_id) ON DELETE CASCADE
                        );

                        CREATE INDEX IF NOT EXISTS idx_game_genres_user_game ON game_genres(user_id, provider, external_id);

                        CREATE TABLE IF NOT EXISTS steam_app_details (
                            app_id TEXT PRIMARY KEY,
                            details_json TEXT NOT NULL,
                            fetched_at TEXT NOT NULL
                        );

                        CREATE INDEX IF NOT EXISTS idx_steam_app_details_fetched_at ON steam_app_details(fetched_at);

                                    CREATE TABLE IF NOT EXISTS steam_app_features (
                                        app_id TEXT PRIMARY KEY,
                                        has_achievements INTEGER NOT NULL DEFAULT 0,
                                        achievements_count INTEGER,
                                        has_cloud_saves INTEGER NOT NULL DEFAULT 0,
                                        cloud_details TEXT,
                                        controller_support TEXT,
                                        fetched_at TEXT NOT NULL
                                    );

                                    CREATE INDEX IF NOT EXISTS idx_steam_app_features_fetched_at ON steam_app_features(fetched_at);

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

    if !games_table_has_column(connection, "last_played_at")? {
        connection
            .execute(
                "ALTER TABLE games ADD COLUMN last_played_at TEXT",
                [],
            )
            .map_err(|error| {
                format!("Failed to migrate games table with last_played_at column: {error}")
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
            // `get_steam_status` is a server-side helper (not exposed to the
            // frontend) and is intentionally not registered here.
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
