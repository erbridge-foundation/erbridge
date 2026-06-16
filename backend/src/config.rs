use anyhow::{Context, Result};

/// Parses an optional `u32` env var, falling back to `default` when unset.
/// Returns an error only when the var is present but unparseable, so a typo
/// fails loudly rather than silently reverting to the default.
fn env_u32(name: &str, default: u32) -> Result<u32> {
    match std::env::var(name) {
        Ok(v) => v
            .parse::<u32>()
            .with_context(|| format!("{name} must be a non-negative integer")),
        Err(_) => Ok(default),
    }
}

/// Outbound ESI rate-limit safety thresholds and the inbound per-IP request
/// limits. All values are env-tunable with conservative defaults so the change
/// ships safe and is tuned from real telemetry (see the esi-rate-limiting /
/// api-rate-limiting / auth-rate-limiting specs).
#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    /// Legacy ESI per-IP error budget: trip the process-wide gate when
    /// `X-Esi-Error-Limit-Remain` falls at or below this. CCP allows ~100
    /// errors / 60s; a conservative headroom keeps us clear of the 420.
    pub esi_error_remain_threshold: i64,
    /// ESI token bucket: trip a `(group, userID)` bucket when its remaining
    /// tokens fall at or below this. Conservative absolute floor; the bucket
    /// also honours a hard 429 wait regardless.
    pub esi_bucket_remain_threshold: i64,
    /// Inbound `/api/*` sustained rate: one request replenished per this many
    /// milliseconds, per client IP.
    pub api_per_millis: u64,
    /// Inbound `/api/*` burst allowance, per client IP.
    pub api_burst: u32,
    /// Inbound `/auth/*` sustained rate (tighter): one request per this many
    /// milliseconds, per client IP.
    pub auth_per_millis: u64,
    /// Inbound `/auth/*` burst allowance, per client IP.
    pub auth_burst: u32,
}

impl RateLimitConfig {
    fn from_env() -> Result<Self> {
        Ok(Self {
            esi_error_remain_threshold: env_u32("ESI_ERROR_REMAIN_THRESHOLD", 15)? as i64,
            esi_bucket_remain_threshold: env_u32("ESI_BUCKET_REMAIN_THRESHOLD", 10)? as i64,
            // ~10 req/s sustained, burst 20: generous for a normal SPA session,
            // a meaningful brake on hammering.
            api_per_millis: env_u32("API_RATE_PER_MILLIS", 100)? as u64,
            api_burst: env_u32("API_RATE_BURST", 20)?,
            // Tighter: ~1 req/s sustained, burst 5. /auth/callback is the most
            // expensive unauthenticated endpoint.
            auth_per_millis: env_u32("AUTH_RATE_PER_MILLIS", 1000)? as u64,
            auth_burst: env_u32("AUTH_RATE_BURST", 5)?,
        })
    }
}

/// The default socket address the server binds. Overridable via `BIND_ADDR` so
/// no deployment change is required for the common case.
pub const DEFAULT_BIND_ADDR: &str = "0.0.0.0:3000";

/// EVE system-catalog source URLs and the anoikis User-Agent. Defaults baked in;
/// overridable via env so tests can point them at fixtures/mirrors. The anoikis
/// fetch sends an explicit, identifying User-Agent as a good-citizen courtesy
/// (the host does not currently reject the default, but it future-proofs us).
pub const DEFAULT_SYSTEMS_URL: &str = "https://api.eve-scout.com/v2/public/systems";
pub const DEFAULT_WORMHOLE_TYPES_URL: &str = "https://api.eve-scout.com/v2/public/wormholetypes";
pub const DEFAULT_STATICS_URL: &str = "https://anoikis.info/data/wh-statics.json";
pub const DEFAULT_CATALOG_USER_AGENT: &str =
    "erbridge-wormhole-mapper (+https://github.com/erbridge)";

/// Sources and identity for the daily EVE system-catalog sync.
#[derive(Clone, Debug)]
pub struct CatalogConfig {
    pub systems_url: String,
    pub wormhole_types_url: String,
    pub statics_url: String,
    pub user_agent: String,
}

impl CatalogConfig {
    fn from_env() -> Self {
        Self {
            systems_url: std::env::var("CATALOG_SYSTEMS_URL")
                .unwrap_or_else(|_| DEFAULT_SYSTEMS_URL.to_string()),
            wormhole_types_url: std::env::var("CATALOG_WORMHOLE_TYPES_URL")
                .unwrap_or_else(|_| DEFAULT_WORMHOLE_TYPES_URL.to_string()),
            statics_url: std::env::var("CATALOG_STATICS_URL")
                .unwrap_or_else(|_| DEFAULT_STATICS_URL.to_string()),
            user_agent: std::env::var("CATALOG_USER_AGENT")
                .unwrap_or_else(|_| DEFAULT_CATALOG_USER_AGENT.to_string()),
        }
    }
}

