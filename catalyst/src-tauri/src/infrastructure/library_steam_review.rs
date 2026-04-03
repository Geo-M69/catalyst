use crate::application::contracts::library::{GameReviewEntryResponse, GameReviewResponse};
use crate::application::error::AppResult;
use crate::build_http_client;
use crate::cleanup_expired_sessions;
use crate::ensure_owned_game_exists;
use crate::get_authenticated_user;
use crate::infrastructure::cache_adapter::CacheAdapter;
use crate::normalize_backend_warning_message;
use crate::normalize_game_identity_input;
use crate::open_connection;
use crate::resolve_steam_root_paths;
use crate::AppState;
use aes::Aes128;
use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use chrono::{TimeZone, Utc};
use rusqlite::Connection;
use scraper::{Html, Selector};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

const STEAM_APP_REVIEWS_ENDPOINT: &str = "https://store.steampowered.com/appreviews/";
const STEAM_REVIEW_CACHE_TTL_SECONDS: i64 = 15 * 60;
const STEAM_REVIEW_CACHE_VERSION: &str = "v2";
const CHROMIUM_COOKIE_V10_AES_KEY: [u8; 16] = [
    0xfd, 0x62, 0x1f, 0xe5, 0xa2, 0xb4, 0x02, 0x53, 0x9d, 0xfa, 0x14, 0x7c, 0xa9, 0x27, 0x27, 0x78,
];
const CHROMIUM_COOKIE_V10_AES_IV: [u8; 16] = [0x20; 16];

#[derive(serde::Deserialize)]
struct SteamAppReviewsApiResponse {
    success: Option<i64>,
    cursor: Option<String>,
    #[serde(default)]
    reviews: Vec<SteamAppReviewEntry>,
}

#[derive(serde::Deserialize)]
struct SteamAppReviewEntry {
    recommendationid: Option<String>,
    author: Option<SteamAppReviewAuthor>,
    review: Option<String>,
    timestamp_created: Option<i64>,
    voted_up: Option<bool>,
    votes_up: Option<i64>,
    comment_count: Option<i64>,
}

