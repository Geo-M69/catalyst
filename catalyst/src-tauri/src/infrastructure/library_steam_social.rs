use crate::application::contracts::library::{
    GameActivityTimelineItemResponse, GameActivityTimelineResponse,
    GameFriendActivityEntryResponse, GameFriendsActivityResponse,
};
use crate::application::error::AppResult;
use crate::infrastructure::cache_adapter::CacheAdapter;
use crate::{
    build_http_client, cleanup_expired_sessions, ensure_owned_game_exists, get_authenticated_user,
    normalize_backend_warning_message, normalize_game_identity_input, open_connection, AppState,
};
use chrono::Utc;
use std::collections::{HashMap, HashSet};

const STEAM_WEB_API_FRIEND_LIST_ENDPOINT: &str =
    "https://api.steampowered.com/ISteamUser/GetFriendList/v1/";
const STEAM_WEB_API_PLAYER_SUMMARIES_ENDPOINT: &str =
    "https://api.steampowered.com/ISteamUser/GetPlayerSummaries/v2/";
const STEAM_WEB_API_OWNED_GAMES_ENDPOINT: &str =
    "https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/";
const STEAM_WEB_API_NEWS_FOR_APP_ENDPOINT: &str =
    "https://api.steampowered.com/ISteamNews/GetNewsForApp/v2/";
const STEAM_WEB_API_PLAYER_ACHIEVEMENTS_ENDPOINT: &str =
    "https://api.steampowered.com/ISteamUserStats/GetPlayerAchievements/v1/";
const STEAM_WEB_API_GAME_SCHEMA_ENDPOINT: &str =
    "https://api.steampowered.com/ISteamUserStats/GetSchemaForGame/v2/";
const STEAM_FRIENDS_ACTIVITY_CACHE_TTL_SECONDS: i64 = 15 * 60;
const STEAM_FRIENDS_ACTIVITY_MAX_FRIENDS_TO_SCAN: usize = 48;
const STEAM_ACTIVITY_TIMELINE_CACHE_TTL_SECONDS: i64 = 15 * 60;
const STEAM_ACTIVITY_TIMELINE_MAX_NEWS_ITEMS: usize = 12;
const STEAM_ACTIVITY_TIMELINE_MAX_ACHIEVEMENTS: usize = 12;
const STEAM_ACTIVITY_TIMELINE_MAX_ITEMS: usize = 24;
const STEAM_ACTIVITY_TIMELINE_MAX_NEWS_PAGE_IMAGE_LOOKUPS: usize = 6;
const STEAM_ACTIVITY_TIMELINE_CACHE_VERSION: &str = "v2";
const STEAM_CLAN_IMAGE_PLACEHOLDER_MARKER: &str = "{steam_clan_image}/";
const STEAM_CLAN_IMAGE_CDN_BASE: &str = "https://clan.akamai.steamstatic.com/images/";

#[derive(Clone, Copy)]
enum SteamFriendListVisibility {
    Public,
    Private,
    Unknown,
}

impl SteamFriendListVisibility {
    fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
            Self::Unknown => "unknown",
        }
    }
}

struct SteamFriendListOutcome {
    friend_ids: Vec<String>,
    visibility: SteamFriendListVisibility,
    warning: Option<String>,
}

#[derive(serde::Deserialize)]
struct SteamFriendListApiResponse {
    friendslist: Option<SteamFriendListPayload>,
}

#[derive(serde::Deserialize)]
struct SteamFriendListPayload {
    #[serde(default)]
    friends: Vec<SteamFriendListEntry>,
}

#[derive(serde::Deserialize)]
struct SteamFriendListEntry {
    steamid: String,
    relationship: Option<String>,
}

#[derive(serde::Deserialize)]
struct SteamPlayerSummariesApiResponse {
    response: Option<SteamPlayerSummariesPayload>,
}

#[derive(serde::Deserialize)]
struct SteamPlayerSummariesPayload {
    #[serde(default)]
    players: Vec<SteamPlayerSummary>,
}

#[derive(serde::Deserialize, Clone)]
struct SteamPlayerSummary {
    steamid: String,
    personaname: Option<String>,
    avatarfull: Option<String>,
    profileurl: Option<String>,
}

#[derive(serde::Deserialize)]
struct SteamOwnedGamesApiResponseLite {
    response: Option<SteamOwnedGamesPayloadLite>,
}

