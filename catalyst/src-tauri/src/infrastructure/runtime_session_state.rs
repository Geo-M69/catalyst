use crate::AppState;
use std::fs;
use std::path::Path;

pub(crate) fn get_state_session_token(state: &AppState) -> Result<Option<String>, String> {
    let guard = state
        .current_session_token
        .lock()
        .map_err(|_| String::from("Failed to acquire session token lock"))?;
    Ok(guard.clone())
}

pub(crate) fn set_state_session_token(
    state: &AppState,
    session_token: Option<String>,
) -> Result<(), String> {
    let mut guard = state
        .current_session_token
        .lock()
        .map_err(|_| String::from("Failed to acquire session token lock"))?;
    *guard = session_token;
    Ok(())
}

pub(crate) fn persist_active_session(state: &AppState, session_token: &str) -> Result<(), String> {
    persist_session_token(&state.session_token_path, session_token)?;
    set_state_session_token(state, Some(session_token.to_owned()))
}

pub(crate) fn clear_active_session(state: &AppState) -> Result<(), String> {
    clear_session_token_file(&state.session_token_path)?;
    set_state_session_token(state, None)
}

pub(crate) fn read_session_token(session_path: &Path) -> Result<Option<String>, String> {
    let content = match fs::read_to_string(session_path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Failed to read session token file: {error}")),
    };

    let token = content.trim().to_owned();
    if token.is_empty() {
        clear_session_token_file(session_path)?;
        return Ok(None);
    }

    Ok(Some(token))
}

fn persist_session_token(session_path: &Path, session_token: &str) -> Result<(), String> {
    if let Some(parent) = session_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create session directory: {error}"))?;
    }
    // On Unix platforms, prefer creating the file with restrictive permissions (rw-------).
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut options = fs::OpenOptions::new();
        options.create(true).write(true).truncate(true).mode(0o600);
        let mut file = options
            .open(session_path)
            .map_err(|error| format!("Failed to open session token file: {error}"))?;
        use std::io::Write;
        file.write_all(session_token.as_bytes())
            .map_err(|error| format!("Failed to write session token file: {error}"))?;
        Ok(())
    }

    #[cfg(not(unix))]
    {
        fs::write(session_path, session_token)
            .map_err(|error| format!("Failed to write session token file: {error}"))
    }
}

fn clear_session_token_file(session_path: &Path) -> Result<(), String> {
    match fs::remove_file(session_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Failed to clear session token file: {error}")),
    }
}