#[derive(serde::Deserialize)]
struct SteamAppReviewAuthor {
    steamid: Option<String>,
    playtime_at_review: Option<i64>,
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

fn unix_seconds_to_rfc3339(unix_seconds: i64) -> Option<String> {
    Utc.timestamp_opt(unix_seconds, 0)
        .single()
        .map(|value| value.to_rfc3339())
}

fn empty_game_review_response(
    provider: &str,
    external_id: &str,
    warning: Option<String>,
) -> GameReviewResponse {
    GameReviewResponse {
        provider: provider.to_owned(),
        external_id: external_id.to_owned(),
        review: None,
        warning,
        last_synced_at: Utc::now().to_rfc3339(),
    }
}

fn fetch_steam_review_for_user(
    client: &reqwest::blocking::Client,
    app_id: u64,
    steam_id: &str,
    steam_cookie_header: Option<&str>,
) -> Result<Option<GameReviewEntryResponse>, String> {
    let trimmed_steam_id = steam_id.trim();
    if trimmed_steam_id.is_empty() {
        return Ok(None);
    }

    let find_review_in_payload =
        |reviews: Vec<SteamAppReviewEntry>| -> Option<GameReviewEntryResponse> {
            for review in reviews {
                let Some(author) = review.author else {
                    continue;
                };
                let author_steam_id = author
                    .steamid
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty());
                if author_steam_id != Some(trimmed_steam_id) {
                    continue;
                }

                let playtime_minutes = author.playtime_at_review.unwrap_or(0).max(0);
                let created_at = review
                    .timestamp_created
                    .and_then(unix_seconds_to_rfc3339)
                    .unwrap_or_else(|| Utc::now().to_rfc3339());
                let text = review
                    .review
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or("")
                    .to_owned();
                let review_id = review
                    .recommendationid
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| format!("steam:{app_id}:{trimmed_steam_id}"));

                return Some(GameReviewEntryResponse {
                    id: review_id,
                    recommended: review.voted_up.unwrap_or(false),
                    text,
                    playtime_minutes,
                    created_at,
                    likes: review.votes_up.unwrap_or(0).max(0),
                    comments: review.comment_count.unwrap_or(0).max(0),
                    source: String::from("steam"),
                });
            }
            None
        };

    let run_query = |filter_value: &str,
                     include_steam_id: bool,
                     max_pages: usize|
     -> Result<Option<GameReviewEntryResponse>, String> {
        let mut cursor = String::from("*");
        for _ in 0..max_pages {
            let mut request_url = url::Url::parse(&format!("{STEAM_APP_REVIEWS_ENDPOINT}{app_id}"))
                .map_err(|error| format!("Failed to parse Steam appreviews endpoint: {error}"))?;
            {
                let mut query = request_url.query_pairs_mut();
                query
                    .append_pair("json", "1")
                    .append_pair("language", "all")
                    .append_pair("filter", filter_value)
                    .append_pair("review_type", "all")
                    .append_pair("purchase_type", "all")
                    .append_pair("day_range", "36500")
                    .append_pair("num_per_page", "100")
                    .append_pair("cursor", &cursor);
                if include_steam_id {
                    // Undocumented but often supported: narrows the result set to one author.
                    query.append_pair("steamid", trimmed_steam_id);
                }
            }

            let mut request = client.get(request_url);
            if let Some(cookie_header) = steam_cookie_header {
                request = request.header(reqwest::header::COOKIE, cookie_header);
            }
            let response = request
                .send()
                .map_err(|error| format!("Steam appreviews request failed: {error}"))?;
            if !response.status().is_success() {
                return Err(format!(
                    "Steam appreviews request failed with status {}",
                    response.status()
                ));
            }

            let payload = response
                .json::<SteamAppReviewsApiResponse>()
                .map_err(|error| format!("Failed to decode Steam appreviews response: {error}"))?;
            if payload.success.unwrap_or(0) != 1 {
                return Err(String::from(
                    "Steam appreviews payload returned an unsuccessful result.",
                ));
            }

            if let Some(found) = find_review_in_payload(payload.reviews) {
                return Ok(Some(found));
            }

            let next_cursor = payload
                .cursor
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            let Some(next_cursor) = next_cursor else {
                break;
            };
            if next_cursor == cursor {
                break;
            }
            cursor = next_cursor;
        }

        Ok(None)
    };

    let mut had_successful_query = false;
    let mut last_query_error: Option<String> = None;
    for (filter_value, include_steam_id, max_pages) in [
        ("updated", true, 6),
        ("recent", true, 6),
        ("all", true, 8),
        ("updated", false, 8),
        ("recent", false, 8),
    ] {
        match run_query(filter_value, include_steam_id, max_pages) {
            Ok(Some(review)) => return Ok(Some(review)),
            Ok(None) => {
                had_successful_query = true;
            }
            Err(error) => {
                last_query_error = Some(error);
            }
        }
    }

    if let Some(profile_review) =
        fetch_steam_profile_review_for_user(client, app_id, trimmed_steam_id, steam_cookie_header)?
    {
        return Ok(Some(profile_review));
    }

    if !had_successful_query {
        if let Some(error) = last_query_error {
            return Err(error);
        }
    }

    Ok(None)
}

fn normalize_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn extract_last_decimal_number(input: &str) -> Option<f64> {
    let mut current = String::new();
    let mut last_value = None;
    for ch in input.chars() {
        if ch.is_ascii_digit() || ch == '.' || ch == ',' {
            current.push(ch);
            continue;
        }
        if !current.is_empty() {
            let normalized = current.replace(',', "");
            if let Ok(parsed) = normalized.parse::<f64>() {
                last_value = Some(parsed);
            }
            current.clear();
        }
    }
    if !current.is_empty() {
        let normalized = current.replace(',', "");
        if let Ok(parsed) = normalized.parse::<f64>() {
            last_value = Some(parsed);
        }
    }
    last_value
}

fn extract_minutes_from_hours_text(playtime_text: &str) -> Option<i64> {
    let lower = playtime_text.to_ascii_lowercase();
    let markers = [
        "hrs at review time",
        "hours at review time",
        "hr at review time",
        "hrs on record",
        "hours on record",
        "hr on record",
    ];
    for marker in markers {
        if let Some(index) = lower.find(marker) {
            let prefix = &playtime_text[..index];
            if let Some(hours) = extract_last_decimal_number(prefix) {
                let minutes = (hours * 60.0).round() as i64;
                return Some(minutes.max(0));
            }
        }
    }
    None
}