#[derive(serde::Deserialize)]
struct SteamOwnedGamesPayloadLite {
    game_count: Option<u64>,
    #[serde(default)]
    games: Vec<SteamOwnedGameLite>,
}

#[derive(serde::Deserialize)]
struct SteamOwnedGameLite {
    appid: u64,
    playtime_forever: Option<u64>,
}

#[derive(Clone, Copy)]
struct FriendOwnedGameStatus {
    owns: bool,
    played: bool,
}

#[derive(Clone)]
struct SteamAchievementSchemaEntry {
    display_name: Option<String>,
    description: Option<String>,
    icon: Option<String>,
}

#[derive(serde::Deserialize)]
struct SteamNewsForAppApiResponse {
    appnews: Option<SteamNewsForAppPayload>,
}

#[derive(serde::Deserialize)]
struct SteamNewsForAppPayload {
    #[serde(default)]
    newsitems: Vec<serde_json::Value>,
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

fn empty_game_friends_activity_response(
    provider: &str,
    external_id: &str,
    friend_list_visibility: SteamFriendListVisibility,
    warning: Option<String>,
) -> GameFriendsActivityResponse {
    GameFriendsActivityResponse {
        provider: provider.to_owned(),
        external_id: external_id.to_owned(),
        played_friends: Vec::new(),
        owned_friends: Vec::new(),
        friend_list_visibility: friend_list_visibility.as_str().to_owned(),
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

fn empty_game_activity_timeline_response(
    provider: &str,
    external_id: &str,
    warning: Option<String>,
) -> GameActivityTimelineResponse {
    GameActivityTimelineResponse {
        provider: provider.to_owned(),
        external_id: external_id.to_owned(),
        items: Vec::new(),
        warning,
        last_synced_at: Utc::now().to_rfc3339(),
    }
}

fn unix_seconds_to_rfc3339(unix_seconds: i64) -> Option<String> {
    chrono::DateTime::<Utc>::from_timestamp(unix_seconds, 0).map(|value| value.to_rfc3339())
}

fn compact_whitespace(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_owned()
}

fn truncate_text(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_owned();
    }
    let mut short = trimmed
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    short.push_str("...");
    short
}

fn strip_bracket_tags(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        if ch == '[' {
            in_tag = true;
            continue;
        }
        if ch == ']' && in_tag {
            in_tag = false;
            continue;
        }
        if !in_tag {
            output.push(ch);
        }
    }
    output
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn is_steam_clan_image_path_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-')
}

fn extract_first_steam_clan_image_path(input: &str) -> Option<String> {
    let lower = input.to_ascii_lowercase();
    let marker_start = lower.find(STEAM_CLAN_IMAGE_PLACEHOLDER_MARKER)?;
    let path_start = marker_start + STEAM_CLAN_IMAGE_PLACEHOLDER_MARKER.len();
    if path_start >= input.len() {
        return None;
    }

    let remainder = &input[path_start..];
    let mut path_end = 0usize;
    for (index, ch) in remainder.char_indices() {
        if !is_steam_clan_image_path_char(ch) {
            break;
        }
        path_end = index + ch.len_utf8();
    }
    if path_end == 0 {
        return None;
    }

    Some(remainder[..path_end].to_owned())
}

fn steam_clan_image_path_to_url(path: &str) -> Option<String> {
    let normalized = path.trim().trim_start_matches('/');
    if normalized.is_empty() {
        return None;
    }

    let has_clan_and_asset = normalized
        .split('/')
        .filter(|part| !part.is_empty())
        .count()
        >= 2;
    if !has_clan_and_asset {
        return None;
    }

    Some(format!("{STEAM_CLAN_IMAGE_CDN_BASE}{normalized}"))
}

fn normalize_news_image_url(candidate: &str) -> Option<String> {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return None;
    }
    if is_http_url(trimmed) {
        return Some(trimmed.to_owned());
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with(STEAM_CLAN_IMAGE_PLACEHOLDER_MARKER) {
        let rest = &trimmed[STEAM_CLAN_IMAGE_PLACEHOLDER_MARKER.len()..];
        return steam_clan_image_path_to_url(rest);
    }

    if let Some(path) = extract_first_steam_clan_image_path(trimmed) {
        return steam_clan_image_path_to_url(&path);
    }

    None
}

fn strip_steam_clan_image_tokens(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0usize;

    while cursor < input.len() {
        let Some(relative_start) = lower[cursor..].find(STEAM_CLAN_IMAGE_PLACEHOLDER_MARKER) else {
            break;
        };
        let marker_start = cursor + relative_start;
        output.push_str(&input[cursor..marker_start]);

        let mut marker_end = marker_start + STEAM_CLAN_IMAGE_PLACEHOLDER_MARKER.len();
        while marker_end < input.len() {
            let Some(next_char) = input[marker_end..].chars().next() else {
                break;
            };
            if !is_steam_clan_image_path_char(next_char) {
                break;
            }
            marker_end += next_char.len_utf8();
        }
        cursor = marker_end;
    }

    output.push_str(&input[cursor..]);
    output
}

fn strip_angle_tags(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        if ch == '<' {
            in_tag = true;
            continue;
        }
        if ch == '>' && in_tag {
            in_tag = false;
            continue;
        }
        if !in_tag {
            output.push(ch);
        }
    }
    output
}

