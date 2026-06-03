//! Authenticated ESI entity-name search.
//!
//! This is the first authenticated outbound ESI call in the backend. It hits
//! `GET /characters/{character_id}/search/?categories=<list>&search=<q>&strict=false`
//! on behalf of the searching character, using that character's access token and
//! the required `esi-search.search_structures.v1` scope. ESI returns only arrays
//! of EVE ids grouped by category; name resolution is a separate step (see
//! [`resolve_character_names`] and [`resolve_entity_names`]).

use serde::Deserialize;

/// The ESI compatibility-date pinned for the search call. ESI requires the
/// `X-Compatibility-Date` header; CCP advances the supported date over time, so
/// this is a single constant to bump when the contract moves.
const ESI_COMPATIBILITY_DATE: &str = "2026-05-19";

/// ESI requires the search fragment to be at least this many characters.
pub const MIN_SEARCH_LEN: usize = 3;

/// A searchable ESI entity category. The `categories=` query value and the
/// public-info path segment used for name resolution are both derived from this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchCategory {
    Character,
    Corporation,
    Alliance,
}

impl SearchCategory {
    /// The ESI `categories` query token for this category.
    pub fn as_query(self) -> &'static str {
        match self {
            SearchCategory::Character => "character",
            SearchCategory::Corporation => "corporation",
            SearchCategory::Alliance => "alliance",
        }
    }
}

/// Why an ESI entity search could not be performed. Distinct from "the search
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

/// The matched ids ESI returns for an entity search, grouped by category. Each
/// field is the raw EVE id list for that category (empty when not requested or
/// when nothing matched).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SearchMatches {
    pub character: Vec<i64>,
    pub corporation: Vec<i64>,
    pub alliance: Vec<i64>,
}

#[derive(Deserialize, Default)]
struct SearchResponse {
    #[serde(default)]
    character: Vec<i64>,
    #[serde(default)]
    corporation: Vec<i64>,
    #[serde(default)]
    alliance: Vec<i64>,
}

