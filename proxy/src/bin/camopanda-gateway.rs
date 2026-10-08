use lightpanda_hudsucker_proxy::sessions::{Config, Gateway};
#[tokio::main]
async fn main() {
    let argument = std::env::args().nth(1);
    if argument.as_deref() == Some("--help") {
        println!(
            "Usage: camopanda-gateway\nRequired: SESSION_API_KEY (at least 32 bytes).\nOptional: SESSION_BIND, SESSION_PUBLIC_URL, SESSION_MAX_SESSIONS, SESSION_IDLE_SECS, SESSION_STATE_DIR, CAMOPANDA_BIN.\nEach session starts an isolated Lightpanda process and proxy."
        );
        return;
    }
    if argument.as_deref() == Some("--healthcheck") {
        std::process::exit(if std::net::TcpStream::connect("127.0.0.1:9223").is_ok() {
            0
        } else {
            1
        });
    }
    if argument.is_some() {
        eprintln!("camopanda-gateway: unexpected argument; use --help");
        std::process::exit(1);
    }
    let result = async { Gateway::new(Config::from_env()?)?.serve().await }.await;
    if let Err(error) = result {
        eprintln!("camopanda-gateway: {error}");
        std::process::exit(1);
    }
}
