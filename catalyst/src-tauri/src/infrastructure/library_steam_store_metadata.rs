use chrono::{Duration as ChronoDuration, Utc};
use url::Url;

use crate::application::canonicalizer::build_canonical_features;
use crate::application::contracts::library::{FeatureResponse, GameStoreMetadataResponse};
use crate::application::error::AppResult;
use crate::build_http_client;
use crate::cache_steam_app_details;
use crate::cache_steam_app_features;
use crate::cleanup_expired_sessions;
use crate::ensure_owned_game_exists;
use crate::find_cached_steam_app_details;
use crate::find_cached_steam_app_features;
use crate::get_authenticated_user;
use crate::normalize_game_identity_input;
use crate::open_connection;
use crate::AppState;
use crate::STEAM_APP_DETAILS_CACHE_TTL_HOURS;

fn empty_game_store_metadata_response() -> GameStoreMetadataResponse {
    GameStoreMetadataResponse {
        developers: None,
        publishers: None,
        franchise: None,
        release_date: None,
        short_description: None,
        header_image: None,
        has_achievements: None,
        achievements_count: None,
        has_cloud_saves: None,
        cloud_details: None,
        controller_support: None,
        features: None,
    }
}

pub(crate) fn get_game_store_metadata(
    state: &AppState,
    provider: String,
    external_id: String,
) -> AppResult<GameStoreMetadataResponse> {
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

    // Only Steam is supported for rich store metadata at the moment.
    if normalized_provider != "steam" {
        return Ok(empty_game_store_metadata_response());
    }

    let app_id = match normalized_external_id.parse::<u64>() {
        Ok(v) => v,
        Err(_) => return Ok(empty_game_store_metadata_response()),
    };

    let stale_before = Utc::now() - ChronoDuration::hours(STEAM_APP_DETAILS_CACHE_TTL_HOURS);

    let mut response = empty_game_store_metadata_response();

    // Keep parsed store data (if available) for normalized feature building.
    let mut maybe_data: Option<serde_json::Value> = None;

    if let Ok(Some(cached)) = find_cached_steam_app_details(&connection, app_id, stale_before) {
        if let Some(data) = cached.get("data") {
            maybe_data = Some(data.clone());

            if let Some(devs) = data.get("developers").and_then(serde_json::Value::as_array) {
                let out: Vec<String> = devs
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect();
                if !out.is_empty() {
                    response.developers = Some(out);
                }
            }
            if let Some(pubs) = data.get("publishers").and_then(serde_json::Value::as_array) {
                let out: Vec<String> = pubs
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect();
                if !out.is_empty() {
                    response.publishers = Some(out);
                }
            }

            response.franchise = data
                .get("franchise")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
                .or_else(|| {
                    data.get("series")
                        .and_then(serde_json::Value::as_array)
                        .map(|arr| {
                            arr.iter()
                                .filter_map(serde_json::Value::as_str)
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                });

            response.release_date = data
                .get("release_date")
                .and_then(|value| value.get("date"))
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
                .or_else(|| {
                    data.get("release_date")
                        .and_then(serde_json::Value::as_str)
                        .map(ToOwned::to_owned)
                });

            if let Some(sd) = data
                .get("short_description")
                .and_then(serde_json::Value::as_str)
            {
                response.short_description = Some(sd.to_owned());
            }
            if let Some(header) = data.get("header_image").and_then(serde_json::Value::as_str) {
                response.header_image = Some(header.to_owned());
            }
        }
    }

    // If cache is missing useful data, attempt a live fetch through Steam's
    // appdetails endpoint using Rust-native HTTP parsing only.
    if response.short_description.is_none()
        || response.developers.is_none()
        || response.publishers.is_none()
        || response.franchise.is_none()
        || response.release_date.is_none()
        || response.header_image.is_none()
    {
        if let Ok(client) = build_http_client() {
            if response.short_description.is_none()
                || response.developers.is_none()
                || response.publishers.is_none()
                || response.franchise.is_none()
                || response.release_date.is_none()
                || response.header_image.is_none()
            {
                let mut request_url = match url::Url::parse(crate::STEAM_APP_DETAILS_ENDPOINT) {
                    Ok(url) => url,
                    Err(_) => Url::parse("https://store.steampowered.com/api/appdetails").unwrap(),
                };
                request_url
                    .query_pairs_mut()
                    .append_pair("appids", &app_id.to_string())
                    .append_pair("l", "english");

                if let Ok(resp) = client.get(request_url).send() {
                    if resp.status().is_success() {
                        if let Ok(payload) = resp.json::<serde_json::Value>() {
                            if let Some(entry) = payload.get(&app_id.to_string()) {
                                if entry
                                    .get("success")
                                    .and_then(serde_json::Value::as_bool)
                                    .unwrap_or(false)
                                {
                                    if let Some(data) = entry.get("data") {
                                        maybe_data = Some(data.clone());
                                        let _ = cache_steam_app_details(&connection, app_id, data);

                                        let has_achievements = data.get("achievements").is_some();
                                        let has_cloud = data
                                            .get("cloud")
                                            .and_then(|value| {
                                                value
                                                    .get("enabled")
                                                    .and_then(serde_json::Value::as_bool)
                                            })
                                            .unwrap_or_else(|| data.get("cloud").is_some());

                                        let mut controller_support: Option<String> = None;
                                        if let Some(categories) = data
                                            .get("categories")
                                            .and_then(serde_json::Value::as_array)
                                        {
                                            for category in categories {
                                                if let Some(description) = category
                                                    .get("description")
                                                    .and_then(serde_json::Value::as_str)
                                                {
                                                    let lowered = description.to_ascii_lowercase();
                                                    if lowered.contains("full controller")
                                                        || lowered
                                                            .contains("full controller support")
                                                    {
                                                        controller_support =
                                                            Some(String::from("Full"));
                                                        break;
                                                    }
                                                    if lowered.contains("partial controller")
                                                        || lowered
                                                            .contains("partial controller support")
                                                    {
                                                        controller_support =
                                                            Some(String::from("Partial"));
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                        if controller_support.is_none() {
                                            controller_support = data
                                                .get("controller_support")
                                                .and_then(serde_json::Value::as_str)
                                                .map(ToOwned::to_owned)
                                                .or_else(|| {
                                                    data.get("controller_supports")
                                                        .and_then(serde_json::Value::as_str)
                                                        .map(ToOwned::to_owned)
                                                });
                                        }

                                        let _ = cache_steam_app_features(
                                            &connection,
                                            app_id,
                                            has_achievements,
                                            None,
                                            has_cloud,
                                            None,
                                            controller_support.as_deref(),
                                        );

                                        if let Some(devs) = data
                                            .get("developers")
                                            .and_then(serde_json::Value::as_array)
                                        {
                                            let out: Vec<String> = devs
                                                .iter()
                                                .filter_map(serde_json::Value::as_str)
                                                .map(ToOwned::to_owned)
                                                .collect();
                                            if !out.is_empty() {
                                                response.developers = Some(out);
                                            }
                                        }
                                        if let Some(pubs) = data
                                            .get("publishers")
                                            .and_then(serde_json::Value::as_array)
                                        {
                                            let out: Vec<String> = pubs
                                                .iter()
                                                .filter_map(serde_json::Value::as_str)
                                                .map(ToOwned::to_owned)
                                                .collect();
                                            if !out.is_empty() {
                                                response.publishers = Some(out);
                                            }
                                        }
                                        if let Some(franchise) = data
                                            .get("franchise")
                                            .and_then(serde_json::Value::as_str)
                                        {
                                            response.franchise = Some(franchise.to_owned());
                                        }
                                        if let Some(release_date) = data
                                            .get("release_date")
                                            .and_then(|value| value.get("date"))
                                            .and_then(serde_json::Value::as_str)
                                        {
                                            response.release_date = Some(release_date.to_owned());
                                        }
                                        if let Some(short_description) = data
                                            .get("short_description")
                                            .and_then(serde_json::Value::as_str)
                                        {
                                            response.short_description =
                                                Some(short_description.to_owned());
                                        }
                                        if let Some(header) = data
                                            .get("header_image")
                                            .and_then(serde_json::Value::as_str)
                                        {
                                            response.header_image = Some(header.to_owned());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if let Ok(Some((has_ach, ach_count_opt, has_cloud, cloud_details_opt, controller_opt))) =
        find_cached_steam_app_features(&connection, app_id, stale_before)
    {
        response.has_achievements = Some(has_ach);
        response.achievements_count = ach_count_opt;
        response.has_cloud_saves = Some(has_cloud);
        response.cloud_details = cloud_details_opt;
        response.controller_support = controller_opt;
    }

    let features = build_canonical_features(
        maybe_data.as_ref(),
        response.has_achievements,
        response.achievements_count,
        response.has_cloud_saves,
        response.cloud_details.clone(),
        response.controller_support.clone(),
    );
    if !features.is_empty() {
        response.features = Some(
            features
                .into_iter()
                .map(|feature| FeatureResponse {
                    key: feature.key,
                    label: feature.label,
                    icon: feature.icon,
                    tooltip: feature.tooltip,
                })
                .collect(),
        );
    }

    Ok(response)
}
