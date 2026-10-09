pub mod browser;
pub mod sessions;
pub type Body = http_body_util::Full<bytes::Bytes>;
pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36";

pub fn validate_user_agent(value: &str) -> Result<(), &'static str> {
    if value.trim().is_empty()
        || value.len() > 512
        || !value.bytes().all(|c| (32..=126).contains(&c))
    {
        Err("user_agent must be nonempty printable ASCII of at most 512 bytes")
    } else {
        Ok(())
    }
}