fn extract_news_preview(contents: &str) -> Option<String> {
    let without_image_tokens = strip_steam_clan_image_tokens(contents);
    let without_brackets = strip_bracket_tags(&without_image_tokens);
    let without_html = strip_angle_tags(&without_brackets);
    let compact = compact_whitespace(&without_html);
    if compact.is_empty() {
        return None;
    }
    Some(truncate_text(&compact, 220))
}

fn extract_first_news_image_url(contents: &str) -> Option<String> {
    let lower = contents.to_ascii_lowercase();
    if let Some(start) = lower.find("[img]") {
        let image_start = start + 5;
        if image_start < contents.len() {
            let rest = &contents[image_start..];
            if let Some(end_rel) = rest.to_ascii_lowercase().find("[/img]") {
                let candidate = rest[..end_rel].trim();
                if let Some(url) = normalize_news_image_url(candidate) {
                    return Some(url);
                }
            }
        }
    }

    if let Some(img_pos) = lower.find("<img") {
        let rest = &contents[img_pos..];
        let rest_lower = rest.to_ascii_lowercase();
        if let Some(src_pos) = rest_lower.find("src=\"") {
            let start = src_pos + 5;
            if start < rest.len() {
                let tail = &rest[start..];
                if let Some(end) = tail.find('"') {
                    let candidate = tail[..end].trim();
                    if let Some(url) = normalize_news_image_url(candidate) {
                        return Some(url);
                    }
                }
            }
        }
    }

    if let Some(clan_image_path) = extract_first_steam_clan_image_path(contents) {
        if let Some(url) = steam_clan_image_path_to_url(&clan_image_path) {
            return Some(url);
        }
    }

    None
}

fn read_news_image_url_from_payload(news_item: &serde_json::Value) -> Option<String> {
    for key in [
        "previewurl",
        "preview_url",
        "image",
        "image_url",
        "header_image",
    ] {
        let candidate = news_item
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if let Some(url) = candidate {
            if let Some(normalized) = normalize_news_image_url(url) {
                return Some(normalized);
            }
        }
    }

    None
}

fn extract_html_meta_image_value(html: &str, marker: &str) -> Option<String> {
    let start = html.find(marker)?;
    let value_start = start + marker.len();
    if value_start >= html.len() {
        return None;
    }
    let tail = &html[value_start..];
    let quote = marker.chars().last().unwrap_or('"');
    let end = tail.find(quote)?;
    let candidate = tail[..end].trim();
    if is_http_url(candidate) {
        return Some(candidate.to_owned());
    }
    None
}

fn fetch_news_preview_image_from_announcement_page(
    client: &reqwest::blocking::Client,
    news_url: &str,
) -> Result<Option<String>, String> {
    let response = client
        .get(news_url)
        .send()
        .map_err(|error| format!("Steam announcement page request failed: {error}"))?;
    if !response.status().is_success() {
        return Ok(None);
    }

    let html = response
        .text()
        .map_err(|error| format!("Failed to read Steam announcement page HTML: {error}"))?;

    for marker in [
        "property=\"og:image\" content=\"",
        "property='og:image' content='",
        "name=\"twitter:image\" content=\"",
        "name='twitter:image' content='",
        "rel=\"image_src\" href=\"",
        "rel='image_src' href='",
    ] {
        if let Some(value) = extract_html_meta_image_value(&html, marker) {
            return Ok(Some(value));
        }
    }

    Ok(None)
}

