use crate::application::error::{AppError, AppResult};
use crate::application::ports::game_actions::GameActionsPort;
use crate::cleanup_expired_sessions;
use crate::domain::game::parse_steam_app_id;
use crate::ensure_owned_game_exists;
use crate::get_authenticated_user;
use crate::infrastructure::launcher_ops::LauncherOps;
use crate::infrastructure::steam_local::SteamLocal;
use crate::load_game_properties_settings;
use crate::normalize_game_identity_input;
use crate::open_connection;
use crate::AppState;
use chrono::Utc;
use rusqlite::{params, OptionalExtension};

#[derive(Clone)]
pub(crate) struct InfrastructureGameActionsPort {
    state: AppState,
}

impl InfrastructureGameActionsPort {
    pub(crate) fn new(state: &AppState) -> Self {
        Self {
            state: state.clone(),
        }
    }
}

impl GameActionsPort for InfrastructureGameActionsPort {
    fn play_game(
        &self,
        provider: String,
        external_id: String,
        launch_options: Option<String>,
    ) -> AppResult<()> {
        let launcher = LauncherOps::new();
        let connection = open_connection(&self.state.db_path)?;
        cleanup_expired_sessions(&connection)?;
        let user = get_authenticated_user(&self.state, &connection)?;
        let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
        ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;
        let resolved_launch_options = match launch_options
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(value) => Some(value.to_owned()),
            None => load_game_properties_settings(&connection, &user.id, &provider, &external_id)
                .ok()
                .and_then(|settings| {
                    let trimmed_value = settings.general.launch_options.trim();
                    if trimmed_value.is_empty() {
                        None
                    } else {
                        Some(trimmed_value.to_owned())
                    }
                }),
        };
        launcher.open_provider_game_uri(
            &provider,
            &external_id,
            "play",
            resolved_launch_options.as_deref(),
        )?;

        let played_at = Utc::now().to_rfc3339();
        if let Err(error) = connection.execute(
            "
            UPDATE games
            SET last_played_at = ?1
            WHERE user_id = ?2 AND provider = ?3 AND external_id = ?4
            ",
            params![played_at, &user.id, &provider, &external_id],
        ) {
            eprintln!("Could not persist game launch timestamp: {error}");
        }

        Ok(())
    }

    fn install_game(
        &self,
        provider: String,
        external_id: String,
        install_path: Option<String>,
        create_desktop_shortcut: Option<bool>,
        create_application_shortcut: Option<bool>,
    ) -> AppResult<()> {
        let launcher = LauncherOps::new();
        let connection = open_connection(&self.state.db_path)?;
        cleanup_expired_sessions(&connection)?;
        let user = get_authenticated_user(&self.state, &connection)?;
        let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
        ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;
        // Steam currently controls install destination and shortcut behavior from its own flow.
        // Keep receiving these values so the UI can evolve without breaking command contracts.
        let _ = (
            install_path,
            create_desktop_shortcut,
            create_application_shortcut,
        );
        launcher.open_provider_game_uri(&provider, &external_id, "install", None)
    }

    fn uninstall_game(&self, provider: String, external_id: String) -> AppResult<()> {
        let launcher = LauncherOps::new();
        let connection = open_connection(&self.state.db_path)?;
        cleanup_expired_sessions(&connection)?;
        let user = get_authenticated_user(&self.state, &connection)?;
        let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
        ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;
        launcher.open_provider_game_uri(&provider, &external_id, "uninstall", None)
    }

    fn browse_game_installed_files(&self, provider: String, external_id: String) -> AppResult<()> {
        let launcher = LauncherOps::new();
        let steam_local = SteamLocal::new(self.state.steam_root_override.as_deref());
        let connection = open_connection(&self.state.db_path)?;
        cleanup_expired_sessions(&connection)?;
        let user = get_authenticated_user(&self.state, &connection)?;
        let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
        ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;

        if provider != "steam" {
            return Err(AppError::validation(
                "unsupported_provider",
                "Browsing installed files is only supported for Steam games.",
            ));
        }

        let app_id = parse_steam_app_id(&external_id)?;
        let install_directory = steam_local.resolve_install_directory_for_app_id(app_id)?;
        if !install_directory.is_dir() {
            return Err(AppError::not_found(
                "install_directory_missing",
                format!(
                    "Install directory is unavailable: {}",
                    install_directory.display()
                ),
            ));
        }

        launcher.open_path_in_file_manager(&install_directory)
    }

    fn backup_game_files(&self, provider: String, external_id: String) -> AppResult<()> {
        let launcher = LauncherOps::new();
        let connection = open_connection(&self.state.db_path)?;
        cleanup_expired_sessions(&connection)?;
        let user = get_authenticated_user(&self.state, &connection)?;
        let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
        ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;
        launcher.open_provider_game_uri(&provider, &external_id, "backup", None)
    }

    fn verify_game_files(&self, provider: String, external_id: String) -> AppResult<()> {
        let launcher = LauncherOps::new();
        let connection = open_connection(&self.state.db_path)?;
        cleanup_expired_sessions(&connection)?;
        let user = get_authenticated_user(&self.state, &connection)?;
        let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
        ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;
        launcher.open_provider_game_uri(&provider, &external_id, "validate", None)
    }

    fn add_game_desktop_shortcut(&self, provider: String, external_id: String) -> AppResult<()> {
        let launcher = LauncherOps::new();
        let connection = open_connection(&self.state.db_path)?;
        cleanup_expired_sessions(&connection)?;
        let user = get_authenticated_user(&self.state, &connection)?;
        let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
        ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;

        let fallback_name = format!("Game {}", external_id);
        let game_name = connection
            .query_row(
                "
                SELECT name
                FROM games
                WHERE user_id = ?1 AND provider = ?2 AND external_id = ?3
                ",
                params![&user.id, &provider, &external_id],
                |record| record.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| format!("Failed to query game name for desktop shortcut: {error}"))?
            .unwrap_or(fallback_name);

        launcher.create_provider_game_desktop_shortcut(&provider, &external_id, &game_name)
    }

    fn open_game_recording_settings(&self, provider: String, external_id: String) -> AppResult<()> {
        let launcher = LauncherOps::new();
        let connection = open_connection(&self.state.db_path)?;
        cleanup_expired_sessions(&connection)?;
        let user = get_authenticated_user(&self.state, &connection)?;
        let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
        ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;

        if provider != "steam" {
            return Err(AppError::validation(
                "unsupported_provider",
                "Game recording settings are currently only available for Steam games.",
            ));
        }

        launcher.open_steam_game_recording_settings()
    }
}
