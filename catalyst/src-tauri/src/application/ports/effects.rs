#![allow(dead_code)]

use std::path::{Path, PathBuf};

use crate::application::error::AppResult;

/// Port for persistence/database effects currently used by application services.
pub(crate) trait PersistencePort {
    fn execute_write(&self, operation: &str) -> AppResult<()>;
    fn execute_read(&self, operation: &str) -> AppResult<()>;
}

/// Port for external HTTP effects currently used by Steam-facing services.
pub(crate) trait HttpPort {
    fn get_text(&self, endpoint: &str) -> AppResult<String>;
    fn post_form_text(&self, endpoint: &str, body: &[(&str, &str)]) -> AppResult<String>;
}

/// Port for filesystem effects currently used by game/library/settings services.
pub(crate) trait FilesystemPort {
    fn read_to_string(&self, path: &Path) -> AppResult<String>;
    fn write_string(&self, path: &Path, content: &str) -> AppResult<()>;
    fn list_paths(&self, dir: &Path) -> AppResult<Vec<PathBuf>>;
}

/// Port for system command/process effects currently used by actions/settings services.
pub(crate) trait ProcessPort {
    fn spawn_command(&self, command: &str, args: &[&str]) -> AppResult<()>;
}
