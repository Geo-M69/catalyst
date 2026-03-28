use crate::application::error::{AppError, AppResult};

pub(crate) async fn run_blocking<T, F>(task: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> AppResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|error| {
            AppError::internal(
                "blocking_task_failed",
                format!("Background task failed: {error}"),
            )
        })?
}
