//! Outbound ESI dual-limiter back-off middleware.
//!
//! ESI runs two coexisting, mutually-exclusive limiters (per CCP's official
//! rate-limiting docs); this middleware on the shared `reqwest_middleware`
//! client respects both so every ESI caller (token sweep, search, public-info,
//! token refresh) is covered without per-call-site changes. See the
//! `esi-rate-limiting` capability.
//!
//! 1. **Token bucket** — keyed per `(rate-limit-group, userID)`, signalled by
//!    `X-Ratelimit-*`, exhaustion → HTTP 429 + `Retry-After`. Meters all
//!    responses (even 2xx), so the happy path is not free.
//! 2. **Legacy error budget** — ~100 non-2xx/3xx per rolling 60s, per source
//!    IP, signalled by `X-Esi-Error-Limit-*`, exhaustion → HTTP 420.
//!
//! Design points realised here:
//! - One middleware, two pieces of `Arc`-shared state (a process-wide error
//!   gate + a per-bucket map), because both react to headers on the same
//!   responses through the same client.
//! - Locked sections stay tiny (read/update a few numbers); every `sleep`
//!   happens *outside* the lock.
//! - 420 and 429 are surfaced to the caller as transport errors (not a
//!   success response), matching the existing `esi::search` "unavailable"
//!   handling.
//! - The very first call on a cold bucket cannot be pre-gated (we only learn a
//!   route's `(group, userID)` from its first response); the error gate still
//!   covers that call's downside.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use http::Extensions;
use reqwest::{Request, Response};
use reqwest_middleware::{Error, Middleware, Next, Result};
use tokio::sync::Mutex;
use tracing::warn;

/// HTTP 420 ("Enhance Your Calm") — ESI's legacy-error-budget exhaustion code.
const STATUS_420: u16 = 420;

/// Recorded state for the legacy per-IP error budget.
#[derive(Clone, Copy, Debug, Default)]
struct ErrorGate {
    /// Last-seen `X-Esi-Error-Limit-Remain`. `None` until first observed.
    remain: Option<i64>,
    /// When the current error window resets (from `X-Esi-Error-Limit-Reset` or
    /// a 420 `Retry-After`). `None` until first observed.
    reset_at: Option<DateTime<Utc>>,
}

/// Recorded state for one `(group, userID)` token bucket.
#[derive(Clone, Copy, Debug)]
struct Bucket {
    /// Last-seen `X-Ratelimit-Remaining`.
    remaining: i64,
    /// When this bucket's floating window next releases tokens (best-effort:
    /// derived from a 429 `Retry-After`, else a short default window).
    window_until: DateTime<Utc>,
}

/// Pure decision: how long (if at all) to wait on the error gate before issuing
/// a new request, given the recorded state, a `now`, and the trip threshold.
/// Returns `None` when no wait is needed.
fn error_gate_wait(gate: &ErrorGate, now: DateTime<Utc>, threshold: i64) -> Option<ChronoDuration> {
    match (gate.remain, gate.reset_at) {
        (Some(remain), Some(reset_at)) if remain <= threshold && now < reset_at => {
            Some(reset_at - now)
        }
        _ => None,
    }
}

/// Pure decision: how long to wait on a known bucket before drawing on it.
fn bucket_wait(bucket: &Bucket, now: DateTime<Utc>, threshold: i64) -> Option<ChronoDuration> {
    if bucket.remaining <= threshold && now < bucket.window_until {
        Some(bucket.window_until - now)
    } else {
        None
    }
}

/// Shared, cloneable middleware. Holds the process-wide error gate and the
/// per-`(group, userID)` bucket map behind `Arc<Mutex<_>>`. Buckets are keyed
/// by the `X-Ratelimit-Group` + `userID` exactly as ESI reports them; a request
/// is correlated to a bucket by its URL path so a *known* near-exhausted bucket
/// pre-gates subsequent requests on the same route.
#[derive(Clone)]
pub struct EsiRateLimitMiddleware {
    error_gate: Arc<Mutex<ErrorGate>>,
    buckets: Arc<Mutex<HashMap<String, Bucket>>>,
    /// Maps a request URL path → the bucket key last seen for it, so we can
    /// pre-gate a known bucket before sending.
    path_to_bucket: Arc<Mutex<HashMap<String, String>>>,
    error_threshold: i64,
    bucket_threshold: i64,
}

