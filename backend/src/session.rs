use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Session {
    pub session_id: String,
    pub account_id: Uuid,
    pub csrf_state: Option<String>,
    pub return_to: Option<String>,
    pub add_character_mode: bool,
}

#[derive(Clone, Default)]
pub struct SessionStore(Arc<RwLock<HashMap<String, Session>>>);

impl SessionStore {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }

    pub async fn add(&self, session: Session) {
        self.0
            .write()
            .await
            .insert(session.session_id.clone(), session);
    }

    pub async fn get(&self, session_id: &str) -> Option<Session> {
        self.0.read().await.get(session_id).cloned()
    }

    pub async fn remove(&self, session_id: &str) {
        self.0.write().await.remove(session_id);
    }

    pub async fn remove_all_for_account(&self, account_id: Uuid) {
        let mut store = self.0.write().await;
        store.retain(|_, s| s.account_id != account_id);
    }

    pub async fn list_session_ids_for_account(&self, account_id: Uuid) -> Vec<String> {
        self.0
            .read()
            .await
            .values()
            .filter(|s| s.account_id == account_id)
            .map(|s| s.session_id.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session(session_id: &str, account_id: Uuid) -> Session {
        Session {
            session_id: session_id.to_string(),
            account_id,
            csrf_state: None,
            return_to: None,
            add_character_mode: false,
        }
    }

    #[tokio::test]
    async fn add_and_get_session() {
        let store = SessionStore::new();
        let account_id = Uuid::new_v4();
        store.add(make_session("sess1", account_id)).await;
        let session = store.get("sess1").await.unwrap();
        assert_eq!(session.account_id, account_id);
    }

    #[tokio::test]
    async fn get_missing_session_returns_none() {
        let store = SessionStore::new();
        assert!(store.get("missing").await.is_none());
    }

    #[tokio::test]
    async fn remove_session() {
        let store = SessionStore::new();
        let account_id = Uuid::new_v4();
        store.add(make_session("sess1", account_id)).await;
        store.remove("sess1").await;
        assert!(store.get("sess1").await.is_none());
    }

    #[tokio::test]
    async fn remove_all_for_account() {
        let store = SessionStore::new();
        let account_id = Uuid::new_v4();
        store.add(make_session("sess1", account_id)).await;
        store.add(make_session("sess2", account_id)).await;
        let other_account = Uuid::new_v4();
        store.add(make_session("sess3", other_account)).await;

        store.remove_all_for_account(account_id).await;

        assert!(store.get("sess1").await.is_none());
        assert!(store.get("sess2").await.is_none());
        assert!(store.get("sess3").await.is_some());
    }
}
