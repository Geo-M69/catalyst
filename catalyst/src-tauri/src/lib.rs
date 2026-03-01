use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{Duration as ChronoDuration, Utc};
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
const STEAM_CALLBACK_TIMEOUT: Duration = Duration::from_secs(180);
const STEAM_APP_DETAILS_BATCH_SIZE: usize = 75;
const STEAM_APP_METADATA_CACHE_TTL_HOURS: i64 = 24 * 7;
const SESSION_TTL_DAYS: i64 = 30;
const STEAM_ID64_ACCOUNT_ID_BASE: u64 = 76_561_197_960_265_728;

struct AppState {
    db_path: PathBuf,
    session_token_path: PathBuf,
    steam_api_key: Option<String>,
    steam_local_install_detection: bool,
    steam_root_override: Option<String>,
    current_session_token: Mutex<Option<String>>,
}

impl AppState {
    fn new(
        db_path: PathBuf,
        session_token_path: PathBuf,
        steam_api_key: Option<String>,
        steam_local_install_detection: bool,
        steam_root_override: Option<String>,
    ) -> Self {
        Self {
            db_path,
            session_token_path,
            steam_api_key,
            steam_local_install_detection,
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
fn create_collection(name: String, state: State<'_, AppState>) -> Result<CollectionResponse, String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    create_user_collection(&connection, &user.id, &name)
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
fn play_game(provider: String, external_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;
    open_provider_game_uri(&provider, &external_id, "play")
}

#[tauri::command]
fn install_game(provider: String, external_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;
    open_provider_game_uri(&provider, &external_id, "install")?;
    mark_game_as_installed(&connection, &user.id, &provider, &external_id)
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

    let state_token = Uuid::new_v4().to_string();
    let callback_url = format!("http://127.0.0.1:{port}/auth/steam/callback?state={state_token}");
    let realm = format!("http://127.0.0.1:{port}");
    let authorization_url = build_steam_authorization_url(&callback_url, &realm)?;

    webbrowser::open(&authorization_url)
        .map_err(|error| format!("Failed to open Steam login in browser: {error}"))?;

    let callback_params = wait_for_steam_callback(listener, &state_token, STEAM_CALLBACK_TIMEOUT)?;
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

fn wait_for_steam_callback(
    listener: TcpListener,
    expected_state: &str,
    timeout: Duration,
) -> Result<HashMap<String, String>, String> {
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("Failed to configure callback listener: {error}"))?;

    let deadline = Instant::now() + timeout;
    loop {
        if Instant::now() >= deadline {
            return Err(String::from(
                "Timed out waiting for Steam callback. Complete Steam sign-in in your browser, and if Windows Firewall prompts for Catalyst, allow local/private access.",
            ));
        }

        match listener.accept() {
            Ok((mut stream, _)) => {
                let request_target = read_http_request_target(&mut stream)?;
                let callback_url = Url::parse(&format!("http://127.0.0.1{request_target}"))
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
            })
        })
        .map_err(|error| format!("Failed to query library rows: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to decode library rows: {error}"))
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

fn open_provider_game_uri(provider: &str, external_id: &str, action: &str) -> Result<(), String> {
    match provider {
        "steam" => {
            let app_id = external_id
                .parse::<u64>()
                .map_err(|_| String::from("Steam external_id must be a numeric app ID"))?;
            let uri = match action {
                "play" => format!("steam://run/{app_id}"),
                "install" => format!("steam://install/{app_id}"),
                _ => return Err(String::from("Unsupported Steam action")),
            };

            webbrowser::open(&uri).map_err(|error| format!("Failed to open Steam URI: {error}"))?;
            Ok(())
        }
        _ => Err(format!(
            "Provider '{provider}' is not supported for action '{action}'"
        )),
    }
}

fn mark_game_as_installed(
    connection: &Connection,
    user_id: &str,
    provider: &str,
    external_id: &str,
) -> Result<(), String> {
    let updated_rows = connection
        .execute(
            "UPDATE games SET installed = 1 WHERE user_id = ?1 AND provider = ?2 AND external_id = ?3",
            params![user_id, provider, external_id],
        )
        .map_err(|error| format!("Failed to update install state: {error}"))?;
    if updated_rows == 0 {
        return Err(String::from("Game not found for current user"));
    }

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

            CREATE TABLE IF NOT EXISTS steam_app_metadata (
              app_id TEXT PRIMARY KEY,
              app_type TEXT NOT NULL,
              fetched_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_steam_app_metadata_fetched_at ON steam_app_metadata(fetched_at);
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
            let steam_root_override = std::env::var("STEAM_ROOT_OVERRIDE")
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty());

            let state = AppState::new(
                db_path,
                session_token_path,
                steam_api_key,
                steam_local_install_detection,
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
            create_collection,
            add_game_to_collection,
            play_game,
            install_game,
            import_steam_collections
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