fn decrypt_chromium_cookie_v10(encrypted_value: &[u8]) -> Option<String> {
    // Steam's embedded Chromium cookie DB commonly stores cookie values in v10 format.
    // On Linux Steam this is typically AES-128-CBC with the historical Chromium key derivation.
    if encrypted_value.len() <= 3 || &encrypted_value[..3] != b"v10" {
        return None;
    }

    let mut ciphertext = encrypted_value[3..].to_vec();
    let decryptor = cbc::Decryptor::<Aes128>::new_from_slices(
        &CHROMIUM_COOKIE_V10_AES_KEY,
        &CHROMIUM_COOKIE_V10_AES_IV,
    )
    .ok()?;
    let plaintext = decryptor
        .decrypt_padded_mut::<Pkcs7>(&mut ciphertext)
        .ok()?;
    let decrypted = String::from_utf8(plaintext.to_vec()).ok()?;
    let trimmed = decrypted.trim().to_owned();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed)
}

fn load_steam_web_cookie_header(
    steam_root_override: Option<&str>,
    steam_id: &str,
) -> Option<String> {
    let mut candidate_paths: Vec<PathBuf> = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        let home_path = PathBuf::from(home);
        candidate_paths.push(home_path.join(
            ".var/app/com.valvesoftware.Steam/.local/share/Steam/config/htmlcache/Default/Cookies",
        ));
        candidate_paths.push(home_path.join(".local/share/Steam/config/htmlcache/Default/Cookies"));
        candidate_paths.push(home_path.join(".steam/root/config/htmlcache/Default/Cookies"));
    }
    for root in resolve_steam_root_paths(steam_root_override) {
        candidate_paths.push(root.join("config/htmlcache/Default/Cookies"));
    }

    let mut visited = HashSet::new();
    let machine_auth_cookie_name = format!("steamMachineAuth{steam_id}");
    for path in candidate_paths {
        let canonical_path = match path.canonicalize() {
            Ok(canonical) => canonical,
            Err(_) => continue,
        };
        if !visited.insert(canonical_path.clone()) {
            continue;
        }
        if !canonical_path.is_file() {
            continue;
        }

        let Ok(connection) = Connection::open(&canonical_path) else {
            continue;
        };
        let mut statement = match connection.prepare(
            "SELECT host_key, name, COALESCE(value, ''), encrypted_value
             FROM cookies
             WHERE (host_key = 'steamcommunity.com'
                    OR host_key = '.steamcommunity.com'
                    OR host_key = 'store.steampowered.com'
                    OR host_key = '.steampowered.com')
               AND (name IN (
                    'steamLoginSecure',
                    'steamRememberLogin',
                    'sessionid',
                    'steamCountry',
                    'birthtime',
                    'lastagecheckage',
                    'wants_mature_content',
                    'Steam_Language',
                    'timezoneOffset',
                    'timezoneName',
                    'clientsessionid')
                    OR name LIKE 'steamMachineAuth%')
             ORDER BY CASE host_key
                       WHEN 'steamcommunity.com' THEN 0
                       WHEN '.steamcommunity.com' THEN 1
                       WHEN 'store.steampowered.com' THEN 2
                       ELSE 3
                      END",
        ) {
            Ok(stmt) => stmt,
            Err(_) => continue,
        };

        let rows = match statement.query_map([], |row| {
            let host_key: String = row.get(0)?;
            let name: String = row.get(1)?;
            let value: String = row.get(2)?;
            let encrypted_value: Vec<u8> = row.get(3)?;
            Ok((host_key, name, value, encrypted_value))
        }) {
            Ok(rows) => rows,
            Err(_) => continue,
        };

        let mut cookie_values_by_name: HashMap<String, String> = HashMap::new();
        for row in rows.flatten() {
            let (_, name, value, encrypted_value) = row;
            if name.starts_with("steamMachineAuth") && name != machine_auth_cookie_name {
                continue;
            }

            let resolved_value = if !value.trim().is_empty() {
                Some(value.trim().to_owned())
            } else {
                decrypt_chromium_cookie_v10(&encrypted_value)
            };
            let Some(resolved_value) = resolved_value else {
                continue;
            };
            if resolved_value.is_empty() {
                continue;
            }
            cookie_values_by_name.entry(name).or_insert(resolved_value);
        }

        if !cookie_values_by_name.contains_key("steamLoginSecure")
            || !cookie_values_by_name.contains_key("sessionid")
        {
            continue;
        }

        let ordered_names = [
            "steamLoginSecure",
            "sessionid",
            "steamRememberLogin",
            "steamCountry",
            "birthtime",
            "lastagecheckage",
            "wants_mature_content",
            "Steam_Language",
            "timezoneOffset",
            "timezoneName",
            "clientsessionid",
        ];
        let mut pairs: Vec<String> = Vec::new();
        for name in ordered_names {
            if let Some(value) = cookie_values_by_name.get(name) {
                pairs.push(format!("{name}={value}"));
            }
        }
        if let Some(value) = cookie_values_by_name.get(&machine_auth_cookie_name) {
            pairs.push(format!("{machine_auth_cookie_name}={value}"));
        }

        if !pairs.is_empty() {
            return Some(pairs.join("; "));
        }
    }

    None
}

