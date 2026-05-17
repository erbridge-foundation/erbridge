use axum::http::{HeaderMap, HeaderValue};

const COOKIE_NAME: &str = "session";

pub fn set_session_cookie(headers: &mut HeaderMap, jwt: &str) {
    let value = format!(
        "{COOKIE_NAME}={jwt}; HttpOnly; SameSite=Lax; Path=/"
    );
    headers.insert(
        axum::http::header::SET_COOKIE,
        HeaderValue::from_str(&value).expect("cookie value is valid header"),
    );
}

pub fn clear_session_cookie(headers: &mut HeaderMap) {
    let value = format!(
        "{COOKIE_NAME}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0"
    );
    headers.insert(
        axum::http::header::SET_COOKIE,
        HeaderValue::from_str(&value).expect("cookie clear value is valid header"),
    );
}

pub fn extract_session_jwt(headers: &HeaderMap) -> Option<String> {
    let cookie_header = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    for part in cookie_header.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix(&format!("{COOKIE_NAME}=")) {
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
    }

    #[test]
    fn extract_session_jwt_finds_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_static("session=abc.def.ghi; other=val"),
        );
        assert_eq!(extract_session_jwt(&headers), Some("abc.def.ghi".to_string()));
    }

    #[test]
    fn extract_session_jwt_returns_none_when_absent() {
        let headers = HeaderMap::new();
        assert_eq!(extract_session_jwt(&headers), None);
    }
}
