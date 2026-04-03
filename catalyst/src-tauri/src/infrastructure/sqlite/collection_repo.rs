use std::collections::HashSet;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::application::error::{AppError, AppResult};
use crate::application::ports::collections::{CollectionLookupTarget, CollectionRecord};
use crate::domain::collection::{CollectionId, CollectionName};
use crate::domain::game::GameIdentity;
use crate::AppState;

pub(crate) struct SQLiteCollectionRepo<'a> {
    state: &'a AppState,
}

impl<'a> SQLiteCollectionRepo<'a> {
    pub(crate) fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub(crate) fn ensure_owned_collection_exists(
        &self,
        user_id: &str,
        collection_id: &CollectionId,
    ) -> AppResult<()> {
        let connection = crate::open_connection(&self.state.db_path).map_err(AppError::from)?;
        self.ensure_owned_collection_exists_with_connection(&connection, user_id, collection_id)
    }

    fn ensure_owned_collection_exists_with_connection(
        &self,
        connection: &Connection,
        user_id: &str,
        collection_id: &CollectionId,
    ) -> AppResult<()> {
        let exists = connection
            .query_row(
                "SELECT 1 FROM collections WHERE id = ?1 AND user_id = ?2",
                params![collection_id.as_str(), user_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|error| {
                AppError::from(format!("Failed to validate collection ownership: {error}"))
            })?;

        if exists.is_none() {
            return Err(AppError::from("Collection not found for current user"));
        }

        Ok(())
    }

    pub(crate) fn list_collections_by_user(
        &self,
        user_id: &str,
        target: Option<&CollectionLookupTarget>,
    ) -> AppResult<Vec<CollectionRecord>> {
        let connection = crate::open_connection(&self.state.db_path).map_err(AppError::from)?;
        let mut statement = connection
            .prepare(
                "
                SELECT
                  c.id,
                  c.name,
                  (
                    SELECT COUNT(*)
                    FROM collection_games membership
                    WHERE membership.user_id = c.user_id
                      AND membership.collection_id = c.id
                  ) AS game_count
                FROM collections c
                WHERE c.user_id = ?1
                ORDER BY c.name COLLATE NOCASE ASC
                ",
            )
            .map_err(|error| {
                AppError::from(format!("Failed to prepare collections query: {error}"))
            })?;

        let rows = statement
            .query_map(params![user_id], |row| {
                let game_count_raw: i64 = row.get(2)?;
                Ok(CollectionRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    game_count: usize::try_from(game_count_raw).unwrap_or_default(),
                    contains_game: false,
                })
            })
            .map_err(|error| AppError::from(format!("Failed to query collections: {error}")))?;
        let mut collections = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| AppError::from(format!("Failed to decode collections: {error}")))?;

        let membership_ids = if let Some(target) = target {
            let mut membership_statement = connection
                .prepare(
                    "
                    SELECT collection_id
                    FROM collection_games
                    WHERE user_id = ?1 AND provider = ?2 AND external_id = ?3
                    ",
                )
                .map_err(|error| {
                    AppError::from(format!(
                        "Failed to prepare collection membership query: {error}"
                    ))
                })?;
            let membership_rows = membership_statement
                .query_map(
                    params![user_id, target.game.provider(), target.game.external_id()],
                    |row| row.get::<_, String>(0),
                )
                .map_err(|error| {
                    AppError::from(format!("Failed to query collection membership: {error}"))
                })?;
            membership_rows
                .collect::<Result<HashSet<_>, _>>()
                .map_err(|error| {
                    AppError::from(format!("Failed to decode collection membership: {error}"))
                })?
        } else {
            HashSet::new()
        };

        for collection in &mut collections {
            collection.contains_game = membership_ids.contains(&collection.id);
        }

