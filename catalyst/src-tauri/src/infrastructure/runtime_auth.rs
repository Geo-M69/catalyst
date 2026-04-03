use crate::{AppState, UserRow, SESSION_TTL_DAYS};
use bcrypt::{hash, DEFAULT_COST};
use chrono::{Duration as ChronoDuration, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::{net::TcpListener, path::Path};
use uuid::Uuid;

pub(crate) fn get_authenticated_user(
    state: &AppState,
    connection: &Connection,
) -> Result<UserRow, String> {
    let session_token =
        crate::infrastructure::runtime_session_state::get_state_session_token(state)?
            .ok_or_else(|| String::from("Not authenticated"))?;
    let user = find_user_by_session_token(connection, &session_token)?;

    match user {
        Some(user_row) => Ok(user_row),
        None => {
            crate::infrastructure::runtime_session_state::clear_active_session(state)?;
            Err(String::from("Session expired or invalid"))
        }
    }
}

pub(crate) fn find_user_by_id(
    connection: &Connection,
    user_id: &str,
) -> Result<Option<UserRow>, String> {
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

pub(crate) fn find_user_by_steam_id(
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

pub(crate) fn create_user(
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

pub(crate) fn create_steam_user(
    connection: &Connection,
    steam_id: &str,
) -> Result<UserRow, String> {
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

pub(crate) fn set_user_steam_id(
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

pub(crate) fn create_session(connection: &Connection, user_id: &str) -> Result<String, String> {
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

pub(crate) fn find_user_by_session_token(
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

pub(crate) fn invalidate_session_by_token(
    connection: &Connection,
    session_token: &str,
) -> Result<(), String> {
    let token_hash = hash_session_token(session_token);
    connection
        .execute(
            "DELETE FROM sessions WHERE token_hash = ?1",
            params![token_hash],
        )
        .map_err(|error| format!("Failed to invalidate session: {error}"))?;
    Ok(())
}

pub(crate) fn cleanup_expired_sessions(connection: &Connection) -> Result<(), String> {
    connection
        .execute(
            "DELETE FROM sessions WHERE expires_at <= ?1",
            params![Utc::now().to_rfc3339()],
        )
        .map_err(|error| format!("Failed to cleanup expired sessions: {error}"))?;
    Ok(())
}

pub(crate) fn complete_steam_auth_flow(
    db_path: &Path,
    steam_api_key: Option<String>,
    steam_local_install_detection: bool,
    steam_root_override: Option<String>,
    current_session_token: Option<String>,
) -> Result<crate::SteamAuthOutcome, String> {
    let connection = crate::infrastructure::runtime_database::open_connection(db_path)?;
    cleanup_expired_sessions(&connection)?;
    let client = crate::infrastructure::runtime_http::build_http_client()?;

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
    let callback_public_host =
        crate::infrastructure::runtime_steam_callback::resolve_steam_callback_public_host();

    let state_token = Uuid::new_v4().to_string();
    let callback_url =
        format!("http://{callback_public_host}:{port}/auth/steam/callback?state={state_token}");
    let realm = format!("http://{callback_public_host}:{port}");
    let authorization_url =
        crate::infrastructure::runtime_steam_callback::build_steam_authorization_url(
            &callback_url,
            &realm,
        )?;

    webbrowser::open(&authorization_url)
        .map_err(|error| format!("Failed to open Steam login in browser: {error}"))?;

    let callback_params = crate::infrastructure::runtime_steam_callback::wait_for_steam_callback(
        listener,
        &state_token,
        crate::STEAM_CALLBACK_TIMEOUT,
        &callback_public_host,
    )?;
    let verified = crate::infrastructure::runtime_steam_callback::verify_steam_openid_response(
        &client,
        &callback_params,
    )?;
    if !verified {
        return Err(String::from("Steam login verification failed"));
    }

    let steam_id =
        crate::infrastructure::runtime_steam_callback::extract_steam_id_from_callback_params(
            &callback_params,
        )?;

    let user = resolve_user_for_steam_auth(&connection, current_user.as_ref(), &steam_id)?;
    let synced_games = crate::sync_steam_games_for_user(
        &connection,
        &user,
        steam_api_key.as_deref(),
        steam_local_install_detection,
        steam_root_override.as_deref(),
        &client,
    )?;
    let session_token = create_session(&connection, &user.id)?;

    Ok(crate::SteamAuthOutcome {
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

pub(crate) fn hash_session_token(session_token: &str) -> String {
    let digest = Sha256::digest(session_token.as_bytes());
    format!("{digest:x}")
}

pub(crate) fn restore_persisted_session(state: &AppState) -> Result<(), String> {
    let Some(session_token) = crate::infrastructure::runtime_session_state::read_session_token(
        &state.session_token_path,
    )?
    else {
        return Ok(());
    };

    let connection = crate::infrastructure::runtime_database::open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;

    if find_user_by_session_token(&connection, &session_token)?.is_some() {
        crate::infrastructure::runtime_session_state::set_state_session_token(
            state,
            Some(session_token),
        )
    } else {
        crate::infrastructure::runtime_session_state::clear_active_session(state)
    }
}
