use utoipa::{
    openapi::security::{ApiKey, ApiKeyValue, Http, HttpAuthScheme, SecurityScheme},
    Modify, OpenApi,
};

use crate::{
    dto::{
        account::{AccountDto, CharacterDto, MeDto},
        keys::{CreateKeyRequest, CreatedKeyDto, KeyMetadataDto},
    },
    error::{ErrorDetail, ErrorEnvelope},
    response::{CharacterResponse, CreatedKeyResponse, KeyListResponse, MeResponse},
};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "E-R Bridge API",
        version = env!("CARGO_PKG_VERSION"),
    ),
    paths(
        crate::handlers::api::v1::me::get_me,
        crate::handlers::api::v1::keys::create_key,
        crate::handlers::api::v1::keys::list_keys,
        crate::handlers::api::v1::keys::delete_key,
        crate::handlers::api::v1::characters::set_main,
        crate::handlers::api::v1::characters::delete_character,
        crate::handlers::api::v1::account::delete_account,
    ),
    components(schemas(
        MeResponse,
        CreatedKeyResponse,
        KeyListResponse,
        CharacterResponse,
        MeDto,
        AccountDto,
        CharacterDto,
        CreateKeyRequest,
        CreatedKeyDto,
        KeyMetadataDto,
        ErrorEnvelope,
        ErrorDetail,
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "account", description = "Account and character management"),
        (name = "keys", description = "API key management"),
        (name = "characters", description = "EVE character operations"),
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