fn looks_like_patch_notes(title: &str, source: &str, description: &str) -> bool {
    let combined = format!("{title} {source} {description}").to_ascii_lowercase();
    [
        "patch notes",
        "patchnote",
        "changelog",
        "hotfix",
        "update",
        "version ",
        "build ",
    ]
    .iter()
    .any(|needle| combined.contains(needle))
}

fn should_use_compact_news_presentation(
    title: &str,
    source: &str,
    description: &str,
    has_post_image: bool,
) -> bool {
    let normalized_source = source.to_ascii_lowercase();
    let combined = format!("{title} {source} {description}").to_ascii_lowercase();

    let has_regular_update_label = ["regular update", "major update", "featured update"]
        .iter()
        .any(|needle| normalized_source.contains(needle));
    if has_regular_update_label {
        return false;
    }

    // If the post itself carries preview media, prefer featured layout.
    if has_post_image {
        return false;
    }

    let has_small_update_label = [
        "small update",
        "patch notes",
        "patchnote",
        "hotfix",
        "minor update",
    ]
    .iter()
    .any(|needle| normalized_source.contains(needle));
    if has_small_update_label {
        return true;
    }

    [
        "hotfix",
        "bugfix",
        "bug fix",
        "maintenance",
        "balance patch",
        "balance update",
    ]
    .iter()
    .any(|needle| combined.contains(needle))
}

fn fetch_steam_news_timeline_items(
    client: &reqwest::blocking::Client,
    app_id: u64,
) -> Result<Vec<GameActivityTimelineItemResponse>, String> {
    let mut request_url = url::Url::parse(STEAM_WEB_API_NEWS_FOR_APP_ENDPOINT)
        .map_err(|error| format!("Failed to parse Steam news endpoint: {error}"))?;
    request_url
        .query_pairs_mut()
        .append_pair("appid", &app_id.to_string())
        .append_pair("count", &STEAM_ACTIVITY_TIMELINE_MAX_NEWS_ITEMS.to_string())
        .append_pair("maxlength", "400")
        .append_pair("format", "json");

    let response = client
        .get(request_url)
        .send()
        .map_err(|error| format!("Steam news request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam news request failed with status {}",
            response.status()
        ));
    }

    let payload = response
        .json::<SteamNewsForAppApiResponse>()
        .map_err(|error| format!("Failed to decode Steam news response: {error}"))?;
    let Some(appnews) = payload.appnews else {
        return Ok(Vec::new());
    };

    let mut items = Vec::new();
    let mut remaining_news_page_image_lookups = STEAM_ACTIVITY_TIMELINE_MAX_NEWS_PAGE_IMAGE_LOOKUPS;
    for news_item in appnews.newsitems {
        let title = news_item
            .get("title")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("Steam News")
            .to_owned();
        let occurred_at = news_item
            .get("date")
            .and_then(serde_json::Value::as_i64)
            .and_then(unix_seconds_to_rfc3339)
            .unwrap_or_else(|| Utc::now().to_rfc3339());
        let source_label = news_item
            .get("feedlabel")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let raw_contents = news_item
            .get("contents")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let description = extract_news_preview(raw_contents);
        let source_for_match = source_label.as_deref().unwrap_or_default();
        let description_for_match = description.as_deref().unwrap_or_default();
        let url = news_item
            .get("url")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let mut extracted_image_url = read_news_image_url_from_payload(&news_item)
            .or_else(|| extract_first_news_image_url(raw_contents));
        if extracted_image_url.is_none() && remaining_news_page_image_lookups > 0 {
            if let Some(news_url) = url.as_deref() {
                let is_external_community_post =
                    news_url.contains("/externalpost/steam_community_announcements/");
                let is_regular_update = source_for_match
                    .to_ascii_lowercase()
                    .contains("regular update");
                if is_external_community_post || is_regular_update {
                    remaining_news_page_image_lookups -= 1;
                    match fetch_news_preview_image_from_announcement_page(client, news_url) {
                        Ok(found_url) => {
                            if found_url.is_some() {
                                extracted_image_url = found_url;
                            }
                        }
                        Err(error) => {
                            eprintln!(
								"Could not fetch Steam announcement preview image from {news_url}: {error}"
							);
                        }
                    }
                }
            }
        }
        let gid = news_item
            .get("gid")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let source_text = source_label.clone().unwrap_or_else(|| String::from("News"));
        let is_compact = should_use_compact_news_presentation(
            &title,
            source_for_match,
            description_for_match,
            extracted_image_url.is_some(),
        );
        let is_major_update =
            looks_like_patch_notes(&title, source_for_match, description_for_match);
        items.push(GameActivityTimelineItemResponse {
            id: gid.unwrap_or_else(|| format!("news:{app_id}:{occurred_at}:{title}")),
            kind: String::from("news"),
            title,
            subtitle: None,
            description: description.clone(),
            image_url: extracted_image_url,
            url,
            source_label: Some(source_text.clone()),
            presentation: Some(if is_compact {
                String::from("compact")
            } else {
                String::from("featured")
            }),
            occurred_at,
            is_major_update: Some(!is_compact && is_major_update),
        });
    }

    items.sort_by(|left, right| right.occurred_at.cmp(&left.occurred_at));
    Ok(items)
}

