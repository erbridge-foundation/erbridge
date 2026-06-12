use reqwest_middleware::ClientWithMiddleware;
use sqlx::PgPool;
use std::sync::Arc;

use crate::{
    config::Config,
    esi::{EsiMetadata, jwks::JwksCache},
    session::{InflightStore, SessionStore},
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: PgPool,
    pub esi_metadata: Arc<EsiMetadata>,
    /// Cached SSO JWKS used to verify ESI access-token JWT signatures, shared
    /// with the token-refresh sweep. Refetches itself on key rotation.
    pub jwks: Arc<JwksCache>,
    pub session_store: SessionStore,
    pub inflight_store: InflightStore,
    pub http_client: ClientWithMiddleware,
}