fn extract_xml_tag_value(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)?;
    let after_open = &xml[start + open.len()..];
    let end = after_open.find(&close)?;
    let mut value = after_open[..end].trim().to_owned();
    if value.starts_with("<![CDATA[") && value.ends_with("]]>") && value.len() >= 12 {
        value = value[9..value.len() - 3].to_owned();
    }
    let normalized = normalize_whitespace(&value);
    if normalized.is_empty() {
        return None;
    }
    Some(normalized)
}

fn pick_text_from_html_document(document: &Html, selectors: &[&str]) -> Option<String> {
    for selector_str in selectors {
        let Ok(selector) = Selector::parse(selector_str) else {
            continue;
        };
        let Some(node) = document.select(&selector).next() else {
            continue;
        };
        let text = normalize_whitespace(&node.text().collect::<Vec<_>>().join(" "));
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}

fn parse_steam_profile_review_html(
    body: &str,
    app_id: u64,
    steam_id: &str,
) -> Option<GameReviewEntryResponse> {
    if body.trim().is_empty() {
        return None;
    }

    let lower_body = body.to_ascii_lowercase();
    if lower_body.contains("there are no reviews to display")
        || lower_body.contains("has not written a review")
        || lower_body.contains("no user review data found")
        || lower_body.contains("error processing your request")
    {
        return None;
    }

    let html = Html::parse_document(body);

    let recommendation_text = pick_text_from_html_document(
        &html,
        &[
            "#ReviewTitle .ratingSummary",
            ".ratingSummaryBlock .ratingSummary",
            ".apphub_Card .title",
            ".apphub_Card .vote_header .title",
        ],
    );
    let recommended = match recommendation_text.as_deref() {
        Some(text) if text.to_ascii_lowercase().contains("not recommended") => false,
        Some(text) if text.to_ascii_lowercase().contains("recommended") => true,
        _ => !lower_body.contains("not recommended"),
    };

    let mut review_text = pick_text_from_html_document(
        &html,
        &[
            "#ReviewText",
            ".review_area #ReviewText",
            ".apphub_Card .apphub_CardTextContent",
            ".apphub_Card .review_box .content",
        ],
    )
    .unwrap_or_default();
    if review_text.to_ascii_lowercase().starts_with("posted:") {
        review_text = review_text
            .split_once(" ")
            .map(|(_, rest)| rest.trim().to_owned())
            .unwrap_or(review_text);
    }

    let hours_text = pick_text_from_html_document(
        &html,
        &[
            "#ReviewTitle .playTime",
            ".ratingSummaryBlock .playTime",
            ".playTime",
            ".apphub_Card .hours",
            ".apphub_Card .hours_content",
        ],
    )
    .unwrap_or_default();
    let playtime_minutes = extract_minutes_from_hours_text(&hours_text)
        .or_else(|| {
            extract_last_decimal_number(&hours_text).map(|hours| (hours * 60.0).round() as i64)
        })
        .unwrap_or(0)
        .max(0);

    if review_text.is_empty() && hours_text.is_empty() {
        return None;
    }

    Some(GameReviewEntryResponse {
        id: format!("steam-profile:{app_id}:{steam_id}"),
        recommended,
        text: review_text,
        playtime_minutes,
        created_at: Utc::now().to_rfc3339(),
        likes: 0,
        comments: 0,
        source: String::from("steam_profile"),
    })
}

fn fetch_steam_profile_review_xml_for_user(
    client: &reqwest::blocking::Client,
    app_id: u64,
    steam_id: &str,
    steam_cookie_header: Option<&str>,
) -> Result<Option<GameReviewEntryResponse>, String> {
    let review_xml_url = format!(
        "https://steamcommunity.com/profiles/{steam_id}/recommended/{app_id}/?xml=1&l=english"
    );
    let mut request = client.get(&review_xml_url);
    if let Some(cookie_header) = steam_cookie_header {
        request = request.header(reqwest::header::COOKIE, cookie_header);
    }
    let response = request
        .send()
        .map_err(|error| format!("Steam profile review XML request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam profile review XML request failed with status {}",
            response.status()
        ));
    }

    let body = response.text().map_err(|error| {
        format!("Could not read Steam profile review XML response body: {error}")
    })?;
    if body.trim().is_empty() {
        return Ok(None);
    }

    let lower_body = body.to_ascii_lowercase();
    if lower_body.contains("<error>") || lower_body.contains("no user review data found") {
        return Ok(None);
    }

    let review_text = extract_xml_tag_value(&body, "review").unwrap_or_default();
    let hours_text = extract_xml_tag_value(&body, "hours").unwrap_or_default();
    if review_text.is_empty() && hours_text.is_empty() {
        return Ok(None);
    }

    let playtime_minutes = extract_minutes_from_hours_text(&hours_text)
        .or_else(|| {
            extract_last_decimal_number(&hours_text).map(|hours| (hours * 60.0).round() as i64)
        })
        .unwrap_or(0)
        .max(0);
    let recommended = match extract_xml_tag_value(&body, "recommended")
        .unwrap_or_else(|| String::from("true"))
        .to_ascii_lowercase()
        .as_str()
    {
        "false" | "0" => false,
        _ => true,
    };
    let created_at = extract_xml_tag_value(&body, "timestamp_created")
        .and_then(|value| value.parse::<i64>().ok())
        .and_then(unix_seconds_to_rfc3339)
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    let likes = extract_xml_tag_value(&body, "votes_up")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);
    let comments = extract_xml_tag_value(&body, "comment_count")
        .or_else(|| extract_xml_tag_value(&body, "comments"))
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);
    let review_id = extract_xml_tag_value(&body, "recommendationid")
        .unwrap_or_else(|| format!("steam-profile-xml:{app_id}:{steam_id}"));

    Ok(Some(GameReviewEntryResponse {
        id: review_id,
        recommended,
        text: review_text,
        playtime_minutes,
        created_at,
        likes,
        comments,
        source: String::from("steam_profile_xml"),
    }))
}

