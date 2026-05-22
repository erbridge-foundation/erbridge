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
