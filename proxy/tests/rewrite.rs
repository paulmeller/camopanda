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

    rewrite_headers(&mut headers, "FetchTest/2.0").unwrap();

    assert_eq!(headers["user-agent"], "FetchTest/2.0");
    assert!(
        !headers
            .keys()
            .any(|name| name.as_str().starts_with("sec-ch-ua"))
    );
    assert_eq!(headers["accept"], "text/html");
}

#[test]
fn rejects_invalid_user_agent_without_mutating_headers() {
    let mut headers = HeaderMap::new();
    headers.insert("user-agent", HeaderValue::from_static("Lightpanda/1.0"));

    assert!(rewrite_headers(&mut headers, "bad\nvalue").is_err());
    assert_eq!(headers["user-agent"], "Lightpanda/1.0");
}