fn fetch_steam_achievement_schema(
    client: &reqwest::blocking::Client,
    api_key: &str,
    app_id: u64,
) -> Result<HashMap<String, SteamAchievementSchemaEntry>, String> {
    let mut request_url = url::Url::parse(STEAM_WEB_API_GAME_SCHEMA_ENDPOINT)
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

fn fetch_steam_achievement_timeline_items(
    client: &reqwest::blocking::Client,
    api_key: &str,
    steam_id: &str,
    app_id: u64,
    schema_by_name: &HashMap<String, SteamAchievementSchemaEntry>,
) -> Result<Vec<GameActivityTimelineItemResponse>, String> {
    let mut request_url = url::Url::parse(STEAM_WEB_API_PLAYER_ACHIEVEMENTS_ENDPOINT)
        .map_err(|error| format!("Failed to parse Steam player achievements endpoint: {error}"))?;
    request_url
        .query_pairs_mut()
        .append_pair("key", api_key)
        .append_pair("steamid", steam_id)
        .append_pair("appid", &app_id.to_string())
        .append_pair("l", "english")
        .append_pair("format", "json");

    let response = client
        .get(request_url)
        .send()
        .map_err(|error| format!("Steam player achievements request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam player achievements request failed with status {}",
            response.status()
        ));
    }

    let payload = response
        .json::<SteamPlayerAchievementsApiResponse>()
        .map_err(|error| format!("Failed to decode Steam player achievements response: {error}"))?;
    let Some(playerstats) = payload.playerstats else {
        return Ok(Vec::new());
    };
    if !playerstats.success.unwrap_or(false) {
        return Ok(Vec::new());
    }

    let mut items = Vec::new();
    for achievement in playerstats.achievements {
        if achievement.achieved.unwrap_or(0) != 1 {
            continue;
        }
        let Some(unlock_unix_time) = achievement.unlocktime else {
            continue;
        };
        if unlock_unix_time <= 0 {
            continue;
        }
        let Some(api_name) = achievement
            .apiname
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let occurred_at =
            unix_seconds_to_rfc3339(unlock_unix_time).unwrap_or_else(|| Utc::now().to_rfc3339());
        let schema_entry = schema_by_name.get(api_name);
        let title = schema_entry
            .and_then(|entry| entry.display_name.clone())
            .unwrap_or_else(|| api_name.to_owned());
        let description = schema_entry.and_then(|entry| entry.description.clone());
        let image_url = schema_entry.and_then(|entry| entry.icon.clone());
        items.push(GameActivityTimelineItemResponse {
            id: format!("achievement:{app_id}:{api_name}:{unlock_unix_time}"),
            kind: String::from("achievement"),
            title,
            subtitle: Some(String::from("Achievement unlocked")),
            description,
            image_url,
            url: None,
            source_label: Some(String::from("Achievements")),
            presentation: None,
            occurred_at,
            is_major_update: None,
        });
    }

    items.sort_by(|left, right| right.occurred_at.cmp(&left.occurred_at));
    items.truncate(STEAM_ACTIVITY_TIMELINE_MAX_ACHIEVEMENTS);
    Ok(items)
}

