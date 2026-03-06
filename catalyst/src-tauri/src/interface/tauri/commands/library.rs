use crate::*;
use crate::application::error::AppResult;
use tauri::State;

#[tauri::command]
pub(crate) fn get_library(state: State<'_, AppState>) -> AppResult<LibraryResponse> {
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
pub(crate) fn get_steam_status(state: State<'_, AppState>) -> AppResult<SteamStatusResponse> {
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
pub(crate) fn sync_steam_library(state: State<'_, AppState>) -> AppResult<SteamSyncResponse> {
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
pub(crate) fn set_game_favorite(
    provider: String,
    external_id: String,
    favorite: bool,
    state: State<'_, AppState>,
) -> AppResult<()> {
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
pub(crate) fn list_steam_downloads(state: State<'_, AppState>) -> AppResult<Vec<SteamDownloadProgressResponse>> {
    let owned_games_by_app_id = match open_connection(&state.db_path) {
        Ok(connection) => {
            if let Err(error) = cleanup_expired_sessions(&connection) {
                eprintln!(
                    "Steam download tracking: failed to cleanup expired sessions ({error}); continuing without ownership map."
                );
                HashMap::new()
            } else {
                match get_authenticated_user(state.inner(), &connection) {
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
