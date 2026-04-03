use rusqlite::{params, OptionalExtension};

use crate::application::error::{AppError, AppResult};
use crate::domain::game::GameIdentity;
use crate::AppState;

pub(crate) struct SQLiteLibraryRepo<'a> {
    state: &'a AppState,
}

impl<'a> SQLiteLibraryRepo<'a> {
    pub(crate) fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub(crate) fn ensure_owned_game_exists(
        &self,
        user_id: &str,
        game: &GameIdentity,
    ) -> AppResult<()> {
        let connection = crate::open_connection(&self.state.db_path).map_err(AppError::from)?;
        let exists = connection
            .query_row(
                "SELECT 1 FROM games WHERE user_id = ?1 AND provider = ?2 AND external_id = ?3",
                params![user_id, game.provider(), game.external_id()],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|error| {
                AppError::from(format!("Failed to validate game ownership: {error}"))
            })?;

        if exists.is_none() {
            return Err(AppError::from("Game not found for current user"));
        }

        Ok(())
    }
}
