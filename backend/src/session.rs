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
        db_sessions::insert(
            &self.pool,
            session_id,
            account_id,
            csrf_state,
            add_character_mode,
        )
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

/// How long an in-flight OAuth record stays valid. Generous for an SSO
/// round-trip; matches the `auth_state` cookie `Max-Age`.
const INFLIGHT_TTL: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// Hard cap on concurrent in-flight records. Beyond this — once expired entries
/// are swept — new logins are refused rather than evicting live records, so an
/// attacker at the cap cannot displace a legitimate in-flight login.
const INFLIGHT_CAP: usize = 10_000;

/// Returned by [`InflightStore::add`] when the store is full of live records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InflightStoreFull;

struct StoredRecord {
    record: InflightRecord,
    created_at: std::time::Instant,
}

#[derive(Clone, Default)]
pub struct InflightStore(Arc<RwLock<HashMap<String, StoredRecord>>>);

impl InflightStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a new in-flight record, first sweeping expired entries. Refuses
    /// the insert with [`InflightStoreFull`] if the store is still at capacity
    /// after the sweep.
    pub async fn add(&self, record: InflightRecord) -> Result<(), InflightStoreFull> {
        let now = std::time::Instant::now();
        let mut map = self.0.write().await;
        map.retain(|_, stored| now.duration_since(stored.created_at) < INFLIGHT_TTL);
        if map.len() >= INFLIGHT_CAP {
            return Err(InflightStoreFull);
        }
        map.insert(
            record.csrf_state.clone(),
            StoredRecord {
                record,
                created_at: now,
            },
        );
        Ok(())
    }

    /// Removes and returns the record for `csrf_state`, treating an expired
    /// record as absent (and dropping it).
    pub async fn take(&self, csrf_state: &str) -> Option<InflightRecord> {
        let stored = self.0.write().await.remove(csrf_state)?;
        if std::time::Instant::now().duration_since(stored.created_at) >= INFLIGHT_TTL {
            return None;
        }
        Some(stored.record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(csrf_state: &str) -> InflightRecord {
        InflightRecord {
            csrf_state: csrf_state.to_string(),
            return_to: None,
            account_id: None,
        }
    }

    /// Inserts a record stamped `age` in the past, bypassing the public `add`
    /// so the test does not have to wait wall-clock time.
    async fn insert_aged(store: &InflightStore, csrf_state: &str, age: std::time::Duration) {
        let created_at = std::time::Instant::now()
            .checked_sub(age)
            .expect("age within Instant range");
        store.0.write().await.insert(
            csrf_state.to_string(),
            StoredRecord {
                record: record(csrf_state),
                created_at,
            },
        );
    }

    #[tokio::test]
    async fn take_returns_live_record() {
        let store = InflightStore::new();
        store.add(record("abc")).await.unwrap();
        let got = store.take("abc").await.expect("live record returned");
        assert_eq!(got.csrf_state, "abc");
    }

    #[tokio::test]
    async fn expired_record_is_not_returned() {
        let store = InflightStore::new();
        insert_aged(
            &store,
            "old",
            INFLIGHT_TTL + std::time::Duration::from_secs(1),
        )
        .await;
        assert!(store.take("old").await.is_none());
    }

    #[tokio::test]
    async fn add_sweeps_expired_records() {
        let store = InflightStore::new();
        insert_aged(
            &store,
            "old",
            INFLIGHT_TTL + std::time::Duration::from_secs(1),
        )
        .await;
        store.add(record("fresh")).await.unwrap();
        // The expired entry was swept on insert, leaving only the fresh one.
        let map = store.0.read().await;
        assert_eq!(map.len(), 1);
        assert!(map.contains_key("fresh"));
    }

    #[tokio::test]
    async fn add_refuses_when_full_of_live_records() {
        let store = InflightStore::new();
        {
            let mut map = store.0.write().await;
            let now = std::time::Instant::now();
            for i in 0..INFLIGHT_CAP {
                let key = format!("live-{i}");
                map.insert(
                    key.clone(),
                    StoredRecord {
                        record: record(&key),
                        created_at: now,
                    },
                );
            }
        }
        assert_eq!(store.add(record("overflow")).await, Err(InflightStoreFull));
    }

    #[tokio::test]
    async fn add_at_cap_evicts_expired_to_make_room() {
        let store = InflightStore::new();
        // Fill to the cap with expired records, then a new insert should sweep
        // them all and succeed.
        for i in 0..INFLIGHT_CAP {
            insert_aged(
                &store,
                &format!("old-{i}"),
                INFLIGHT_TTL + std::time::Duration::from_secs(1),
            )
            .await;
        }
        store.add(record("fresh")).await.unwrap();
        let map = store.0.read().await;
        assert_eq!(map.len(), 1);
        assert!(map.contains_key("fresh"));
    }
}
