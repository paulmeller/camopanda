use camopanda::Body;
use camopanda::sessions::{Config, Gateway};
use hyper::{Request, StatusCode};

fn config() -> Config {
    Config {
        bind: "127.0.0.1:9223".parse().unwrap(),
        public_url: "ws://127.0.0.1:9223".into(),
        api_key: "test-key-with-at-least-thirty-two-bytes".into(),
        max_sessions: 2,
        idle_secs: 300,
        browser: "missing-browser-fixture".into(),
    }
}
fn gateway() -> Gateway {
    Gateway::new(config()).unwrap()
}

#[tokio::test]
async fn api_requires_auth_before_parsing_body() {
    let response = gateway()
        .handle(
            Request::post("/v1/sessions")
                .body(Body::new("not json".into()))
                .unwrap(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn rejects_invalid_user_agent_without_starting_process() {
    for ua in ["", "bad\r\nvalue", &"x".repeat(513)] {
        let response = gateway()
            .handle(
                Request::post("/v1/sessions")
                    .header(
                        "Authorization",
                        "Bearer test-key-with-at-least-thirty-two-bytes",
                    )
                    .header("Content-Type", "application/json")
                    .body(Body::new(
                        serde_json::json!({"user_agent": ua}).to_string().into(),
                    ))
                    .unwrap(),
            )
            .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
async fn rejects_oversized_body_and_unknown_fields() {
    for (body, expected) in [
        ("x".repeat(4097), StatusCode::PAYLOAD_TOO_LARGE),
        (
            r#"{"user_agent":"Test/1", "args":["--host","0.0.0.0"]}"#.into(),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let response = gateway()
            .handle(
                Request::post("/v1/sessions")
                    .header(
                        "Authorization",
                        "Bearer test-key-with-at-least-thirty-two-bytes",
                    )
                    .header("Content-Type", "application/json")
                    .body(Body::new(body.into()))
                    .unwrap(),
            )
            .await;
        assert_eq!(response.status(), expected);
    }
}

#[tokio::test]
async fn missing_session_capability_fails_closed() {
    let response = gateway()
        .handle(
            Request::get("/v1/sessions/unknown/cdp")
                .body(Body::new(bytes::Bytes::new()))
                .unwrap(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn startup_failure_does_not_consume_capacity() {
    let gateway = gateway();
    for _ in 0..3 {
        let response = gateway
            .handle(
                Request::post("/v1/sessions")
                    .header(
                        "Authorization",
                        "Bearer test-key-with-at-least-thirty-two-bytes",
                    )
                    .header("Content-Type", "application/json")
                    .body(Body::new(r#"{"user_agent":"Test/1"}"#.into()))
                    .unwrap(),
            )
            .await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}

#[test]
fn rejects_unsafe_gateway_configuration() {
    for field in ["key", "limit", "timeout", "url"] {
        let mut config = config();
        match field {
            "key" => config.api_key = "short".into(),
            "limit" => config.max_sessions = 0,
            "timeout" => config.idle_secs = 0,
            _ => config.public_url = "http://127.0.0.1:9223".into(),
        }
        assert!(Gateway::new(config).is_err(), "accepted invalid {field}");
    }
}
