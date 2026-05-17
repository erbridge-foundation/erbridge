use sqlx::PgPool;
use std::sync::Arc;

use crate::{config::Config, esi::EsiMetadata, session::SessionStore};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: PgPool,
    pub esi_metadata: Arc<EsiMetadata>,
    pub session_store: SessionStore,
    pub http_client: reqwest::Client,
}
