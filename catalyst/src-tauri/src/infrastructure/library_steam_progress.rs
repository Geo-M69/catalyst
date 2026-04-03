use crate::application::error::AppResult;
use crate::application::contracts::library::{
    GameAchievementEntryResponse,
    GameAchievementsResponse,
    GameTradingCardEntryResponse,
    GameTradingCardsResponse,
};
use crate::infrastructure::cache_adapter::CacheAdapter;
use crate::{
    AppState,
    STEAM_APP_DETAILS_CACHE_TTL_HOURS,
    build_http_client,
    cache_steam_app_details,
    cleanup_expired_sessions,
    ensure_owned_game_exists,
    find_cached_steam_app_details,
    get_authenticated_user,
    normalize_backend_warning_message,
    normalize_game_identity_input,
    open_connection,
};
use chrono::{Duration as ChronoDuration, Utc};
use rusqlite::Connection;
use scraper::{Html, Selector};
use std::collections::HashMap;
use url::Url;

const STEAM_WEB_API_PLAYER_ACHIEVEMENTS_ENDPOINT: &str =
    "https://api.steampowered.com/ISteamUserStats/GetPlayerAchievements/v1/";
const STEAM_WEB_API_GAME_SCHEMA_ENDPOINT: &str =
    "https://api.steampowered.com/ISteamUserStats/GetSchemaForGame/v2/";
const STEAM_WEB_API_BADGES_ENDPOINT: &str =
    "https://api.steampowered.com/IPlayerService/GetBadges/v1/";
const STEAM_ACHIEVEMENTS_CACHE_TTL_SECONDS: i64 = 15 * 60;
const STEAM_ACHIEVEMENTS_CACHE_VERSION: &str = "v1";
const STEAM_TRADING_CARDS_CACHE_TTL_SECONDS: i64 = 15 * 60;
const STEAM_TRADING_CARDS_CACHE_VERSION: &str = "v1";

#[derive(serde::Deserialize)]
struct SteamBadgesApiResponse {
    response: Option<SteamBadgesPayload>,
}

#[derive(serde::Deserialize)]
struct SteamBadgesPayload {
    #[serde(default)]
    badges: Vec<SteamBadgeEntry>,
}

#[derive(serde::Deserialize)]
struct SteamBadgeEntry {
    appid: Option<u64>,
    level: Option<i64>,
    xp: Option<i64>,
}

#[derive(Clone)]
struct SteamAchievementSchemaEntry {
    display_name: Option<String>,
    description: Option<String>,
    icon: Option<String>,
}

#[derive(serde::Deserialize)]
struct SteamPlayerAchievementsApiResponse {
    playerstats: Option<SteamPlayerAchievementsPayload>,
}

#[derive(serde::Deserialize)]
struct SteamPlayerAchievementsPayload {
    success: Option<bool>,
    #[serde(default)]
    achievements: Vec<SteamPlayerAchievementEntry>,
}

#[derive(serde::Deserialize)]
struct SteamPlayerAchievementEntry {
    apiname: Option<String>,
    achieved: Option<u8>,
    unlocktime: Option<i64>,
}

#[derive(serde::Deserialize)]
struct SteamGameSchemaApiResponse {
    game: Option<SteamGameSchemaPayload>,
}

#[derive(serde::Deserialize)]
struct SteamGameSchemaPayload {
    #[serde(rename = "availableGameStats")]
    available_game_stats: Option<SteamGameSchemaStats>,
}

#[derive(serde::Deserialize)]
struct SteamGameSchemaStats {
    #[serde(default)]
    achievements: Vec<SteamGameSchemaAchievement>,
}

