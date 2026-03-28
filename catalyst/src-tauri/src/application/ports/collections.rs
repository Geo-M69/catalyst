use crate::application::error::AppResult;
use crate::domain::collection::{CollectionId, CollectionName};
use crate::domain::game::GameIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollectionRecord {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) game_count: usize,
    pub(crate) contains_game: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollectionLookupTarget {
    pub(crate) game: GameIdentity,
}

pub(crate) trait CollectionsPort {
    fn authenticated_user_id(&self) -> AppResult<String>;

    fn ensure_owned_game_exists(
        &self,
        user_id: &str,
        game: &GameIdentity,
    ) -> AppResult<()>;

    fn ensure_owned_collection_exists(
        &self,
        user_id: &str,
        collection_id: &CollectionId,
    ) -> AppResult<()>;

    fn list_collections_by_user(
        &self,
        user_id: &str,
        target: Option<&CollectionLookupTarget>,
    ) -> AppResult<Vec<CollectionRecord>>;

    fn create_collection(
        &self,
        user_id: &str,
        name: &CollectionName,
    ) -> AppResult<CollectionRecord>;

    fn rename_collection(
        &self,
        user_id: &str,
        collection_id: &CollectionId,
        name: &CollectionName,
    ) -> AppResult<CollectionRecord>;

    fn delete_collection(
        &self,
        user_id: &str,
        collection_id: &CollectionId,
    ) -> AppResult<()>;

    fn add_game_to_collection_membership(
        &self,
        user_id: &str,
        collection_id: &CollectionId,
        game: &GameIdentity,
    ) -> AppResult<()>;
}
