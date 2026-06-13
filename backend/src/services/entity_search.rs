//! Account-authenticated entity search over ESI.
//!
//! Resolves a name fragment to the identifiers an `acl_member` stores — a
//! character to its `eve_character.id` UUID (minting an orphan row when the
//! character has no row yet), and a corporation/alliance to its numeric
//! `eve_entity_id`. The search runs on behalf of one of the requesting
//! account's characters, using that character's best-effort-refreshed access
//! token; any reason a usable token can't be obtained (or ESI rejects / is
//! unreachable) resolves to the graceful [`EntitySearchOutcome::Unavailable`],
//! never a 5xx.
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

/// A single character match, resolved to the referenceable `eve_character.id`
/// UUID (minted as an orphan if the character had no row).
pub struct CharacterMatch {
    pub id: Uuid,
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
/// Character matches resolve to the `eve_character.id` UUID, minting an orphan
/// (public-info populated, no tokens) when the character has no row yet.
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

    // Characters: resolve names, then find-or-mint each to its UUID.
    let resolved_chars = search::resolve_character_names(
        ctx.http,
        ctx.esi_base_url,
        &matches.character,
        RESULT_LIMIT,
    )
    .await;
    let mut characters = Vec::with_capacity(resolved_chars.len());
    for (eve_character_id, name) in resolved_chars {
        let id = find_or_mint_character(pool, ctx, eve_character_id, &name).await?;
        characters.push(CharacterMatch {
            id,
            eve_character_id,
            name,
        });
    }

    // Corporations / alliances: name-resolve only; the numeric id is the
    // identifier the member row stores.
    let corporations =
        resolve_entities(ctx, SearchCategory::Corporation, &matches.corporation).await;
    let alliances = resolve_entities(ctx, SearchCategory::Alliance, &matches.alliance).await;

    Ok(EntitySearchOutcome::Available(EntitySearchResults {
        characters,
        corporations,
        alliances,
    }))
}

/// Resolves a corp/alliance id batch to `EntityMatch`es, dropping unresolvable
/// ids and capping at [`RESULT_LIMIT`].
async fn resolve_entities(
    ctx: &EsiSearchContext<'_>,
    category: SearchCategory,
    ids: &[i64],
) -> Vec<EntityMatch> {
    search::resolve_entity_names(ctx.http, ctx.esi_base_url, category, ids, RESULT_LIMIT)
        .await
        .into_iter()
        .map(|(eve_entity_id, name)| EntityMatch {
            eve_entity_id,
            name,
        })
        .collect()
}

/// Resolves a matched character's `eve_character_id` to its `eve_character.id`
/// UUID, minting an orphan row (public-info populated, no tokens) when none
/// exists. The corp/alliance public-info for the orphan is fetched best-effort;
/// a failed corp lookup leaves a placeholder so the row's NOT NULL columns hold.
async fn find_or_mint_character(
    pool: &PgPool,
    ctx: &EsiSearchContext<'_>,
    eve_character_id: i64,
    name: &str,
) -> Result<Uuid, AppError> {
    if let Some(id) = characters::find_id_by_eve_character_id(pool, eve_character_id).await? {
        return Ok(id);
    }

    // No row — mint an orphan. Fetch the character's corp (and alliance) for the
    // public-info snapshot. corporation_id/corporation_name are NOT NULL, so a
    // failed public-info fetch falls back to a 0/"" placeholder rather than
    // failing the whole search.
    let (corporation_id, corporation_name, alliance_id, alliance_name) =
        fetch_affiliations(ctx, eve_character_id).await;

    let id = characters::create_orphan(
        pool,
        eve_character_id,
        name,
        corporation_id,
        &corporation_name,
        alliance_id,
        alliance_name.as_deref(),
    )
    .await?;
    Ok(id)
}

/// Best-effort fetch of a character's corporation (and alliance) affiliation for
/// the orphan snapshot. Returns `(corporation_id, corporation_name, alliance_id,
/// alliance_name)`; on any failure the corp falls back to `(0, "")` and the
/// alliance to `None`, so the orphan's NOT NULL columns are always satisfiable.
async fn fetch_affiliations(
    ctx: &EsiSearchContext<'_>,
    eve_character_id: i64,
) -> (i64, String, Option<i64>, Option<String>) {
    #[derive(serde::Deserialize)]
    struct CharacterAffiliation {
        corporation_id: i64,
        #[serde(default)]
        alliance_id: Option<i64>,
    }

    let url = format!("{}/characters/{eve_character_id}/", ctx.esi_base_url);
    let affil: Option<CharacterAffiliation> = async {
        ctx.http
            .get(&url)
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

    let Some(affil) = affil else {
        return (0, String::new(), None, None);
    };

    let corporation_name = search::resolve_entity_names(
        ctx.http,
        ctx.esi_base_url,
        SearchCategory::Corporation,
        &[affil.corporation_id],
        1,
    )
    .await
    .into_iter()
    .next()
    .map(|(_, name)| name)
    .unwrap_or_default();

    let (alliance_id, alliance_name) = match affil.alliance_id {
        Some(aid) => {
            let name = search::resolve_entity_names(
                ctx.http,
                ctx.esi_base_url,
                SearchCategory::Alliance,
                &[aid],
                1,
            )
            .await
            .into_iter()
            .next()
            .map(|(_, name)| name);
            (Some(aid), name)
        }
        None => (None, None),
    };

    (
        affil.corporation_id,
        corporation_name,
        alliance_id,
        alliance_name,
    )
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
        char_db::promote_if_no_main(&mut tx, account_id, char_id)
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
        Mock::given(method("GET"))
            .and(path("/characters/555/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "name": "Wasp 223" })),
            )
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
        assert_eq!(results.characters[0].id, existing_id);
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
    async fn unknown_character_is_minted_as_orphan(pool: PgPool) {
        let account = account_with_main(&pool, 1).await;

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/characters/1/search/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "character": [999] })),
            )
            .mount(&server)
            .await;
        // Name resolution for the matched character.
        Mock::given(method("GET"))
            .and(path("/characters/999/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "name": "New Pilot",
                "corporation_id": 2000001
            })))
            .mount(&server)
            .await;
        // Corp name for the orphan snapshot.
        Mock::given(method("GET"))
            .and(path("/corporations/2000001/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "name": "New Corp" })),
            )
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
        let minted = &results.characters[0];
        assert_eq!(minted.eve_character_id, 999);

        // An orphan row was minted with the public-info snapshot and no tokens.
        let row = sqlx::query!(
            r#"
            SELECT id, account_id, name, corporation_id, corporation_name,
                   encrypted_refresh_token, is_main
            FROM eve_character WHERE eve_character_id = 999
            "#
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.id, minted.id);
        assert!(row.account_id.is_none());
        assert_eq!(row.name, "New Pilot");
        assert_eq!(row.corporation_id, 2000001);
        assert_eq!(row.corporation_name, "New Corp");
        assert!(row.encrypted_refresh_token.is_none());
        assert!(!row.is_main);
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
        Mock::given(method("GET"))
            .and(path("/characters/555/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "name": "Wasp 223" })),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/corporations/98000001/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "name": "Wasp Corp" })),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/alliances/99000001/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "name": "Wasp Alliance" })),
            )
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