#[derive(serde::Deserialize)]
struct SteamGameSchemaAchievement {
    name: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    description: Option<String>,
    icon: Option<String>,
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

fn empty_game_trading_cards_response(
    provider: &str,
    external_id: &str,
    supported: bool,
    view_url: String,
    warning: Option<String>,
) -> GameTradingCardsResponse {
    GameTradingCardsResponse {
        provider: provider.to_owned(),
        external_id: external_id.to_owned(),
        supported,
        badge_level: None,
        badge_xp: None,
        total_cards: 0,
        owned_cards: 0,
        cards: Vec::new(),
        warning,
        view_url,
        last_synced_at: Utc::now().to_rfc3339(),
    }
}

fn steam_store_data_supports_trading_cards(data: &serde_json::Value) -> bool {
    if let Some(categories) = data.get("categories").and_then(serde_json::Value::as_array) {
        for category in categories {
            let description = category
                .get("description")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .unwrap_or_default()
                .to_ascii_lowercase();
            if description.contains("trading card") || description.contains("trading cards") {
                return true;
            }
        }
    }

    let serialized = data.to_string().to_ascii_lowercase();
    serialized.contains("trading card") || serialized.contains("trading cards")
}

fn extract_cached_trading_cards_support(cached_details: &serde_json::Value) -> Option<bool> {
    if let Some(data) = cached_details.get("data") {
        return Some(steam_store_data_supports_trading_cards(data));
    }
    if cached_details.is_object() {
        return Some(steam_store_data_supports_trading_cards(cached_details));
    }
    None
}

fn resolve_steam_trading_cards_support(
    connection: &Connection,
    client: &reqwest::blocking::Client,
    app_id: u64,
) -> Result<Option<bool>, String> {
    let stale_before = Utc::now() - ChronoDuration::hours(STEAM_APP_DETAILS_CACHE_TTL_HOURS);
    if let Ok(Some(cached)) = find_cached_steam_app_details(connection, app_id, stale_before) {
        if let Some(cached_support) = extract_cached_trading_cards_support(&cached) {
            return Ok(Some(cached_support));
        }
    }

    let mut request_url = match Url::parse(crate::STEAM_APP_DETAILS_ENDPOINT) {
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
        return Ok(None);
    };
    if !entry
        .get("success")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(None);
    }
    let Some(data) = entry.get("data") else {
        return Ok(None);
    };

    // Keep cache compatible with callers that expect `details_json` to include a top-level `data` field.
    let _ = cache_steam_app_details(connection, app_id, entry);
    Ok(Some(steam_store_data_supports_trading_cards(data)))
}

fn decode_html_entities(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn normalize_steam_community_asset_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }
    let decoded = decode_html_entities(trimmed);
    if decoded.starts_with("http://") || decoded.starts_with("https://") {
        return Some(decoded);
    }
    if decoded.starts_with("//") {
        return Some(format!("https:{decoded}"));
    }
    if decoded.starts_with('/') {
        return Some(format!("https://steamcommunity.com{decoded}"));
    }
    Some(decoded)
}

fn extract_style_background_image_url(style_value: &str) -> Option<String> {
    let lower = style_value.to_ascii_lowercase();
    let url_start = lower.find("url(")?;
    let after_marker = &style_value[(url_start + 4)..];
    let mut end_offset = after_marker.find(')')?;
    while end_offset > 0 && after_marker[..end_offset].ends_with(char::is_whitespace) {
        end_offset -= 1;
    }
    let candidate = after_marker[..end_offset]
        .trim()
        .trim_matches('\'')
        .trim_matches('"');
    normalize_steam_community_asset_url(candidate)
}

fn parse_trading_card_qty(raw_text: &str) -> i64 {
    let digits = raw_text
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return 0;
    }
    digits.parse::<i64>().unwrap_or(0)
}

fn compact_whitespace(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_owned()
}

