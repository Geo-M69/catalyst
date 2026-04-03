use crate::*;
use crate::infrastructure::runtime_vdf::{
    VdfValue, parse_vdf_document, serialize_vdf_document, vdf_ensure_object_path_mut,
    vdf_find_object_value, vdf_get_text_entry, vdf_remove_entry, vdf_set_text_entry,
};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use std::{collections::HashSet, fs, path::Path, time::SystemTime};

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

pub(crate) fn normalize_game_properties_settings_payload(
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

pub(crate) fn load_game_properties_settings(
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

pub(crate) fn save_game_properties_settings(
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

pub(crate) fn resolve_steam_compatibility_tools(
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

pub(crate) fn clear_steam_game_overlay_data(state: &AppState, user: &UserRow, app_id: u64) -> Result<(), String> {
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

pub(crate) fn apply_steam_game_privacy_settings(
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

pub(crate) fn apply_steam_game_properties_settings(
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

pub(crate) fn load_game_privacy_settings(
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

pub(crate) fn save_game_privacy_settings(
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