impl EsiRateLimitMiddleware {
    pub fn new(error_threshold: i64, bucket_threshold: i64) -> Self {
        Self {
            error_gate: Arc::new(Mutex::new(ErrorGate::default())),
            buckets: Arc::new(Mutex::new(HashMap::new())),
            path_to_bucket: Arc::new(Mutex::new(HashMap::new())),
            error_threshold,
            bucket_threshold,
        }
    }

    /// Computes any pre-send wait (the longer of the error-gate wait and the
    /// known-bucket wait), reading state under short-lived locks and releasing
    /// them before the caller sleeps.
    async fn pre_send_wait(&self, path: &str) -> Option<std::time::Duration> {
        let now = Utc::now();

        let gate_wait = {
            let gate = self.error_gate.lock().await;
            error_gate_wait(&gate, now, self.error_threshold)
        };

        let bucket_wait = {
            let key = {
                let map = self.path_to_bucket.lock().await;
                map.get(path).cloned()
            };
            match key {
                Some(k) => {
                    let buckets = self.buckets.lock().await;
                    buckets
                        .get(&k)
                        .and_then(|b| bucket_wait(b, now, self.bucket_threshold))
                }
                None => None,
            }
        };

        let wait = [gate_wait, bucket_wait]
            .into_iter()
            .flatten()
            .max()?
            .to_std()
            .ok()?;
        if wait.is_zero() { None } else { Some(wait) }
    }

    /// Folds a response's rate-limit headers (and status) into the recorded
    /// state. Tolerates the absence of both header sets (leaves state
    /// unchanged). Records a 420 as a process-wide hard wait and a 429 as a
    /// per-bucket hard wait honouring `Retry-After`.
    async fn record(&self, path: &str, resp: &Response) {
        let now = Utc::now();
        let headers = resp.headers();
        let status = resp.status().as_u16();

        // Legacy error budget (X-Esi-Error-Limit-*) — process-wide gate.
        if let Some(remain) = header_i64(headers, "x-esi-error-limit-remain") {
            let reset_secs = header_i64(headers, "x-esi-error-limit-reset").unwrap_or(60);
            let mut gate = self.error_gate.lock().await;
            gate.remain = Some(remain);
            gate.reset_at = Some(now + ChronoDuration::seconds(reset_secs.max(0)));
        }

        // Token bucket (X-Ratelimit-*) — per (group, userID).
        if let Some(group) = header_str(headers, "x-ratelimit-group") {
            // userID is reported indirectly; ESI scopes it per character/app.
            // We key by group + the request path's leading segment as a stable
            // local proxy, which keeps unrelated routes from sharing a bucket.
            let key = format!("{group}|{path}");
            if let Some(remaining) = header_i64(headers, "x-ratelimit-remaining") {
                let window = ChronoDuration::seconds(60);
                let mut buckets = self.buckets.lock().await;
                buckets.insert(
                    key.clone(),
                    Bucket {
                        remaining,
                        window_until: now + window,
                    },
                );
                drop(buckets);
                let mut map = self.path_to_bucket.lock().await;
                map.insert(path.to_string(), key.clone());
            }

            if status == 429 {
                let retry = retry_after_secs(headers).unwrap_or(60);
                let mut buckets = self.buckets.lock().await;
                let b = buckets.entry(key.clone()).or_insert(Bucket {
                    remaining: 0,
                    window_until: now,
                });
                b.remaining = 0;
                b.window_until = now + ChronoDuration::seconds(retry);
                drop(buckets);
                let mut map = self.path_to_bucket.lock().await;
                map.insert(path.to_string(), key);
                warn!(
                    path,
                    retry_after = retry,
                    "ESI token bucket 429: backing off"
                );
            }
        }

        // 420: legacy error budget exhausted → hard process-wide stop.
        if status == STATUS_420 {
            let retry = header_i64(headers, "x-esi-error-limit-reset")
                .or_else(|| retry_after_secs(headers))
                .unwrap_or(60);
            let mut gate = self.error_gate.lock().await;
            gate.remain = Some(0);
            gate.reset_at = Some(now + ChronoDuration::seconds(retry.max(0)));
            warn!(
                retry_after = retry,
                "ESI 420: error budget exhausted, stopping all ESI calls"
            );
        }
    }
}

