use hudsucker::hyper::header::{HeaderMap, HeaderValue};
use lightpanda_hudsucker_proxy::rewrite_headers;

#[test]
fn replaces_user_agent_and_removes_every_client_hint_header() {
    let mut headers = HeaderMap::new();
    headers.insert("user-agent", HeaderValue::from_static("Lightpanda/1.0"));
    headers.insert("sec-ch-ua", HeaderValue::from_static("Lightpanda"));
    headers.insert(
        "sec-ch-ua-full-version-list",
        HeaderValue::from_static("Lightpanda/1.1"),
    );
    headers.insert("accept", HeaderValue::from_static("text/html"));
    headers.insert("x-camopanda-profile", HeaderValue::from_static("private"));

    rewrite_headers(&mut headers, "FetchTest/2.0").unwrap();

    assert_eq!(headers["user-agent"], "FetchTest/2.0");
    assert!(
        !headers
            .keys()
            .any(|name| name.as_str().starts_with("sec-ch-ua"))
    );
    assert_eq!(headers["accept"], "text/html");
    assert!(!headers.contains_key("x-camopanda-profile"));
}

#[test]
fn rejects_invalid_user_agent_without_mutating_headers() {
    let mut headers = HeaderMap::new();
    headers.insert("user-agent", HeaderValue::from_static("Lightpanda/1.0"));
    headers.insert("sec-ch-ua", HeaderValue::from_static("Lightpanda"));
    let original = headers.clone();

    assert!(rewrite_headers(&mut headers, "bad\nvalue").is_err());
    assert_eq!(headers["user-agent"], "Lightpanda/1.0");
    assert_eq!(headers, original);
}

#[test]
fn handles_duplicate_headers_and_preserves_unrelated_client_hints() {
    let mut headers = HeaderMap::new();
    headers.append("user-agent", HeaderValue::from_static("old-one"));
    headers.append("user-agent", HeaderValue::from_static("old-two"));
    headers.append("sec-ch-ua-mobile", HeaderValue::from_static("?0"));
    headers.append("sec-ch-ua-mobile", HeaderValue::from_static("?1"));
    headers.insert("sec-ch-viewport-width", HeaderValue::from_static("800"));
    headers.insert("authorization", HeaderValue::from_static("Bearer fixture"));

    rewrite_headers(&mut headers, "Test/1.0").unwrap();

    assert_eq!(headers.get_all("user-agent").iter().count(), 1);
    assert_eq!(headers["user-agent"], "Test/1.0");
    assert!(!headers.contains_key("sec-ch-ua-mobile"));
    assert_eq!(headers["sec-ch-viewport-width"], "800");
    assert_eq!(headers["authorization"], "Bearer fixture");
}
