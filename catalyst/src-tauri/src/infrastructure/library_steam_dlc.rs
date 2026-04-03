use crate::application::contracts::library::{GameDlcEntryResponse, GameDlcResponse};
use crate::application::error::AppResult;
use crate::{
    build_http_client, cache_steam_app_details, cleanup_expired_sessions, ensure_owned_game_exists,
    find_cached_steam_app_details, get_authenticated_user, normalize_backend_warning_message,
    normalize_game_identity_input, open_connection, AppState, STEAM_APP_DETAILS_CACHE_TTL_HOURS,
};
use chrono::{Duration as ChronoDuration, Utc};
use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet};
use url::Url;

fn empty_game_dlc_response(
    provider: &str,
    external_id: &str,
    warning: Option<String>,
) -> GameDlcResponse {
    GameDlcResponse {
        provider: provider.to_owned(),
        external_id: external_id.to_owned(),
        entries: Vec::new(),
        warning,
        last_synced_at: Utc::now().to_rfc3339(),
    }
}

fn append_warning(existing_warning: &mut Option<String>, next_warning: impl Into<String>) {
    let next_warning = next_warning.into();
    let next_warning = next_warning.trim();
    if next_warning.is_empty() {
        return;
    }

    match existing_warning {
        Some(current) => {
            if !current.is_empty() && !current.ends_with(' ') {
                current.push(' ');
            }
            current.push_str(next_warning);
        }
        None => {
            *existing_warning = Some(next_warning.to_owned());
        }
    }
}

fn parse_steam_dlc_app_ids_from_data(data: &serde_json::Value) -> Vec<u64> {
    let mut app_ids = Vec::new();
    let mut seen_app_ids = HashSet::new();

    let Some(raw_dlc_entries) = data.get("dlc").and_then(serde_json::Value::as_array) else {
        return app_ids;
    };

    for raw_dlc_entry in raw_dlc_entries {
        let parsed_app_id = raw_dlc_entry.as_u64().or_else(|| {
            raw_dlc_entry
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .and_then(|value| value.parse::<u64>().ok())
        });

        if let Some(app_id) = parsed_app_id {
            if seen_app_ids.insert(app_id) {
                app_ids.push(app_id);
            }
        }
    }

    app_ids
}

