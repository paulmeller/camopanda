use camopanda::{DEFAULT_USER_AGENT, browser, validate_user_agent};
use std::{env, path::PathBuf};
use tokio::{
    process::Command,
    time::{Duration, timeout},
};

async fn run() -> Result<i32, browser::Error> {
    let mut args = env::args().skip(1).peekable();
    let mut path = browser::executable()?;
    let mut ua = env::var("UPSTREAM_USER_AGENT").unwrap_or_else(|_| DEFAULT_USER_AGENT.into());
    while args
        .peek()
        .is_some_and(|a| matches!(a.as_str(), "--user-agent" | "--lightpanda" | "--state-dir"))
    {
        let option = args.next().unwrap();
        let value = args.next().ok_or("Camopanda option requires a value")?;
        match option.as_str() {
            "--user-agent" => ua = value,
            "--lightpanda" => path = PathBuf::from(value),
            _ => eprintln!(
                "camopanda: --state-dir is deprecated; the direct browser backend creates no certificates"
            ),
        }
    }
    let forwarded: Vec<String> = args.collect();
    if forwarded.is_empty() || forwarded == ["--help"] {
        println!(
            "Usage: camopanda [--user-agent STRING] [--lightpanda PATH] <lightpanda command> [arguments]\nUses the bundled patched Lightpanda directly. --state-dir is accepted for migration and ignored."
        );
        return Ok(0);
    }
    if forwarded == ["--healthcheck"] {
        let address = env::var("CAMOPANDA_HEALTH_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:9222".into())
            .parse()?;
        return Ok(
            if std::net::TcpStream::connect_timeout(&address, Duration::from_secs(2)).is_ok() {
                0
            } else {
                1
            },
        );
    }
    if forwarded == ["--version"] || forwarded == ["version"] {
        println!(
            "camopanda {} (direct Lightpanda header profiles v1)",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(0);
    }
    for arg in &forwarded {
        if [
            "--user-agent",
            "--user-agent-suffix",
            "--camopanda-header-profiles",
            "--camopanda-lock-user-agent",
        ]
        .iter()
        .any(|flag| arg == flag || arg.starts_with(&format!("{flag}=")))
        {
            return Err("profile options are managed by Camopanda; put --user-agent before the browser command".into());
        }
    }
    if forwarded[0] == "help" {
        return Ok(Command::new(path)
            .args(forwarded)
            .status()
            .await?
            .code()
            .unwrap_or(1));
    }
    validate_user_agent(&ua)?;
    browser::verify(&path).await?;
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut child = browser::command(&path, &ua, &forwarded).spawn()?;
    let code = tokio::select! {
        status = child.wait() => return Ok(status?.code().unwrap_or(1)),
        result = tokio::signal::ctrl_c() => { result?; 130 },
        _ = terminate.recv() => 143,
    };
    if let Some(pid) = child.id() {
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }
    }
    if timeout(Duration::from_secs(2), child.wait()).await.is_err() {
        child.kill().await?;
    }
    Ok(code)
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