        Ok(collections)
    }

    pub(crate) fn create_collection(
        &self,
        user_id: &str,
        name: &CollectionName,
    ) -> AppResult<CollectionRecord> {
        let connection = crate::open_connection(&self.state.db_path).map_err(AppError::from)?;
        let collection_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        let insert_result = connection.execute(
            "
            INSERT INTO collections (id, user_id, name, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            params![collection_id, user_id, name.as_str(), now, now],
        );

        match insert_result {
            Ok(_) => Ok(CollectionRecord {
                id: collection_id,
                name: name.as_str().to_owned(),
                game_count: 0,
                contains_game: false,
            }),
            Err(error)
                if error.to_string().contains(
                    "UNIQUE constraint failed: collections.user_id, collections.name",
                ) =>
            {
                Err(AppError::from("Collection name already exists"))
            }
            Err(error) => Err(AppError::from(format!(
                "Failed to create collection: {error}"
            ))),
        }
    }

    pub(crate) fn rename_collection(
        &self,
        user_id: &str,
        collection_id: &CollectionId,
        name: &CollectionName,
    ) -> AppResult<CollectionRecord> {
        let connection = crate::open_connection(&self.state.db_path).map_err(AppError::from)?;
        self.ensure_owned_collection_exists_with_connection(&connection, user_id, collection_id)?;
        let now = Utc::now().to_rfc3339();

        let update_result = connection.execute(
            "
            UPDATE collections
            SET name = ?1, updated_at = ?2
            WHERE id = ?3 AND user_id = ?4
            ",
            params![name.as_str(), now, collection_id.as_str(), user_id],
        );

        match update_result {
            Ok(updated_rows) => {
                if updated_rows == 0 {
                    return Err(AppError::from("Collection not found for current user"));
                }
            }
            Err(error)
                if error.to_string().contains(
                    "UNIQUE constraint failed: collections.user_id, collections.name",
                ) =>
            {
                return Err(AppError::from("Collection name already exists"));
            }
            Err(error) => {
                return Err(AppError::from(format!(
                    "Failed to rename collection: {error}"
                )));
            }
        }

        let game_count_raw = connection
            .query_row(
                "
                SELECT COUNT(*)
                FROM collection_games
                WHERE user_id = ?1 AND collection_id = ?2
                ",
                params![user_id, collection_id.as_str()],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| {
                AppError::from(format!("Failed to query renamed collection size: {error}"))
            })?;

        Ok(CollectionRecord {
            id: collection_id.as_str().to_owned(),
            name: name.as_str().to_owned(),
            game_count: usize::try_from(game_count_raw).unwrap_or_default(),
            contains_game: false,
        })
    }

    pub(crate) fn delete_collection(
        &self,
        user_id: &str,
        collection_id: &CollectionId,
    ) -> AppResult<()> {
        let connection = crate::open_connection(&self.state.db_path).map_err(AppError::from)?;
        self.ensure_owned_collection_exists_with_connection(&connection, user_id, collection_id)?;

        let deleted_rows = connection
            .execute(
                "DELETE FROM collections WHERE id = ?1 AND user_id = ?2",
                params![collection_id.as_str(), user_id],
            )
            .map_err(|error| AppError::from(format!("Failed to delete collection: {error}")))?;
        if deleted_rows == 0 {
            return Err(AppError::from("Collection not found for current user"));
        }

        Ok(())
    }

    pub(crate) fn add_game_to_collection_membership(
        &self,
        user_id: &str,
        collection_id: &CollectionId,
        game: &GameIdentity,
    ) -> AppResult<()> {
        let connection = crate::open_connection(&self.state.db_path).map_err(AppError::from)?;
        connection
            .execute(
                "
                INSERT OR IGNORE INTO collection_games (user_id, collection_id, provider, external_id, created_at)
                VALUES (?1, ?2, ?3, ?4, ?5)
                ",
                params![
                    user_id,
                    collection_id.as_str(),
                    game.provider(),
                    game.external_id(),
                    Utc::now().to_rfc3339()
                ],
            )
            .map_err(|error| AppError::from(format!("Failed to add game to collection: {error}")))?;
        Ok(())
    }
}
