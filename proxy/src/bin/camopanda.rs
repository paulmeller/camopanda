use hudsucker::{
    Proxy,
    certificate_authority::RcgenAuthority,
    hyper::header::HeaderValue,
    rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, Issuer, KeyPair, KeyUsagePurpose},
    rustls::crypto::aws_lc_rs,
};
use lightpanda_hudsucker_proxy::Rewriter;
use std::{env, error::Error, fs, io::Write, path::PathBuf, process::Stdio};
use tokio::{net::TcpListener, process::Command};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn state_dir() -> Result<PathBuf> {
    if let Some(path) = env::var_os("XDG_DATA_HOME") {
        return Ok(PathBuf::from(path).join("camopanda"));
    }
    let home = PathBuf::from(env::var_os("HOME").ok_or("HOME or XDG_DATA_HOME must be set")?);
    Ok(if cfg!(target_os = "macos") {
        home.join("Library/Application Support/camopanda")
    } else {
        home.join(".local/share/camopanda")
    })
}

fn ca(dir: &std::path::Path) -> Result<(String, PathBuf)> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::create_dir_all(dir)?;
    let path = dir.join("ca.pem");
    if !path.exists() {
        let key = KeyPair::generate()?;
        let mut params = CertificateParams::new(Vec::<String>::new())?;
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params
            .distinguished_name
            .push(DnType::CommonName, "Camopanda local proxy CA");
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        let certificate = params.self_signed(&key)?;
        let temp = dir.join(format!("ca-{}.tmp", std::process::id()));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)?;
        let write_result = (|| -> Result<()> {
            write!(file, "{}{}", certificate.pem(), key.serialize_pem())?;
            file.sync_all()?;
            match fs::hard_link(&temp, &path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
                Err(e) => Err(e.into()),
            }
        })();
        fs::remove_file(temp)?;
        write_result?;
    }
    Ok((fs::read_to_string(&path)?, path))
}

async fn shutdown_signal() -> Result<i32> {
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => { result?; Ok(130) }
        _ = terminate.recv() => Ok(143),
    }
}

async fn run() -> Result<i32> {
    let mut args = env::args().skip(1).peekable();
    let mut browser = env::var("LIGHTPANDA_BIN").unwrap_or_else(|_| "lightpanda".into());
    let mut dir = None;
    let mut ua = env::var("UPSTREAM_USER_AGENT").unwrap_or_else(|_| "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36".into());
    while let Some(option) = args.peek() {
        if !matches!(
            option.as_str(),
            "--user-agent" | "--state-dir" | "--lightpanda"
        ) {
            break;
        }
        let option = args.next().unwrap();
        let value = args.next().ok_or("Camopanda option requires a value")?;
        match option.as_str() {
            "--user-agent" => ua = value,
            "--state-dir" => dir = Some(PathBuf::from(value)),
            _ => browser = value,
        }
    }
    let forwarded: Vec<String> = args.collect();
    if forwarded.is_empty() || forwarded == ["--help"] {
        println!(
            "Usage: camopanda [--user-agent STRING] [--state-dir PATH] [--lightpanda PATH] <lightpanda command> [arguments]\nCommands and arguments are passed to Lightpanda. Proxy and CA options are managed by Camopanda."
        );
        return Ok(0);
    }
    for arg in &forwarded {
        if ["--http-proxy", "--ca-cert"]
            .iter()
            .any(|flag| arg == flag || arg.starts_with(&format!("{flag}=")))
        {
            return Err("--http-proxy and --ca-cert are managed by Camopanda".into());
        }
    }
    if matches!(forwarded[0].as_str(), "help" | "version" | "--version") {
        let status = Command::new(browser).args(forwarded).status().await?;
        return Ok(status.code().unwrap_or(1));
    }
    HeaderValue::from_str(&ua)?;
    let (pem, cert_path) = ca(&dir.map(Ok).unwrap_or_else(state_dir)?)?;
    let key_pem = pem
        .get(
            pem.find("-----BEGIN PRIVATE KEY-----")
                .ok_or("CA private key missing")?..,
        )
        .ok_or("CA private key missing")?;
    let key = KeyPair::from_pem(key_pem)?;
    let issuer = Issuer::from_ca_cert_pem(&pem, key)?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let proxy = Proxy::builder()
        .with_listener(listener)
        .with_ca(RcgenAuthority::new(
            issuer,
            1_000,
            aws_lc_rs::default_provider(),
        ))
        .with_rustls_connector(aws_lc_rs::default_provider())
        .with_http_handler(Rewriter { user_agent: ua })
        .build()?;
    let mut proxy_task = tokio::spawn(proxy.start());
    let mut child = Command::new(browser)
        .args(forwarded)
        .args(["--http-proxy", &format!("http://{address}"), "--ca-cert"])
        .arg(cert_path)
        .env(
            "LIGHTPANDA_DISABLE_TELEMETRY",
            env::var("LIGHTPANDA_DISABLE_TELEMETRY").unwrap_or_else(|_| "true".into()),
        )
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()?;
    let outcome = tokio::select! {
        status = child.wait() => status.map(|s| s.code().unwrap_or(1)).map_err(|e| Box::new(e) as Box<dyn Error>),
        signal = shutdown_signal() => signal,
        result = &mut proxy_task => {
            Err(format!("proxy stopped unexpectedly: {result:?}").into())
        }
    };
    if child.id().is_some() {
        let _ = child.kill().await;
    }
    if !proxy_task.is_finished() {
        proxy_task.abort();
        let _ = proxy_task.await;
    }
    outcome
}

#[tokio::main]
async fn main() {
    match run().await {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("camopanda: {error}");
            std::process::exit(1);
        }
    }
}
