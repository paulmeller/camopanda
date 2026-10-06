use hudsucker::hyper::header::{HeaderMap, HeaderValue, InvalidHeaderValue};

pub fn rewrite_headers(
    headers: &mut HeaderMap,
    user_agent: &str,
) -> Result<(), InvalidHeaderValue> {
    let user_agent = HeaderValue::from_str(user_agent)?;
    headers.insert("user-agent", user_agent);
    let client_hints: Vec<_> = headers
        .keys()
        .filter(|name| name.as_str().starts_with("sec-ch-ua"))
        .cloned()
        .collect();
    for name in client_hints {
        headers.remove(name);
    }
    Ok(())
}
