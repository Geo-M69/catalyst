use std::{
    collections::HashMap,
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
const STEAM_CALLBACK_TIMEOUT: Duration = Duration::from_secs(180);
const SESSION_TTL_DAYS: i64 = 30;

struct AppState {
    db_path: PathBuf,
    session_token_path: PathBuf,
    steam_api_key: Option<String>,
    current_session_token: Mutex<Option<String>>,
}

impl AppState {
    fn new(db_path: PathBuf, session_token_path: PathBuf, steam_api_key: Option<String>) -> Self {
        Self {
            db_path,
            session_token_path,
            steam_api_key,
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
    playtime_minutes: i64,
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
    playtime_minutes: i64,
    artwork_url: Option<String>,
    last_synced_at: String,
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
struct SteamSyncResponse {
    user_id: String,
    provider: String,
    synced_games: usize,
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
    let current_session_token = get_state_session_token(state.inner())?;

    let outcome = tauri::async_runtime::spawn_blocking(move || {
        complete_steam_auth_flow(&db_path, steam_api_key, current_session_token)
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
    let synced_games =
        sync_steam_games_for_user(&connection, &user, state.steam_api_key.as_deref(), &client)?;

    Ok(SteamSyncResponse {
        user_id: user.id,
        provider: String::from("steam"),
        synced_games,
    })
}

fn complete_steam_auth_flow(
    db_path: &Path,
    steam_api_key: Option<String>,
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
    let synced_games =
        sync_steam_games_for_user(&connection, &user, steam_api_key.as_deref(), &client)?;
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
    client: &Client,
) -> Result<usize, String> {
    let steam_id = user
        .steam_id
        .as_deref()
        .ok_or_else(|| String::from("User is not linked to Steam"))?;

    let Some(api_key) = steam_api_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
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

    let games = payload
        .response
        .and_then(|response| response.games)
        .unwrap_or_default()
        .into_iter()
        .map(map_steam_game)
        .collect::<Vec<_>>();

    replace_provider_games(connection, &user.id, "steam", &games)?;
    Ok(games.len())
}

fn map_steam_game(game: SteamOwnedGame) -> LibraryGameInput {
    let external_id = game.appid.to_string();
    let name = game
        .name
        .map(|raw_name| raw_name.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("Steam App {external_id}"));
    let artwork_url = game.img_logo_url.map(|logo_hash| {
        format!(
            "https://media.steampowered.com/steamcommunity/public/images/apps/{external_id}/{logo_hash}.jpg"
        )
    });

    LibraryGameInput {
        external_id,
        name,
        playtime_minutes: game.playtime_forever.unwrap_or(0),
        artwork_url,
        last_synced_at: Utc::now().to_rfc3339(),
    }
}

fn replace_provider_games(
    connection: &Connection,
    user_id: &str,
    provider: &str,
    games: &[LibraryGameInput],
) -> Result<(), String> {
    connection
        .execute(
            "DELETE FROM games WHERE user_id = ?1 AND provider = ?2",
            params![user_id, provider],
        )
        .map_err(|error| format!("Failed to clear existing provider games: {error}"))?;

    let mut insert = connection
        .prepare(
            "INSERT INTO games (user_id, provider, external_id, name, playtime_minutes, artwork_url, last_synced_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .map_err(|error| format!("Failed to prepare game insert statement: {error}"))?;

    for game in games {
        insert
            .execute(params![
                user_id,
                provider,
                game.external_id,
                game.name,
                game.playtime_minutes,
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
            "SELECT provider, external_id, name, playtime_minutes, artwork_url, last_synced_at FROM games WHERE user_id = ?1 ORDER BY name COLLATE NOCASE ASC",
        )
        .map_err(|error| format!("Failed to prepare library query: {error}"))?;

    let rows = statement
        .query_map(params![user_id], |row| {
            let provider: String = row.get(0)?;
            let external_id: String = row.get(1)?;
            Ok(GameResponse {
                id: format!("{provider}:{external_id}"),
                provider,
                external_id,
                name: row.get(2)?,
                playtime_minutes: row.get(3)?,
                artwork_url: row.get(4)?,
                last_synced_at: row.get(5)?,
            })
        })
        .map_err(|error| format!("Failed to query library rows: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to decode library rows: {error}"))
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
              playtime_minutes INTEGER NOT NULL,
              artwork_url TEXT,
              last_synced_at TEXT NOT NULL,
              PRIMARY KEY (user_id, provider, external_id),
              FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_games_user_id ON games(user_id);
            CREATE INDEX IF NOT EXISTS idx_games_provider ON games(provider);
            ",
        )
        .map_err(|error| format!("Failed to run SQLite migrations: {error}"))?;

    Ok(())
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

            let state = AppState::new(db_path, session_token_path, steam_api_key);
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
            sync_steam_library
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
