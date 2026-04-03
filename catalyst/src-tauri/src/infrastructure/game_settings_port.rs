use crate::application::contracts::game_settings::{
    GameCompatibilitySettingsPayload, GameCompatibilityToolResponse, GameControllerSettingsPayload,
    GameCustomizationArtworkResponse, GameCustomizationSettingsPayload, GameGeneralSettingsPayload,
    GameInstallLocationResponse, GameInstallationDetailsResponse, GamePrivacySettingsResponse,
    GamePropertiesSettingsPayload, GameScreenshotResponse, GameUpdatesSettingsPayload,
    GameVersionsBetasSettingsPayload,
};
use crate::application::error::{AppError, AppResult};
use crate::application::ports::game_settings::GameSettingsPort;
use crate::domain::game::parse_steam_app_id;
use crate::{
    apply_steam_game_privacy_settings, apply_steam_game_properties_settings, build_http_client,
    cache_steam_app_languages, cleanup_expired_sessions, clear_steam_game_overlay_data,
    detect_available_disk_space_bytes, empty_game_customization_artwork_response,
    ensure_owned_game_exists, fetch_steam_app_linux_platform_support_from_store,
    fetch_steam_install_size_estimate_from_store, fetch_steam_supported_languages,
    find_cached_steam_app_languages, get_authenticated_user, load_game_privacy_settings,
    load_game_properties_settings, normalize_game_identity_input,
    normalize_game_properties_settings_payload, open_connection,
    parse_steam_manifest_size_on_disk_bytes, resolve_steam_compatibility_tools,
    resolve_steam_customization_artwork, resolve_steam_manifest_path_for_app_id,
    resolve_steam_root_path, resolve_steam_root_paths, resolve_steam_userdata_directory,
    resolve_steamapps_directories, save_game_privacy_settings, save_game_properties_settings,
    AppState, STEAM_APP_LANGUAGES_CACHE_TTL_HOURS,
};
use chrono::{Duration as ChronoDuration, Utc};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

#[derive(Clone)]
pub(crate) struct InfrastructureGameSettingsPort {
    state: AppState,
}

impl InfrastructureGameSettingsPort {
    pub(crate) fn new(state: &AppState) -> Self {
        Self {
            state: state.clone(),
        }
    }

    fn to_contract_compatibility_tool(
        value: crate::GameCompatibilityToolResponse,
    ) -> GameCompatibilityToolResponse {
        GameCompatibilityToolResponse {
            id: value.id,
            label: value.label,
        }
    }

    fn to_contract_compatibility_tools(
        values: Vec<crate::GameCompatibilityToolResponse>,
    ) -> Vec<GameCompatibilityToolResponse> {
        values
            .into_iter()
            .map(Self::to_contract_compatibility_tool)
            .collect()
    }

    fn to_contract_privacy_settings(
        value: crate::GamePrivacySettingsResponse,
    ) -> GamePrivacySettingsResponse {
        GamePrivacySettingsResponse {
            hide_in_library: value.hide_in_library,
            mark_as_private: value.mark_as_private,
            overlay_data_deleted: value.overlay_data_deleted,
        }
    }

    fn to_contract_properties_settings(
        value: crate::GamePropertiesSettingsPayload,
    ) -> GamePropertiesSettingsPayload {
        GamePropertiesSettingsPayload {
            general: GameGeneralSettingsPayload {
                language: value.general.language,
                launch_options: value.general.launch_options,
                steam_overlay_enabled: value.general.steam_overlay_enabled,
            },
            compatibility: GameCompatibilitySettingsPayload {
                force_steam_play_compatibility_tool: value
                    .compatibility
                    .force_steam_play_compatibility_tool,
                steam_play_compatibility_tool: value.compatibility.steam_play_compatibility_tool,
            },
            updates: GameUpdatesSettingsPayload {
                automatic_updates_mode: value.updates.automatic_updates_mode,
                background_downloads_mode: value.updates.background_downloads_mode,
            },
            controller: GameControllerSettingsPayload {
                steam_input_override: value.controller.steam_input_override,
            },
            customization: GameCustomizationSettingsPayload {
                custom_sort_name: value.customization.custom_sort_name,
            },
            game_versions_betas: GameVersionsBetasSettingsPayload {
                private_access_code: value.game_versions_betas.private_access_code,
                selected_version_id: value.game_versions_betas.selected_version_id,
            },
        }
    }

