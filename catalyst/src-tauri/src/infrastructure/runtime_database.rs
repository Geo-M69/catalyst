use rusqlite::Connection;
use std::fs;
use std::path::Path;

pub(crate) fn open_connection(db_path: &Path) -> Result<Connection, String> {
    let connection = Connection::open(db_path)
        .map_err(|error| format!("Failed to open SQLite database: {error}"))?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|error| format!("Failed to configure SQLite connection: {error}"))?;
    Ok(connection)
}

pub(crate) fn initialize_database(db_path: &Path) -> Result<(), String> {
    if let Some(parent_dir) = db_path.parent() {
        fs::create_dir_all(parent_dir)
            .map_err(|error| format!("Failed to create app data directory: {error}"))?;
    }

    let connection = open_connection(db_path)?;
    connection
        .execute_batch(
            "
            PRAGMA journal_mode = WAL;

            CREATE TABLE IF NOT EXISTS users (
              id TEXT PRIMARY KEY,
              email TEXT NOT NULL UNIQUE,
              password_hash TEXT NOT NULL,
              steam_id TEXT UNIQUE,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS sessions (
              token_hash TEXT PRIMARY KEY,
              user_id TEXT NOT NULL,
              created_at TEXT NOT NULL,
              expires_at TEXT NOT NULL,
              last_seen_at TEXT NOT NULL,
              FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id);
            CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions(expires_at);

            CREATE TABLE IF NOT EXISTS games (
              user_id TEXT NOT NULL,
              provider TEXT NOT NULL,
              external_id TEXT NOT NULL,
              name TEXT NOT NULL,
              kind TEXT NOT NULL DEFAULT 'unknown',
              playtime_minutes INTEGER NOT NULL,
              installed INTEGER NOT NULL DEFAULT 0,
              artwork_url TEXT,
                            last_synced_at TEXT NOT NULL,
                            last_played_at TEXT,
              PRIMARY KEY (user_id, provider, external_id),
              FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_games_user_id ON games(user_id);
            CREATE INDEX IF NOT EXISTS idx_games_provider ON games(provider);

            CREATE TABLE IF NOT EXISTS game_favorites (
              user_id TEXT NOT NULL,
              provider TEXT NOT NULL,
              external_id TEXT NOT NULL,
              created_at TEXT NOT NULL,
              PRIMARY KEY (user_id, provider, external_id),
              FOREIGN KEY (user_id, provider, external_id) REFERENCES games(user_id, provider, external_id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_game_favorites_user_id ON game_favorites(user_id);

            CREATE TABLE IF NOT EXISTS collections (
              id TEXT PRIMARY KEY,
              user_id TEXT NOT NULL,
              name TEXT NOT NULL COLLATE NOCASE,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL,
              UNIQUE (user_id, name),
              UNIQUE (id, user_id),
              FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_collections_user_id ON collections(user_id);

            CREATE TABLE IF NOT EXISTS collection_games (
              user_id TEXT NOT NULL,
              collection_id TEXT NOT NULL,
              provider TEXT NOT NULL,
              external_id TEXT NOT NULL,
              created_at TEXT NOT NULL,
              PRIMARY KEY (user_id, collection_id, provider, external_id),
              FOREIGN KEY (user_id, provider, external_id) REFERENCES games(user_id, provider, external_id) ON DELETE CASCADE,
              FOREIGN KEY (collection_id, user_id) REFERENCES collections(id, user_id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_collection_games_user_game
              ON collection_games(user_id, provider, external_id);
            CREATE INDEX IF NOT EXISTS idx_collection_games_collection_id
              ON collection_games(collection_id);

            CREATE TABLE IF NOT EXISTS game_privacy_settings (
              user_id TEXT NOT NULL,
              provider TEXT NOT NULL,
              external_id TEXT NOT NULL,
              hide_in_library INTEGER NOT NULL DEFAULT 0,
              mark_as_private INTEGER NOT NULL DEFAULT 0,
              overlay_data_deleted INTEGER NOT NULL DEFAULT 0,
              updated_at TEXT NOT NULL,
              PRIMARY KEY (user_id, provider, external_id),
              FOREIGN KEY (user_id, provider, external_id) REFERENCES games(user_id, provider, external_id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_game_privacy_settings_user_id ON game_privacy_settings(user_id);

            CREATE TABLE IF NOT EXISTS game_properties_settings (
              user_id TEXT NOT NULL,
              provider TEXT NOT NULL,
              external_id TEXT NOT NULL,
              settings_json TEXT NOT NULL,
              updated_at TEXT NOT NULL,
              PRIMARY KEY (user_id, provider, external_id),
              FOREIGN KEY (user_id, provider, external_id) REFERENCES games(user_id, provider, external_id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_game_properties_settings_user_id ON game_properties_settings(user_id);

            CREATE TABLE IF NOT EXISTS steam_app_metadata (
              app_id TEXT PRIMARY KEY,
              app_type TEXT NOT NULL,
              fetched_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_steam_app_metadata_fetched_at ON steam_app_metadata(fetched_at);

            CREATE TABLE IF NOT EXISTS steam_app_languages (
              app_id TEXT PRIMARY KEY,
              languages_json TEXT NOT NULL,
              fetched_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_steam_app_languages_fetched_at ON steam_app_languages(fetched_at);

            CREATE TABLE IF NOT EXISTS steam_app_betas (
              app_id TEXT PRIMARY KEY,
              betas_json TEXT NOT NULL,
              fetched_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_steam_app_betas_fetched_at ON steam_app_betas(fetched_at);

            CREATE TABLE IF NOT EXISTS steam_app_store_tags (
              app_id TEXT PRIMARY KEY,
              tags_json TEXT NOT NULL,
              fetched_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS steam_friends_activity_cache (
              steam_id TEXT NOT NULL,
              app_id TEXT NOT NULL,
              friend_list_fingerprint TEXT NOT NULL DEFAULT '',
              response_json TEXT NOT NULL,
              fetched_at TEXT NOT NULL,
              PRIMARY KEY (steam_id, app_id)
            );

            CREATE INDEX IF NOT EXISTS idx_steam_friends_activity_cache_fetched_at
              ON steam_friends_activity_cache(fetched_at);

            CREATE TABLE IF NOT EXISTS steam_review_cache (
              steam_id TEXT NOT NULL,
              app_id TEXT NOT NULL,
              response_json TEXT NOT NULL,
              fetched_at TEXT NOT NULL,
              PRIMARY KEY (steam_id, app_id)
            );

            CREATE INDEX IF NOT EXISTS idx_steam_review_cache_fetched_at
              ON steam_review_cache(fetched_at);

            CREATE TABLE IF NOT EXISTS steam_activity_timeline_cache (
              steam_id TEXT NOT NULL,
              app_id TEXT NOT NULL,
              response_json TEXT NOT NULL,
              fetched_at TEXT NOT NULL,
              PRIMARY KEY (steam_id, app_id)
            );

            CREATE INDEX IF NOT EXISTS idx_steam_activity_timeline_cache_fetched_at
              ON steam_activity_timeline_cache(fetched_at);

            CREATE TABLE IF NOT EXISTS steam_achievements_cache (
              steam_id TEXT NOT NULL,
              app_id TEXT NOT NULL,
              response_json TEXT NOT NULL,
              fetched_at TEXT NOT NULL,
              PRIMARY KEY (steam_id, app_id)
            );

            CREATE INDEX IF NOT EXISTS idx_steam_achievements_cache_fetched_at
              ON steam_achievements_cache(fetched_at);

                        CREATE TABLE IF NOT EXISTS game_genres (
                            user_id TEXT NOT NULL,
                            provider TEXT NOT NULL,
                            external_id TEXT NOT NULL,
                            genre TEXT NOT NULL,
                            PRIMARY KEY (user_id, provider, external_id, genre),
                            FOREIGN KEY (user_id, provider, external_id) REFERENCES games(user_id, provider, external_id) ON DELETE CASCADE
                        );

                        CREATE INDEX IF NOT EXISTS idx_game_genres_user_game ON game_genres(user_id, provider, external_id);

            CREATE TABLE IF NOT EXISTS steam_app_details (
                app_id TEXT PRIMARY KEY,
                details_json TEXT NOT NULL,
                fetched_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_steam_app_details_fetched_at ON steam_app_details(fetched_at);

            CREATE TABLE IF NOT EXISTS steam_public_app_names (
                app_id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                fetched_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_steam_public_app_names_fetched_at
              ON steam_public_app_names(fetched_at);

                        CREATE TABLE IF NOT EXISTS steam_app_features (
                            app_id TEXT PRIMARY KEY,
                            has_achievements INTEGER NOT NULL DEFAULT 0,
                                        achievements_count INTEGER,
                                        has_cloud_saves INTEGER NOT NULL DEFAULT 0,
                                        cloud_details TEXT,
                                        controller_support TEXT,
                                        fetched_at TEXT NOT NULL
                                    );

                                    CREATE INDEX IF NOT EXISTS idx_steam_app_features_fetched_at ON steam_app_features(fetched_at);

            CREATE INDEX IF NOT EXISTS idx_steam_app_store_tags_fetched_at ON steam_app_store_tags(fetched_at);
            ",
        )
        .map_err(|error| format!("Failed to run SQLite migrations: {error}"))?;
    migrate_games_table(&connection)?;
    migrate_steam_friends_activity_cache_table(&connection)?;

    Ok(())
}

fn migrate_games_table(connection: &Connection) -> Result<(), String> {
    if !games_table_has_column(connection, "kind")? {
        connection
            .execute(
                "ALTER TABLE games ADD COLUMN kind TEXT NOT NULL DEFAULT 'unknown'",
                [],
            )
            .map_err(|error| format!("Failed to migrate games table with kind column: {error}"))?;
    }

    if !games_table_has_column(connection, "installed")? {
        connection
            .execute(
                "ALTER TABLE games ADD COLUMN installed INTEGER NOT NULL DEFAULT 0",
                [],
            )
            .map_err(|error| {
                format!("Failed to migrate games table with installed column: {error}")
            })?;
    }

    if !games_table_has_column(connection, "last_played_at")? {
        connection
            .execute("ALTER TABLE games ADD COLUMN last_played_at TEXT", [])
            .map_err(|error| {
                format!("Failed to migrate games table with last_played_at column: {error}")
            })?;
    }

    Ok(())
}

fn games_table_has_column(connection: &Connection, expected_column: &str) -> Result<bool, String> {
    table_has_column(connection, "games", expected_column)
}

fn migrate_steam_friends_activity_cache_table(connection: &Connection) -> Result<(), String> {
    if !table_has_column(
        connection,
        "steam_friends_activity_cache",
        "friend_list_fingerprint",
    )? {
        connection
            .execute(
                "ALTER TABLE steam_friends_activity_cache ADD COLUMN friend_list_fingerprint TEXT NOT NULL DEFAULT ''",
                [],
            )
            .map_err(|error| {
                format!(
                    "Failed to migrate steam_friends_activity_cache with friend_list_fingerprint column: {error}"
                )
            })?;
    }
    Ok(())
}

fn table_has_column(
    connection: &Connection,
    table_name: &str,
    expected_column: &str,
) -> Result<bool, String> {
    let pragma_query = format!("PRAGMA table_info({table_name})");
    let mut statement = connection
        .prepare(&pragma_query)
        .map_err(|error| format!("Failed to inspect {table_name} table schema: {error}"))?;

    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| format!("Failed to query {table_name} table schema: {error}"))?;

    for row in rows {
        let column_name = row
            .map_err(|error| format!("Failed to decode {table_name} table schema row: {error}"))?;
        if column_name == expected_column {
            return Ok(true);
        }
    }

    Ok(false)
}
