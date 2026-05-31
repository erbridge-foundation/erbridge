use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Text-size preference. `Auto` follows the browser/OS default (no override).
#[derive(Serialize, Deserialize, ToSchema, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TextSize {
    Auto,
    Small,
    Regular,
    Large,
}

/// A tri-state toggle for preferences that have an OS media-query default.
/// `Auto` follows the OS (no stored override); `On`/`Off` force the value.
#[derive(Serialize, Deserialize, ToSchema, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TriState {
    Auto,
    On,
    Off,
}

/// A plain on/off toggle for preferences with no OS default.
#[derive(Serialize, Deserialize, ToSchema, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Toggle {
    Off,
    On,
}

/// The interface language. This is the accepted-locale set for the API; it must
/// stay in sync with Paraglide's compiled locale list on the frontend (see the
/// i18n change's design.md). `En` is the default.
#[derive(Serialize, Deserialize, ToSchema, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Locale {
    En,
    De,
    Fr,
}

/// The full accessibility preference set as returned to clients. Absent keys
/// are serialised as their default (`auto` / `off`), so the response is always
/// a complete, explicit set the frontend can apply directly.
#[derive(Serialize, ToSchema, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreferencesDto {
    pub text_size: TextSize,
    pub reduce_motion: TriState,
    pub high_contrast: TriState,
    pub large_targets: Toggle,
    pub dyslexia_font: Toggle,
    pub locale: Locale,
}

impl Default for PreferencesDto {
    fn default() -> Self {
        Self {
            text_size: TextSize::Auto,
            reduce_motion: TriState::Auto,
            high_contrast: TriState::Auto,
            large_targets: Toggle::Off,
            dyslexia_font: Toggle::Off,
            locale: Locale::En,
        }
    }
}

/// A partial update. Every field is optional; only the present keys are merged
/// into the stored bag. Deserialisation rejects unknown keys (`deny_unknown_fields`)
/// and invalid enum values, so a malformed body fails before reaching the service.
#[derive(Deserialize, ToSchema, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PreferencesPatch {
    #[serde(default)]
    pub text_size: Option<TextSize>,
    #[serde(default)]
    pub reduce_motion: Option<TriState>,
    #[serde(default)]
    pub high_contrast: Option<TriState>,
    #[serde(default)]
    pub large_targets: Option<Toggle>,
    #[serde(default)]
    pub dyslexia_font: Option<Toggle>,
    #[serde(default)]
    pub locale: Option<Locale>,
}

impl PreferencesPatch {
    /// True when no field is set — an empty patch is a no-op the service rejects.
    pub fn is_empty(&self) -> bool {
        self.text_size.is_none()
            && self.reduce_motion.is_none()
            && self.high_contrast.is_none()
            && self.large_targets.is_none()
            && self.dyslexia_font.is_none()
            && self.locale.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn default_preferences_are_auto_and_off() {
        let d = PreferencesDto::default();
        assert_eq!(d.text_size, TextSize::Auto);
        assert_eq!(d.reduce_motion, TriState::Auto);
        assert_eq!(d.high_contrast, TriState::Auto);
        assert_eq!(d.large_targets, Toggle::Off);
        assert_eq!(d.dyslexia_font, Toggle::Off);
        assert_eq!(d.locale, Locale::En);
    }

    #[test]
    fn patch_deserialises_partial_body() {
        let p: PreferencesPatch = serde_json::from_value(json!({"text_size": "large"})).unwrap();
        assert_eq!(p.text_size, Some(TextSize::Large));
        assert!(p.reduce_motion.is_none());
    }

    #[test]
    fn patch_deserialises_locale() {
        let p: PreferencesPatch = serde_json::from_value(json!({"locale": "en"})).unwrap();
        assert_eq!(p.locale, Some(Locale::En));
        assert!(p.text_size.is_none());

        let de: PreferencesPatch = serde_json::from_value(json!({"locale": "de"})).unwrap();
        assert_eq!(de.locale, Some(Locale::De));

        let fr: PreferencesPatch = serde_json::from_value(json!({"locale": "fr"})).unwrap();
        assert_eq!(fr.locale, Some(Locale::Fr));
    }

    #[test]
    fn patch_rejects_unknown_key() {
        let err = serde_json::from_value::<PreferencesPatch>(json!({"not_a_pref": "x"}));
        assert!(err.is_err());
    }

    #[test]
    fn patch_rejects_invalid_locale_value() {
        let err = serde_json::from_value::<PreferencesPatch>(json!({"locale": "martian"}));
        assert!(err.is_err());
    }

    #[test]
    fn patch_rejects_invalid_enum_value() {
        let err = serde_json::from_value::<PreferencesPatch>(json!({"text_size": "enormous"}));
        assert!(err.is_err());
    }

    #[test]
    fn empty_patch_detected() {
        assert!(PreferencesPatch::default().is_empty());
        let p = PreferencesPatch {
            text_size: Some(TextSize::Small),
            ..Default::default()
        };
        assert!(!p.is_empty());
    }

    #[test]
    fn dto_serialises_to_snake_case_values() {
        let json = serde_json::to_value(PreferencesDto::default()).unwrap();
        assert_eq!(json["text_size"], "auto");
        assert_eq!(json["large_targets"], "off");
        assert_eq!(json["locale"], "en");
    }
}