fn fetch_steam_profile_review_for_user(
    client: &reqwest::blocking::Client,
    app_id: u64,
    steam_id: &str,
    steam_cookie_header: Option<&str>,
) -> Result<Option<GameReviewEntryResponse>, String> {
    if let Some(xml_review) =
        fetch_steam_profile_review_xml_for_user(client, app_id, steam_id, steam_cookie_header)?
    {
        return Ok(Some(xml_review));
    }

    let mut review_urls = Vec::new();
    review_urls.push(format!(
        "https://steamcommunity.com/profiles/{steam_id}/recommended/{app_id}/?l=english"
    ));
    if steam_cookie_header.is_some() {
        review_urls.push(format!(
            "https://steamcommunity.com/my/recommended/{app_id}/?l=english"
        ));
    }

    let mut had_successful_response = false;
    let mut last_error: Option<String> = None;
    for review_url in review_urls {
        let mut request = client.get(&review_url);
        if let Some(cookie_header) = steam_cookie_header {
            request = request.header(reqwest::header::COOKIE, cookie_header);
        }
        let response = match request.send() {
            Ok(response) => response,
            Err(error) => {
                last_error = Some(format!("Steam profile review request failed: {error}"));
                continue;
            }
        };
        if !response.status().is_success() {
            last_error = Some(format!(
                "Steam profile review request failed with status {}",
                response.status()
            ));
            continue;
        }

        had_successful_response = true;
        let body = match response.text() {
            Ok(body) => body,
            Err(error) => {
                last_error = Some(format!(
                    "Could not read Steam profile review response body: {error}"
                ));
                continue;
            }
        };
        if let Some(parsed_review) = parse_steam_profile_review_html(&body, app_id, steam_id) {
            return Ok(Some(parsed_review));
        }
    }

    if !had_successful_response {
        if let Some(error) = last_error {
            return Err(error);
        }
    }

    Ok(None)
}