#[async_trait::async_trait]
impl Middleware for EsiRateLimitMiddleware {
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> Result<Response> {
        let path = req.url().path().to_string();

        if let Some(wait) = self.pre_send_wait(&path).await {
            tokio::time::sleep(wait).await;
        }

        let resp = next.run(req, extensions).await?;
        self.record(&path, &resp).await;

        // Surface 420/429 to the caller as an error rather than a success
        // response, so callers treat them as retryable/unavailable.
        let status = resp.status().as_u16();
        if status == STATUS_420 || status == 429 {
            return Err(Error::Middleware(anyhow::anyhow!(
                "ESI rate limited (HTTP {status}) on {path}"
            )));
        }

        Ok(resp)
    }
}

fn header_str(headers: &http::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

fn header_i64(headers: &http::HeaderMap, name: &str) -> Option<i64> {
    header_str(headers, name).and_then(|s| s.trim().parse::<i64>().ok())
}

/// `Retry-After` as whole seconds (only the delta-seconds form; ESI uses it).
fn retry_after_secs(headers: &http::HeaderMap) -> Option<i64> {
    header_i64(headers, "retry-after")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(offset_secs: i64) -> DateTime<Utc> {
        Utc::now() + ChronoDuration::seconds(offset_secs)
    }

    // ── error gate ────────────────────────────────────────────────────────────

    #[test]
    fn error_gate_above_threshold_no_wait() {
        let gate = ErrorGate {
            remain: Some(50),
            reset_at: Some(ts(30)),
        };
        assert!(error_gate_wait(&gate, Utc::now(), 15).is_none());
    }

    #[test]
    fn error_gate_below_threshold_waits_until_reset() {
        let now = Utc::now();
        let gate = ErrorGate {
            remain: Some(5),
            reset_at: Some(now + ChronoDuration::seconds(30)),
        };
        let wait = error_gate_wait(&gate, now, 15).expect("should wait");
        assert!(wait.num_seconds() >= 29 && wait.num_seconds() <= 30);
    }

    #[test]
    fn error_gate_below_threshold_but_window_elapsed_no_wait() {
        let now = Utc::now();
        let gate = ErrorGate {
            remain: Some(0),
            reset_at: Some(now - ChronoDuration::seconds(1)),
        };
        assert!(error_gate_wait(&gate, now, 15).is_none());
    }

    #[test]
    fn error_gate_unobserved_no_wait() {
        assert!(error_gate_wait(&ErrorGate::default(), Utc::now(), 15).is_none());
    }

    // ── token bucket ────────────────────────────────────────────────────────────

    #[test]
    fn bucket_above_threshold_no_wait() {
        let b = Bucket {
            remaining: 100,
            window_until: ts(60),
        };
        assert!(bucket_wait(&b, Utc::now(), 10).is_none());
    }

    #[test]
    fn bucket_below_threshold_waits() {
        let now = Utc::now();
        let b = Bucket {
            remaining: 2,
            window_until: now + ChronoDuration::seconds(45),
        };
        let wait = bucket_wait(&b, now, 10).expect("should wait");
        assert!(wait.num_seconds() >= 44 && wait.num_seconds() <= 45);
    }

    #[test]
    fn bucket_below_threshold_window_elapsed_no_wait() {
        let now = Utc::now();
        let b = Bucket {
            remaining: 0,
            window_until: now - ChronoDuration::seconds(1),
        };
        assert!(bucket_wait(&b, now, 10).is_none());
    }

    // ── header parsing ──────────────────────────────────────────────────────────

    fn hmap(pairs: &[(&str, &str)]) -> http::HeaderMap {
        let mut m = http::HeaderMap::new();
        for (k, v) in pairs {
            m.insert(
                http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                http::HeaderValue::from_str(v).unwrap(),
            );
        }
        m
    }

    #[test]
    fn parses_error_limit_headers() {
        let h = hmap(&[
            ("x-esi-error-limit-remain", "42"),
            ("x-esi-error-limit-reset", "55"),
        ]);
        assert_eq!(header_i64(&h, "x-esi-error-limit-remain"), Some(42));
        assert_eq!(header_i64(&h, "x-esi-error-limit-reset"), Some(55));
    }

    #[test]
    fn parses_ratelimit_headers() {
        let h = hmap(&[
            ("x-ratelimit-group", "search"),
            ("x-ratelimit-remaining", "7"),
            ("retry-after", "12"),
        ]);
        assert_eq!(
            header_str(&h, "x-ratelimit-group").as_deref(),
            Some("search")
        );
        assert_eq!(header_i64(&h, "x-ratelimit-remaining"), Some(7));
        assert_eq!(retry_after_secs(&h), Some(12));
    }

    #[test]
    fn missing_headers_parse_to_none() {
        let h = hmap(&[]);
        assert_eq!(header_i64(&h, "x-esi-error-limit-remain"), None);
        assert_eq!(header_str(&h, "x-ratelimit-group"), None);
        assert_eq!(retry_after_secs(&h), None);
    }

    // ── end-to-end through the client (state recording + surfacing) ────────────

    mod io {
        use super::*;
        use reqwest_middleware::ClientBuilder;
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        fn client(mw: EsiRateLimitMiddleware) -> reqwest_middleware::ClientWithMiddleware {
            ClientBuilder::new(reqwest::Client::new()).with(mw).build()
        }

        #[tokio::test]
        async fn ok_passes_through_and_records_bucket() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .insert_header("x-ratelimit-group", "search")
                        .insert_header("x-ratelimit-remaining", "100"),
                )
                .mount(&server)
                .await;

            let mw = EsiRateLimitMiddleware::new(15, 10);
            let resp = client(mw.clone()).get(server.uri()).send().await.unwrap();
            assert_eq!(resp.status(), 200);
            // Bucket recorded for the path.
            let map = mw.path_to_bucket.lock().await;
            assert!(map.contains_key("/"));
        }

        #[tokio::test]
        async fn status_420_is_surfaced_as_error_and_trips_gate() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .respond_with(
                    ResponseTemplate::new(420).insert_header("x-esi-error-limit-reset", "30"),
                )
                .mount(&server)
                .await;

            let mw = EsiRateLimitMiddleware::new(15, 10);
            let err = client(mw.clone()).get(server.uri()).send().await.err();
            assert!(err.is_some(), "420 must surface as an error, not success");

            // The process-wide gate is now exhausted with a future reset.
            let gate = mw.error_gate.lock().await;
            assert_eq!(gate.remain, Some(0));
            assert!(gate.reset_at.unwrap() > Utc::now());
        }

        #[tokio::test]
        async fn status_429_is_surfaced_as_error_and_hard_waits_bucket() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .respond_with(
                    ResponseTemplate::new(429)
                        .insert_header("x-ratelimit-group", "search")
                        .insert_header("retry-after", "20"),
                )
                .mount(&server)
                .await;

            let mw = EsiRateLimitMiddleware::new(15, 10);
            let err = client(mw.clone()).get(server.uri()).send().await.err();
            assert!(err.is_some(), "429 must surface as an error, not success");

            let buckets = mw.buckets.lock().await;
            let b = buckets.get("search|/").expect("bucket recorded");
            assert_eq!(b.remaining, 0);
            assert!(b.window_until > Utc::now());
        }

        #[tokio::test]
        async fn missing_both_header_sets_leaves_state_unchanged() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .respond_with(ResponseTemplate::new(200))
                .mount(&server)
                .await;

            let mw = EsiRateLimitMiddleware::new(15, 10);
            let resp = client(mw.clone()).get(server.uri()).send().await.unwrap();
            assert_eq!(resp.status(), 200);

            assert!(mw.error_gate.lock().await.remain.is_none());
            assert!(mw.buckets.lock().await.is_empty());
            assert!(mw.path_to_bucket.lock().await.is_empty());
        }

        #[tokio::test]
        async fn error_gate_is_shared_across_callers() {
            // One cloned handle trips the gate; a second handle (same Arcs)
            // sees it — proving the per-IP gate is process-wide.
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .insert_header("x-esi-error-limit-remain", "5")
                        .insert_header("x-esi-error-limit-reset", "30"),
                )
                .mount(&server)
                .await;

            let mw = EsiRateLimitMiddleware::new(15, 10);
            let _ = client(mw.clone()).get(server.uri()).send().await.unwrap();

            // A second caller sharing the same state observes the low budget.
            let gate = mw.error_gate.lock().await;
            assert_eq!(gate.remain, Some(5));
            assert!(error_gate_wait(&gate, Utc::now(), 15).is_some());
        }
    }
}