fn fetch_steam_friend_list(
    client: &reqwest::blocking::Client,
    api_key: &str,
    steam_id: &str,
) -> Result<SteamFriendListOutcome, String> {
    let mut request_url = url::Url::parse(STEAM_WEB_API_FRIEND_LIST_ENDPOINT)
        .map_err(|error| format!("Failed to parse Steam friend list endpoint: {error}"))?;
    request_url
        .query_pairs_mut()
        .append_pair("key", api_key)
        .append_pair("steamid", steam_id)
        .append_pair("relationship", "friend")
        .append_pair("format", "json");

    let response = client
        .get(request_url)
        .send()
        .map_err(|error| format!("Steam friend list request failed: {error}"))?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Ok(SteamFriendListOutcome {
            friend_ids: Vec::new(),
            visibility: SteamFriendListVisibility::Private,
            warning: Some(String::from(
                "Your Steam friend list is private, so friends activity is unavailable.",
            )),
        });
    }

    if !response.status().is_success() {
        return Ok(SteamFriendListOutcome {
            friend_ids: Vec::new(),
            visibility: SteamFriendListVisibility::Unknown,
            warning: Some(format!(
                "Could not load your Steam friend list right now (status {}).",
                response.status()
            )),
        });
    }

    let payload = response
        .json::<SteamFriendListApiResponse>()
        .map_err(|error| format!("Failed to decode Steam friend list response: {error}"))?;
    let mut friend_ids = Vec::new();
    let mut seen_ids = HashSet::new();

    if let Some(list_payload) = payload.friendslist {
        for entry in list_payload.friends {
            if !entry
                .relationship
                .as_deref()
                .map(|value| value.eq_ignore_ascii_case("friend"))
                .unwrap_or(true)
            {
                continue;
            }

            let steam_id_value = entry.steamid.trim();
            if steam_id_value.is_empty() || !seen_ids.insert(steam_id_value.to_owned()) {
                continue;
            }
            friend_ids.push(steam_id_value.to_owned());
        }
    }

    Ok(SteamFriendListOutcome {
        friend_ids,
        visibility: SteamFriendListVisibility::Public,
        warning: None,
    })
}

fn fetch_steam_player_summaries(
    client: &reqwest::blocking::Client,
    api_key: &str,
    steam_ids: &[String],
) -> Result<HashMap<String, SteamPlayerSummary>, String> {
    let mut summaries_by_id = HashMap::new();
    if steam_ids.is_empty() {
        return Ok(summaries_by_id);
    }

    for steam_ids_chunk in steam_ids.chunks(100) {
        let mut request_url = url::Url::parse(STEAM_WEB_API_PLAYER_SUMMARIES_ENDPOINT)
            .map_err(|error| format!("Failed to parse Steam player summaries endpoint: {error}"))?;
        request_url
            .query_pairs_mut()
            .append_pair("key", api_key)
            .append_pair("steamids", &steam_ids_chunk.join(","))
            .append_pair("format", "json");

        let response = client
            .get(request_url)
            .send()
            .map_err(|error| format!("Steam player summaries request failed: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "Steam player summaries request failed with status {}",
                response.status()
            ));
        }

        let payload = response
            .json::<SteamPlayerSummariesApiResponse>()
            .map_err(|error| {
                format!("Failed to decode Steam player summaries response: {error}")
            })?;
        if let Some(response_payload) = payload.response {
            for summary in response_payload.players {
                if summary.steamid.trim().is_empty() {
                    continue;
                }
                summaries_by_id.insert(summary.steamid.clone(), summary);
            }
        }
    }

    Ok(summaries_by_id)
}

fn fetch_friend_owned_game_status(
    client: &reqwest::blocking::Client,
    api_key: &str,
    friend_steam_id: &str,
    app_id: u64,
) -> Result<FriendOwnedGameStatus, String> {
    let mut request_url = url::Url::parse(STEAM_WEB_API_OWNED_GAMES_ENDPOINT)
        .map_err(|error| format!("Failed to parse Steam owned games endpoint: {error}"))?;
    request_url
        .query_pairs_mut()
        .append_pair("key", api_key)
        .append_pair("steamid", friend_steam_id)
        .append_pair("appids_filter", &app_id.to_string())
        .append_pair("include_played_free_games", "true")
        .append_pair("format", "json");

    let response = client
        .get(request_url)
        .send()
        .map_err(|error| format!("Steam owned games request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam owned games request failed with status {}",
            response.status()
        ));
    }

    let payload = response
        .json::<SteamOwnedGamesApiResponseLite>()
        .map_err(|error| format!("Failed to decode Steam owned games response: {error}"))?;
    let Some(response_payload) = payload.response else {
        return Ok(FriendOwnedGameStatus {
            owns: false,
            played: false,
        });
    };

    let owns = response_payload
        .games
        .iter()
        .any(|game| game.appid == app_id)
        || response_payload.game_count.unwrap_or(0) > 0;
    let played = response_payload
        .games
        .iter()
        .filter(|game| game.appid == app_id)
        .any(|game| game.playtime_forever.unwrap_or(0) > 0);

    Ok(FriendOwnedGameStatus { owns, played })
}

