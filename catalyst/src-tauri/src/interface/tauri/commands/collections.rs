use crate::application::error::AppResult;
use crate::application::services::collection_service::{CollectionService, CollectionView};
use crate::infrastructure::collections_port::InfrastructureCollectionsPort;
use crate::{AppState, CollectionResponse};
use tauri::State;

fn to_collection_response(value: CollectionView) -> CollectionResponse {
    CollectionResponse {
        id: value.id,
        name: value.name,
        game_count: value.game_count,
        contains_game: value.contains_game,
    }
}

#[tauri::command]
pub(crate) fn list_collections(
    provider: Option<String>,
    external_id: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Vec<CollectionResponse>> {
    let service = CollectionService::new(InfrastructureCollectionsPort::new(state.inner()));
    let values = service.list_collections(provider, external_id)?;
    Ok(values.into_iter().map(to_collection_response).collect())
}

#[tauri::command]
pub(crate) fn create_collection(
    name: String,
    state: State<'_, AppState>,
) -> AppResult<CollectionResponse> {
    let service = CollectionService::new(InfrastructureCollectionsPort::new(state.inner()));
    let value = service.create_collection(name)?;
    Ok(to_collection_response(value))
}

#[tauri::command]
pub(crate) fn rename_collection(
    collection_id: String,
    name: String,
    state: State<'_, AppState>,
) -> AppResult<CollectionResponse> {
    let service = CollectionService::new(InfrastructureCollectionsPort::new(state.inner()));
    let value = service.rename_collection(collection_id, name)?;
    Ok(to_collection_response(value))
}

#[tauri::command]
pub(crate) fn delete_collection(
    collection_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let service = CollectionService::new(InfrastructureCollectionsPort::new(state.inner()));
    service.delete_collection(collection_id)
}

#[tauri::command]
pub(crate) fn add_game_to_collection(
    provider: String,
    external_id: String,
    collection_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let service = CollectionService::new(InfrastructureCollectionsPort::new(state.inner()));
    service.add_game_to_collection(provider, external_id, collection_id)
}
