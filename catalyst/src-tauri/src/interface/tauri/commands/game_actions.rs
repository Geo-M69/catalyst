use crate::*;
use crate::application::error::{AppError, AppResult};
use rusqlite::params;
use tauri::State;

#[tauri::command]
pub(crate) fn play_game(
    provider: String,
    external_id: String,
    launch_options: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<()> {
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
    Ok(open_provider_game_uri(
        &provider,
        &external_id,
        "play",
        resolved_launch_options.as_deref(),
    )?)
}

#[tauri::command]
pub(crate) fn install_game(
    provider: String,
    external_id: String,
    install_path: Option<String>,
    create_desktop_shortcut: Option<bool>,
    create_application_shortcut: Option<bool>,
    state: State<'_, AppState>,
) -> AppResult<()> {
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
    Ok(open_provider_game_uri(&provider, &external_id, "install", None)?)
}

#[tauri::command]
pub(crate) fn uninstall_game(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;
    Ok(open_provider_game_uri(&provider, &external_id, "uninstall", None)?)
}

#[tauri::command]
pub(crate) fn browse_game_installed_files(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;

    if provider != "steam" {
        return Err(AppError::validation(
            "unsupported_provider",
            "Browsing installed files is only supported for Steam games.",
        ));
    }

    let app_id = external_id
        .parse::<u64>()
        .map_err(|_| AppError::validation("invalid_external_id", "Steam external_id must be a numeric app ID"))?;
    let install_directory =
        resolve_steam_install_directory_for_app_id(state.steam_root_override.as_deref(), app_id)?;
    if !install_directory.is_dir() {
        return Err(AppError::not_found(
            "install_directory_missing",
            format!("Install directory is unavailable: {}", install_directory.display()),
        ));
    }

    Ok(open_path_in_file_manager(&install_directory)?)
}

#[tauri::command]
pub(crate) fn backup_game_files(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;
    Ok(open_provider_game_uri(&provider, &external_id, "backup", None)?)
}

#[tauri::command]
pub(crate) fn verify_game_files(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;
    Ok(open_provider_game_uri(&provider, &external_id, "validate", None)?)
}

#[tauri::command]
pub(crate) fn add_game_desktop_shortcut(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;

    let fallback_name = format!("Game {}", external_id);
    let game_name = connection
        .query_row(
            "
            SELECT name
            FROM games
            WHERE user_id = ?1 AND provider = ?2 AND external_id = ?3
            ",
            params![&user.id, &provider, &external_id],
            |record| record.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("Failed to query game name for desktop shortcut: {error}"))?
        .unwrap_or(fallback_name);

    Ok(create_provider_game_desktop_shortcut(&provider, &external_id, &game_name)?)
}

#[tauri::command]
pub(crate) fn open_game_recording_settings(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;

    if provider != "steam" {
        return Err(AppError::validation(
            "unsupported_provider",
            "Game recording settings are currently only available for Steam games.",
        ));
    }

    Ok(open_steam_game_recording_settings()?)
}
