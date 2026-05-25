// This is the generic account-preferences substrate. Accessibility keys live in
// PreferencesDto/PreferencesPatch (dto/preferences.rs). Other features add their
// own keys here — e.g. add-internationalisation-support adds `locale` as a
// validated key on this same JSONB bag + endpoint, with NO new column or route.
// See openspec/changes/add-internationalisation-support/.

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    db::preferences as db,
    dto::preferences::{PreferencesDto, PreferencesPatch, TextSize, Toggle, TriState},
    error::AppError,
};

/// Read an account's accessibility preferences as a complete set, filling any
/// absent key with its default. Tolerates unknown keys stored in the bag (e.g.
/// preferences owned by other features such as locale) — they are ignored here.
pub async fn get_preferences(pool: &PgPool, account_id: Uuid) -> Result<PreferencesDto, AppError> {
    let bag = db::get_preferences(pool, account_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    Ok(dto_from_bag(&bag))
}

/// Validate and apply a partial preference update, returning the full merged set.
/// The patch is already type-validated (unknown keys / bad enum values are
/// rejected at deserialisation); the service additionally rejects an empty patch
/// and serialises only the present keys so other stored keys are preserved.
pub async fn update_preferences(
    pool: &PgPool,
    account_id: Uuid,
    patch: PreferencesPatch,
) -> Result<PreferencesDto, AppError> {
    if patch.is_empty() {
        return Err(AppError::BadRequest(
            "no preference fields supplied".to_string(),
        ));
    }

    let patch_json = patch_to_json(&patch);

    let merged = db::merge_preferences(pool, account_id, &patch_json)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    Ok(dto_from_bag(&merged))
}

/// Build a `PreferencesDto` from a stored JSONB bag, defaulting each key whose
/// value is missing or unrecognised. Forward-compatible: extra keys are ignored.
fn dto_from_bag(bag: &Value) -> PreferencesDto {
    let d = PreferencesDto::default();
    PreferencesDto {
        text_size: bag
            .get("text_size")
            .and_then(|v| serde_json::from_value::<TextSize>(v.clone()).ok())
            .unwrap_or(d.text_size),
        reduce_motion: bag
            .get("reduce_motion")
            .and_then(|v| serde_json::from_value::<TriState>(v.clone()).ok())
            .unwrap_or(d.reduce_motion),
        high_contrast: bag
            .get("high_contrast")
            .and_then(|v| serde_json::from_value::<TriState>(v.clone()).ok())
            .unwrap_or(d.high_contrast),
        large_targets: bag
            .get("large_targets")
            .and_then(|v| serde_json::from_value::<Toggle>(v.clone()).ok())
            .unwrap_or(d.large_targets),
        dyslexia_font: bag
            .get("dyslexia_font")
            .and_then(|v| serde_json::from_value::<Toggle>(v.clone()).ok())
            .unwrap_or(d.dyslexia_font),
    }
}

/// Serialise only the patch's present keys into a JSON object for the merge.
fn patch_to_json(patch: &PreferencesPatch) -> Value {
    let mut map = serde_json::Map::new();
    if let Some(v) = patch.text_size {
        map.insert(
            "text_size".into(),
            serde_json::to_value(v).unwrap_or_default(),
        );
    }
    if let Some(v) = patch.reduce_motion {
        map.insert(
            "reduce_motion".into(),
            serde_json::to_value(v).unwrap_or_default(),
        );
    }
    if let Some(v) = patch.high_contrast {
        map.insert(
            "high_contrast".into(),
            serde_json::to_value(v).unwrap_or_default(),
        );
    }
    if let Some(v) = patch.large_targets {
        map.insert(
            "large_targets".into(),
            serde_json::to_value(v).unwrap_or_default(),
        );
    }
    if let Some(v) = patch.dyslexia_font {
        map.insert(
            "dyslexia_font".into(),
            serde_json::to_value(v).unwrap_or_default(),
        );
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::accounts;
    use serde_json::json;

    #[test]
    fn dto_from_empty_bag_is_all_defaults() {
        assert_eq!(dto_from_bag(&json!({})), PreferencesDto::default());
    }

    #[test]
    fn dto_from_bag_reads_known_keys() {
        let dto = dto_from_bag(&json!({"text_size": "large", "reduce_motion": "on"}));
        assert_eq!(dto.text_size, TextSize::Large);
        assert_eq!(dto.reduce_motion, TriState::On);
        // Untouched keys keep defaults.
        assert_eq!(dto.high_contrast, TriState::Auto);
    }

    #[test]
    fn dto_from_bag_ignores_unknown_and_invalid_values() {
        // `locale` is unknown to this feature; `text_size: "huge"` is invalid.
        let dto = dto_from_bag(&json!({"locale": "en", "text_size": "huge"}));
        assert_eq!(dto, PreferencesDto::default());
    }

    #[test]
    fn patch_to_json_includes_only_present_keys() {
        let patch = PreferencesPatch {
            text_size: Some(TextSize::Small),
            dyslexia_font: Some(Toggle::On),
            ..Default::default()
        };
        let j = patch_to_json(&patch);
        assert_eq!(j, json!({"text_size": "small", "dyslexia_font": "on"}));
    }

    #[sqlx::test]
    async fn get_preferences_defaults_for_new_account(pool: PgPool) {
        let id = accounts::create_account(&pool).await.unwrap();
        let dto = get_preferences(&pool, id).await.unwrap();
        assert_eq!(dto, PreferencesDto::default());
    }

    #[sqlx::test]
    async fn get_preferences_not_found_for_missing_account(pool: PgPool) {
        let err = get_preferences(&pool, Uuid::new_v4()).await.unwrap_err();
        assert!(matches!(err, AppError::NotFound));
    }

    #[sqlx::test]
    async fn update_preferences_merges_and_returns_full_set(pool: PgPool) {
        let id = accounts::create_account(&pool).await.unwrap();
        let patch = PreferencesPatch {
            text_size: Some(TextSize::Large),
            ..Default::default()
        };
        let dto = update_preferences(&pool, id, patch).await.unwrap();
        assert_eq!(dto.text_size, TextSize::Large);
        assert_eq!(dto.reduce_motion, TriState::Auto);
    }

    #[sqlx::test]
    async fn update_preferences_partial_merge_preserves_prior_keys(pool: PgPool) {
        let id = accounts::create_account(&pool).await.unwrap();
        update_preferences(
            &pool,
            id,
            PreferencesPatch {
                text_size: Some(TextSize::Large),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let dto = update_preferences(
            &pool,
            id,
            PreferencesPatch {
                reduce_motion: Some(TriState::On),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(dto.text_size, TextSize::Large);
        assert_eq!(dto.reduce_motion, TriState::On);
    }

    #[sqlx::test]
    async fn update_preferences_rejects_empty_patch(pool: PgPool) {
        let id = accounts::create_account(&pool).await.unwrap();
        let err = update_preferences(&pool, id, PreferencesPatch::default())
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[sqlx::test]
    async fn update_preferences_not_found_for_missing_account(pool: PgPool) {
        let patch = PreferencesPatch {
            text_size: Some(TextSize::Large),
            ..Default::default()
        };
        let err = update_preferences(&pool, Uuid::new_v4(), patch)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotFound));
    }

    #[sqlx::test]
    async fn update_preferences_does_not_clobber_foreign_keys(pool: PgPool) {
        // A key owned by another feature (e.g. locale) survives an accessibility update.
        let id = accounts::create_account(&pool).await.unwrap();
        crate::db::preferences::merge_preferences(&pool, id, &json!({"locale": "fr"}))
            .await
            .unwrap();
        update_preferences(
            &pool,
            id,
            PreferencesPatch {
                text_size: Some(TextSize::Large),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let bag = crate::db::preferences::get_preferences(&pool, id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(bag["locale"], "fr");
        assert_eq!(bag["text_size"], "large");
    }
}