pub(crate) fn get_game_friends_activity(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameFriendsActivityResponse> {
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
        return Ok(empty_game_friends_activity_response(
            &normalized_provider,
            &normalized_external_id,
            SteamFriendListVisibility::Unknown,
            Some(String::from(
                "Friends activity is currently available for Steam titles only.",
            )),
        ));
    }

    let app_id = match normalized_external_id.parse::<u64>() {
        Ok(value) => value,
        Err(_) => {
            return Ok(empty_game_friends_activity_response(
                &normalized_provider,
                &normalized_external_id,
                SteamFriendListVisibility::Unknown,
                Some(String::from("This Steam app ID is invalid.")),
            ));
        }
    };

    let Some(steam_id) = user
        .steam_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(empty_game_friends_activity_response(
            &normalized_provider,
            &normalized_external_id,
            SteamFriendListVisibility::Unknown,
            Some(String::from("Connect Steam to view friends activity.")),
        ));
    };

    let Some(api_key) = state
        .steam_api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(empty_game_friends_activity_response(
            &normalized_provider,
            &normalized_external_id,
            SteamFriendListVisibility::Unknown,
            Some(String::from(
                "Set STEAM_API_KEY to enable Steam friends activity sync.",
            )),
        ));
    };

    let cache_key = format!("steam_friends_activity:{steam_id}:{app_id}");
    if !force_refresh {
        if let Some(cached_value) =
            CacheAdapter::new().get_json(&cache_key, STEAM_FRIENDS_ACTIVITY_CACHE_TTL_SECONDS)
        {
            if let Ok(cached_response) =
                serde_json::from_value::<GameFriendsActivityResponse>(cached_value)
            {
                return Ok(cached_response);
            }
        }
    }

    let client = build_http_client()?;
    let friend_list_outcome = fetch_steam_friend_list(&client, api_key, steam_id)?;
    let mut response = empty_game_friends_activity_response(
        &normalized_provider,
        &normalized_external_id,
        friend_list_outcome.visibility,
        friend_list_outcome.warning,
    );
    let total_friend_count = friend_list_outcome.friend_ids.len();
    let friend_ids = friend_list_outcome
        .friend_ids
        .into_iter()
        .take(STEAM_FRIENDS_ACTIVITY_MAX_FRIENDS_TO_SCAN)
        .collect::<Vec<_>>();

    if total_friend_count > STEAM_FRIENDS_ACTIVITY_MAX_FRIENDS_TO_SCAN {
        append_warning(
            &mut response.warning,
            format!(
                "Showing activity from the first {} friends for performance.",
                STEAM_FRIENDS_ACTIVITY_MAX_FRIENDS_TO_SCAN
            ),
        );
    }

    if friend_ids.is_empty()
        || !matches!(
            friend_list_outcome.visibility,
            SteamFriendListVisibility::Public
        )
    {
        if let Ok(serialized_response) = serde_json::to_value(&response) {
            CacheAdapter::new().set_json(&cache_key, serialized_response);
        }
        return Ok(response);
    }

    let player_summaries_by_id = match fetch_steam_player_summaries(&client, api_key, &friend_ids) {
        Ok(summaries) => summaries,
        Err(error) => {
            append_warning(
                &mut response.warning,
                format!(
                    "Could not load some Steam profile details: {}",
                    normalize_backend_warning_message(&error)
                ),
            );
            HashMap::new()
        }
    };

    let mut played_friends = Vec::new();
    let mut owned_friends = Vec::new();
    for friend_id in &friend_ids {
        let status = match fetch_friend_owned_game_status(&client, api_key, friend_id, app_id) {
            Ok(value) => value,
            Err(error) => {
                append_warning(
                    &mut response.warning,
                    format!(
                        "Could not check all friend game ownership data: {}",
                        normalize_backend_warning_message(&error)
                    ),
                );
                break;
            }
        };

        if !status.owns {
            continue;
        }

        let summary = player_summaries_by_id.get(friend_id);
        let persona_name = summary
            .and_then(|value| value.personaname.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(friend_id)
            .to_owned();
        let friend_entry = GameFriendActivityEntryResponse {
            steam_id: friend_id.clone(),
            persona_name,
            avatar_url: summary.and_then(|value| {
                value
                    .avatarfull
                    .as_deref()
                    .map(str::trim)
                    .filter(|avatar| !avatar.is_empty())
                    .map(ToOwned::to_owned)
            }),
            profile_url: summary.and_then(|value| {
                value
                    .profileurl
                    .as_deref()
                    .map(str::trim)
                    .filter(|profile| !profile.is_empty())
                    .map(ToOwned::to_owned)
            }),
        };

        if status.played {
            played_friends.push(friend_entry.clone());
        }
        owned_friends.push(friend_entry);
    }

    played_friends.sort_by(|left, right| {
        left.persona_name
            .to_ascii_lowercase()
            .cmp(&right.persona_name.to_ascii_lowercase())
    });
    owned_friends.sort_by(|left, right| {
        left.persona_name
            .to_ascii_lowercase()
            .cmp(&right.persona_name.to_ascii_lowercase())
    });

    response.played_friends = played_friends;
    response.owned_friends = owned_friends;
    response.last_synced_at = Utc::now().to_rfc3339();

    if let Ok(serialized_response) = serde_json::to_value(&response) {
        CacheAdapter::new().set_json(&cache_key, serialized_response);
    }

    Ok(response)
}