fn parse_steam_gamecards_page_cards(html: &str, app_id: u64) -> Vec<GameTradingCardEntryResponse> {
    let document = Html::parse_document(html);
    let card_selector = match Selector::parse(".badge_card_set_card") {
        Ok(selector) => selector,
        Err(_) => return Vec::new(),
    };
    let title_selector = Selector::parse(".badge_card_set_title").ok();
    let qty_selector = Selector::parse(".badge_card_set_text_qty").ok();
    let image_selector = Selector::parse("img").ok();
    let all_descendants_selector = Selector::parse("*").ok();

    let mut cards = Vec::new();
    for (index, card_element) in document.select(&card_selector).enumerate() {
        let card_name = title_selector
            .as_ref()
            .and_then(|selector| card_element.select(selector).next())
            .map(|title| compact_whitespace(&title.text().collect::<Vec<_>>().join(" ")))
            .unwrap_or_default();

        let qty_text = qty_selector
            .as_ref()
            .and_then(|selector| card_element.select(selector).next())
            .map(|qty| qty.text().collect::<Vec<_>>().join(" "))
            .unwrap_or_default();
        let owned_count = parse_trading_card_qty(&qty_text);

        let mut image_url = image_selector
            .as_ref()
            .and_then(|selector| card_element.select(selector).next())
            .and_then(|image| image.value().attr("src"))
            .and_then(normalize_steam_community_asset_url);
        if image_url.is_none() {
            if let Some(selector) = all_descendants_selector.as_ref() {
                for node in card_element.select(selector) {
                    let Some(style_value) = node.value().attr("style") else {
                        continue;
                    };
                    if let Some(url) = extract_style_background_image_url(style_value) {
                        image_url = Some(url);
                        break;
                    }
                }
            }
        }

        if card_name.is_empty() && image_url.is_none() {
            continue;
        }

        let fallback_name = format!("Card {}", index + 1);
        cards.push(GameTradingCardEntryResponse {
            id: format!("steam:{app_id}:card:{}", index + 1),
            name: if card_name.is_empty() {
                fallback_name
            } else {
                card_name
            },
            image_url,
            owned_count,
            is_owned: owned_count > 0,
        });
    }

    cards
}

fn fetch_steam_gamecards_page_cards(
    client: &reqwest::blocking::Client,
    steam_id: &str,
    app_id: u64,
) -> Result<Vec<GameTradingCardEntryResponse>, String> {
    let trimmed_steam_id = steam_id.trim();
    if trimmed_steam_id.is_empty() {
        return Err(String::from(
            "Steam account ID is missing; cannot resolve trading-card page.",
        ));
    }

    let gamecards_url = format!(
        "https://steamcommunity.com/profiles/{trimmed_steam_id}/gamecards/{app_id}/?l=english"
    );
    let response = client
        .get(&gamecards_url)
        .send()
        .map_err(|error| format!("Steam gamecards page request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam gamecards page request failed with status {}",
            response.status()
        ));
    }

    let html = response
        .text()
        .map_err(|error| format!("Failed to read Steam gamecards page HTML: {error}"))?;
    Ok(parse_steam_gamecards_page_cards(&html, app_id))
}

fn unix_seconds_to_rfc3339(unix_seconds: i64) -> Option<String> {
    chrono::DateTime::<Utc>::from_timestamp(unix_seconds, 0).map(|value| value.to_rfc3339())
}

fn fetch_steam_achievement_schema(
    client: &reqwest::blocking::Client,
    api_key: &str,
    app_id: u64,
) -> Result<HashMap<String, SteamAchievementSchemaEntry>, String> {
    let mut request_url = Url::parse(STEAM_WEB_API_GAME_SCHEMA_ENDPOINT)
        .map_err(|error| format!("Failed to parse Steam schema endpoint: {error}"))?;
    request_url
        .query_pairs_mut()
        .append_pair("key", api_key)
        .append_pair("appid", &app_id.to_string())
        .append_pair("l", "english")
        .append_pair("format", "json");

    let response = client
        .get(request_url)
        .send()
        .map_err(|error| format!("Steam achievement schema request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam achievement schema request failed with status {}",
            response.status()
        ));
    }

    let payload = response
        .json::<SteamGameSchemaApiResponse>()
        .map_err(|error| format!("Failed to decode Steam achievement schema response: {error}"))?;
    let mut schema_by_name = HashMap::new();
    let Some(game_schema) = payload.game else {
        return Ok(schema_by_name);
    };
    let Some(stats) = game_schema.available_game_stats else {
        return Ok(schema_by_name);
    };
    for achievement in stats.achievements {
        let Some(api_name) = achievement
            .name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        schema_by_name.insert(
            api_name.to_owned(),
            SteamAchievementSchemaEntry {
                display_name: achievement
                    .display_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned),
                description: achievement
                    .description
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned),
                icon: achievement
                    .icon
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned),
            },
        );
    }
    Ok(schema_by_name)
}