fn extract_steam_app_name_from_details(details: &serde_json::Value) -> Option<String> {
    let data = details.get("data").unwrap_or(details);
    data.get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn resolve_steam_app_names_best_effort(
    connection: &Connection,
    client: &reqwest::blocking::Client,
    app_ids: &[u64],
    force_refresh: bool,
) -> (HashMap<u64, String>, Option<String>) {
    let mut names_by_app_id = HashMap::new();
    let mut warning: Option<String> = None;
    let mut missing_app_ids = Vec::new();
    let mut seen_app_ids = HashSet::new();

    let stale_before = Utc::now() - ChronoDuration::hours(STEAM_APP_DETAILS_CACHE_TTL_HOURS);
    for app_id in app_ids {
        if !seen_app_ids.insert(*app_id) {
            continue;
        }

        if !force_refresh {
            if let Ok(Some(cached_details)) =
                find_cached_steam_app_details(connection, *app_id, stale_before)
            {
                if let Some(cached_name) = extract_steam_app_name_from_details(&cached_details) {
                    names_by_app_id.insert(*app_id, cached_name);
                    continue;
                }
            }
        }

        missing_app_ids.push(*app_id);
    }

    const APP_DETAILS_BATCH_SIZE: usize = 50;
    for app_id_batch in missing_app_ids.chunks(APP_DETAILS_BATCH_SIZE) {
        let mut request_url = match url::Url::parse(crate::STEAM_APP_DETAILS_ENDPOINT) {
            Ok(url) => url,
            Err(error) => {
                append_warning(
                    &mut warning,
                    format!("Could not parse Steam appdetails endpoint: {error}"),
                );
                return (names_by_app_id, warning);
            }
        };
        let app_ids_param = app_id_batch
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        request_url
            .query_pairs_mut()
            .append_pair("appids", &app_ids_param)
            .append_pair("l", "english");

        let response = match client.get(request_url).send() {
            Ok(value) => value,
            Err(error) => {
                append_warning(
                    &mut warning,
                    format!("Steam appdetails request failed: {error}"),
                );
                continue;
            }
        };
        if !response.status().is_success() {
            append_warning(
                &mut warning,
                format!(
                    "Steam appdetails request failed with status {}",
                    response.status()
                ),
            );
            continue;
        }

        let payload = match response.json::<serde_json::Value>() {
            Ok(value) => value,
            Err(error) => {
                append_warning(
                    &mut warning,
                    format!("Could not decode Steam appdetails response: {error}"),
                );
                continue;
            }
        };

        for app_id in app_id_batch {
            let Some(entry) = payload.get(&app_id.to_string()) else {
                continue;
            };
            if !entry
                .get("success")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            {
                continue;
            }

            if let Some(name) = extract_steam_app_name_from_details(entry) {
                names_by_app_id.insert(*app_id, name);
            }

            let _ = cache_steam_app_details(connection, *app_id, entry);
        }
    }

    (names_by_app_id, warning)
}

fn normalize_dlc_name_for_dedupe(name: &str) -> String {
    let cleaned = name
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>();
    let ignored_tokens = [
        "dlc",
        "addon",
        "add",
        "on",
        "content",
        "pack",
        "package",
        "edition",
        "premium",
        "bundle",
        "deluxe",
        "collector",
        "collectors",
        "digital",
        "upgrade",
        "bonus",
        "preorder",
        "pre",
        "order",
        "ost",
        "soundtrack",
        "artbook",
        "mini",
    ];
    let mut normalized_tokens = Vec::new();
    for token in cleaned.split_whitespace() {
        if ignored_tokens.contains(&token) {
            continue;
        }
        normalized_tokens.push(token);
    }

    normalized_tokens.join(" ")
}

fn dlc_entry_is_placeholder_name(name: &str, external_id: &str) -> bool {
    let trimmed_name = name.trim();
    let expected = format!("DLC App {external_id}");
    trimmed_name.eq_ignore_ascii_case(&expected)
}

fn should_replace_collapsed_dlc_entry(
    existing: &GameDlcEntryResponse,
    candidate: &GameDlcEntryResponse,
) -> bool {
    if candidate.in_library != existing.in_library {
        return candidate.in_library;
    }
    if candidate.installed != existing.installed {
        return candidate.installed;
    }

    let existing_is_placeholder =
        dlc_entry_is_placeholder_name(&existing.name, &existing.external_id);
    let candidate_is_placeholder =
        dlc_entry_is_placeholder_name(&candidate.name, &candidate.external_id);
    if candidate_is_placeholder != existing_is_placeholder {
        return !candidate_is_placeholder;
    }

    let existing_len = existing.name.trim().len();
    let candidate_len = candidate.name.trim().len();
    if candidate_len != existing_len {
        // Prefer the shorter/canonical title once install/library status tie-breakers are equal.
        return candidate_len < existing_len;
    }

    candidate.external_id < existing.external_id
}

fn collapse_near_duplicate_dlc_entries(
    entries: Vec<GameDlcEntryResponse>,
) -> Vec<GameDlcEntryResponse> {
    let mut collapsed_entries = Vec::new();
    let mut index_by_key = HashMap::<String, usize>::new();

    for entry in entries {
        let normalized_key = normalize_dlc_name_for_dedupe(&entry.name);
        let dedupe_key = if normalized_key.trim().is_empty() {
            format!("app:{}", entry.external_id)
        } else {
            normalized_key
        };

        if let Some(existing_index) = index_by_key.get(&dedupe_key).copied() {
            if let Some(existing_entry) = collapsed_entries.get(existing_index) {
                if should_replace_collapsed_dlc_entry(existing_entry, &entry) {
                    collapsed_entries[existing_index] = entry;
                }
            }
            continue;
        }

        index_by_key.insert(dedupe_key, collapsed_entries.len());
        collapsed_entries.push(entry);
    }

    collapsed_entries.sort_by(|left, right| {
        left.name
            .to_ascii_lowercase()
            .cmp(&right.name.to_ascii_lowercase())
            .then_with(|| left.external_id.cmp(&right.external_id))
    });
    collapsed_entries
}

fn extract_cached_steam_dlc_app_ids(cached_details: &serde_json::Value) -> Option<Vec<u64>> {
    if let Some(data) = cached_details.get("data") {
        return Some(parse_steam_dlc_app_ids_from_data(data));
    }
    if cached_details.is_object() {
        return Some(parse_steam_dlc_app_ids_from_data(cached_details));
    }
    None
}

fn fetch_steam_dlc_app_ids(
    connection: &Connection,
    client: &reqwest::blocking::Client,
    app_id: u64,
    force_refresh: bool,
) -> Result<Vec<u64>, String> {
    let stale_before = Utc::now() - ChronoDuration::hours(STEAM_APP_DETAILS_CACHE_TTL_HOURS);
    if !force_refresh {
        if let Ok(Some(cached)) = find_cached_steam_app_details(connection, app_id, stale_before) {
            if let Some(cached_dlc_app_ids) = extract_cached_steam_dlc_app_ids(&cached) {
                return Ok(cached_dlc_app_ids);
            }
        }
    }

    let mut request_url = match url::Url::parse(crate::STEAM_APP_DETAILS_ENDPOINT) {
        Ok(url) => url,
        Err(_) => Url::parse("https://store.steampowered.com/api/appdetails")
            .map_err(|error| format!("Failed to parse Steam appdetails URL: {error}"))?,
    };
    request_url
        .query_pairs_mut()
        .append_pair("appids", &app_id.to_string())
        .append_pair("l", "english");

    let response = client
        .get(request_url)
        .send()
        .map_err(|error| format!("Steam appdetails request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam appdetails request failed with status {}",
            response.status()
        ));
    }

    let payload = response
        .json::<serde_json::Value>()
        .map_err(|error| format!("Failed to decode Steam appdetails payload: {error}"))?;
    let Some(entry) = payload.get(&app_id.to_string()) else {
        return Ok(Vec::new());
    };
    if !entry
        .get("success")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(Vec::new());
    }
    let Some(data) = entry.get("data") else {
        return Ok(Vec::new());
    };

    let _ = cache_steam_app_details(connection, app_id, entry);
    Ok(parse_steam_dlc_app_ids_from_data(data))
}

