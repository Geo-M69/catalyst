use crate::*;
use crate::application::error::{AppError, AppResult};
use chrono::Duration as ChronoDuration;

pub(crate) fn list_game_languages(
	state: &AppState,
	provider: String,
	external_id: String,
) -> AppResult<Vec<String>> {
	let connection = open_connection(&state.db_path)?;
	cleanup_expired_sessions(&connection)?;
	let user = get_authenticated_user(state, &connection)?;
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

			Err(fetch_error.into())
		}
	}
}

pub(crate) fn list_game_compatibility_tools(
	state: &AppState,
	provider: String,
	external_id: String,
) -> AppResult<Vec<GameCompatibilityToolResponse>> {
	let connection = open_connection(&state.db_path)?;
	cleanup_expired_sessions(&connection)?;
	let user = get_authenticated_user(state, &connection)?;
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

	Ok(resolve_steam_compatibility_tools(
		state.steam_root_override.as_deref(),
		include_linux_runtime_tools,
	)?)
}

pub(crate) fn get_game_privacy_settings(
	state: &AppState,
	provider: String,
	external_id: String,
) -> AppResult<GamePrivacySettingsResponse> {
	let connection = open_connection(&state.db_path)?;
	cleanup_expired_sessions(&connection)?;
	let user = get_authenticated_user(state, &connection)?;
	let (normalized_provider, normalized_external_id) =
		normalize_game_identity_input(&provider, &external_id)?;
	ensure_owned_game_exists(
		&connection,
		&user.id,
		&normalized_provider,
		&normalized_external_id,
	)?;

	Ok(load_game_privacy_settings(
		&connection,
		&user.id,
		&normalized_provider,
		&normalized_external_id,
	)?)
}

pub(crate) fn set_game_privacy_settings(
	state: &AppState,
	provider: String,
	external_id: String,
	hide_in_library: bool,
	mark_as_private: bool,
) -> AppResult<()> {
	let connection = open_connection(&state.db_path)?;
	cleanup_expired_sessions(&connection)?;
	let user = get_authenticated_user(state, &connection)?;
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

	if normalized_provider == "steam" {
		let app_id = normalized_external_id
			.parse::<u64>()
			.map_err(|_| AppError::validation("invalid_external_id", "Steam external_id must be a numeric app ID"))?;
		apply_steam_game_privacy_settings(state, &user, app_id, &settings)?;
	}

	Ok(save_game_privacy_settings(
		&connection,
		&user.id,
		&normalized_provider,
		&normalized_external_id,
		settings,
	)?)
}

pub(crate) fn clear_game_overlay_data(
	state: &AppState,
	provider: String,
	external_id: String,
) -> AppResult<()> {
	let connection = open_connection(&state.db_path)?;
	cleanup_expired_sessions(&connection)?;
	let user = get_authenticated_user(state, &connection)?;
	let (normalized_provider, normalized_external_id) =
		normalize_game_identity_input(&provider, &external_id)?;
	ensure_owned_game_exists(
		&connection,
		&user.id,
		&normalized_provider,
		&normalized_external_id,
	)?;

	if normalized_provider == "steam" {
		let app_id = normalized_external_id
			.parse::<u64>()
			.map_err(|_| AppError::validation("invalid_external_id", "Steam external_id must be a numeric app ID"))?;
		clear_steam_game_overlay_data(state, &user, app_id)?;
	}

	let mut settings = load_game_privacy_settings(
		&connection,
		&user.id,
		&normalized_provider,
		&normalized_external_id,
	)?;
	settings.overlay_data_deleted = true;
	Ok(save_game_privacy_settings(
		&connection,
		&user.id,
		&normalized_provider,
		&normalized_external_id,
		settings,
	)?)
}

