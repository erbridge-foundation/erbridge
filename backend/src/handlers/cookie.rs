use axum::http::{HeaderMap, HeaderValue};

const COOKIE_NAME: &str = "session";
const AUTH_STATE_COOKIE_NAME: &str = "auth_state";

pub fn set_session_cookie(headers: &mut HeaderMap, jwt: &str) {
    let value = format!("{COOKIE_NAME}={jwt}; HttpOnly; SameSite=Lax; Secure; Path=/");
    // The JWT is base64url segments joined by dots — always a valid header
    // value, so this cannot panic in practice.
    #[allow(clippy::expect_used)]
    headers.insert(
        axum::http::header::SET_COOKIE,
        HeaderValue::from_str(&value).expect("cookie value is valid header"),
    );
}

pub fn clear_session_cookie(headers: &mut HeaderMap) {
    let value = format!("{COOKIE_NAME}=; HttpOnly; SameSite=Lax; Secure; Path=/; Max-Age=0");
    // Built entirely from fixed ASCII — always a valid header value.
    #[allow(clippy::expect_used)]
    headers.insert(
        axum::http::header::SET_COOKIE,
        HeaderValue::from_str(&value).expect("cookie clear value is valid header"),
    );
}

pub fn extract_session_jwt(headers: &HeaderMap) -> Option<String> {
    extract_cookie(headers, COOKIE_NAME)
}

/// Sets the short-lived OAuth state cookie binding the in-flight login to this
/// browser. Scoped to `Path=/auth` so it is never sent on API requests, and
/// expires after 15 minutes to match the in-flight record TTL.
pub fn set_auth_state_cookie(headers: &mut HeaderMap, csrf_state: &str) {
    let value = format!(
        "{AUTH_STATE_COOKIE_NAME}={csrf_state}; HttpOnly; SameSite=Lax; Secure; Path=/auth; Max-Age=900"
    );
    // The csrf_state is a UUID — always a valid header value, so this cannot
    // panic in practice. Appended (not inserted) so it can coexist with a
    // session-cookie `Set-Cookie` on the same response.
    #[allow(clippy::expect_used)]
    headers.append(
        axum::http::header::SET_COOKIE,
        HeaderValue::from_str(&value).expect("auth_state cookie value is valid header"),
    );
}

pub fn clear_auth_state_cookie(headers: &mut HeaderMap) {
    let value =
        format!("{AUTH_STATE_COOKIE_NAME}=; HttpOnly; SameSite=Lax; Secure; Path=/auth; Max-Age=0");
    // Built entirely from fixed ASCII — always a valid header value. Appended
    // (not inserted) so it can coexist with a session-cookie `Set-Cookie` on
    // the same response (the successful-callback path sets both).
    #[allow(clippy::expect_used)]
    headers.append(
        axum::http::header::SET_COOKIE,
        HeaderValue::from_str(&value).expect("auth_state clear value is valid header"),
    );
}

pub fn extract_auth_state(headers: &HeaderMap) -> Option<String> {
    extract_cookie(headers, AUTH_STATE_COOKIE_NAME)
}

fn extract_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let cookie_header = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    let prefix = format!("{name}=");
    for part in cookie_header.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix(&prefix) {
            return Some(value.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_cookie_contains_required_attributes() {
        let mut headers = HeaderMap::new();
        set_session_cookie(&mut headers, "my.jwt.token");
        let cookie = headers
            .get(axum::http::header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(cookie.contains("session=my.jwt.token"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Secure"));
        assert!(cookie.contains("Path=/"));
    }

    #[test]
    fn clear_cookie_sets_max_age_zero() {
        let mut headers = HeaderMap::new();
        clear_session_cookie(&mut headers);
        let cookie = headers
            .get(axum::http::header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(cookie.contains("Max-Age=0"));
        assert!(cookie.contains("Secure"));
    }

    #[test]
    fn set_auth_state_cookie_contains_required_attributes() {
        let mut headers = HeaderMap::new();
        set_auth_state_cookie(&mut headers, "the-csrf-state");
        let cookie = headers
            .get(axum::http::header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(cookie.contains("auth_state=the-csrf-state"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Secure"));
        assert!(cookie.contains("Path=/auth"));
        assert!(cookie.contains("Max-Age=900"));
    }

    #[test]
    fn clear_auth_state_cookie_sets_max_age_zero() {
        let mut headers = HeaderMap::new();
        clear_auth_state_cookie(&mut headers);
        let cookie = headers
            .get(axum::http::header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(cookie.contains("auth_state="));
        assert!(cookie.contains("Max-Age=0"));
        assert!(cookie.contains("Path=/auth"));
        assert!(cookie.contains("Secure"));
    }

    #[test]
    fn extract_auth_state_finds_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_static("session=abc.def.ghi; auth_state=xyz-123"),
        );
        assert_eq!(extract_auth_state(&headers), Some("xyz-123".to_string()));
    }

    #[test]
    fn extract_auth_state_returns_none_when_absent() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_static("session=abc.def.ghi"),
        );
        assert_eq!(extract_auth_state(&headers), None);
    }

    #[test]
    fn extract_session_jwt_finds_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_static("session=abc.def.ghi; other=val"),
        );
        assert_eq!(
            extract_session_jwt(&headers),
            Some("abc.def.ghi".to_string())
        );
    }

    #[test]
    fn extract_session_jwt_returns_none_when_absent() {
        let headers = HeaderMap::new();
        assert_eq!(extract_session_jwt(&headers), None);
    }
}
