use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;

use crate::{
    app_state::AppState,
    dto::entity::EntitySearchPageDto,
    error::{AppError, ErrorEnvelope},
    esi::{
        public_info,
        search::{self, SearchCategory},
    },
    handlers::middleware::AuthenticatedAccount,
    response::{ApiResponse, EntitySearchResponse},
    services::entity_search::{self, EntitySearchOutcome, EsiSearchContext},
};

#[derive(Deserialize)]
pub struct EntitySearchQuery {
    /// Name fragment (min 3 characters).
    pub q: String,
    /// Optional comma-separated subset of `character,corporation,alliance`.
    /// Omitted or empty → all three.
    pub categories: Option<String>,
}

/// Parses the optional `categories` query value into the set of categories to
/// search. Unknown tokens are ignored; an omitted, empty, or all-unknown value
/// defaults to all three categories so the picker gets a blended result.
fn parse_categories(raw: Option<&str>) -> Vec<SearchCategory> {
    let all = [
        SearchCategory::Character,
        SearchCategory::Corporation,
        SearchCategory::Alliance,
    ];
    let Some(raw) = raw else {
        return all.to_vec();
    };

    let selected: Vec<SearchCategory> = raw
        .split(',')
        .filter_map(|token| match token.trim() {
            "character" => Some(SearchCategory::Character),
            "corporation" => Some(SearchCategory::Corporation),
            "alliance" => Some(SearchCategory::Alliance),
            _ => None,
        })
        .collect();

    if selected.is_empty() {
        all.to_vec()
    } else {
        selected
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/entities/search",
    params(
        ("q" = String, Query, description = "Case-insensitive name fragment (min 3 chars)"),
        ("categories" = Option<String>, Query, description = "Comma-separated subset of character,corporation,alliance (default: all)"),
    ),
    responses(
        (status = 200, description = "Matched entities grouped by category, or an unavailable indicator", body = EntitySearchResponse),
        (status = 400, description = "Fragment shorter than 3 characters", body = ErrorEnvelope),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "entities",
)]
pub async fn search_entities(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
    Query(query): Query<EntitySearchQuery>,
) -> Result<Json<ApiResponse<EntitySearchPageDto>>, AppError> {
    // Enforce ESI's minimum fragment length before any token/ESI work.
    if query.q.chars().count() < search::MIN_SEARCH_LEN {
        return Err(AppError::BadRequest(format!(
            "search term must be at least {} characters",
            search::MIN_SEARCH_LEN
        )));
    }

    let categories = parse_categories(query.categories.as_deref());

    let encryption_key = crate::crypto::token_encryption_key(&state.config.encryption_secret)?;

    let ctx = EsiSearchContext {
        http: &state.http_client,
        jwks: &state.jwks,
        esi_base_url: public_info::ESI_BASE,
        token_endpoint: &state.esi_metadata.token_endpoint,
        client_id: &state.config.esi_client_id,
        client_secret: &state.config.esi_client_secret,
        encryption_key: &encryption_key,
    };

    let page =
        match entity_search::search_entities(&state.db, &ctx, account_id, &query.q, &categories)
            .await?
        {
            EntitySearchOutcome::Available(results) => EntitySearchPageDto::from(results),
            EntitySearchOutcome::Unavailable => EntitySearchPageDto::unavailable(),
        };

    Ok(Json(ApiResponse::data(page)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_categories_defaults_to_all_when_absent() {
        assert_eq!(
            parse_categories(None),
            vec![
                SearchCategory::Character,
                SearchCategory::Corporation,
                SearchCategory::Alliance
            ]
        );
    }

    #[test]
    fn parse_categories_defaults_to_all_when_empty_or_unknown() {
        assert_eq!(parse_categories(Some("")).len(), 3);
        assert_eq!(parse_categories(Some("bogus")).len(), 3);
    }

    #[test]
    fn parse_categories_filters_to_requested_subset() {
        assert_eq!(
            parse_categories(Some("corporation")),
            vec![SearchCategory::Corporation]
        );
        assert_eq!(
            parse_categories(Some("character, alliance")),
            vec![SearchCategory::Character, SearchCategory::Alliance]
        );
    }

    #[test]
    fn parse_categories_ignores_unknown_tokens_mixed_with_known() {
        assert_eq!(
            parse_categories(Some("character,bogus")),
            vec![SearchCategory::Character]
        );
    }
}
