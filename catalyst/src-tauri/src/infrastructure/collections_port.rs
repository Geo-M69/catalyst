use crate::application::error::AppResult;
use crate::application::ports::collections::{
    CollectionLookupTarget,
    CollectionRecord,
    CollectionsPort,
};
use crate::domain::collection::{CollectionId, CollectionName};
use crate::domain::game::GameIdentity;
use crate::infrastructure::sqlite::collection_repo::SQLiteCollectionRepo;
use crate::infrastructure::sqlite::library_repo::SQLiteLibraryRepo;
use crate::infrastructure::sqlite::session_repo::SQLiteSessionRepo;
use crate::AppState;

pub(crate) struct InfrastructureCollectionsPort<'a> {
    state: &'a AppState,
}

impl<'a> InfrastructureCollectionsPort<'a> {
    pub(crate) fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    fn session_repo(&self) -> SQLiteSessionRepo<'a> {
        SQLiteSessionRepo::new(self.state)
    }

    fn library_repo(&self) -> SQLiteLibraryRepo<'a> {
        SQLiteLibraryRepo::new(self.state)
    }

    fn collection_repo(&self) -> SQLiteCollectionRepo<'a> {
        SQLiteCollectionRepo::new(self.state)
    }
}

impl CollectionsPort for InfrastructureCollectionsPort<'_> {
    fn authenticated_user_id(&self) -> AppResult<String> {
        self.session_repo().authenticated_user_id()
    }

    fn ensure_owned_game_exists(
        &self,
        user_id: &str,
        game: &GameIdentity,
    ) -> AppResult<()> {
        self.library_repo().ensure_owned_game_exists(user_id, game)
    }

    fn ensure_owned_collection_exists(
        &self,
        user_id: &str,
        collection_id: &CollectionId,
    ) -> AppResult<()> {
        self.collection_repo()
            .ensure_owned_collection_exists(user_id, collection_id)
    }

    fn list_collections_by_user(
        &self,
        user_id: &str,
        target: Option<&CollectionLookupTarget>,
    ) -> AppResult<Vec<CollectionRecord>> {
        self.collection_repo().list_collections_by_user(user_id, target)
    }

    fn create_collection(
        &self,
        user_id: &str,
        name: &CollectionName,
    ) -> AppResult<CollectionRecord> {
        self.collection_repo().create_collection(user_id, name)
    }

    fn rename_collection(
        &self,
        user_id: &str,
        collection_id: &CollectionId,
        name: &CollectionName,
    ) -> AppResult<CollectionRecord> {
        self.collection_repo()
            .rename_collection(user_id, collection_id, name)
    }

    fn delete_collection(
        &self,
        user_id: &str,
        collection_id: &CollectionId,
    ) -> AppResult<()> {
        self.collection_repo().delete_collection(user_id, collection_id)
    }

    fn add_game_to_collection_membership(
        &self,
        user_id: &str,
        collection_id: &CollectionId,
        game: &GameIdentity,
    ) -> AppResult<()> {
        self.collection_repo()
            .add_game_to_collection_membership(user_id, collection_id, game)
    }
}
