use std::error::Error;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::Manager;

#[derive(Clone)]
pub(crate) struct AppContext {
    pub(crate) db_path: PathBuf,
    pub(crate) session_token_path: PathBuf,
    pub(crate) steam_api_key: Option<String>,
    pub(crate) steam_local_install_detection: bool,
    pub(crate) steam_settings_debug_logging: bool,
    pub(crate) steam_root_override: Option<String>,
}

impl AppContext {
    fn new(
        db_path: PathBuf,
        session_token_path: PathBuf,
        steam_api_key: Option<String>,
        steam_local_install_detection: bool,
        steam_settings_debug_logging: bool,
        steam_root_override: Option<String>,
    ) -> Self {
        Self {
            db_path,
            session_token_path,
            steam_api_key,
            steam_local_install_detection,
            steam_settings_debug_logging,
            steam_root_override,
        }
    }
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) context: AppContext,
    pub(crate) current_session_token: Arc<Mutex<Option<String>>>,
}

impl AppState {
    pub(crate) fn new(
        db_path: PathBuf,
        session_token_path: PathBuf,
        steam_api_key: Option<String>,
        steam_local_install_detection: bool,
        steam_settings_debug_logging: bool,
        steam_root_override: Option<String>,
    ) -> Self {
        Self::from_context(AppContext::new(
            db_path,
            session_token_path,
            steam_api_key,
            steam_local_install_detection,
            steam_settings_debug_logging,
            steam_root_override,
        ))
    }

    pub(crate) fn from_context(context: AppContext) -> Self {
        Self {
            context,
            current_session_token: Arc::new(Mutex::new(None)),
        }
    }
}

impl Deref for AppState {
    type Target = AppContext;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

struct BootstrapPaths {
    db_path: PathBuf,
    session_token_path: PathBuf,
}

pub(crate) fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn Error>> {
    setup_app_inner(app)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error).into())
}

fn setup_app_inner(app: &mut tauri::App) -> Result<(), String> {
    let paths = resolve_paths(app)?;
    crate::initialize_database(&paths.db_path)?;
    let configured_steam_root_override = optional_env("STEAM_ROOT_OVERRIDE");

    let context = AppContext::new(
        paths.db_path,
        paths.session_token_path,
        optional_env("STEAM_API_KEY"),
        env_flag("STEAM_LOCAL_INSTALL_DETECTION", true),
        env_flag("STEAM_SETTINGS_DEBUG_LOGGING", false),
        configured_steam_root_override,
    );

    let state = AppState::new(
        context.db_path.clone(),
        context.session_token_path.clone(),
        context.steam_api_key.clone(),
        context.steam_local_install_detection,
        context.steam_settings_debug_logging,
        context.steam_root_override.clone(),
    );
    crate::restore_persisted_session(&state)?;

    app.manage(context);
    app.manage(state);
    Ok(())
}

fn resolve_paths(app: &tauri::App) -> Result<BootstrapPaths, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Failed to resolve app data directory: {error}"))?;

    Ok(BootstrapPaths {
        db_path: app_data_dir.join("catalyst.db"),
        session_token_path: app_data_dir.join("session.token"),
    })
}

fn optional_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn env_flag(name: &str, default_value: bool) -> bool {
    let Ok(raw_value) = std::env::var(name) else {
        return default_value;
    };

    match raw_value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => default_value,
    }
}
