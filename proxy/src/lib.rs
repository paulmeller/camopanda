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

#[derive(Clone)]
pub struct Rewriter {
    pub user_agent: String,
}

impl hudsucker::HttpHandler for Rewriter {
    async fn handle_request(
        &mut self,
        _context: &hudsucker::HttpContext,
        mut request: hudsucker::hyper::Request<hudsucker::Body>,
    ) -> hudsucker::RequestOrResponse {
        rewrite_headers(request.headers_mut(), &self.user_agent)
            .expect("user agent must be validated before starting proxy");
        hudsucker::RequestOrResponse::Request(request)
    }
}