pub(crate) fn get_game_activity_timeline(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameActivityTimelineResponse> {
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
        return Ok(empty_game_activity_timeline_response(
            &normalized_provider,
            &normalized_external_id,
            Some(String::from(
                "Activity timeline is currently available for Steam titles only.",
            )),
        ));
    }

    let app_id = match normalized_external_id.parse::<u64>() {
        Ok(value) => value,
        Err(_) => {
            return Ok(empty_game_activity_timeline_response(
                &normalized_provider,
                &normalized_external_id,
                Some(String::from("This Steam app ID is invalid.")),
            ));
        }
    };

    let steam_id = user
        .steam_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown");
    let cache_key = format!(
        "steam_activity_timeline:{STEAM_ACTIVITY_TIMELINE_CACHE_VERSION}:{steam_id}:{app_id}"
    );
    if !force_refresh {
        if let Some(cached_value) =
            CacheAdapter::new().get_json(&cache_key, STEAM_ACTIVITY_TIMELINE_CACHE_TTL_SECONDS)
        {
            if let Ok(cached_response) =
                serde_json::from_value::<GameActivityTimelineResponse>(cached_value)
            {
                return Ok(cached_response);
            }
        }
    }

    let client = build_http_client()?;
    let mut response =
        empty_game_activity_timeline_response(&normalized_provider, &normalized_external_id, None);

    match fetch_steam_news_timeline_items(&client, app_id) {
        Ok(news_items) => response.items.extend(news_items),
        Err(error) => {
            append_warning(
                &mut response.warning,
                format!(
                    "Could not load Steam news right now: {}",
                    normalize_backend_warning_message(&error)
                ),
            );
        }
    }

    let maybe_api_key = state
        .steam_api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let maybe_steam_id = user
        .steam_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match (maybe_api_key, maybe_steam_id) {
        (Some(api_key), Some(steam_id_value)) => {
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

            match fetch_steam_achievement_timeline_items(
                &client,
                api_key,
                steam_id_value,
                app_id,
                &schema_by_name,
            ) {
                Ok(achievement_items) => response.items.extend(achievement_items),
                Err(error) => {
                    append_warning(
                        &mut response.warning,
                        format!(
                            "Could not load recent achievements: {}",
                            normalize_backend_warning_message(&error)
                        ),
                    );
                }
            }
        }
        (None, Some(_)) => {
            append_warning(
                &mut response.warning,
                "Set STEAM_API_KEY to include recent achievement unlocks.",
            );
        }
        (_, None) => {
            append_warning(
                &mut response.warning,
                "Connect Steam to include recent achievement unlocks.",
            );
        }
    }

    response
        .items
        .sort_by(|left, right| right.occurred_at.cmp(&left.occurred_at));
    if response.items.len() > STEAM_ACTIVITY_TIMELINE_MAX_ITEMS {
        response.items.truncate(STEAM_ACTIVITY_TIMELINE_MAX_ITEMS);
    }
    if response.items.is_empty() && response.warning.is_none() {
        response.warning = Some(String::from("No recent activity found for this game."));
    }
    response.last_synced_at = Utc::now().to_rfc3339();

    if let Ok(serialized_response) = serde_json::to_value(&response) {
        CacheAdapter::new().set_json(&cache_key, serialized_response);
    }

    Ok(response)
}