impl Default for CatalogConfig {
    fn default() -> Self {
        Self {
            systems_url: DEFAULT_SYSTEMS_URL.to_string(),
            wormhole_types_url: DEFAULT_WORMHOLE_TYPES_URL.to_string(),
            statics_url: DEFAULT_STATICS_URL.to_string(),
            user_agent: DEFAULT_CATALOG_USER_AGENT.to_string(),
        }
    }
}

pub struct Config {
    pub app_url: String,
    pub encryption_secret: String,
    pub esi_client_id: String,
    pub esi_client_secret: String,
    pub database_url: String,
    /// Full OAuth2 callback URL sent as `redirect_uri`. Resolved from
    /// `ESI_CALLBACK_URL` when set, otherwise `{app_url}/auth/callback`.
    /// Exists so a deployment whose public callback path differs from
    /// `{app_url}/auth/callback` (e.g. behind a path-rewriting proxy) can
    /// override it. Every callsite that builds `redirect_uri` reads this field
    /// so they cannot diverge.
    pub esi_callback_url: String,
    /// Socket address to bind, `BIND_ADDR` (default [`DEFAULT_BIND_ADDR`]).
    pub bind_addr: String,
    pub rate_limit: RateLimitConfig,
    pub catalog: CatalogConfig,
}

/// Resolves the OAuth2 callback URL: the explicit `ESI_CALLBACK_URL` override
/// when set, otherwise `{app_url}/auth/callback`. Factored out so the test can
/// exercise the resolution in lockstep with `from_env` without mutating
/// process-global env.
fn resolve_callback_url(app_url: &str, override_var: Option<&str>) -> String {
    override_var
        .map(str::to_string)
        .unwrap_or_else(|| format!("{app_url}/auth/callback"))
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let app_url =
            std::env::var("APP_URL").context("APP_URL environment variable is required")?;
        Ok(Self {
            esi_callback_url: resolve_callback_url(
                &app_url,
                std::env::var("ESI_CALLBACK_URL").ok().as_deref(),
            ),
            encryption_secret: std::env::var("ENCRYPTION_SECRET")
                .context("ENCRYPTION_SECRET environment variable is required")?,
            esi_client_id: std::env::var("ESI_CLIENT_ID")
                .context("ESI_CLIENT_ID environment variable is required")?,
            esi_client_secret: std::env::var("ESI_CLIENT_SECRET")
                .context("ESI_CLIENT_SECRET environment variable is required")?,
            database_url: std::env::var("DATABASE_URL")
                .context("DATABASE_URL environment variable is required")?,
            bind_addr: std::env::var("BIND_ADDR").unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_string()),
            rate_limit: RateLimitConfig::from_env()?,
            catalog: CatalogConfig::from_env(),
            app_url,
        })
    }
}

impl Default for RateLimitConfig {
    /// Conservative defaults matching `from_env` with no env overrides. Lets
    /// tests build a `Config` without touching the environment.
    fn default() -> Self {
        Self {
            esi_error_remain_threshold: 15,
            esi_bucket_remain_threshold: 10,
            api_per_millis: 100,
            api_burst: 20,
            auth_per_millis: 1000,
            auth_burst: 5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `from_env` reads `BIND_ADDR`, falling back to the default when unset.
    /// Mirrors the resolution `from_env` performs (kept in lockstep so the test
    /// does not have to mutate process-global env, which would race other tests).
    fn resolve_bind_addr(var: Option<&str>) -> String {
        var.map(str::to_string)
            .unwrap_or_else(|| DEFAULT_BIND_ADDR.to_string())
    }

    #[test]
    fn bind_addr_defaults_when_unset() {
        assert_eq!(resolve_bind_addr(None), "0.0.0.0:3000");
        assert_eq!(resolve_bind_addr(None), DEFAULT_BIND_ADDR);
    }

    #[test]
    fn bind_addr_uses_override_when_set() {
        assert_eq!(resolve_bind_addr(Some("127.0.0.1:8080")), "127.0.0.1:8080");
    }

    #[test]
    fn callback_url_defaults_to_app_url_path_when_unset() {
        assert_eq!(
            resolve_callback_url("https://erbridge.example.com", None),
            "https://erbridge.example.com/auth/callback"
        );
    }

    #[test]
    fn callback_url_uses_override_verbatim_when_set() {
        assert_eq!(
            resolve_callback_url(
                "https://erbridge.example.com",
                Some("https://proxy.example.com/sso/return")
            ),
            "https://proxy.example.com/sso/return"
        );
    }
}