pub(crate) fn get_game_review(
    state: &AppState,
    provider: String,
    external_id: String,
    force_refresh: bool,
) -> AppResult<GameReviewResponse> {
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
        return Ok(empty_game_review_response(
            &normalized_provider,
            &normalized_external_id,
            Some(String::from(
                "Reviews are currently available for Steam titles only.",
            )),
        ));
    }

    let app_id = match normalized_external_id.parse::<u64>() {
        Ok(value) => value,
        Err(_) => {
            return Ok(empty_game_review_response(
                &normalized_provider,
                &normalized_external_id,
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
        return Ok(empty_game_review_response(
            &normalized_provider,
            &normalized_external_id,
            Some(String::from("Connect Steam to load your review.")),
        ));
    };

    let cache_key = format!("steam_review:{STEAM_REVIEW_CACHE_VERSION}:{steam_id}:{app_id}");
    if !force_refresh {
        if let Some(cached_value) =
            CacheAdapter::new().get_json(&cache_key, STEAM_REVIEW_CACHE_TTL_SECONDS)
        {
            if let Ok(cached_response) = serde_json::from_value::<GameReviewResponse>(cached_value)
            {
                return Ok(cached_response);
            }
        }
    }

    let client = build_http_client()?;
    let mut response =
        empty_game_review_response(&normalized_provider, &normalized_external_id, None);
    let steam_cookie_header =
        load_steam_web_cookie_header(state.steam_root_override.as_deref(), steam_id);
    match fetch_steam_review_for_user(&client, app_id, steam_id, steam_cookie_header.as_deref()) {
        Ok(Some(review)) => {
            response.review = Some(review);
        }
        Ok(None) => {
            append_warning(
                &mut response.warning,
                String::from("No public Steam review found for this game on your account."),
            );
        }
        Err(error) => {
            append_warning(
                &mut response.warning,
                format!(
                    "Could not load your Steam review right now: {}",
                    normalize_backend_warning_message(&error)
                ),
            );
        }
    }
    response.last_synced_at = Utc::now().to_rfc3339();

    if let Ok(serialized_response) = serde_json::to_value(&response) {
        CacheAdapter::new().set_json(&cache_key, serialized_response);
    }

    Ok(response)
}

#[cfg(test)]
mod review_parsing_tests {
    use super::*;

    #[test]
    fn extracts_review_time_minutes_from_rating_summary_layout() {
        let html = r#"
            <div class="ratingSummaryBlock" id="ReviewTitle">
                <div class="ratingSummaryHeader">
                    <div class="ratingSummary">Recommended</div>
                    <div class="playTime">
                        4.7 hrs last two weeks / 62.3 hrs on record (12.8 hrs at review time)
                    </div>
                </div>
                <div class="recommendation_date">Posted: Sep 7, 2022 @ 6:53am</div>
            </div>
            <div class="review_area">
                <div class="review_area_content">
                    <div id="ReviewText">Brings memories of my most toxic and civil moments in gaming.</div>
                </div>
            </div>
        "#;

        let parsed = parse_steam_profile_review_html(html, 222880, "76561198000000000")
            .expect("expected review to be parsed");
        assert!(parsed.recommended);
        assert_eq!(parsed.playtime_minutes, 768);
        assert!(parsed.text.contains("Brings memories"));
    }

    #[test]
    fn extract_minutes_prefers_at_review_time_marker() {
        let input = "4.7 hrs last two weeks / 62.3 hrs on record (12.8 hrs at review time)";
        assert_eq!(extract_minutes_from_hours_text(input), Some(768));
    }
}
