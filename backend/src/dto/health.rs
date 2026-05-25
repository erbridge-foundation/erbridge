use serde::Serialize;
use utoipa::ToSchema;

/// Overall or per-component health status.
///
/// Serialised as a snake_case string (`"ok"` / `"degraded"`) so orchestration
/// tooling can shallow-parse it.
#[derive(Serialize, ToSchema, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Ok,
    Degraded,
}

/// Status of a single named component (e.g. `db`).
#[derive(Serialize, ToSchema, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComponentStatus {
    Ok,
    Degraded,
}

#[derive(Serialize, ToSchema, Debug, PartialEq, Eq)]
pub struct ComponentHealth {
    pub name: String,
    pub status: ComponentStatus,
}

/// Flat health document returned by `GET /api/health`.
///
/// Intentionally NOT wrapped in the `ApiResponse<T>` envelope — see the
/// `api-contract` carve-out for `/api/health`.
#[derive(Serialize, ToSchema, Debug, PartialEq, Eq)]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub version: String,
    pub commit: String,
    pub components: Vec<ComponentHealth>,
}
