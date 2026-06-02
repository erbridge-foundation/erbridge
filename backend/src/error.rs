use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

#[derive(Serialize, ToSchema)]
pub struct ErrorEnvelope {
    pub error: ErrorDetail,
}

impl ErrorEnvelope {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: ErrorDetail {
                code: code.into(),
                message: message.into(),
                details: None,
            },
        }
    }
}

#[derive(Debug)]
pub enum ConflictKind {
    CannotRemoveLastCharacter,
    CannotRemoveMain,
    CannotRemoveLastServerAdmin,
    CannotBlockSelf,
    ApiKeyNameAlreadyExists,
    MapSlugAlreadyExists,
}

impl ConflictKind {
    pub fn code(&self) -> &'static str {
        match self {
            ConflictKind::CannotRemoveLastCharacter => "cannot_remove_last_character",
            ConflictKind::CannotRemoveMain => "cannot_remove_main",
            ConflictKind::CannotRemoveLastServerAdmin => "cannot_remove_last_server_admin",
            ConflictKind::CannotBlockSelf => "cannot_block_self",
            ConflictKind::ApiKeyNameAlreadyExists => "api_key_name_already_exists",
            ConflictKind::MapSlugAlreadyExists => "map_slug_already_exists",
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            ConflictKind::CannotRemoveLastCharacter => {
                "Cannot remove the last character on this account"
            }
            ConflictKind::CannotRemoveMain => "Cannot remove the main character",
            ConflictKind::CannotRemoveLastServerAdmin => {
                "Cannot remove the last server administrator; promote another admin first"
            }
            ConflictKind::CannotBlockSelf => "Cannot block a character on your own account",
            ConflictKind::ApiKeyNameAlreadyExists => "A key with this name already exists",
            ConflictKind::MapSlugAlreadyExists => "A map with this slug already exists",
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("unauthorized")]
    Unauthorized,

    #[error("account soft deleted")]
    AccountSoftDeleted,

    #[error("account blocked")]
    AccountBlocked,

    #[error("forbidden")]
    Forbidden,

    #[error("forbidden: server admin required")]
    ForbiddenAdminRequired,

    #[error("not found")]
    NotFound,

    #[error("conflict: {0:?}")]
    Conflict(ConflictKind),

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("bad gateway: {0}")]
    BadGateway(String),

    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                "Authentication required".to_string(),
            ),
            AppError::AccountSoftDeleted => (
                StatusCode::UNAUTHORIZED,
                "account_soft_deleted",
                "This account has been soft-deleted".to_string(),
            ),
            AppError::AccountBlocked => (
                StatusCode::UNAUTHORIZED,
                "account_blocked",
                "This account is blocked".to_string(),
            ),
            AppError::Forbidden => (
                StatusCode::FORBIDDEN,
                "forbidden",
                "Access denied".to_string(),
            ),
            AppError::ForbiddenAdminRequired => (
                StatusCode::FORBIDDEN,
                "forbidden_admin_required",
                "Server administrator access is required".to_string(),
            ),
            AppError::NotFound => (
                StatusCode::NOT_FOUND,
                "not_found",
                "Resource not found".to_string(),
            ),
            AppError::Conflict(kind) => (
                StatusCode::CONFLICT,
                kind.code(),
                kind.message().to_string(),
            ),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg.clone()),
            AppError::BadGateway(msg) => (StatusCode::BAD_GATEWAY, "bad_gateway", msg.clone()),
            AppError::Internal(e) => {
                tracing::error!("internal error: {e:#}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "An internal error occurred".to_string(),
                )
            }
        };

        (status, Json(ErrorEnvelope::new(code, message))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    fn status(err: AppError) -> StatusCode {
        err.into_response().status()
    }

    async fn body_json(err: AppError) -> serde_json::Value {
        use http_body_util::BodyExt;
        let bytes = err
            .into_response()
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[test]
    fn unauthorized_maps_to_401() {
        assert_eq!(status(AppError::Unauthorized), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn account_soft_deleted_maps_to_401() {
        assert_eq!(
            status(AppError::AccountSoftDeleted),
            StatusCode::UNAUTHORIZED
        );
    }

    #[test]
    fn account_blocked_maps_to_401() {
        assert_eq!(status(AppError::AccountBlocked), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn forbidden_maps_to_403() {
        assert_eq!(status(AppError::Forbidden), StatusCode::FORBIDDEN);
    }

    #[test]
    fn forbidden_admin_required_maps_to_403() {
        assert_eq!(
            status(AppError::ForbiddenAdminRequired),
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn account_blocked_body_has_expected_code() {
        let body = body_json(AppError::AccountBlocked).await;
        assert_eq!(body["error"]["code"], "account_blocked");
    }

    #[tokio::test]
    async fn forbidden_admin_required_body_has_expected_code() {
        let body = body_json(AppError::ForbiddenAdminRequired).await;
        assert_eq!(body["error"]["code"], "forbidden_admin_required");
    }

    #[test]
    fn not_found_maps_to_404() {
        assert_eq!(status(AppError::NotFound), StatusCode::NOT_FOUND);
    }

    #[test]
    fn conflict_maps_to_409() {
        assert_eq!(
            status(AppError::Conflict(ConflictKind::CannotRemoveMain)),
            StatusCode::CONFLICT
        );
    }

    #[test]
    fn map_slug_conflict_maps_to_409_with_code() {
        let kind = ConflictKind::MapSlugAlreadyExists;
        assert_eq!(kind.code(), "map_slug_already_exists");
        assert_eq!(
            status(AppError::Conflict(ConflictKind::MapSlugAlreadyExists)),
            StatusCode::CONFLICT
        );
    }

    #[test]
    fn bad_request_maps_to_400() {
        assert_eq!(
            status(AppError::BadRequest("test".to_string())),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn bad_gateway_maps_to_502() {
        assert_eq!(
            status(AppError::BadGateway("test".to_string())),
            StatusCode::BAD_GATEWAY
        );
    }

    #[test]
    fn internal_maps_to_500() {
        assert_eq!(
            status(AppError::Internal(anyhow::anyhow!("boom"))),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
