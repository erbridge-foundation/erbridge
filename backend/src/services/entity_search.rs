//! Account-authenticated entity search over ESI.
//!
//! Resolves a name fragment to the identifiers an `acl_member` stores — a
//! character to its `eve_character.id` UUID **when a local row already exists**
//! (account-owned or orphan), and a corporation/alliance to its numeric
//! `eve_entity_id`. The search is write-free: it never mints rows. An unknown
//! character carries no UUID; minting happens at the ACL member add instead.
//!
//! The search runs on behalf of one of the requesting account's characters,
//! using that character's best-effort-refreshed access token; any reason a usable
//! token can't be obtained (or ESI rejects / is unreachable) resolves to the
//! graceful [`EntitySearchOutcome::Unavailable`], never a 5xx.
//!
//! This module owns the token-acquisition path shared with the admin
//! character search (`services::admin`), so both refresh tokens identically.

use chrono::Utc;
use reqwest_middleware::ClientWithMiddleware;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    crypto,
    db::characters,
    error::AppError,
    esi::{
        search::{self, SearchCategory},
        token,
    },
};

/// Per-category result cap. Mirrors the existing character-search cap; tune if
/// the blended picker feels unbalanced.
pub const RESULT_LIMIT: usize = 25;

/// Inputs the ESI search needs from config/state. Bundled so the service stays
/// free of HTTP framework types while still receiving the client + credentials
/// for the authenticated outbound call. Shared with the admin character search.
pub struct EsiSearchContext<'a> {
    pub http: &'a ClientWithMiddleware,
    /// Cached SSO JWKS for verifying the refreshed access-token JWT.
    pub jwks: &'a crate::esi::jwks::JwksCache,
    /// ESI base URL for the search + name-resolution calls. Prod passes the real
    /// ESI base; tests point it at a mock server.
    pub esi_base_url: &'a str,
    pub token_endpoint: &'a str,
    pub client_id: &'a str,
    pub client_secret: &'a str,
    pub encryption_key: &'a [u8],
}

/// A single character match. `id` carries the referenceable `eve_character.id`
/// UUID **only when a local row already exists** (account-owned or orphan); it is
/// `None` for an unknown character — the search mints nothing, so the UUID is
/// minted later at the ACL member add.
pub struct CharacterMatch {
    pub id: Option<Uuid>,
    pub eve_character_id: i64,
    pub name: String,
}

/// A single corporation or alliance match, carrying the numeric `eve_entity_id`
/// an `acl_member` stores for those member types.
pub struct EntityMatch {
    pub eve_entity_id: i64,
    pub name: String,
}

/// The matches of a completed search, grouped by category. Empty lists mean
/// "ran but matched nothing" (distinct from [`EntitySearchOutcome::Unavailable`]).
pub struct EntitySearchResults {
    pub characters: Vec<CharacterMatch>,
    pub corporations: Vec<EntityMatch>,
    pub alliances: Vec<EntityMatch>,
}

/// The outcome of an entity search. `Unavailable` is a graceful, non-error state
/// the handler maps to a `200` with empty groups and an `unavailable` indicator
/// — never a 5xx. Distinct from `Available` with empty groups ("ran, no match").
pub enum EntitySearchOutcome {
    Available(EntitySearchResults),
    Unavailable,
}