    fn to_runtime_properties_settings(
        value: GamePropertiesSettingsPayload,
    ) -> crate::GamePropertiesSettingsPayload {
        crate::GamePropertiesSettingsPayload {
            general: crate::GameGeneralSettingsPayload {
                language: value.general.language,
                launch_options: value.general.launch_options,
                steam_overlay_enabled: value.general.steam_overlay_enabled,
            },
            compatibility: crate::GameCompatibilitySettingsPayload {
                force_steam_play_compatibility_tool: value
                    .compatibility
                    .force_steam_play_compatibility_tool,
                steam_play_compatibility_tool: value.compatibility.steam_play_compatibility_tool,
            },
            updates: crate::GameUpdatesSettingsPayload {
                automatic_updates_mode: value.updates.automatic_updates_mode,
                background_downloads_mode: value.updates.background_downloads_mode,
            },
            controller: crate::GameControllerSettingsPayload {
                steam_input_override: value.controller.steam_input_override,
            },
            customization: crate::GameCustomizationSettingsPayload {
                custom_sort_name: value.customization.custom_sort_name,
            },
            game_versions_betas: crate::GameVersionsBetasSettingsPayload {
                private_access_code: value.game_versions_betas.private_access_code,
                selected_version_id: value.game_versions_betas.selected_version_id,
            },
        }
    }

    fn to_contract_customization_artwork(
        value: crate::GameCustomizationArtworkResponse,
    ) -> GameCustomizationArtworkResponse {
        GameCustomizationArtworkResponse {
            cover: value.cover,
            background: value.background,
            logo: value.logo,
            wide_cover: value.wide_cover,
        }
    }
}

impl GameSettingsPort for InfrastructureGameSettingsPort {
    fn list_game_languages(&self, provider: String, external_id: String) -> AppResult<Vec<String>> {
        self::list_game_languages(&self.state, provider, external_id)
    }

