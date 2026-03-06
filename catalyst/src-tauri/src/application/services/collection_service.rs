use crate::*;
use crate::application::error::{AppError, AppResult};

pub(crate) fn list_collections(
	state: &AppState,
	provider: Option<String>,
	external_id: Option<String>,
) -> AppResult<Vec<CollectionResponse>> {
	let connection = open_connection(&state.db_path)?;
	cleanup_expired_sessions(&connection)?;
	let user = get_authenticated_user(state, &connection)?;

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
			return Err(AppError::validation(
				"missing_identity_pair",
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

pub(crate) fn create_collection(
	state: &AppState,
	name: String,
) -> AppResult<CollectionResponse> {
	let connection = open_connection(&state.db_path)?;
	cleanup_expired_sessions(&connection)?;
	let user = get_authenticated_user(state, &connection)?;
	Ok(create_user_collection(&connection, &user.id, &name)?)
}

pub(crate) fn rename_collection(
	state: &AppState,
	collection_id: String,
	name: String,
) -> AppResult<CollectionResponse> {
	let trimmed_collection_id = collection_id.trim();
	if trimmed_collection_id.is_empty() {
		return Err(AppError::validation(
			"collection_id_required",
			"Collection ID is required",
		));
	}

	let connection = open_connection(&state.db_path)?;
	cleanup_expired_sessions(&connection)?;
	let user = get_authenticated_user(state, &connection)?;
	Ok(rename_user_collection(&connection, &user.id, trimmed_collection_id, &name)?)
}

pub(crate) fn delete_collection(
	state: &AppState,
	collection_id: String,
) -> AppResult<()> {
	let trimmed_collection_id = collection_id.trim();
	if trimmed_collection_id.is_empty() {
		return Err(AppError::validation(
			"collection_id_required",
			"Collection ID is required",
		));
	}

	let connection = open_connection(&state.db_path)?;
	cleanup_expired_sessions(&connection)?;
	let user = get_authenticated_user(state, &connection)?;
	Ok(delete_user_collection(&connection, &user.id, trimmed_collection_id)?)
}

pub(crate) fn add_game_to_collection(
	state: &AppState,
	provider: String,
	external_id: String,
	collection_id: String,
) -> AppResult<()> {
	let trimmed_collection_id = collection_id.trim();
	if trimmed_collection_id.is_empty() {
		return Err(AppError::validation(
			"collection_id_required",
			"Collection ID is required",
		));
	}

	let connection = open_connection(&state.db_path)?;
	cleanup_expired_sessions(&connection)?;
	let user = get_authenticated_user(state, &connection)?;
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

#[cfg(test)]
mod tests {
	use super::*;
	use std::fs;
	use tempfile::tempdir;

	#[test]
	fn create_and_list_collections_for_user() {
		let dir = tempdir().unwrap();
		let db_path = dir.path().join("test.db");
		let session_path = dir.path().join("session");
		let state = AppState::new(
			db_path.clone(),
			session_path,
			None,
			false,
			false,
			None,
		);

		// initialize database
		initialize_database(&db_path).expect("init db");

		let conn = open_connection(&db_path).expect("open conn");
		let user = create_user(&conn, "test@example.com", "password", None).expect("create user");
		let token = create_session(&conn, &user.id).expect("create session");
		*state.current_session_token.lock().unwrap() = Some(token);

		// create collection
		let created = create_collection(&state, "My Collection".to_string()).expect("create collection");
		assert_eq!(created.name, "My Collection");

		// list collections
		let list = list_collections(&state, None, None).expect("list");
		assert!(list.iter().any(|c| c.id == created.id));

		// cleanup
		drop(conn);
		fs::remove_file(db_path).ok();
	}
}

