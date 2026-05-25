use sqlx::PgPool;

use crate::dto::health::{ComponentStatus, HealthStatus};

/// A snapshot of each component's live status. The handler maps this into the
/// wire `components` array and derives the overall status from it.
pub struct HealthSnapshot {
    pub db: ComponentStatus,
}

/// Probe every component and return its status. Never propagates an error —
/// "degraded" is the success signal, so `GET /api/health` returns 200 whether
/// or not the DB is reachable.
pub async fn check(pool: &PgPool) -> HealthSnapshot {
    let db = match sqlx::query!("SELECT 1 AS one").fetch_one(pool).await {
        Ok(_) => ComponentStatus::Ok,
        Err(_) => ComponentStatus::Degraded,
    };
    HealthSnapshot { db }
}

/// Derive the overall status from the component statuses: `Ok` iff every
/// component is `Ok`, otherwise `Degraded`. Pure function — the single place
/// the aggregation rule is defined, so callers and tests cannot disagree.
pub fn overall_status(components: &[ComponentStatus]) -> HealthStatus {
    if components.iter().all(|s| *s == ComponentStatus::Ok) {
        HealthStatus::Ok
    } else {
        HealthStatus::Degraded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overall_is_ok_when_all_components_ok() {
        let components = [ComponentStatus::Ok, ComponentStatus::Ok];
        assert_eq!(overall_status(&components), HealthStatus::Ok);
    }

    #[test]
    fn overall_is_ok_for_empty_components() {
        // Vacuously: no component is degraded.
        assert_eq!(overall_status(&[]), HealthStatus::Ok);
    }

    #[test]
    fn overall_is_degraded_when_any_component_degraded() {
        let components = [ComponentStatus::Ok, ComponentStatus::Degraded];
        assert_eq!(overall_status(&components), HealthStatus::Degraded);
    }

    #[test]
    fn overall_is_degraded_when_all_components_degraded() {
        let components = [ComponentStatus::Degraded];
        assert_eq!(overall_status(&components), HealthStatus::Degraded);
    }
}
