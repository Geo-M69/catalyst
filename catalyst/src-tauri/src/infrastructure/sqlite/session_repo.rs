use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::application::error::{AppError, AppResult};
use crate::AppState;

pub(crate) struct SQLiteSessionRepo<'a> {
    state: &'a AppState,
}

impl<'a> SQLiteSessionRepo<'a> {
    pub(crate) fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub(crate) fn authenticated_user_id(&self) -> AppResult<String> {
        let session_token = crate::get_state_session_token(self.state)?
            .ok_or_else(|| AppError::from("Not authenticated"))?;

        let connection = self.connection()?;
        self.cleanup_expired_sessions(&connection)?;

        match self.find_user_id_by_session_token(&connection, &session_token)? {
            Some(user_id) => Ok(user_id),
            None => {
                crate::clear_active_session(self.state)?;
                Err(AppError::from("Session expired or invalid"))
            }
        }
    }

    fn connection(&self) -> AppResult<Connection> {
        crate::open_connection(&self.state.db_path).map_err(AppError::from)
    }

    fn cleanup_expired_sessions(&self, connection: &Connection) -> AppResult<()> {
        connection
            .execute(
                "DELETE FROM sessions WHERE expires_at <= ?1",
                params![Utc::now().to_rfc3339()],
            )
            .map_err(|error| AppError::from(format!("Failed to cleanup expired sessions: {error}")))?;
        Ok(())
    }

    fn find_user_id_by_session_token(
        &self,
        connection: &Connection,
        session_token: &str,
    ) -> AppResult<Option<String>> {
        let token_hash = hash_session_token(session_token);
        let now = Utc::now().to_rfc3339();

        let user_id = connection
            .query_row(
                "SELECT u.id FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.token_hash = ?1 AND s.expires_at > ?2",
                params![token_hash, now],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| AppError::from(format!("Failed to query session user: {error}")))?;

        if user_id.is_some() {
            connection
                .execute(
                    "UPDATE sessions SET last_seen_at = ?1 WHERE token_hash = ?2",
                    params![Utc::now().to_rfc3339(), hash_session_token(session_token)],
                )
                .map_err(|error| AppError::from(format!("Failed to touch session: {error}")))?;
        }

        Ok(user_id)
    }
}

fn hash_session_token(session_token: &str) -> String {
    let digest = Sha256::digest(session_token.as_bytes());
    format!("{digest:x}")
}