    fn list_game_compatibility_tools(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<Vec<GameCompatibilityToolResponse>> {
        self::list_game_compatibility_tools(&self.state, provider, external_id)
    }

    fn get_game_privacy_settings(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<GamePrivacySettingsResponse> {
        self::get_game_privacy_settings(&self.state, provider, external_id)
    }

    fn set_game_privacy_settings(
        &self,
        provider: String,
        external_id: String,
        hide_in_library: bool,
        mark_as_private: bool,
    ) -> AppResult<()> {
        self::set_game_privacy_settings(
            &self.state,
            provider,
            external_id,
            hide_in_library,
            mark_as_private,
        )
    }

    fn clear_game_overlay_data(&self, provider: String, external_id: String) -> AppResult<()> {
        self::clear_game_overlay_data(&self.state, provider, external_id)
    }

    fn get_game_properties_settings(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<GamePropertiesSettingsPayload> {
        self::get_game_properties_settings(&self.state, provider, external_id)
    }

    fn set_game_properties_settings(
        &self,
        provider: String,
        external_id: String,
        settings: GamePropertiesSettingsPayload,
    ) -> AppResult<()> {
        self::set_game_properties_settings(&self.state, provider, external_id, settings)
    }

    fn get_game_customization_artwork(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<GameCustomizationArtworkResponse> {
        self::get_game_customization_artwork(&self.state, provider, external_id)
    }

    fn get_game_screenshots(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<Vec<GameScreenshotResponse>> {
        self::get_game_screenshots(&self.state, provider, external_id)
    }

    fn get_game_installation_details(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<GameInstallationDetailsResponse> {
        self::get_game_installation_details(&self.state, provider, external_id)
    }

    fn get_game_install_size_estimate(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<Option<u64>> {
        self::get_game_install_size_estimate(&self.state, provider, external_id)
    }

    fn list_game_install_locations(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<Vec<GameInstallLocationResponse>> {
        self::list_game_install_locations(&self.state, provider, external_id)
    }
}

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
    match fetch_steam_supported_languages(&connection, &client, app_id) {
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
    let include_linux_runtime_tools = match build_http_client().and_then(|client| {
        fetch_steam_app_linux_platform_support_from_store(&connection, &client, app_id)
    }) {
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

    let tools = resolve_steam_compatibility_tools(
        state.steam_root_override.as_deref(),
        include_linux_runtime_tools,
    )?;
    Ok(InfrastructureGameSettingsPort::to_contract_compatibility_tools(tools))
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

    let settings = load_game_privacy_settings(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )?;
    Ok(InfrastructureGameSettingsPort::to_contract_privacy_settings(settings))
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
        let app_id = parse_steam_app_id(&normalized_external_id)?;
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
        let app_id = parse_steam_app_id(&normalized_external_id)?;
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

    let settings = load_game_properties_settings(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
    )?;
    Ok(InfrastructureGameSettingsPort::to_contract_properties_settings(settings))
}

pub(crate) fn get_game_screenshots(
    state: &AppState,
    provider: String,
    external_id: String,
) -> AppResult<Vec<GameScreenshotResponse>> {
    use std::time::SystemTime;
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

    let Some(steam_id) = user
        .steam_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(Vec::new());
    };

    let steam_root =
        resolve_steam_root_path(state.steam_root_override.as_deref()).ok_or_else(|| {
            AppError::not_found(
                "steam_install_not_found",
                "Could not locate local Steam installation",
            )
        })?;
    let userdata_directory = resolve_steam_userdata_directory(&steam_root, steam_id)?;

    let app_id = parse_steam_app_id(&normalized_external_id)?;

    // Common Steam screenshots path: userdata/<steamid>/760/remote/<app_id>
    let candidate_dir = userdata_directory
        .join("760")
        .join("remote")
        .join(app_id.to_string());
    if !candidate_dir.is_dir() {
        return Ok(Vec::new());
    }

    fn collect_image_files(
        dir: &std::path::Path,
        out: &mut Vec<std::path::PathBuf>,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let _ = collect_image_files(&path, out);
            } else if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    let ext = ext.to_ascii_lowercase();
                    if ext == "png" || ext == "jpg" || ext == "jpeg" {
                        out.push(path.clone());
                    }
                }
            }
        }
        Ok(())
    }

    let mut found: Vec<std::path::PathBuf> = Vec::new();
    let _ = collect_image_files(&candidate_dir, &mut found);

    // Sort by modified time desc
    found.sort_by(|a, b| {
        let ma = std::fs::metadata(a)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let mb = std::fs::metadata(b)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        mb.cmp(&ma)
    });

    const MAX: usize = 24;
    found.truncate(MAX);

    let results = found
        .into_iter()
        .map(|path| GameScreenshotResponse {
            id: path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
                .unwrap_or_default(),
            path: path.to_string_lossy().to_string(),
            thumbnail_path: None,
        })
        .collect();

    Ok(results)
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

    let runtime_settings = InfrastructureGameSettingsPort::to_runtime_properties_settings(settings);
    let normalized_settings = normalize_game_properties_settings_payload(runtime_settings);
    save_game_properties_settings(
        &connection,
        &user.id,
        &normalized_provider,
        &normalized_external_id,
        &normalized_settings,
    )?;

    if normalized_provider == "steam" {
        let app_id = parse_steam_app_id(&normalized_external_id)?;
        if let Err(error) =
            apply_steam_game_properties_settings(state, &user, app_id, &normalized_settings)
        {
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
        return Ok(
            InfrastructureGameSettingsPort::to_contract_customization_artwork(
                empty_game_customization_artwork_response(),
            ),
        );
    }

    let Some(steam_id) = user
        .steam_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(
            InfrastructureGameSettingsPort::to_contract_customization_artwork(
                empty_game_customization_artwork_response(),
            ),
        );
    };

    let artwork = resolve_steam_customization_artwork(
        state.steam_root_override.as_deref(),
        steam_id,
        &normalized_external_id,
    );
    Ok(InfrastructureGameSettingsPort::to_contract_customization_artwork(artwork))
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

    let manifest_path = match resolve_steam_manifest_path_for_app_id(
        state.steam_root_override.as_deref(),
        app_id,
    ) {
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
            if let Some(size_on_disk_bytes) =
                parse_steam_manifest_size_on_disk_bytes(&manifest_contents)
            {
                return Ok(Some(size_on_disk_bytes));
            }
        }
    }

    let client = build_http_client()?;
    Ok(fetch_steam_install_size_estimate_from_store(
        &connection,
        &client,
        app_id,
    )?)
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
