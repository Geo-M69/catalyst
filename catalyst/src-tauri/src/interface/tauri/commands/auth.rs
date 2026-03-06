use crate::*;
use crate::application::error::{AppError, AppResult};
use tauri::State;

#[tauri::command]
pub(crate) fn register(
    email: String,
    password: String,
    state: State<'_, AppState>,
) -> AppResult<AuthResponse> {
    let normalized_email = normalize_email(&email)?;
    validate_password(&password)?;

    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;

    if find_auth_user_by_email(&connection, &normalized_email)?.is_some() {
        return Err(AppError::conflict(
            "email_in_use",
            "Email is already in use",
        ));
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
pub(crate) fn login(
    email: String,
    password: String,
    state: State<'_, AppState>,
) -> AppResult<AuthResponse> {
    let normalized_email = normalize_email(&email)?;
    validate_password(&password)?;

    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;

    let auth_user = find_auth_user_by_email(&connection, &normalized_email)?
        .ok_or_else(|| AppError::unauthorized("invalid_credentials", "Invalid email or password"))?;
    let valid_password = verify(password, auth_user.password_hash.as_str())
        .map_err(|error| format!("Failed to verify password: {error}"))?;
    if !valid_password {
        return Err(AppError::unauthorized(
            "invalid_credentials",
            "Invalid email or password",
        ));
    }

    let session_token = create_session(&connection, &auth_user.user.id)?;
    persist_active_session(state.inner(), &session_token)?;

    Ok(AuthResponse {
        user: public_user_from_row(&auth_user.user),
    })
}

#[tauri::command]
pub(crate) fn logout(state: State<'_, AppState>) -> AppResult<()> {
    let session_token = get_state_session_token(state.inner())?;
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;

    if let Some(token) = session_token {
        invalidate_session_by_token(&connection, &token)?;
    }

    Ok(clear_active_session(state.inner())?)
}

#[tauri::command]
pub(crate) fn get_session(state: State<'_, AppState>) -> AppResult<Option<PublicUser>> {
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
pub(crate) async fn start_steam_auth(state: State<'_, AppState>) -> AppResult<SteamAuthResponse> {
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