pub(crate) fn get_game_achievements(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameAchievementsResponse> {
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
        return Ok(GameAchievementsResponse {
            provider: normalized_provider,
            external_id: normalized_external_id,
            total: 0,
            unlocked_count: 0,
            percent: None,
            entries: Vec::new(),
            warning: Some(String::from(
                "Achievements are currently available for Steam titles only.",
            )),
            last_synced_at: Utc::now().to_rfc3339(),
        });
    }

    let app_id = match normalized_external_id.parse::<u64>() {
        Ok(value) => value,
        Err(_) => {
            return Ok(GameAchievementsResponse {
                provider: normalized_provider,
                external_id: normalized_external_id,
                total: 0,
                unlocked_count: 0,
                percent: None,
                entries: Vec::new(),
                warning: Some(String::from("This Steam app ID is invalid.")),
                last_synced_at: Utc::now().to_rfc3339(),
            });
        }
    };

    let steam_id_opt = user
        .steam_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let api_key_opt = state
        .steam_api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let cache_key = format!(
        "steam_achievements:{STEAM_ACHIEVEMENTS_CACHE_VERSION}:{steam_id}:{app_id}",
        steam_id = steam_id_opt.as_deref().unwrap_or("unknown")
    );
    if !force_refresh {
        if let Some(cached_value) =
            CacheAdapter::new().get_json(&cache_key, STEAM_ACHIEVEMENTS_CACHE_TTL_SECONDS)
        {
            if let Ok(cached_response) = serde_json::from_value::<GameAchievementsResponse>(cached_value)
            {
                return Ok(cached_response);
            }
        }
    }

    let client = build_http_client()?;
    let mut response = GameAchievementsResponse {
        provider: normalized_provider.clone(),
        external_id: normalized_external_id.clone(),
        total: 0,
        unlocked_count: 0,
        percent: None,
        entries: Vec::new(),
        warning: None,
        last_synced_at: Utc::now().to_rfc3339(),
    };

    match (api_key_opt.as_deref(), steam_id_opt.as_deref()) {
        (Some(api_key), Some(steam_id)) => {
            let schema_by_name = match fetch_steam_achievement_schema(&client, api_key, app_id) {
                Ok(schema) => schema,
                Err(error) => {
                    append_warning(
                        &mut response.warning,
                        format!(
                            "Could not load full achievement metadata: {}",
                            normalize_backend_warning_message(&error)
                        ),
                    );
                    HashMap::new()
                }
            };

            let mut player_achievements_map = HashMap::new();
            match (|| -> Result<(), String> {
                let mut request_url = Url::parse(STEAM_WEB_API_PLAYER_ACHIEVEMENTS_ENDPOINT)
                    .map_err(|error| {
                        format!("Failed to parse Steam player achievements endpoint: {error}")
                    })?;
                request_url
                    .query_pairs_mut()
                    .append_pair("key", api_key)
                    .append_pair("steamid", steam_id)
                    .append_pair("appid", &app_id.to_string())
                    .append_pair("l", "english")
                    .append_pair("format", "json");

                let resp = client
                    .get(request_url)
                    .send()
                    .map_err(|error| format!("Steam player achievements request failed: {error}"))?;
                if !resp.status().is_success() {
                    return Err(format!(
                        "Steam player achievements request failed with status {}",
                        resp.status()
                    ));
                }
                let payload = resp
                    .json::<SteamPlayerAchievementsApiResponse>()
                    .map_err(|error| {
                        format!("Failed to decode Steam player achievements response: {error}")
                    })?;
                let Some(playerstats) = payload.playerstats else {
                    return Ok(());
                };
                if !playerstats.success.unwrap_or(false) {
                    return Ok(());
                }
                for achievement in playerstats.achievements {
                    if let Some(api_name) = achievement
                        .apiname
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    {
                        player_achievements_map.insert(api_name.to_owned(), achievement);
                    }
                }
                Ok(())
            })() {
                Ok(()) => {}
                Err(error) => {
                    append_warning(
                        &mut response.warning,
                        format!(
                            "Could not load player achievement data: {}",
                            normalize_backend_warning_message(&error)
                        ),
                    );
                }
            }

            let mut entries: Vec<GameAchievementEntryResponse> = Vec::new();
            for (api_name, schema_entry) in &schema_by_name {
                let player_entry_opt = player_achievements_map.get(api_name.as_str());
                let unlocked = player_entry_opt.and_then(|entry| entry.achieved).unwrap_or(0) == 1;
                let unlocked_at = player_entry_opt
                    .and_then(|entry| entry.unlocktime)
                    .and_then(unix_seconds_to_rfc3339);
                entries.push(GameAchievementEntryResponse {
                    api_name: api_name.to_owned(),
                    name: schema_entry
                        .display_name
                        .clone()
                        .unwrap_or_else(|| api_name.to_owned()),
                    description: schema_entry.description.clone(),
                    icon: schema_entry.icon.clone(),
                    unlocked,
                    unlocked_at,
                });
            }

            for (api_name, player_achievement) in &player_achievements_map {
                if !schema_by_name.contains_key(api_name) {
                    let unlocked = player_achievement.achieved.unwrap_or(0) == 1;
                    let unlocked_at = player_achievement.unlocktime.and_then(unix_seconds_to_rfc3339);
                    entries.push(GameAchievementEntryResponse {
                        api_name: api_name.clone(),
                        name: api_name.clone(),
                        description: None,
                        icon: None,
                        unlocked,
                        unlocked_at,
                    });
                }
            }

            entries.sort_by(|left, right| {
                left.name
                    .to_ascii_lowercase()
                    .cmp(&right.name.to_ascii_lowercase())
            });

            let total = entries.len() as i64;
            let unlocked_count = entries.iter().filter(|entry| entry.unlocked).count() as i64;
            let percent = if total > 0 {
                Some((unlocked_count as f64 / total as f64) * 100.0)
            } else {
                None
            };

            response.total = total;
            response.unlocked_count = unlocked_count;
            response.percent = percent;
            response.entries = entries;
        }
        (None, Some(_)) => {
            append_warning(
                &mut response.warning,
                String::from("Set STEAM_API_KEY to include achievements."),
            );
        }
        (_, None) => {
            append_warning(
                &mut response.warning,
                String::from("Connect Steam to include achievements."),
            );
        }
    }

    response.last_synced_at = Utc::now().to_rfc3339();

    if let Ok(serialized_response) = serde_json::to_value(&response) {
        CacheAdapter::new().set_json(&cache_key, serialized_response);
    }

    Ok(response)
}