/// Searches EVE entities by name fragment across `categories` on behalf of
/// `account_id`, resolving each match to the identifier its member type needs.
/// Character matches carry the `eve_character.id` UUID only when a local row
/// already exists; unknown characters carry `None` and no row is minted — the
/// search is write-free.
///
/// Any failure to obtain a usable token, an ESI rejection (missing scope / bad
/// token), or ESI being unreachable resolves to [`EntitySearchOutcome::Unavailable`].
///
/// The caller MUST guarantee `q.len() >= search::MIN_SEARCH_LEN` and that
/// `categories` is non-empty.
pub async fn search_entities(
    pool: &PgPool,
    ctx: &EsiSearchContext<'_>,
    account_id: Uuid,
    q: &str,
    categories: &[SearchCategory],
) -> Result<EntitySearchOutcome, AppError> {
    // Resolve a usable access token for one of the account's characters. Any
    // reason we can't get one → graceful Unavailable.
    let token = match get_usable_main_access_token(pool, ctx, account_id).await? {
        Some(t) => t,
        None => return Ok(EntitySearchOutcome::Unavailable),
    };

    let matches = match search::entity_search(
        ctx.http,
        ctx.esi_base_url,
        token.eve_character_id,
        &token.access_token,
        q,
        categories,
    )
    .await
    {
        Ok(m) => m,
        // Both rejected (missing scope / bad token) and unavailable (network)
        // degrade to the same graceful outcome for the UI.
        Err(_) => return Ok(EntitySearchOutcome::Unavailable),
    };

    // Cap each category, then resolve every matched id to its name in a SINGLE
    // bulk call across all categories.
    let character_ids: Vec<i64> = matches.character.into_iter().take(RESULT_LIMIT).collect();
    let corporation_ids: Vec<i64> = matches.corporation.into_iter().take(RESULT_LIMIT).collect();
    let alliance_ids: Vec<i64> = matches.alliance.into_iter().take(RESULT_LIMIT).collect();

    let all_ids: Vec<i64> = character_ids
        .iter()
        .chain(corporation_ids.iter())
        .chain(alliance_ids.iter())
        .copied()
        .collect();
    let resolved = search::resolve_names_bulk(ctx.http, ctx.esi_base_url, &all_ids).await;

    // Characters: attach the existing-row UUID with one batched lookup; unknown
    // characters carry `None`. No write happens in the search path.
    let known_ids = characters::find_ids_by_eve_character_ids(
        pool,
        &resolved
            .characters
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>(),
    )
    .await?;
    let characters = resolved
        .characters
        .into_iter()
        .map(|(eve_character_id, name)| CharacterMatch {
            id: known_ids.get(&eve_character_id).copied(),
            eve_character_id,
            name,
        })
        .collect();

    // Corporations / alliances: the numeric id is the identifier the member row
    // stores; carry the resolved name.
    let corporations = resolved
        .corporations
        .into_iter()
        .map(|(eve_entity_id, name)| EntityMatch {
            eve_entity_id,
            name,
        })
        .collect();
    let alliances = resolved
        .alliances
        .into_iter()
        .map(|(eve_entity_id, name)| EntityMatch {
            eve_entity_id,
            name,
        })
        .collect();

    Ok(EntitySearchOutcome::Available(EntitySearchResults {
        characters,
        corporations,
        alliances,
    }))
}

/// A usable, decrypted access token for a character.
pub(crate) struct UsableToken {
    pub eve_character_id: i64,
    pub access_token: String,
}

