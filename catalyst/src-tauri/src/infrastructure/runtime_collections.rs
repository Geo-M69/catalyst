use crate::infrastructure::runtime_vdf::{
    parse_vdf_document, vdf_collect_objects_by_key, vdf_collect_text_leaves, vdf_find_object_value,
    VdfValue,
};
use crate::*;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

fn normalize_collection_name(
    name: &str,
) -> Result<String, crate::domain::error::DomainValidationError> {
    Ok(crate::domain::collection::CollectionName::parse(name)?.into_inner())
}

fn add_game_to_collection_membership(
    connection: &Connection,
    user_id: &str,
    collection_id: &str,
    provider: &str,
    external_id: &str,
) -> Result<bool, String> {
    let inserted_rows = connection
        .execute(
            "
            INSERT OR IGNORE INTO collection_games (user_id, collection_id, provider, external_id, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            params![user_id, collection_id, provider, external_id, Utc::now().to_rfc3339()],
        )
        .map_err(|error| format!("Failed to add game to collection: {error}"))?;
    Ok(inserted_rows > 0)
}

fn find_collection_id_by_name(
    connection: &Connection,
    user_id: &str,
    name: &str,
) -> Result<Option<String>, String> {
    connection
        .query_row(
            "SELECT id FROM collections WHERE user_id = ?1 AND name = ?2 COLLATE NOCASE",
            params![user_id, name],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("Failed to query collection by name: {error}"))
}

fn get_or_create_collection_id_by_name(
    connection: &Connection,
    user_id: &str,
    name: &str,
) -> Result<(String, bool), String> {
    let normalized_name = normalize_collection_name(name).map_err(|error| error.message())?;
    if let Some(existing_id) = find_collection_id_by_name(connection, user_id, &normalized_name)? {
        return Ok((existing_id, false));
    }

    let collection_id = Uuid::new_v4().to_string();
    let timestamp = Utc::now().to_rfc3339();
    let inserted_rows = connection
        .execute(
            "
            INSERT OR IGNORE INTO collections (id, user_id, name, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            params![
                collection_id,
                user_id,
                normalized_name,
                timestamp,
                timestamp
            ],
        )
        .map_err(|error| format!("Failed to create collection during Steam import: {error}"))?;
    if inserted_rows > 0 {
        return Ok((collection_id, true));
    }

    let existing_id = find_collection_id_by_name(connection, user_id, &normalized_name)?
        .ok_or_else(|| String::from("Failed to resolve collection created during Steam import"))?;
    Ok((existing_id, false))
}

fn load_provider_game_external_ids(
    connection: &Connection,
    user_id: &str,
    provider: &str,
) -> Result<HashSet<String>, String> {
    let mut statement = connection
        .prepare("SELECT external_id FROM games WHERE user_id = ?1 AND provider = ?2")
        .map_err(|error| format!("Failed to prepare provider game list query: {error}"))?;
    let rows = statement
        .query_map(params![user_id, provider], |row| row.get::<_, String>(0))
        .map_err(|error| format!("Failed to query provider game list: {error}"))?;

    rows.collect::<Result<HashSet<_>, _>>()
        .map_err(|error| format!("Failed to decode provider game list: {error}"))
}

pub(crate) fn parse_steam_collections_from_vdf(
    contents: &str,
) -> Result<HashMap<String, HashSet<String>>, String> {
    let root_value = parse_vdf_document(contents)?;
    let mut collections_by_app_id = HashMap::new();
    let mut apps_objects = Vec::new();
    vdf_collect_objects_by_key(&root_value, "apps", &mut apps_objects);

    for apps_value in apps_objects {
        let VdfValue::Object(app_entries) = apps_value else {
            continue;
        };

        for (app_id, app_value) in app_entries {
            let normalized_app_id = app_id
                .trim_matches(|character: char| character.is_whitespace() || character == '\0');
            if normalized_app_id.is_empty()
                || !normalized_app_id
                    .chars()
                    .all(|character| character.is_ascii_digit())
            {
                continue;
            }

            let Some(VdfValue::Object(tag_entries)) = vdf_find_object_value(app_value, "tags")
            else {
                continue;
            };
            let mut collection_names = HashSet::new();
            for (tag_key, tag_value) in tag_entries {
                if let Some(collection_name) =
                    crate::domain::collection::parse_collection_name_candidate(tag_key)
                {
                    collection_names.insert(collection_name);
                }
                let mut tag_value_text_candidates = Vec::new();
                vdf_collect_text_leaves(tag_value, &mut tag_value_text_candidates);
                for candidate in tag_value_text_candidates {
                    if let Some(collection_name) =
                        crate::domain::collection::parse_collection_name_candidate(&candidate)
                    {
                        collection_names.insert(collection_name);
                    }
                }
            }

            if !collection_names.is_empty() {
                collections_by_app_id
                    .entry(normalized_app_id.to_owned())
                    .or_insert_with(HashSet::new)
                    .extend(collection_names);
            }
        }
    }

    Ok(collections_by_app_id)
}

pub(crate) fn merge_collections_by_app_id(
    target: &mut HashMap<String, HashSet<String>>,
    source: HashMap<String, HashSet<String>>,
) {
    for (app_id, collections) in source {
        target
            .entry(app_id)
            .or_insert_with(HashSet::new)
            .extend(collections);
    }
}

pub(crate) fn import_steam_collections_for_user(
    connection: &Connection,
    user_id: &str,
    collections_by_app_id: HashMap<String, HashSet<String>>,
) -> Result<SteamCollectionsImportResponse, String> {
    let owned_steam_game_external_ids =
        load_provider_game_external_ids(connection, user_id, "steam")?;
    let mut collection_ids_by_name: HashMap<String, String> = HashMap::new();
    let mut apps_tagged = 0usize;
    let mut collections_created = 0usize;
    let mut memberships_added = 0usize;
    let mut skipped_games = 0usize;
    let mut tags_discovered = 0usize;

    for (external_id, collection_names) in collections_by_app_id {
        apps_tagged += 1;
        for collection_name in collection_names {
            tags_discovered += 1;

            if !owned_steam_game_external_ids.contains(&external_id) {
                skipped_games += 1;
                continue;
            }

            let normalized_key = collection_name.trim().to_ascii_lowercase();
            if normalized_key.is_empty() {
                continue;
            }

            let collection_id =
                if let Some(existing_collection_id) = collection_ids_by_name.get(&normalized_key) {
                    existing_collection_id.clone()
                } else {
                    let (collection_id, created) =
                        get_or_create_collection_id_by_name(connection, user_id, &collection_name)?;
                    if created {
                        collections_created += 1;
                    }
                    collection_ids_by_name.insert(normalized_key, collection_id.clone());
                    collection_id
                };

            if add_game_to_collection_membership(
                connection,
                user_id,
                &collection_id,
                "steam",
                &external_id,
            )? {
                memberships_added += 1;
            }
        }
    }

    Ok(SteamCollectionsImportResponse {
        apps_tagged,
        collections_created,
        memberships_added,
        skipped_games,
        tags_discovered,
    })
}