pub(crate) fn get_game_trading_cards(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameTradingCardsResponse> {
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
        return Ok(empty_game_trading_cards_response(
            &normalized_provider,
            &normalized_external_id,
            false,
            String::from("https://steamcommunity.com/tradingcards/"),
            Some(String::from(
                "Trading cards are currently available for Steam titles only.",
            )),
        ));
    }

    let app_id = match normalized_external_id.parse::<u64>() {
        Ok(value) => value,
        Err(_) => {
            return Ok(empty_game_trading_cards_response(
                &normalized_provider,
                &normalized_external_id,
                false,
                String::from("https://steamcommunity.com/tradingcards/"),
                Some(String::from("This Steam app ID is invalid.")),
            ));
        }
    };
    let view_url = format!("https://steamcommunity.com/my/gamecards/{app_id}");

    let steam_id_opt = user
        .steam_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let cache_key = format!(
        "steam_trading_cards:{STEAM_TRADING_CARDS_CACHE_VERSION}:{steam_id}:{app_id}",
        steam_id = steam_id_opt.as_deref().unwrap_or("unknown")
    );
    if !force_refresh {
        if let Some(cached_value) =
            CacheAdapter::new().get_json(&cache_key, STEAM_TRADING_CARDS_CACHE_TTL_SECONDS)
        {
            if let Ok(cached_response) = serde_json::from_value::<GameTradingCardsResponse>(cached_value)
            {
                return Ok(cached_response);
            }
        }
    }

    let client = build_http_client()?;
    let support_result = resolve_steam_trading_cards_support(&connection, &client, app_id);
    let mut response = GameTradingCardsResponse {
        provider: normalized_provider.clone(),
        external_id: normalized_external_id.clone(),
        supported: true,
        badge_level: None,
        badge_xp: None,
        total_cards: 0,
        owned_cards: 0,
        cards: Vec::new(),
        warning: None,
        view_url,
        last_synced_at: Utc::now().to_rfc3339(),
    };

    match support_result {
        Ok(Some(supported)) => {
            response.supported = supported;
            if !supported {
                append_warning(
                    &mut response.warning,
                    String::from("This title does not appear to support Steam Trading Cards."),
                );
            }
        }
        Ok(None) => {
            append_warning(
                &mut response.warning,
                String::from("Could not verify trading-card support from Steam Store metadata."),
            );
        }
        Err(error) => {
            append_warning(
                &mut response.warning,
                format!(
                    "Could not load trading-card support metadata: {}",
                    normalize_backend_warning_message(&error)
                ),
            );
        }
    }

    match steam_id_opt.as_deref() {
        Some(steam_id) => match fetch_steam_gamecards_page_cards(&client, steam_id, app_id) {
            Ok(cards) => {
                if !cards.is_empty() {
                    response.supported = true;
                    response.owned_cards = cards.iter().filter(|card| card.is_owned).count() as i64;
                    response.total_cards = cards.len() as i64;
                    response.cards = cards;
                }
            }
            Err(error) => {
                append_warning(
                    &mut response.warning,
                    format!(
                        "Could not load card tiles from Steam community page: {}",
                        normalize_backend_warning_message(&error)
                    ),
                );
            }
        },
        None => {
            append_warning(
                &mut response.warning,
                String::from("Connect Steam to include per-card owned/missing tiles."),
            );
        }
    }

    if response.supported && response.cards.is_empty() {
        append_warning(
            &mut response.warning,
            String::from(
                "Trading cards appear supported, but card tiles could not be read from your community profile.",
            ),
        );
    }

    let api_key_opt = state
        .steam_api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    match (api_key_opt.as_deref(), steam_id_opt.as_deref()) {
        (Some(api_key), Some(steam_id)) => match (|| -> Result<(), String> {
            let mut request_url = Url::parse(STEAM_WEB_API_BADGES_ENDPOINT)
                .map_err(|error| format!("Failed to parse Steam badges endpoint: {error}"))?;
            request_url
                .query_pairs_mut()
                .append_pair("key", api_key)
                .append_pair("steamid", steam_id)
                .append_pair("format", "json");
            let api_response = client
                .get(request_url)
                .send()
                .map_err(|error| format!("Steam badges request failed: {error}"))?;
            if !api_response.status().is_success() {
                return Err(format!(
                    "Steam badges request failed with status {}",
                    api_response.status()
                ));
            }
            let payload = api_response
                .json::<SteamBadgesApiResponse>()
                .map_err(|error| format!("Failed to decode Steam badges payload: {error}"))?;
            let badges = payload
                .response
                .map(|response| response.badges)
                .unwrap_or_default();
            if let Some(badge) = badges
                .into_iter()
                .find(|entry| entry.appid.map(|value| value == app_id).unwrap_or(false))
            {
                response.badge_level = badge.level;
                response.badge_xp = badge.xp;
            }
            Ok(())
        })() {
            Ok(()) => {}
            Err(error) => {
                append_warning(
                    &mut response.warning,
                    format!(
                        "Could not load badge progression: {}",
                        normalize_backend_warning_message(&error)
                    ),
                );
            }
        },
        (None, Some(_)) => {
            append_warning(
                &mut response.warning,
                String::from("Set STEAM_API_KEY to include badge progression."),
            );
        }
        (_, None) => {
            append_warning(
                &mut response.warning,
                String::from("Connect Steam to include badge progression."),
            );
        }
    }

    response.last_synced_at = Utc::now().to_rfc3339();
    if let Ok(serialized_response) = serde_json::to_value(&response) {
        CacheAdapter::new().set_json(&cache_key, serialized_response);
    }
    Ok(response)
}