fn load_owned_steam_dlc_by_app_id(
    connection: &Connection,
    user_id: &str,
) -> Result<HashMap<u64, (String, bool)>, String> {
    let mut owned_dlc_by_app_id = HashMap::new();
    let mut statement = connection
        .prepare(
            "SELECT external_id, name, installed
			 FROM games
			 WHERE user_id = ?1 AND provider = 'steam' AND kind = 'dlc'",
        )
        .map_err(|error| format!("Failed to prepare owned Steam DLC query: {error}"))?;

    let rows = statement
        .query_map(params![user_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(|error| format!("Failed to query owned Steam DLC entries: {error}"))?;

    for row in rows {
        let (external_id, name, installed_raw) =
            row.map_err(|error| format!("Failed to decode owned Steam DLC row: {error}"))?;
        let Some(app_id) = external_id.parse::<u64>().ok() else {
            continue;
        };
        owned_dlc_by_app_id.insert(app_id, (name, installed_raw > 0));
    }

    Ok(owned_dlc_by_app_id)
}

pub(crate) fn get_game_dlc(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameDlcResponse> {
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
        return Ok(empty_game_dlc_response(
            &normalized_provider,
            &normalized_external_id,
            Some(String::from(
                "DLC details are currently available for Steam titles only.",
            )),
        ));
    }

    let app_id = match normalized_external_id.parse::<u64>() {
        Ok(value) => value,
        Err(_) => {
            return Ok(empty_game_dlc_response(
                &normalized_provider,
                &normalized_external_id,
                Some(String::from("This Steam app ID is invalid.")),
            ));
        }
    };

    let client = build_http_client()?;
    let mut response = empty_game_dlc_response(&normalized_provider, &normalized_external_id, None);

    let dlc_app_ids = match fetch_steam_dlc_app_ids(&connection, &client, app_id, force_refresh) {
        Ok(entries) => entries,
        Err(error) => {
            append_warning(
                &mut response.warning,
                format!(
                    "Could not load DLC metadata from Steam: {}",
                    normalize_backend_warning_message(&error)
                ),
            );
            Vec::new()
        }
    };

    let owned_dlc_by_app_id = match load_owned_steam_dlc_by_app_id(&connection, &user.id) {
        Ok(map) => map,
        Err(error) => {
            append_warning(
                &mut response.warning,
                format!(
                    "Could not resolve owned DLC status: {}",
                    normalize_backend_warning_message(&error)
                ),
            );
            HashMap::new()
        }
    };

    let (resolved_dlc_names_by_app_id, name_resolution_warning) =
        resolve_steam_app_names_best_effort(&connection, &client, &dlc_app_ids, force_refresh);
    if let Some(warning_message) = name_resolution_warning {
        append_warning(
            &mut response.warning,
            format!(
                "Could not resolve names for some DLC entries: {}",
                normalize_backend_warning_message(&warning_message)
            ),
        );
    }

    let mut entries = Vec::new();
    for dlc_app_id in dlc_app_ids {
        let maybe_owned_dlc = owned_dlc_by_app_id.get(&dlc_app_id);
        let external_id_value = dlc_app_id.to_string();
        let entry_name = maybe_owned_dlc
            .map(|(name, _)| name.clone())
            .or_else(|| resolved_dlc_names_by_app_id.get(&dlc_app_id).cloned())
            .unwrap_or_else(|| format!("DLC App {external_id_value}"));
        let installed = maybe_owned_dlc
            .map(|(_, installed_value)| *installed_value)
            .unwrap_or(false);
        let in_library = maybe_owned_dlc.is_some();

        entries.push(GameDlcEntryResponse {
            id: format!("steam:{external_id_value}"),
            provider: String::from("steam"),
            external_id: external_id_value.clone(),
            name: entry_name,
            installed,
            in_library,
            store_url: format!("{}/{}", crate::STEAM_STORE_APP_ENDPOINT, external_id_value),
        });
    }

    response.entries = collapse_near_duplicate_dlc_entries(entries);
    response.last_synced_at = Utc::now().to_rfc3339();
    Ok(response)
}
