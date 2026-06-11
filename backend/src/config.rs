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

pub struct Config {
    pub app_url: String,
    pub encryption_secret: String,
    pub esi_client_id: String,
    pub esi_client_secret: String,
    pub database_url: String,
    pub rate_limit: RateLimitConfig,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            app_url: std::env::var("APP_URL")
                .context("APP_URL environment variable is required")?,
            encryption_secret: std::env::var("ENCRYPTION_SECRET")
                .context("ENCRYPTION_SECRET environment variable is required")?,
            esi_client_id: std::env::var("ESI_CLIENT_ID")
                .context("ESI_CLIENT_ID environment variable is required")?,
            esi_client_secret: std::env::var("ESI_CLIENT_SECRET")
                .context("ESI_CLIENT_SECRET environment variable is required")?,
            database_url: std::env::var("DATABASE_URL")
                .context("DATABASE_URL environment variable is required")?,
            rate_limit: RateLimitConfig::from_env()?,
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
