use camopanda::validate_user_agent;
#[test]
fn accepts_browser_profiles_and_rejects_injection_and_non_ascii() {
    assert!(validate_user_agent("Mozilla/5.0 Safari/604.1").is_ok());
    for value in ["", " ", "bad\r\nInjected: yes", "é", "bad\u{7f}"] {
        assert!(validate_user_agent(value).is_err(), "{value:?}");
    }
}
#[test]
fn enforces_exact_header_length_limit() {
    assert!(validate_user_agent(&"x".repeat(512)).is_ok());
    assert!(validate_user_agent(&"x".repeat(513)).is_err());
}