pub(crate) fn get_game_properties_settings(
	state: &AppState,
	provider: String,
	external_id: String,
) -> AppResult<GamePropertiesSettingsPayload> {
	let connection = open_connection(&state.db_path)?;
	cleanup_expired_sessions(&connection)?;
	let user = get_authenticated_user(state, &connection)?;
	let (normalized_provider, normalized_external_id) =
		normalize_game_identity_input(&provider, &external_id)?;
	ensure_owned_game_exists(
		&connection,
		&user.id,
		&normalized_provider,
		&normalized_external_id,
	)?;

	Ok(load_game_properties_settings(
		&connection,
		&user.id,
		&normalized_provider,
		&normalized_external_id,
	)?)
}

pub(crate) fn set_game_properties_settings(
	state: &AppState,
	provider: String,
	external_id: String,
	settings: GamePropertiesSettingsPayload,
) -> AppResult<()> {
	let connection = open_connection(&state.db_path)?;
	cleanup_expired_sessions(&connection)?;
	let user = get_authenticated_user(state, &connection)?;
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
			.map_err(|_| AppError::validation("invalid_external_id", "Steam external_id must be a numeric app ID"))?;
		if let Err(error) = apply_steam_game_properties_settings(
			state,
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

pub(crate) fn get_game_customization_artwork(
	state: &AppState,
	provider: String,
	external_id: String,
) -> AppResult<GameCustomizationArtworkResponse> {
	let connection = open_connection(&state.db_path)?;
	cleanup_expired_sessions(&connection)?;
	let user = get_authenticated_user(state, &connection)?;
	let (normalized_provider, normalized_external_id) =
		normalize_game_identity_input(&provider, &external_id)?;
	ensure_owned_game_exists(
		&connection,
		&user.id,
		&normalized_provider,
		&normalized_external_id,
	)?;

	if normalized_provider != "steam" || normalized_external_id.parse::<u64>().is_err() {
		return Ok(empty_game_customization_artwork_response());
	}

	let Some(steam_id) = user
		.steam_id
		.as_deref()
		.map(str::trim)
		.filter(|value| !value.is_empty())
	else {
		return Ok(empty_game_customization_artwork_response());
	};

	Ok(resolve_steam_customization_artwork(
		state.steam_root_override.as_deref(),
		steam_id,
		&normalized_external_id,
	))
}

pub(crate) fn get_game_installation_details(
	state: &AppState,
	provider: String,
	external_id: String,
) -> AppResult<GameInstallationDetailsResponse> {
	let connection = open_connection(&state.db_path)?;
	cleanup_expired_sessions(&connection)?;
	let user = get_authenticated_user(state, &connection)?;
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

pub(crate) fn get_game_install_size_estimate(
	state: &AppState,
	provider: String,
	external_id: String,
) -> AppResult<Option<u64>> {
	let connection = open_connection(&state.db_path)?;
	cleanup_expired_sessions(&connection)?;
	let user = get_authenticated_user(state, &connection)?;
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
	Ok(fetch_steam_install_size_estimate_from_store(&client, app_id)?)
}

pub(crate) fn list_game_install_locations(
	state: &AppState,
	provider: String,
	external_id: String,
) -> AppResult<Vec<GameInstallLocationResponse>> {
	let connection = open_connection(&state.db_path)?;
	cleanup_expired_sessions(&connection)?;
	let user = get_authenticated_user(state, &connection)?;
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

	let steam_roots = resolve_steam_root_paths(state.steam_root_override.as_deref());
	if steam_roots.is_empty() {
		return Ok(Vec::new());
	}

	let mut locations = Vec::new();
	let mut seen_paths = HashSet::new();
	for steam_root in &steam_roots {
		let steamapps_directories = match resolve_steamapps_directories(steam_root) {
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
	}

	if locations.is_empty() {
		for steam_root in steam_roots {
			let path_label = steam_root.display().to_string();
			let normalized_key = path_label.to_ascii_lowercase();
			if !seen_paths.insert(normalized_key) {
				continue;
			}
			locations.push(GameInstallLocationResponse {
				free_space_bytes: detect_available_disk_space_bytes(&steam_root),
				path: path_label,
			});
		}
	}

	Ok(locations)
}