/// Obtains a usable access token for `account_id`'s main character: decrypts the
/// stored access token, and if it is expired attempts a best-effort refresh
/// (persisting the rotated tokens). Returns `None` — never an error — when no
/// usable token can be obtained (no main, no stored tokens, refresh failed). The
/// decrypted token is held only transiently and never returned to a client.
///
/// Shared with the admin character search so both paths refresh identically.
pub(crate) async fn get_usable_main_access_token(
    pool: &PgPool,
    ctx: &EsiSearchContext<'_>,
    account_id: Uuid,
) -> Result<Option<UsableToken>, AppError> {
    let material = match characters::get_main_token_material(pool, account_id).await? {
        Some(m) => m,
        None => return Ok(None),
    };

    let expired = material
        .access_token_expires_at
        .map(|exp| exp <= Utc::now())
        .unwrap_or(true);

    if !expired {
        // Decrypt the stored access token for transient use. Falls through to
        // refresh if decrypt fails or no access token is stored.
        if let Some(enc) = &material.encrypted_access_token
            && let Ok(access_token) = crypto::decrypt_token(enc, ctx.encryption_key)
        {
            return Ok(Some(UsableToken {
                eve_character_id: material.eve_character_id,
                access_token,
            }));
        }
    }

    // Expired (or undecryptable): best-effort refresh using the stored refresh
    // token. No refresh token, or a rejected refresh → no usable token.
    let refresh_plaintext = match &material.encrypted_refresh_token {
        Some(enc) => match crypto::decrypt_token(enc, ctx.encryption_key) {
            Ok(t) => t,
            Err(_) => return Ok(None),
        },
        None => return Ok(None),
    };

    let refreshed = match token::refresh_access_token(
        ctx.http,
        ctx.jwks,
        ctx.token_endpoint,
        ctx.client_id,
        ctx.client_secret,
        &refresh_plaintext,
    )
    .await
    {
        Some(r) => r,
        None => return Ok(None),
    };

    // Persist the rotated tokens (best-effort; a failed write still lets the
    // current search proceed with the fresh access token).
    let _ = characters::update_tokens_by_eve_id(
        pool,
        material.eve_character_id,
        &refreshed.access_token,
        &refreshed.refresh_token,
        refreshed.access_token_expires_at,
        &refreshed.owner_hash,
        ctx.encryption_key,
    )
    .await;

    Ok(Some(UsableToken {
        eve_character_id: material.eve_character_id,
        access_token: refreshed.access_token,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{accounts, characters as char_db};
    use chrono::Utc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const KEY: &[u8] = &[0u8; 32];

    fn http() -> ClientWithMiddleware {
        reqwest::Client::new().into()
    }

    /// A throwaway JWKS cache for the search tests, which all use valid stored
    /// tokens or an unused token endpoint and so never reach JWT verification.
    fn test_jwks() -> crate::esi::jwks::JwksCache {
        use crate::esi::test_support::{jwks_json, test_keypair};
        crate::esi::jwks::JwksCache::from_keys_for_test(
            http(),
            "http://unused",
            crate::esi::jwks::decode_keys_for_test(jwks_json(&[&test_keypair("kid-1")]).as_bytes()),
        )
    }

    fn ctx<'a>(
        http: &'a ClientWithMiddleware,
        jwks: &'a crate::esi::jwks::JwksCache,
        esi_base: &'a str,
    ) -> EsiSearchContext<'a> {
        EsiSearchContext {
            http,
            jwks,
            esi_base_url: esi_base,
            token_endpoint: "http://unused",
            client_id: "client",
            client_secret: "secret",
            encryption_key: KEY,
        }
    }

    /// Seeds an account whose main character has a *valid* (non-expired) token,
    /// so the search path can decrypt and use it without a refresh.
    async fn account_with_main(pool: &PgPool, eve_id: i64) -> Uuid {
        let account_id = accounts::create_account(pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let char_id = char_db::upsert_tokens(
            &mut tx,
            account_id,
            eve_id,
            "Searcher",
            1_000_001,
            "Corp",
            None,
            None,
            "client",
            "access",
            "refresh",
            Utc::now() + chrono::Duration::hours(1),
            &[],
            "owner-hash",
            KEY,
        )
        .await
        .unwrap();
        char_db::promote_if_no_main(&mut tx, account_id, char_id, eve_id, "Searcher")
            .await
            .unwrap();
        tx.commit().await.unwrap();
        account_id
    }

    #[sqlx::test]
    async fn unavailable_when_account_has_no_main(pool: PgPool) {
        // No characters → no main token material → Unavailable.
        let account = accounts::create_account(&pool).await.unwrap();
        let client = http();
        let outcome = search_entities(
            &pool,
            &ctx(&client, &test_jwks(), "http://unused"),
            account,
            "wasp",
            &[SearchCategory::Character],
        )
        .await
        .unwrap();
        assert!(matches!(outcome, EntitySearchOutcome::Unavailable));
    }

    #[sqlx::test]
    async fn unavailable_when_esi_rejects(pool: PgPool) {
        let account = account_with_main(&pool, 1).await;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/characters/1/search/"))
            .respond_with(ResponseTemplate::new(403)) // missing scope
            .mount(&server)
            .await;

        let client = http();
        let outcome = search_entities(
            &pool,
            &ctx(&client, &test_jwks(), &server.uri()),
            account,
            "wasp",
            &[SearchCategory::Character],
        )
        .await
        .unwrap();
        assert!(matches!(outcome, EntitySearchOutcome::Unavailable));
    }

    #[sqlx::test]
    async fn empty_search_is_available_not_unavailable(pool: PgPool) {
        let account = account_with_main(&pool, 1).await;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/characters/1/search/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        let client = http();
        let outcome = search_entities(
            &pool,
            &ctx(&client, &test_jwks(), &server.uri()),
            account,
            "zzz",
            &[
                SearchCategory::Character,
                SearchCategory::Corporation,
                SearchCategory::Alliance,
            ],
        )
        .await
        .unwrap();
        match outcome {
            EntitySearchOutcome::Available(r) => {
                assert!(r.characters.is_empty());
                assert!(r.corporations.is_empty());
                assert!(r.alliances.is_empty());
            }
            EntitySearchOutcome::Unavailable => panic!("empty result must be Available"),
        }
    }

    #[sqlx::test]
    async fn existing_character_resolves_to_its_uuid_without_minting(pool: PgPool) {
        let account = account_with_main(&pool, 1).await;
        // Pre-existing orphan for the matched character.
        let existing_id = char_db::create_orphan(&pool, 555, "Wasp 223", 1, "Corp", None, None)
            .await
            .unwrap();

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/characters/1/search/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "character": [555] })),
            )
            .mount(&server)
            .await;
        // Bulk name resolution for all matched ids.
        Mock::given(method("POST"))
            .and(path("/universe/names/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": 555, "name": "Wasp 223", "category": "character" }
            ])))
            .mount(&server)
            .await;

        let client = http();
        let outcome = search_entities(
            &pool,
            &ctx(&client, &test_jwks(), &server.uri()),
            account,
            "wasp",
            &[SearchCategory::Character],
        )
        .await
        .unwrap();
        let results = match outcome {
            EntitySearchOutcome::Available(r) => r,
            EntitySearchOutcome::Unavailable => panic!("expected Available"),
        };
        assert_eq!(results.characters.len(), 1);
        assert_eq!(results.characters[0].id, Some(existing_id));
        assert_eq!(results.characters[0].eve_character_id, 555);

        // No second row was created for 555.
        let count = sqlx::query!(
            "SELECT COUNT(*) AS \"c!\" FROM eve_character WHERE eve_character_id = 555"
        )
        .fetch_one(&pool)
        .await
        .unwrap()
        .c;
        assert_eq!(count, 1);
    }

    #[sqlx::test]
    async fn unknown_character_carries_no_uuid_and_mints_nothing(pool: PgPool) {
        let account = account_with_main(&pool, 1).await;

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/characters/1/search/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "character": [999] })),
            )
            .mount(&server)
            .await;
        // Bulk name resolution; the character is unknown locally.
        Mock::given(method("POST"))
            .and(path("/universe/names/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": 999, "name": "New Pilot", "category": "character" }
            ])))
            .mount(&server)
            .await;

        let client = http();
        let outcome = search_entities(
            &pool,
            &ctx(&client, &test_jwks(), &server.uri()),
            account,
            "new",
            &[SearchCategory::Character],
        )
        .await
        .unwrap();
        let results = match outcome {
            EntitySearchOutcome::Available(r) => r,
            EntitySearchOutcome::Unavailable => panic!("expected Available"),
        };
        assert_eq!(results.characters.len(), 1);
        let matched = &results.characters[0];
        assert_eq!(matched.eve_character_id, 999);
        assert_eq!(matched.name, "New Pilot");
        // Unknown character → no UUID and no row minted.
        assert!(matched.id.is_none());

        let count = sqlx::query!(
            "SELECT COUNT(*) AS \"c!\" FROM eve_character WHERE eve_character_id = 999"
        )
        .fetch_one(&pool)
        .await
        .unwrap()
        .c;
        assert_eq!(count, 0, "search must not mint a row");
    }

    #[sqlx::test]
    async fn search_is_write_free_across_a_mixed_result(pool: PgPool) {
        // A search matching characters (known + unknown), a corp and an alliance
        // must not insert or update any row. The account's own main is the only
        // pre-existing row; the table count must be unchanged afterwards.
        let account = account_with_main(&pool, 1).await;
        char_db::create_orphan(&pool, 555, "Known", 1, "Corp", None, None)
            .await
            .unwrap();
        let before = sqlx::query!("SELECT COUNT(*) AS \"c!\" FROM eve_character")
            .fetch_one(&pool)
            .await
            .unwrap()
            .c;

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/characters/1/search/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "character": [555, 999],
                "corporation": [98000001],
                "alliance": [99000001]
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/universe/names/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": 555, "name": "Known", "category": "character" },
                { "id": 999, "name": "Unknown", "category": "character" },
                { "id": 98000001, "name": "Some Corp", "category": "corporation" },
                { "id": 99000001, "name": "Some Alliance", "category": "alliance" }
            ])))
            .mount(&server)
            .await;

        let client = http();
        search_entities(
            &pool,
            &ctx(&client, &test_jwks(), &server.uri()),
            account,
            "x",
            &[
                SearchCategory::Character,
                SearchCategory::Corporation,
                SearchCategory::Alliance,
            ],
        )
        .await
        .unwrap();

        let after = sqlx::query!("SELECT COUNT(*) AS \"c!\" FROM eve_character")
            .fetch_one(&pool)
            .await
            .unwrap()
            .c;
        assert_eq!(before, after, "search must not insert any row");
    }

    #[sqlx::test]
    async fn multi_category_groups_and_resolves(pool: PgPool) {
        let account = account_with_main(&pool, 1).await;
        // Existing character so no mint is needed for this assertion.
        char_db::create_orphan(&pool, 555, "Wasp 223", 1, "Corp", None, None)
            .await
            .unwrap();

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/characters/1/search/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "character": [555],
                "corporation": [98000001],
                "alliance": [99000001]
            })))
            .mount(&server)
            .await;
        // One bulk call resolves all three categories.
        Mock::given(method("POST"))
            .and(path("/universe/names/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": 555, "name": "Wasp 223", "category": "character" },
                { "id": 98000001, "name": "Wasp Corp", "category": "corporation" },
                { "id": 99000001, "name": "Wasp Alliance", "category": "alliance" }
            ])))
            .expect(1)
            .mount(&server)
            .await;

        let client = http();
        let outcome = search_entities(
            &pool,
            &ctx(&client, &test_jwks(), &server.uri()),
            account,
            "wasp",
            &[
                SearchCategory::Character,
                SearchCategory::Corporation,
                SearchCategory::Alliance,
            ],
        )
        .await
        .unwrap();
        let results = match outcome {
            EntitySearchOutcome::Available(r) => r,
            EntitySearchOutcome::Unavailable => panic!("expected Available"),
        };
        assert_eq!(results.characters.len(), 1);
        assert_eq!(results.corporations.len(), 1);
        assert_eq!(results.corporations[0].eve_entity_id, 98000001);
        assert_eq!(results.corporations[0].name, "Wasp Corp");
        assert_eq!(results.alliances.len(), 1);
        assert_eq!(results.alliances[0].eve_entity_id, 99000001);
        assert_eq!(results.alliances[0].name, "Wasp Alliance");
    }
}
