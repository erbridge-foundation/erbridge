//! Typed fetches for the EVE system-catalog sources: eve-scout's `/systems` and
//! `/wormholetypes` feeds plus the anoikis `wh-statics.json`. All go through the
//! shared `ClientWithMiddleware` so they inherit the tracing/rate-limit chain.
//!
//! The anoikis fetch sends an explicit, identifying User-Agent header as a
//! good-citizen courtesy (the host does not currently reject the default UA, but
//! it future-proofs against UA gating).

use reqwest_middleware::ClientWithMiddleware;

use crate::db::eve_system::{StaticsMap, SystemRow, WormholeTypeRow};

/// Fetch the system spine from eve-scout `GET /v2/public/systems`.
pub async fn fetch_systems(
    http: &ClientWithMiddleware,
    url: &str,
) -> anyhow::Result<Vec<SystemRow>> {
    http.get(url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("eve-scout systems request failed: {e}"))?
        .error_for_status()
        .map_err(|e| anyhow::anyhow!("eve-scout systems returned non-2xx: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("failed to parse eve-scout systems response: {e}"))
}

/// Fetch the wormhole-type dictionary from eve-scout `GET /v2/public/wormholetypes`.
pub async fn fetch_wormhole_types(
    http: &ClientWithMiddleware,
    url: &str,
) -> anyhow::Result<Vec<WormholeTypeRow>> {
    http.get(url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("eve-scout wormholetypes request failed: {e}"))?
        .error_for_status()
        .map_err(|e| anyhow::anyhow!("eve-scout wormholetypes returned non-2xx: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("failed to parse eve-scout wormholetypes response: {e}"))
}

/// Fetch the J-code -> statics map from anoikis `GET /data/wh-statics.json`.
/// Sends `user_agent` as an identifying courtesy header (see module docs).
pub async fn fetch_statics(
    http: &ClientWithMiddleware,
    url: &str,
    user_agent: &str,
) -> anyhow::Result<StaticsMap> {
    http.get(url)
        .header(reqwest::header::USER_AGENT, user_agent)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("anoikis statics request failed: {e}"))?
        .error_for_status()
        .map_err(|e| anyhow::anyhow!("anoikis statics returned non-2xx: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("failed to parse anoikis statics response: {e}"))
}