/// Searches EVE entities by name fragment against ESI across `categories`,
/// authenticated as `character_id` with `access_token`. Returns the matched ids
/// grouped by category (empty lists when nothing matched), or an
/// [`EsiSearchError`] when the search could not be performed.
///
/// The caller MUST guarantee `q.len() >= MIN_SEARCH_LEN` (ESI rejects shorter
/// fragments) and that `categories` is non-empty; this function does not
/// re-validate either.
pub async fn entity_search(
    http: &reqwest_middleware::ClientWithMiddleware,
    base_url: &str,
    character_id: i64,
    access_token: &str,
    q: &str,
    categories: &[SearchCategory],
) -> Result<SearchMatches, EsiSearchError> {
    let url = format!("{base_url}/characters/{character_id}/search/");

    // ESI takes a comma-separated `categories` list and returns all matches in
    // one round-trip.
    let categories_param = categories
        .iter()
        .map(|c| c.as_query())
        .collect::<Vec<_>>()
        .join(",");

    let resp = http
        .get(&url)
        .bearer_auth(access_token)
        .header("X-Compatibility-Date", ESI_COMPATIBILITY_DATE)
        .query(&[
            ("categories", categories_param.as_str()),
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
    Ok(SearchMatches {
        character: body.character,
        corporation: body.corporation,
        alliance: body.alliance,
    })
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
    resolve_named(http, base_url, "characters", ids, limit).await
}

/// Resolves a batch of corporation OR alliance ids to `(eve_entity_id, name)`
/// pairs via the corresponding ESI public-info path (`corporations` or
/// `alliances`), best-effort per id (an id that fails to resolve is dropped),
/// capped at `limit`. The path segment is selected from `category`; passing
/// [`SearchCategory::Character`] is a programmer error and yields no results.
pub async fn resolve_entity_names(
    http: &reqwest_middleware::ClientWithMiddleware,
    base_url: &str,
    category: SearchCategory,
    ids: &[i64],
    limit: usize,
) -> Vec<(i64, String)> {
    let path_segment = match category {
        SearchCategory::Corporation => "corporations",
        SearchCategory::Alliance => "alliances",
        // Characters resolve via `resolve_character_names`; nothing to do here.
        SearchCategory::Character => return Vec::new(),
    };
    resolve_named(http, base_url, path_segment, ids, limit).await
}

/// Shared best-effort name resolver: for each id (capped at `limit`) GETs
/// `{base_url}/{path_segment}/{id}/` and reads its `name`, dropping any id whose
/// lookup fails so results are always displayable.
async fn resolve_named(
    http: &reqwest_middleware::ClientWithMiddleware,
    base_url: &str,
    path_segment: &str,
    ids: &[i64],
    limit: usize,
) -> Vec<(i64, String)> {
    #[derive(Deserialize)]
    struct NamedInfo {
        name: String,
    }

    let mut out = Vec::new();
    for &id in ids.iter().take(limit) {
        let url = format!("{base_url}/{path_segment}/{id}/");
        let name: Option<String> = async {
            http.get(&url)
                .send()
                .await
                .ok()?
                .error_for_status()
                .ok()?
                .json::<NamedInfo>()
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
    async fn entity_search_returns_matched_character_ids() {
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

        let matches = entity_search(
            &client(),
            &server.uri(),
            90000001,
            "token",
            "wasp",
            &[SearchCategory::Character],
        )
        .await
        .unwrap();
        assert_eq!(matches.character, vec![95465499, 90000002]);
        assert!(matches.corporation.is_empty());
        assert!(matches.alliance.is_empty());
    }

    #[tokio::test]
    async fn entity_search_multi_category_groups_by_type() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/characters/90000001/search/"))
            // All three categories are sent in one comma-separated query.
            .and(query_param("categories", "character,corporation,alliance"))
            .and(query_param("search", "wasp"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "character": [555],
                "corporation": [98000001],
                "alliance": [99000001, 99000002]
            })))
            .mount(&server)
            .await;

        let matches = entity_search(
            &client(),
            &server.uri(),
            90000001,
            "token",
            "wasp",
            &[
                SearchCategory::Character,
                SearchCategory::Corporation,
                SearchCategory::Alliance,
            ],
        )
        .await
        .unwrap();
        assert_eq!(matches.character, vec![555]);
        assert_eq!(matches.corporation, vec![98000001]);
        assert_eq!(matches.alliance, vec![99000001, 99000002]);
    }

    #[tokio::test]
    async fn entity_search_empty_category_is_ok_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/characters/90000001/search/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .mount(&server)
            .await;

        let matches = entity_search(
            &client(),
            &server.uri(),
            90000001,
            "token",
            "zzz",
            &[SearchCategory::Character],
        )
        .await
        .unwrap();
        assert_eq!(matches, SearchMatches::default());
    }

    #[tokio::test]
    async fn entity_search_403_is_rejected() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/characters/90000001/search/"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;

        let err = entity_search(
            &client(),
            &server.uri(),
            90000001,
            "token",
            "wasp",
            &[SearchCategory::Character],
        )
        .await
        .unwrap_err();
        assert_eq!(err, EsiSearchError::Rejected);
    }

    #[tokio::test]
    async fn entity_search_unreachable_is_unavailable() {
        // No server bound at this address → transport error → Unavailable.
        let err = entity_search(
            &client(),
            "http://127.0.0.1:1",
            90000001,
            "token",
            "wasp",
            &[SearchCategory::Character],
        )
        .await
        .unwrap_err();
        assert_eq!(err, EsiSearchError::Unavailable);
    }

    #[tokio::test]
    async fn resolve_entity_names_resolves_corporations() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/corporations/98000001/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "name": "Wasp Corp" })))
            .mount(&server)
            .await;

        let resolved = resolve_entity_names(
            &client(),
            &server.uri(),
            SearchCategory::Corporation,
            &[98000001],
            10,
        )
        .await;
        assert_eq!(resolved, vec![(98000001, "Wasp Corp".to_string())]);
    }

    #[tokio::test]
    async fn resolve_entity_names_resolves_alliances_and_drops_unresolvable() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/alliances/99000001/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({ "name": "Wasp Alliance" })),
            )
            .mount(&server)
            .await;
        // 99000002 fails → dropped from the results.
        Mock::given(method("GET"))
            .and(path("/alliances/99000002/"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let resolved = resolve_entity_names(
            &client(),
            &server.uri(),
            SearchCategory::Alliance,
            &[99000001, 99000002],
            10,
        )
        .await;
        assert_eq!(resolved, vec![(99000001, "Wasp Alliance".to_string())]);
    }

    #[tokio::test]
    async fn resolve_entity_names_character_category_is_empty() {
        // Characters resolve via resolve_character_names; the entity path is a
        // no-op for the character category.
        let resolved = resolve_entity_names(
            &client(),
            "http://unused",
            SearchCategory::Character,
            &[1],
            10,
        )
        .await;
        assert!(resolved.is_empty());
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
