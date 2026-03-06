use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AppErrorKind {
    Validation,
    Unauthorized,
    NotFound,
    Conflict,
    External,
    Internal,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppError {
    pub(crate) kind: AppErrorKind,
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

pub(crate) type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub(crate) fn validation(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind: AppErrorKind::Validation,
            code,
            message: message.into(),
        }
    }

    pub(crate) fn unauthorized(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind: AppErrorKind::Unauthorized,
            code,
            message: message.into(),
        }
    }

    pub(crate) fn not_found(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind: AppErrorKind::NotFound,
            code,
            message: message.into(),
        }
    }

    pub(crate) fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind: AppErrorKind::Conflict,
            code,
            message: message.into(),
        }
    }

    pub(crate) fn external(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind: AppErrorKind::External,
            code,
            message: message.into(),
        }
    }

    pub(crate) fn internal(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind: AppErrorKind::Internal,
            code,
            message: message.into(),
        }
    }
}

pub(crate) fn map_app_error_message(message: impl Into<String>) -> AppError {
    let message = message.into();
    let normalized = message.trim().to_ascii_lowercase();

    if normalized.is_empty() {
        return AppError::internal("unknown_error", "Unknown backend error");
    }

    if normalized.contains("invalid email or password") || normalized.contains("authentication") {
        return AppError::unauthorized("auth_error", message);
    }

    if normalized.contains("already in use") || normalized.contains("already exists") {
        return AppError::conflict("conflict_error", message);
    }

    if normalized.contains("is required")
        || normalized.contains("must be")
        || normalized.contains("invalid")
        || normalized.contains("enter ")
    {
        return AppError::validation("validation_error", message);
    }

    if normalized.contains("not found") || normalized.contains("could not locate") {
        return AppError::not_found("not_found_error", message);
    }

    if normalized.contains("steam")
        || normalized.contains("request failed")
        || normalized.contains("timed out")
    {
        return AppError::external("external_service_error", message);
    }

    AppError::internal("internal_error", message)
}

impl From<String> for AppError {
    fn from(value: String) -> Self {
        map_app_error_message(value)
    }
}

impl From<&str> for AppError {
    fn from(value: &str) -> Self {
        map_app_error_message(value.to_owned())
    }
}
