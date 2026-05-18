use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::db::sessions as db_sessions;

/// Established (post-callback) session: persisted in Postgres.
#[derive(Clone, Debug)]
pub struct Session {
    pub session_id: String,
    pub account_id: Uuid,
    pub csrf_state: Option<String>,
    pub add_character_mode: bool,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl From<db_sessions::SessionRow> for Session {
    fn from(r: db_sessions::SessionRow) -> Self {
        Self {
            session_id: r.session_id,
            account_id: r.account_id,
            csrf_state: r.csrf_state,
            add_character_mode: r.add_character_mode,
            created_at: r.created_at,
            last_seen_at: r.last_seen_at,
            expires_at: r.expires_at,
        }
    }
}

/// Postgres-backed store for established sessions.
#[derive(Clone)]
pub struct SessionStore {
    pool: PgPool,
}

impl SessionStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn add(
        &self,
        session_id: &str,
        account_id: Uuid,
        csrf_state: Option<&str>,
        add_character_mode: bool,
    ) -> Result<()> {
        db_sessions::insert(&self.pool, session_id, account_id, csrf_state, add_character_mode)
            .await
    }

    /// Reads the session for `session_id`, atomically advancing `last_seen_at`
    /// and `expires_at` if the row is non-expired. Returns `None` when the
    /// session is missing or already expired.
    pub async fn get(&self, session_id: &str) -> Result<Option<Session>> {
        Ok(db_sessions::refresh_and_get(&self.pool, session_id)
            .await?
            .map(Session::from))
    }

    pub async fn remove(&self, session_id: &str) -> Result<()> {
        db_sessions::delete(&self.pool, session_id).await
    }

    pub async fn remove_all_for_account(&self, account_id: Uuid) -> Result<u64> {
        db_sessions::delete_for_account(&self.pool, account_id).await
    }

    pub async fn list_session_ids_for_account(&self, account_id: Uuid) -> Result<Vec<String>> {
        db_sessions::list_ids_for_account(&self.pool, account_id).await
    }
}

/// In-flight OAuth record (between `/auth/login` and `/auth/callback`).
///
/// Kept in memory by design: these records have no `account_id` yet and carry
/// transient state (`csrf_state`, `return_to`). They are intentionally
/// restart-volatile — losing an in-flight login on a backend restart is fine;
/// the user retries login.
#[derive(Clone, Debug)]
pub struct InflightRecord {
    pub csrf_state: String,
    pub return_to: Option<String>,
    /// For add-character-mode requests, the existing account ID of the user
    /// initiating the add. `None` for a fresh login.
    pub account_id: Option<Uuid>,
}

#[derive(Clone, Default)]
pub struct InflightStore(Arc<RwLock<HashMap<String, InflightRecord>>>);

impl InflightStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn add(&self, record: InflightRecord) {
        self.0
            .write()
            .await
            .insert(record.csrf_state.clone(), record);
    }

    pub async fn take(&self, csrf_state: &str) -> Option<InflightRecord> {
        self.0.write().await.remove(csrf_state)
    }
}
