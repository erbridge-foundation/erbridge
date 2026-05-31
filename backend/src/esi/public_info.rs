// Uses the documented ESI base URL directly rather than the well-known discovery
// document — public-info endpoints are stable and not listed in the OIDC metadata.
const ESI_BASE: &str = "https://esi.evetech.net/latest";

pub async fn fetch_corporation_name(
    http: &reqwest_middleware::ClientWithMiddleware,
    corporation_id: i64,
) -> anyhow::Result<String> {
    #[derive(serde::Deserialize)]
    struct CorporationInfo {
        name: String,
    }

    let url = format!("{ESI_BASE}/corporations/{corporation_id}/");
    let info: CorporationInfo = http
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("ESI corporation request failed: {e}"))?
        .error_for_status()
        .map_err(|e| anyhow::anyhow!("ESI corporation returned non-2xx: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("failed to parse ESI corporation response: {e}"))?;

    Ok(info.name)
}

/// Best-effort fetch of a character's name and corporation name for the block
/// snapshot. Returns `(character_name, corporation_name)`, each `None` on any
/// failure (network, non-2xx, parse) — a block is a security action and SHALL
/// succeed even when ESI is unavailable, so this never errors; enforcement keys
/// on the immutable character id, not the snapshot. The two names are fetched as
/// two calls (character → its corp id → corp name); a failure of either leaves
/// the corresponding field `None`.
pub async fn fetch_character_block_snapshot(
    http: &reqwest_middleware::ClientWithMiddleware,
    eve_character_id: i64,
) -> (Option<String>, Option<String>) {
    #[derive(serde::Deserialize)]
    struct CharacterInfo {
        name: String,
        corporation_id: i64,
    }

    let url = format!("{ESI_BASE}/characters/{eve_character_id}/");
    let char_info: Option<CharacterInfo> = async {
        http.get(&url)
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json()
            .await
            .ok()
    }
    .await;

    match char_info {
        None => (None, None),
        Some(info) => {
            // Corp name is a second best-effort hop; tolerate its failure
            // independently so we still capture the character name.
            let corp_name = fetch_corporation_name(http, info.corporation_id).await.ok();
            (Some(info.name), corp_name)
        }
    }
}

pub async fn fetch_alliance_name(
    http: &reqwest_middleware::ClientWithMiddleware,
    alliance_id: i64,
) -> anyhow::Result<String> {
    #[derive(serde::Deserialize)]
    struct AllianceInfo {
        name: String,
    }

    let url = format!("{ESI_BASE}/alliances/{alliance_id}/");
    let info: AllianceInfo = http
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("ESI alliance request failed: {e}"))?
        .error_for_status()
        .map_err(|e| anyhow::anyhow!("ESI alliance returned non-2xx: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("failed to parse ESI alliance response: {e}"))?;

    Ok(info.name)
}
