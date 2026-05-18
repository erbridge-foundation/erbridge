use sqlx::PgPool;
use std::sync::Arc;

use crate::{
    config::Config,
    esi::EsiMetadata,
    session::{InflightStore, SessionStore},
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: PgPool,
    pub esi_metadata: Arc<EsiMetadata>,
    pub session_store: SessionStore,
    pub inflight_store: InflightStore,
    pub http_client: reqwest::Client,
}
