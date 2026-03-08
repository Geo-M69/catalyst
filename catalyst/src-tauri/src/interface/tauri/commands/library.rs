use crate::*;
use crate::application::error::AppResult;
use crate::application::services::library_service::GameStoreMetadataResponse;
use tauri::{State, AppHandle};
use tauri::Emitter;

#[tauri::command]
pub(crate) fn get_library(state: State<'_, AppState>) -> AppResult<LibraryResponse> {
    crate::application::services::library_service::get_library(state.inner())
}

// `get_steam_status` command removed; Steam status is available via server-side
// logic and no longer exposed directly to the frontend.
#[tauri::command]
pub(crate) fn sync_steam_library(state: State<'_, AppState>) -> AppResult<SteamSyncResponse> {
    crate::application::services::library_service::sync_steam_library(state.inner())
}

#[tauri::command]
pub(crate) fn set_game_favorite(
    provider: String,
    external_id: String,
    favorite: bool,
    state: State<'_, AppState>,
) -> AppResult<()> {
    crate::application::services::library_service::set_game_favorite(
        state.inner(),
        provider,
        external_id,
        favorite,
    )
}

#[tauri::command]
pub(crate) fn list_steam_downloads(state: State<'_, AppState>) -> AppResult<Vec<SteamDownloadProgressResponse>> {
    crate::application::services::library_service::list_steam_downloads(state.inner())
}

#[tauri::command]
pub(crate) fn get_game_store_metadata(
    provider: String,
    external_id: String,
    state: State<'_, AppState>,
) -> AppResult<GameStoreMetadataResponse> {
    crate::application::services::library_service::get_game_store_metadata(
        state.inner(),
        provider,
        external_id,
    )
}

/// Run the blocking local Steam scan and call the provided emitter with the result.
/// This is separated out so tests can capture the result without requiring a real
/// `AppHandle` instance.
pub(crate) fn run_local_steam_scan_and_call<F>(emitter: F)
where
    F: FnOnce(Result<Vec<u64>, String>) + Send + 'static,
{
    run_local_steam_scan_and_call_with_override(None, emitter)
}

/// Variant that accepts a `steam_root_override` which is forwarded to
/// `detect_locally_installed_steam_app_ids`. Tests can pass an empty temp
/// directory as the override to avoid scanning the real filesystem.
pub(crate) fn run_local_steam_scan_and_call_with_override<F>(
    steam_root_override: Option<&str>,
    emitter: F,
) where
    F: FnOnce(Result<Vec<u64>, String>) + Send + 'static,
{
    match crate::detect_locally_installed_steam_app_ids(steam_root_override) {
        Ok(set) => {
            let ids: Vec<u64> = set.into_iter().collect();
            emitter(Ok(ids));
        }
        Err(err) => {
            emitter(Err(err));
        }
    }
}

#[tauri::command]
#[allow(dead_code)]
pub(crate) fn start_local_steam_scan(
    _state: State<'_, AppState>,
    app_handle: AppHandle,
) -> AppResult<()> {
    // Spawn a background thread to run the local Steam install detection
    let _ = std::thread::Builder::new()
        .name("local-steam-scan".into())
        .spawn(move || {
            crate::interface::tauri::commands::library::run_local_steam_scan_and_call(
                move |result| match result {
                    Ok(ids) => {
                        let _ = app_handle.emit("local-scan-complete", ids);
                    }
                    Err(err) => {
                        let _ = app_handle.emit("local-scan-error", err);
                    }
                },
            );
        });

    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;
    use std::time::Duration;

    #[test]
    fn run_local_scan_emits_to_closure() {
        let (tx, rx) = channel();

        // Run the blocking scan in a thread so the test remains responsive.
        // Create an empty temporary directory and use it as a steam root override so
        // the scan completes quickly without touching the user's actual Steam data.
        let temp_dir = std::env::temp_dir().join(format!("catalyst_test_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
        ));
        let _ = std::fs::create_dir_all(&temp_dir);

        std::thread::spawn(move || {
            run_local_steam_scan_and_call_with_override(Some(temp_dir.to_str().unwrap()), move |result| {
                let _ = tx.send(result);
            });
        });

        // Wait a reasonable time for the background scan to complete on CI/Dev machines.
        let received = rx.recv_timeout(Duration::from_secs(30)).expect("expected scan result");
        match received {
            Ok(ids) => {
                // We don't assert on a specific value; just confirm we received a Vec.
                let _ = ids.len();
            }
            Err(err) => {
                // Error string should be non-empty
                assert!(!err.is_empty());
            }
        }
    }
}
