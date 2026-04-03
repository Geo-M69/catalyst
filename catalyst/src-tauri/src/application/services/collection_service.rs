use super::super::error::AppResult;
use super::super::ports::collections::{CollectionLookupTarget, CollectionRecord, CollectionsPort};
use crate::domain::collection::{CollectionId, CollectionName};
use crate::domain::error::DomainValidationError;
use crate::domain::game::GameIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollectionView {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) game_count: usize,
    pub(crate) contains_game: bool,
}

impl From<CollectionRecord> for CollectionView {
    fn from(value: CollectionRecord) -> Self {
        Self {
            id: value.id,
            name: value.name,
            game_count: value.game_count,
            contains_game: value.contains_game,
        }
    }
}

pub(crate) struct CollectionService<P> {
    ports: P,
}

impl<P> CollectionService<P>
where
    P: CollectionsPort,
{
    pub(crate) fn new(ports: P) -> Self {
        Self { ports }
    }

    pub(crate) fn list_collections(
        &self,
        provider: Option<String>,
        external_id: Option<String>,
    ) -> AppResult<Vec<CollectionView>> {
        let user_id = self.ports.authenticated_user_id()?;

        let target = match (provider.as_deref(), external_id.as_deref()) {
            (None, None) => None,
            (Some(target_provider), Some(target_external_id)) => Some(CollectionLookupTarget {
                game: GameIdentity::parse(target_provider, target_external_id)?,
            }),
            _ => return Err(DomainValidationError::MissingIdentityPair.into()),
        };

        if let Some(target_game) = target.as_ref() {
            self.ports
                .ensure_owned_game_exists(&user_id, &target_game.game)?;
        }

        let rows = self
            .ports
            .list_collections_by_user(&user_id, target.as_ref())?;
        Ok(rows.into_iter().map(CollectionView::from).collect())
    }

    pub(crate) fn create_collection(&self, name: String) -> AppResult<CollectionView> {
        let user_id = self.ports.authenticated_user_id()?;
        let normalized_name = CollectionName::parse(&name)?;
        let created = self.ports.create_collection(&user_id, &normalized_name)?;
        Ok(created.into())
    }

    pub(crate) fn rename_collection(
        &self,
        collection_id: String,
        name: String,
    ) -> AppResult<CollectionView> {
        let user_id = self.ports.authenticated_user_id()?;
        let normalized_collection_id = CollectionId::parse(&collection_id)?;
        let normalized_name = CollectionName::parse(&name)?;
        let renamed =
            self.ports
                .rename_collection(&user_id, &normalized_collection_id, &normalized_name)?;
        Ok(renamed.into())
    }

    pub(crate) fn delete_collection(&self, collection_id: String) -> AppResult<()> {
        let user_id = self.ports.authenticated_user_id()?;
        let normalized_collection_id = CollectionId::parse(&collection_id)?;
        self.ports
            .delete_collection(&user_id, &normalized_collection_id)
    }

    pub(crate) fn add_game_to_collection(
        &self,
        provider: String,
        external_id: String,
        collection_id: String,
    ) -> AppResult<()> {
        let user_id = self.ports.authenticated_user_id()?;
        let game = GameIdentity::parse(&provider, &external_id)?;
        let normalized_collection_id = CollectionId::parse(&collection_id)?;

        self.ports.ensure_owned_game_exists(&user_id, &game)?;
        self.ports
            .ensure_owned_collection_exists(&user_id, &normalized_collection_id)?;
        self.ports
            .add_game_to_collection_membership(&user_id, &normalized_collection_id, &game)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::{HashMap, HashSet};

    use crate::application::error::AppError;

    #[derive(Default)]
    struct FakeCollectionsPort {
        user_id: String,
        owned_games: HashSet<GameIdentity>,
        collections: RefCell<Vec<CollectionRecord>>,
        memberships: RefCell<HashSet<(String, GameIdentity)>>,
    }

    impl FakeCollectionsPort {
        fn seeded() -> Self {
            let mut owned_games = HashSet::new();
            owned_games.insert(GameIdentity::parse("steam", "123").expect("valid seed game"));
            Self {
                user_id: String::from("user-1"),
                owned_games,
                collections: RefCell::new(Vec::new()),
                memberships: RefCell::new(HashSet::new()),
            }
        }
    }

    impl CollectionsPort for FakeCollectionsPort {
        fn authenticated_user_id(&self) -> AppResult<String> {
            Ok(self.user_id.clone())
        }

        fn ensure_owned_game_exists(&self, _user_id: &str, game: &GameIdentity) -> AppResult<()> {
            if self.owned_games.contains(game) {
                return Ok(());
            }

            Err(AppError::not_found(
                "game_not_found",
                "Game not found for current user",
            ))
        }

        fn ensure_owned_collection_exists(
            &self,
            _user_id: &str,
            collection_id: &CollectionId,
        ) -> AppResult<()> {
            if self
                .collections
                .borrow()
                .iter()
                .any(|entry| entry.id == collection_id.as_str())
            {
                return Ok(());
            }

            Err(AppError::not_found(
                "collection_not_found",
                "Collection not found for current user",
            ))
        }

        fn list_collections_by_user(
            &self,
            _user_id: &str,
            target: Option<&CollectionLookupTarget>,
        ) -> AppResult<Vec<CollectionRecord>> {
            let memberships = self.memberships.borrow();
            let contains_by_collection_id = memberships.iter().fold(
                HashMap::<String, Vec<GameIdentity>>::new(),
                |mut acc, entry| {
                    acc.entry(entry.0.clone())
                        .or_default()
                        .push(entry.1.clone());
                    acc
                },
            );

            let mut rows = self.collections.borrow().clone();
            if let Some(target) = target {
                for row in &mut rows {
                    row.contains_game = contains_by_collection_id
                        .get(&row.id)
                        .map(|games| games.contains(&target.game))
                        .unwrap_or(false);
                }
            }

            Ok(rows)
        }

        fn create_collection(
            &self,
            _user_id: &str,
            name: &CollectionName,
        ) -> AppResult<CollectionRecord> {
            if self
                .collections
                .borrow()
                .iter()
                .any(|entry| entry.name.eq_ignore_ascii_case(name.as_str()))
            {
                return Err(AppError::conflict(
                    "collection_name_exists",
                    "Collection name already exists",
                ));
            }

            let id = format!("collection-{}", self.collections.borrow().len() + 1);
            let record = CollectionRecord {
                id,
                name: name.as_str().to_owned(),
                game_count: 0,
                contains_game: false,
            };
            self.collections.borrow_mut().push(record.clone());
            Ok(record)
        }

        fn rename_collection(
            &self,
            _user_id: &str,
            collection_id: &CollectionId,
            name: &CollectionName,
        ) -> AppResult<CollectionRecord> {
            let mut collections = self.collections.borrow_mut();
            let Some(entry) = collections
                .iter_mut()
                .find(|entry| entry.id == collection_id.as_str())
            else {
                return Err(AppError::not_found(
                    "collection_not_found",
                    "Collection not found for current user",
                ));
            };

            entry.name = name.as_str().to_owned();
            Ok(entry.clone())
        }

        fn delete_collection(&self, _user_id: &str, collection_id: &CollectionId) -> AppResult<()> {
            let mut collections = self.collections.borrow_mut();
            let before = collections.len();
            collections.retain(|entry| entry.id != collection_id.as_str());
            if collections.len() == before {
                return Err(AppError::not_found(
                    "collection_not_found",
                    "Collection not found for current user",
                ));
            }
            Ok(())
        }

        fn add_game_to_collection_membership(
            &self,
            _user_id: &str,
            collection_id: &CollectionId,
            game: &GameIdentity,
        ) -> AppResult<()> {
            self.memberships
                .borrow_mut()
                .insert((collection_id.as_str().to_owned(), game.clone()));
            Ok(())
        }
    }

    #[test]
    fn create_and_list_collections_for_user() {
        let service = CollectionService::new(FakeCollectionsPort::seeded());

        let created = service
            .create_collection(String::from("My Collection"))
            .expect("create collection");
        assert_eq!(created.name, "My Collection");

        let list = service
            .list_collections(None, None)
            .expect("list collections");
        assert!(list.iter().any(|entry| entry.id == created.id));
    }

    #[test]
    fn list_collections_requires_complete_identity_pair() {
        let service = CollectionService::new(FakeCollectionsPort::seeded());

        let error = service
            .list_collections(Some(String::from("steam")), None)
            .expect_err("should reject partial identity");
        assert_eq!(error.code, "missing_identity_pair");
    }
}
