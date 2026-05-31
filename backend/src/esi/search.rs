//! Authenticated ESI character-name search.
//!
//! This is the first authenticated outbound ESI call in the backend. It hits
//! `GET /characters/{character_id}/search/?categories=character&search=<q>&strict=false`
//! on behalf of the searching character, using that character's access token and
//! the required `esi-search.search_structures.v1` scope. ESI returns only an
//! array of character IDs; name resolution is a separate step (see
//! [`resolve_character_names`]).

use serde::Deserialize;

/// The ESI compatibility-date pinned for the search call. ESI requires the
/// `X-Compatibility-Date` header; CCP advances the supported date over time, so
/// this is a single constant to bump when the contract moves.
const ESI_COMPATIBILITY_DATE: &str = "2026-05-19";

/// ESI requires the search fragment to be at least this many characters.
pub const MIN_SEARCH_LEN: usize = 3;

/// Why an ESI character search could not be performed. Distinct from "the search
/// ran and matched nothing" — the caller maps this to a graceful "unavailable"
/// outcome, never a 5xx.
#[derive(Debug, PartialEq, Eq)]
pub enum EsiSearchError {
    /// ESI rejected the request — typically a 403 because the token lacks the
    /// `esi-search.search_structures.v1` scope, or any other non-success status.
    Rejected,
    /// ESI was unreachable or the response could not be parsed.
    Unavailable,
}

#[derive(Deserialize, Default)]
struct SearchResponse {
    #[serde(default)]
    character: Vec<i64>,
}

/// Searches characters by name fragment against ESI, authenticated as
/// `character_id` with `access_token`. Returns the matched character IDs (an
/// empty `Vec` when nothing matched), or an [`EsiSearchError`] when the search
/// could not be performed.
///
/// The caller MUST guarantee `q.len() >= MIN_SEARCH_LEN` (ESI rejects shorter
/// fragments); this function does not re-validate length.
pub async fn character_search(
    http: &reqwest_middleware::ClientWithMiddleware,
    base_url: &str,
    character_id: i64,
    access_token: &str,
    q: &str,
) -> Result<Vec<i64>, EsiSearchError> {
    let url = format!("{base_url}/characters/{character_id}/search/");

    let resp = http
        .get(&url)
        .bearer_auth(access_token)
        .header("X-Compatibility-Date", ESI_COMPATIBILITY_DATE)
        .query(&[
            ("categories", "character"),
            ("search", q),
            ("strict", "false"),
        ])
        .send()
        .await
        .map_err(|_| EsiSearchError::Unavailable)?;

    // A non-success status is "rejected" (e.g. 403 missing scope, 401 bad
    // token) — distinct from a transport failure, but both degrade gracefully.
    let resp = resp
        .error_for_status()
        .map_err(|_| EsiSearchError::Rejected)?;

    let body: SearchResponse = resp.json().await.map_err(|_| EsiSearchError::Unavailable)?;
    Ok(body.character)
}

/// Resolves a batch of character IDs to `(eve_character_id, name)` pairs via ESI
/// public-info, best-effort per id (an id that fails to resolve is dropped), and
/// capped at `limit`. Used to turn the ID-only search response into displayable
/// results.
pub async fn resolve_character_names(
    http: &reqwest_middleware::ClientWithMiddleware,
    base_url: &str,
    ids: &[i64],
    limit: usize,
) -> Vec<(i64, String)> {
    #[derive(Deserialize)]
    struct CharacterName {
        name: String,
    }

    let mut out = Vec::new();
    for &id in ids.iter().take(limit) {
        let url = format!("{base_url}/characters/{id}/");
        let name: Option<String> = async {
            http.get(&url)
                .send()
                .await
                .ok()?
                .error_for_status()
                .ok()?
                .json::<CharacterName>()
                .await
                .ok()
                .map(|c| c.name)
        }
        .await;
        if let Some(name) = name {
            out.push((id, name));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client() -> reqwest_middleware::ClientWithMiddleware {
        reqwest::Client::new().into()
    }

    #[tokio::test]
    async fn character_search_returns_matched_ids() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/characters/90000001/search/"))
            .and(query_param("categories", "character"))
            .and(query_param("search", "wasp"))
            .and(query_param("strict", "false"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "character": [95465499, 90000002]
            })))
            .mount(&server)
            .await;

        let ids = character_search(&client(), &server.uri(), 90000001, "token", "wasp")
            .await
            .unwrap();
        assert_eq!(ids, vec![95465499, 90000002]);
    }

    #[tokio::test]
    async fn character_search_empty_category_is_ok_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/characters/90000001/search/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .mount(&server)
            .await;

        let ids = character_search(&client(), &server.uri(), 90000001, "token", "zzz")
            .await
            .unwrap();
        assert!(ids.is_empty());
    }

    #[tokio::test]
    async fn character_search_403_is_rejected() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/characters/90000001/search/"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;

        let err = character_search(&client(), &server.uri(), 90000001, "token", "wasp")
            .await
            .unwrap_err();
        assert_eq!(err, EsiSearchError::Rejected);
    }

    #[tokio::test]
    async fn character_search_unreachable_is_unavailable() {
        // No server bound at this address → transport error → Unavailable.
        let err = character_search(&client(), "http://127.0.0.1:1", 90000001, "token", "wasp")
            .await
            .unwrap_err();
        assert_eq!(err, EsiSearchError::Unavailable);
    }

    #[tokio::test]
    async fn resolve_character_names_resolves_and_drops_failures_and_caps() {
        let server = MockServer::start().await;
        // id 1 resolves
        Mock::given(method("GET"))
            .and(path("/characters/1/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "name": "Wasp 223" })))
            .mount(&server)
            .await;
        // id 2 fails (404) → dropped
        Mock::given(method("GET"))
            .and(path("/characters/2/"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        // id 3 resolves but is beyond the cap of 2 ids requested below
        Mock::given(method("GET"))
            .and(path("/characters/3/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "name": "Three" })))
            .mount(&server)
            .await;

        // limit = 2 → only ids 1 and 2 are attempted; 2 fails and is dropped.
        let resolved = resolve_character_names(&client(), &server.uri(), &[1, 2, 3], 2).await;
        assert_eq!(resolved, vec![(1, "Wasp 223".to_string())]);
    }
}
