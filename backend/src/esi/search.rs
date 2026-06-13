//! Authenticated ESI entity-name search.
//!
//! This is the first authenticated outbound ESI call in the backend. It hits
//! `GET /characters/{character_id}/search/?categories=<list>&search=<q>&strict=false`
//! on behalf of the searching character, using that character's access token and
//! the required `esi-search.search_structures.v1` scope. ESI returns only arrays
//! of EVE ids grouped by category; name resolution is a separate step done with a
//! single bulk `POST /universe/names/` call (see [`resolve_names_bulk`]).

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

/// Names resolved by a single bulk [`resolve_names_bulk`] call, partitioned by
/// the response's `category` field. Each field holds the `(eve_id, name)` pairs
/// for that category; ids the endpoint could not resolve are simply absent.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ResolvedNames {
    pub characters: Vec<(i64, String)>,
    pub corporations: Vec<(i64, String)>,
    pub alliances: Vec<(i64, String)>,
}

/// One `{ id, name, category }` entry of the `POST /universe/names/` response.
#[derive(Deserialize)]
struct NameEntry {
    id: i64,
    name: String,
    category: String,
}

/// Resolves a mixed batch of EVE ids to names with a **single** bulk
/// `POST /universe/names/` call, partitioning the response by its `category`
/// field. `ids` may span characters, corporations, and alliances; ESI resolves
/// up to 1 000 mixed ids per call. Ids the endpoint omits are dropped (so results
/// are always displayable), and a 404 for a non-empty id set means "no ids
/// resolved" (all dropped) rather than a failure — both yield an empty/partial
/// [`ResolvedNames`] rather than an error.
///
/// Only the `character`, `corporation`, and `alliance` categories are retained;
/// the endpoint also resolves other id types, but the entity search never sends
/// them, and any unexpected category is ignored.
pub async fn resolve_names_bulk(
    http: &reqwest_middleware::ClientWithMiddleware,
    base_url: &str,
    ids: &[i64],
) -> ResolvedNames {
    if ids.is_empty() {
        return ResolvedNames::default();
    }

    let url = format!("{base_url}/universe/names/");
    // Serialize the id array as the JSON body. `reqwest-middleware`'s builder does
    // not expose `.json()`, so set the body + content-type explicitly. Infallible
    // (a slice of i64 always serialises); fall back to empty on the off chance.
    let body = match serde_json::to_vec(ids) {
        Ok(b) => b,
        Err(_) => return ResolvedNames::default(),
    };
    let resp = match http
        .post(&url)
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
    {
        Ok(r) => r,
        // Transport failure → drop everything (graceful empty).
        Err(_) => return ResolvedNames::default(),
    };

    // A 404 for a non-empty id set means ESI resolved none of them; treat it as
    // "all dropped", not a failure. Any other non-2xx likewise yields empties.
    let resp = match resp.error_for_status() {
        Ok(r) => r,
        Err(_) => return ResolvedNames::default(),
    };

    let entries: Vec<NameEntry> = match resp.json().await {
        Ok(e) => e,
        Err(_) => return ResolvedNames::default(),
    };

    let mut out = ResolvedNames::default();
    for entry in entries {
        match entry.category.as_str() {
            "character" => out.characters.push((entry.id, entry.name)),
            "corporation" => out.corporations.push((entry.id, entry.name)),
            "alliance" => out.alliances.push((entry.id, entry.name)),
            // An id type the search never requested — ignore it.
            _ => {}
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
    async fn resolve_names_bulk_partitions_by_category_in_one_call() {
        let server = MockServer::start().await;
        // A single POST resolves ids across all three categories at once.
        Mock::given(method("POST"))
            .and(path("/universe/names/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "id": 555, "name": "Wasp 223", "category": "character" },
                { "id": 98000001, "name": "Wasp Corp", "category": "corporation" },
                { "id": 99000001, "name": "Wasp Alliance", "category": "alliance" }
            ])))
            // `expect(1)` asserts exactly one request is issued for the batch.
            .expect(1)
            .mount(&server)
            .await;

        let resolved =
            resolve_names_bulk(&client(), &server.uri(), &[555, 98000001, 99000001]).await;
        assert_eq!(resolved.characters, vec![(555, "Wasp 223".to_string())]);
        assert_eq!(
            resolved.corporations,
            vec![(98000001, "Wasp Corp".to_string())]
        );
        assert_eq!(
            resolved.alliances,
            vec![(99000001, "Wasp Alliance".to_string())]
        );
    }

    #[tokio::test]
    async fn resolve_names_bulk_drops_omitted_ids() {
        let server = MockServer::start().await;
        // The response omits 222 → it is dropped rather than appearing nameless.
        Mock::given(method("POST"))
            .and(path("/universe/names/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "id": 111, "name": "Pilot One", "category": "character" }
            ])))
            .mount(&server)
            .await;

        let resolved = resolve_names_bulk(&client(), &server.uri(), &[111, 222]).await;
        assert_eq!(resolved.characters, vec![(111, "Pilot One".to_string())]);
    }

    #[tokio::test]
    async fn resolve_names_bulk_404_is_all_dropped_not_failure() {
        let server = MockServer::start().await;
        // ESI 404s the whole batch when none of the ids resolve.
        Mock::given(method("POST"))
            .and(path("/universe/names/"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let resolved = resolve_names_bulk(&client(), &server.uri(), &[1, 2, 3]).await;
        assert_eq!(resolved, ResolvedNames::default());
    }

    #[tokio::test]
    async fn resolve_names_bulk_empty_ids_makes_no_call() {
        // No ids → no request, empty result. (No mock mounted, so any request
        // would 404 from the mock server; an empty result proves none was sent.)
        let server = MockServer::start().await;
        let resolved = resolve_names_bulk(&client(), &server.uri(), &[]).await;
        assert_eq!(resolved, ResolvedNames::default());
    }
}
