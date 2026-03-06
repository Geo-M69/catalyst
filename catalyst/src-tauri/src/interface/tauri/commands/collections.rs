use crate::*;
use tauri::State;

#[tauri::command]
pub(crate) fn list_collections(
    provider: Option<String>,
    external_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<CollectionResponse>, String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;

    let target = match (provider.as_deref(), external_id.as_deref()) {
        (None, None) => None,
        (Some(target_provider), Some(target_external_id)) => {
            let (normalized_provider, normalized_external_id) =
                normalize_game_identity_input(target_provider, target_external_id)?;
            ensure_owned_game_exists(
                &connection,
                &user.id,
                &normalized_provider,
                &normalized_external_id,
            )?;
            Some((normalized_provider, normalized_external_id))
        }
        _ => {
            return Err(String::from(
                "provider and external_id must be supplied together",
            ))
        }
    };

    let list = if let Some((target_provider, target_external_id)) = target {
        list_collections_by_user(
            &connection,
            &user.id,
            Some(target_provider.as_str()),
            Some(target_external_id.as_str()),
        )?
    } else {
        list_collections_by_user(&connection, &user.id, None, None)?
    };

    Ok(list)
}

#[tauri::command]
pub(crate) fn create_collection(name: String, state: State<'_, AppState>) -> Result<CollectionResponse, String> {
    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    create_user_collection(&connection, &user.id, &name)
}

#[tauri::command]
pub(crate) fn rename_collection(
    collection_id: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<CollectionResponse, String> {
    let trimmed_collection_id = collection_id.trim();
    if trimmed_collection_id.is_empty() {
        return Err(String::from("Collection ID is required"));
    }

    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    rename_user_collection(&connection, &user.id, trimmed_collection_id, &name)
}

#[tauri::command]
pub(crate) fn delete_collection(collection_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let trimmed_collection_id = collection_id.trim();
    if trimmed_collection_id.is_empty() {
        return Err(String::from("Collection ID is required"));
    }

    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    delete_user_collection(&connection, &user.id, trimmed_collection_id)
}

#[tauri::command]
pub(crate) fn add_game_to_collection(
    provider: String,
    external_id: String,
    collection_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let trimmed_collection_id = collection_id.trim();
    if trimmed_collection_id.is_empty() {
        return Err(String::from("Collection ID is required"));
    }

    let connection = open_connection(&state.db_path)?;
    cleanup_expired_sessions(&connection)?;
    let user = get_authenticated_user(state.inner(), &connection)?;
    let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
    ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;
    ensure_owned_collection_exists(&connection, &user.id, trimmed_collection_id)?;
    add_game_to_collection_membership(
        &connection,
        &user.id,
        trimmed_collection_id,
        &provider,
        &external_id,
    )?;
    Ok(())
}
