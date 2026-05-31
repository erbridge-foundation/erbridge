use utoipa::{
    Modify, OpenApi,
    openapi::security::{ApiKey, ApiKeyValue, Http, HttpAuthScheme, SecurityScheme},
};

use crate::{
    dto::{
        account::{AccountDto, CharacterDto, MeDto},
        admin::{
            AdminAccountCharacterDto, AdminAccountDto, AuditLogEntryDto, AuditLogPageDto,
            BlockCharacterRequest, BlockedCharacterDto, CharacterSearchResultDto,
        },
        health::{ComponentHealth, ComponentStatus, HealthResponse, HealthStatus},
        keys::{CreateKeyRequest, CreatedKeyDto, KeyMetadataDto},
        preferences::{PreferencesDto, PreferencesPatch, TextSize, Toggle, TriState},
    },
    error::{ErrorDetail, ErrorEnvelope},
    response::{
        AdminAccountListResponse, AuditLogPageResponse, BlockListResponse, CharacterResponse,
        CharacterSearchResponse, CreatedKeyResponse, KeyListResponse, MeResponse,
        PreferencesResponse,
    },
};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "E-R Bridge API",
        version = env!("CARGO_PKG_VERSION"),
    ),
    paths(
        crate::handlers::api::v1::me::get_me,
        crate::handlers::api::v1::preferences::get_preferences,
        crate::handlers::api::v1::preferences::update_preferences,
        crate::handlers::api::v1::keys::create_key,
        crate::handlers::api::v1::keys::list_keys,
        crate::handlers::api::v1::keys::delete_key,
        crate::handlers::api::v1::characters::set_main,
        crate::handlers::api::v1::characters::delete_character,
        crate::handlers::api::v1::account::delete_account,
        crate::handlers::api::v1::admin::list_accounts,
        crate::handlers::api::v1::admin::search_characters,
        crate::handlers::api::v1::admin::grant_admin,
        crate::handlers::api::v1::admin::revoke_admin,
        crate::handlers::api::v1::admin::list_blocks,
        crate::handlers::api::v1::admin::block_character,
        crate::handlers::api::v1::admin::unblock_character,
        crate::handlers::api::v1::admin::list_audit,
        crate::handlers::health::get_health,
    ),
    components(schemas(
        HealthResponse,
        HealthStatus,
        ComponentHealth,
        ComponentStatus,
        MeResponse,
        CreatedKeyResponse,
        KeyListResponse,
        CharacterResponse,
        PreferencesResponse,
        PreferencesDto,
        PreferencesPatch,
        TextSize,
        TriState,
        Toggle,
        MeDto,
        AccountDto,
        CharacterDto,
        CreateKeyRequest,
        CreatedKeyDto,
        KeyMetadataDto,
        AdminAccountListResponse,
        AdminAccountDto,
        AdminAccountCharacterDto,
        CharacterSearchResponse,
        CharacterSearchResultDto,
        BlockListResponse,
        BlockedCharacterDto,
        BlockCharacterRequest,
        AuditLogPageResponse,
        AuditLogPageDto,
        AuditLogEntryDto,
        ErrorEnvelope,
        ErrorDetail,
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "account", description = "Account and character management"),
        (name = "preferences", description = "Per-account accessibility preferences"),
        (name = "keys", description = "API key management"),
        (name = "characters", description = "EVE character operations"),
        (name = "admin", description = "Server administration: admin management, block list, audit log"),
        (name = "health", description = "Liveness and component health"),
    ),
)]
pub struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "session_cookie",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("session"))),
        );
        components.add_security_scheme(
            "bearer_token",
            SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
        );
    }
}
