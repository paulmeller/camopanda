use hudsucker::{
    Body, HttpContext, HttpHandler, Proxy, RequestOrResponse,
    certificate_authority::RcgenAuthority,
    hyper::{Request, header::HeaderValue},
    rcgen::{Issuer, KeyPair},
    rustls::crypto::aws_lc_rs,
};
use lightpanda_hudsucker_proxy::rewrite_headers;
use std::{
    env,
    error::Error,
    fs,
    net::{SocketAddr, TcpStream},
};

#[derive(Clone)]
struct Rewriter {
    user_agent: String,
}

impl HttpHandler for Rewriter {
    async fn handle_request(
        &mut self,
        _context: &HttpContext,
        mut request: Request<Body>,
    ) -> RequestOrResponse {
        eprintln!("proxy request: {} {}", request.method(), request.uri());
        rewrite_headers(request.headers_mut(), &self.user_agent)
            .expect("validated user agent changed after startup");
        RequestOrResponse::Request(request)
    }
}

fn ca_material() -> Result<(String, String), Box<dyn Error>> {
    let key_path = env::var("CA_KEY_PATH").unwrap_or_else(|_| "/state/hudsucker-ca.key".into());
    let cert_path = env::var("CA_CERT_PATH").unwrap_or_else(|_| "/state/hudsucker-ca.crt".into());
    Ok((
        fs::read_to_string(key_path)?,
        fs::read_to_string(cert_path)?,
    ))
}

fn main() -> Result<(), Box<dyn Error>> {
    if env::args().nth(1).as_deref() == Some("--healthcheck") {
        return TcpStream::connect("127.0.0.1:8080")
            .map(|_| ())
            .map_err(Into::into);
    }

    let user_agent = env::var("UPSTREAM_USER_AGENT").unwrap_or_else(|_| {
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
         (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36"
            .into()
    });
    HeaderValue::from_str(&user_agent)?;

    let (key_pem, cert_pem) = ca_material()?;
    let key_pair = KeyPair::from_pem(&key_pem)?;
    let issuer = Issuer::from_ca_cert_pem(&cert_pem, key_pair)?;
    let authority = RcgenAuthority::new(issuer, 1_000, aws_lc_rs::default_provider());

    let address: SocketAddr = "0.0.0.0:8080".parse()?;
    let proxy = Proxy::builder()
        .with_addr(address)
        .with_ca(authority)
        .with_rustls_connector(aws_lc_rs::default_provider())
        .with_http_handler(Rewriter { user_agent })
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .build()?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move { proxy.start().await })?;
    Ok(())
}
