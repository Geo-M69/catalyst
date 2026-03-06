use crate::*;
use crate::application::error::{AppError, AppResult};
use tauri::State;

#[tauri::command]
pub(crate) fn list_game_versions_betas(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<GameVersionBetasResponse> {
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
pub(crate) fn validate_game_beta_access_code(
    provider: String,
    external_id: String,
    access_code: String,
    state: State<'_, AppState>,
) -> AppResult<GameBetaAccessCodeValidationResponse> {
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
pub(crate) fn import_steam_collections(state: State<'_, AppState>) -> AppResult<SteamCollectionsImportResponse> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let steam_id = user
        .steam_id
        .as_deref()
        .ok_or_else(|| AppError::unauthorized("steam_not_linked", "Steam is not linked for this account"))?;
    let steam_root = resolve_steam_root_path(state.steam_root_override.as_deref())
        .ok_or_else(|| AppError::not_found("steam_install_not_found", "Could not locate local Steam installation"))?;
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
        return Err(AppError::not_found(
            "steam_collection_config_not_found",
            format!(
                "Could not locate Steam collection config files for account {steam_id} in {}",
                userdata_directory.display()
            ),
        ));
    }

    if combined_collections_by_app_id.is_empty() {
        let files_label = if loaded_config_paths.is_empty() {
            String::from("none")
        } else {
            loaded_config_paths.join(", ")
        };
        return Err(AppError::validation(
            "steam_collections_empty",
            format!(
                "No Steam collections were found in local Steam configuration. Checked files: {files_label}"
            ),
        ));
    }

    Ok(import_steam_collections_for_user(
        &connection,
        &user.id,
        combined_collections_by_app_id,
    )?)
}
