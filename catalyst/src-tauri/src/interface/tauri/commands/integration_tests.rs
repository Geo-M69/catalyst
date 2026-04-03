use super::{auth, library, steam};
use crate::application::bootstrap::AppState;
use chrono::{Duration as ChronoDuration, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{
    get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY,
};
use tauri::webview::InvokeRequest;
use tauri::{WebviewWindow, WebviewWindowBuilder};
use tempfile::TempDir;

struct CommandTestHarness {
    _temp_dir: TempDir,
    state: AppState,
    db_path: PathBuf,
}

impl CommandTestHarness {
    fn new(steam_api_key: Option<&str>) -> Self {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let db_path = temp_dir.path().join("catalyst.test.db");
        let session_token_path = temp_dir.path().join("session.token");

        crate::initialize_database(&db_path).expect("initialize database");
        let state = AppState::new(
            db_path.clone(),
            session_token_path,
            steam_api_key.map(str::to_owned),
            false,
            false,
            None,
        );

        Self {
            _temp_dir: temp_dir,
            state,
            db_path,
        }
    }

    fn connection(&self) -> Connection {
        crate::open_connection(&self.db_path).expect("open database")
    }

    fn create_user(&self, email: &str, steam_id: Option<&str>) -> String {
        let connection = self.connection();
        let user = crate::infrastructure::runtime_auth::create_user(
            &connection,
            email,
            "$2b$12$testhash",
            steam_id,
        )
        .expect("create user");
        user.id
    }

    fn create_session(&self, user_id: &str) -> String {
        let connection = self.connection();
        crate::infrastructure::runtime_auth::create_session(&connection, user_id)
            .expect("create session")
    }

    fn set_active_session(&self, session_token: Option<&str>) {
        crate::infrastructure::runtime_session_state::set_state_session_token(
            &self.state,
            session_token.map(str::to_owned),
        )
        .expect("set active session");
    }

    fn current_session_token(&self) -> Option<String> {
        crate::infrastructure::runtime_session_state::get_state_session_token(&self.state)
            .expect("read active session")
    }

    fn expire_session(&self, session_token: &str) {
        let token_hash = crate::infrastructure::runtime_auth::hash_session_token(session_token);
        self.connection()
            .execute(
                "UPDATE sessions SET expires_at = ?1 WHERE token_hash = ?2",
                params![
                    (Utc::now() - ChronoDuration::minutes(5)).to_rfc3339(),
                    token_hash
                ],
            )
            .expect("expire session");
    }

    fn session_exists(&self, session_token: &str) -> bool {
        let token_hash = crate::infrastructure::runtime_auth::hash_session_token(session_token);
        self.connection()
            .query_row(
                "SELECT 1 FROM sessions WHERE token_hash = ?1",
                params![token_hash],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .expect("query session")
            .is_some()
    }

    fn insert_owned_game(&self, user_id: &str, provider: &str, external_id: &str) {
        self.connection()
            .execute(
                "INSERT INTO games (user_id, provider, external_id, name, kind, playtime_minutes, installed, artwork_url, last_synced_at, last_played_at)
                 VALUES (?1, ?2, ?3, ?4, 'game', 0, 0, NULL, ?5, NULL)",
                params![
                    user_id,
                    provider,
                    external_id,
                    format!("{provider}-{external_id}"),
                    Utc::now().to_rfc3339()
                ],
            )
            .expect("insert owned game");
    }

    fn favorite_count(&self, user_id: &str, provider: &str, external_id: &str) -> i64 {
        self.connection()
            .query_row(
                "SELECT COUNT(*) FROM game_favorites WHERE user_id = ?1 AND provider = ?2 AND external_id = ?3",
                params![user_id, provider, external_id],
                |row| row.get::<_, i64>(0),
            )
            .expect("count favorites")
    }
}

struct CommandApp {
    _app: tauri::App<MockRuntime>,
    webview: WebviewWindow<MockRuntime>,
}

impl CommandApp {
    fn new(state: AppState) -> Self {
        let app = mock_builder()
            .manage(state)
            .invoke_handler(tauri::generate_handler![
                auth::logout,
                auth::get_session,
                library::set_game_favorite,
                library::get_game_store_metadata,
                library::get_game_review,
                steam::list_game_versions_betas,
                steam::validate_game_beta_access_code,
            ])
            .build(mock_context(noop_assets()))
            .expect("build mock app");

        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");

        Self { _app: app, webview }
    }

    fn invoke_no_args(&self, command: &str) -> Result<Value, Value> {
        self.invoke(command, InvokeBody::default())
    }

    fn invoke_json(&self, command: &str, payload: Value) -> Result<Value, Value> {
        self.invoke(command, InvokeBody::Json(payload))
    }

    fn invoke(&self, command: &str, body: InvokeBody) -> Result<Value, Value> {
        let request = InvokeRequest {
            cmd: command.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "http://tauri.localhost".parse().expect("valid test URL"),
            body,
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        };

        get_ipc_response(&self.webview, request).map(|response| {
            response
                .deserialize::<Value>()
                .expect("deserialize command response")
        })
    }
}

fn assert_error_kind_and_code(error: &Value, expected_kind: &str, expected_code: &str) {
    assert_eq!(
        error.get("kind").and_then(Value::as_str),
        Some(expected_kind),
        "unexpected error payload: {error}"
    );
    assert_eq!(
        error.get("code").and_then(Value::as_str),
        Some(expected_code),
        "unexpected error payload: {error}"
    );
}

#[test]
fn get_session_returns_null_without_active_session() {
    let harness = CommandTestHarness::new(None);
    let app = CommandApp::new(harness.state.clone());

    let response = app
        .invoke_no_args("get_session")
        .expect("get_session should succeed");

    assert!(response.is_null());
}

#[test]
fn get_session_clears_invalid_session_token_in_state() {
    let harness = CommandTestHarness::new(None);
    harness.create_user("session-invalid@example.com", None);
    harness.set_active_session(Some("missing.session.token"));
    let app = CommandApp::new(harness.state.clone());

    let response = app
        .invoke_no_args("get_session")
        .expect("get_session should return null for invalid token");

    assert!(response.is_null());
    assert_eq!(harness.current_session_token(), None);
}

#[test]
fn get_session_expires_old_sessions_and_clears_active_token() {
    let harness = CommandTestHarness::new(None);
    let user_id = harness.create_user("session-expired@example.com", None);
    let session_token = harness.create_session(&user_id);
    harness.expire_session(&session_token);
    harness.set_active_session(Some(&session_token));
    let app = CommandApp::new(harness.state.clone());

    let response = app
        .invoke_no_args("get_session")
        .expect("expired session should return null");

    assert!(response.is_null());
    assert_eq!(harness.current_session_token(), None);
    assert!(!harness.session_exists(&session_token));
}

#[test]
fn logout_invalidates_database_session_and_clears_state() {
    let harness = CommandTestHarness::new(None);
    let user_id = harness.create_user("logout@example.com", None);
    let session_token = harness.create_session(&user_id);
    harness.set_active_session(Some(&session_token));
    let app = CommandApp::new(harness.state.clone());

    let response = app.invoke_no_args("logout").expect("logout should succeed");

    assert!(response.is_null());
    assert_eq!(harness.current_session_token(), None);
    assert!(!harness.session_exists(&session_token));
}

#[test]
fn set_game_favorite_rejects_unowned_game() {
    let harness = CommandTestHarness::new(None);
    let user_id = harness.create_user("favorite-unowned@example.com", None);
    let session_token = harness.create_session(&user_id);
    harness.set_active_session(Some(&session_token));
    let app = CommandApp::new(harness.state.clone());

    let error = app
        .invoke_json(
            "set_game_favorite",
            json!({
                "provider": "steam",
                "externalId": "10",
                "favorite": true,
            }),
        )
        .expect_err("set_game_favorite should fail when game is not owned");

    assert_error_kind_and_code(&error, "not_found", "not_found_error");
}

#[test]
fn set_game_favorite_rejects_game_owned_by_another_user() {
    let harness = CommandTestHarness::new(None);
    let user_id = harness.create_user("favorite-active@example.com", None);
    let other_user_id = harness.create_user("favorite-other@example.com", None);
    let session_token = harness.create_session(&user_id);
    harness.set_active_session(Some(&session_token));
    harness.insert_owned_game(&other_user_id, "steam", "20");
    let app = CommandApp::new(harness.state.clone());

    let error = app
        .invoke_json(
            "set_game_favorite",
            json!({
                "provider": "steam",
                "externalId": "20",
                "favorite": true,
            }),
        )
        .expect_err("set_game_favorite should fail for another user's game");

    assert_error_kind_and_code(&error, "not_found", "not_found_error");
}

#[test]
fn set_game_favorite_persists_membership_for_owned_game() {
    let harness = CommandTestHarness::new(None);
    let user_id = harness.create_user("favorite-owned@example.com", None);
    let session_token = harness.create_session(&user_id);
    harness.set_active_session(Some(&session_token));
    harness.insert_owned_game(&user_id, "steam", "30");
    let app = CommandApp::new(harness.state.clone());

    let response = app
        .invoke_json(
            "set_game_favorite",
            json!({
                "provider": "steam",
                "externalId": "30",
                "favorite": true,
            }),
        )
        .expect("set_game_favorite should succeed for owned game");

    assert!(response.is_null());
    assert_eq!(harness.favorite_count(&user_id, "steam", "30"), 1);
}

#[test]
fn get_game_store_metadata_returns_empty_response_for_non_steam_provider() {
    let harness = CommandTestHarness::new(None);
    let user_id = harness.create_user("metadata-gog@example.com", None);
    let session_token = harness.create_session(&user_id);
    harness.set_active_session(Some(&session_token));
    harness.insert_owned_game(&user_id, "gog", "gog-1");
    let app = CommandApp::new(harness.state.clone());

    let response = app
        .invoke_json(
            "get_game_store_metadata",
            json!({
                "provider": "gog",
                "externalId": "gog-1",
            }),
        )
        .expect("metadata command should succeed");

    assert!(response.get("shortDescription").is_some_and(Value::is_null));
    assert!(response.get("headerImage").is_some_and(Value::is_null));
    assert!(response.get("features").is_some_and(Value::is_null));
}

#[test]
fn get_game_store_metadata_returns_empty_response_for_invalid_steam_app_id() {
    let harness = CommandTestHarness::new(None);
    let user_id = harness.create_user("metadata-invalid@example.com", Some("76561198000000001"));
    let session_token = harness.create_session(&user_id);
    harness.set_active_session(Some(&session_token));
    harness.insert_owned_game(&user_id, "steam", "invalid-app-id");
    let app = CommandApp::new(harness.state.clone());

    let response = app
        .invoke_json(
            "get_game_store_metadata",
            json!({
                "provider": "steam",
                "externalId": "invalid-app-id",
            }),
        )
        .expect("metadata command should succeed");

    assert!(response.get("shortDescription").is_some_and(Value::is_null));
    assert!(response.get("headerImage").is_some_and(Value::is_null));
    assert!(response.get("features").is_some_and(Value::is_null));
}

#[test]
fn get_game_review_non_steam_provider_returns_fallback_warning() {
    let harness = CommandTestHarness::new(None);
    let user_id = harness.create_user("review-gog@example.com", None);
    let session_token = harness.create_session(&user_id);
    harness.set_active_session(Some(&session_token));
    harness.insert_owned_game(&user_id, "gog", "gog-review");
    let app = CommandApp::new(harness.state.clone());

    let response = app
        .invoke_json(
            "get_game_review",
            json!({
                "provider": "gog",
                "externalId": "gog-review",
                "forceRefresh": false,
            }),
        )
        .expect("review command should succeed");

    let warning = response
        .get("warning")
        .and_then(Value::as_str)
        .expect("warning should be present");
    assert!(warning.contains("Steam titles only"));
    assert!(response.get("review").is_some_and(Value::is_null));
}

#[test]
fn get_game_review_steam_game_without_linked_account_returns_warning() {
    let harness = CommandTestHarness::new(None);
    let user_id = harness.create_user("review-steam-unlinked@example.com", None);
    let session_token = harness.create_session(&user_id);
    harness.set_active_session(Some(&session_token));
    harness.insert_owned_game(&user_id, "steam", "570");
    let app = CommandApp::new(harness.state.clone());

    let response = app
        .invoke_json(
            "get_game_review",
            json!({
                "provider": "steam",
                "externalId": "570",
                "forceRefresh": false,
            }),
        )
        .expect("review command should succeed");

    let warning = response
        .get("warning")
        .and_then(Value::as_str)
        .expect("warning should be present");
    assert!(warning.contains("Connect Steam"));
    assert!(response.get("review").is_some_and(Value::is_null));
}

#[test]
fn list_game_versions_betas_without_api_key_uses_default_options_and_warning() {
    let harness = CommandTestHarness::new(None);
    let user_id = harness.create_user("betas-no-key@example.com", Some("76561198000000001"));
    let session_token = harness.create_session(&user_id);
    harness.set_active_session(Some(&session_token));
    harness.insert_owned_game(&user_id, "steam", "570");
    let app = CommandApp::new(harness.state.clone());

    let response = app
        .invoke_json(
            "list_game_versions_betas",
            json!({
                "provider": "steam",
                "externalId": "570",
            }),
        )
        .expect("list_game_versions_betas should succeed");

    let warning = response
        .get("warning")
        .and_then(Value::as_str)
        .expect("warning should be present");
    assert!(warning.contains("STEAM_API_KEY is not configured"));

    let options = response
        .get("options")
        .and_then(Value::as_array)
        .expect("options should be an array");
    assert!(!options.is_empty(), "expected at least one default option");
}

#[test]
fn validate_game_beta_access_code_empty_input_returns_validation_message() {
    let harness = CommandTestHarness::new(None);
    let user_id = harness.create_user("betas-empty-code@example.com", Some("76561198000000001"));
    let session_token = harness.create_session(&user_id);
    harness.set_active_session(Some(&session_token));
    harness.insert_owned_game(&user_id, "steam", "570");
    let app = CommandApp::new(harness.state.clone());

    let response = app
        .invoke_json(
            "validate_game_beta_access_code",
            json!({
                "provider": "steam",
                "externalId": "570",
                "accessCode": "   ",
            }),
        )
        .expect("validate_game_beta_access_code should succeed");

    assert_eq!(response.get("valid").and_then(Value::as_bool), Some(false));
    assert_eq!(response.get("branchId"), Some(&Value::Null));
    let message = response
        .get("message")
        .and_then(Value::as_str)
        .expect("message should be present");
    assert!(message.contains("Enter an access code"));
}
