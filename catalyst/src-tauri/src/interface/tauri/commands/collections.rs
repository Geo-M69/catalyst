use crate::*;
use crate::application::error::AppResult;
use tauri::State;

#[tauri::command]
pub(crate) fn list_collections(
    provider: Option<String>,
    external_id: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Vec<CollectionResponse>> {
    crate::application::services::collection_service::list_collections(
        state.inner(),
        provider,
        external_id,
    )
}

#[tauri::command]
pub(crate) fn create_collection(name: String, state: State<'_, AppState>) -> AppResult<CollectionResponse> {
    crate::application::services::collection_service::create_collection(state.inner(), name)
}

#[tauri::command]
pub(crate) fn rename_collection(
    collection_id: String,
    name: String,
    state: State<'_, AppState>,
) -> AppResult<CollectionResponse> {
    crate::application::services::collection_service::rename_collection(
        state.inner(),
        collection_id,
        name,
    )
}

#[tauri::command]
pub(crate) fn delete_collection(collection_id: String, state: State<'_, AppState>) -> AppResult<()> {
    crate::application::services::collection_service::delete_collection(state.inner(), collection_id)
}

#[tauri::command]
pub(crate) fn add_game_to_collection(
    provider: String,
    external_id: String,
    collection_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    crate::application::services::collection_service::add_game_to_collection(
        state.inner(),
        provider,
        external_id,
        collection_id,
    )
}
