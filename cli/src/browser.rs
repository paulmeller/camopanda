use std::{env, path::PathBuf, process::Stdio, time::Duration};
use tokio::{process::Command, time::timeout};

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub fn executable() -> Result<PathBuf, Error> {
    if let Some(path) = env::var_os("LIGHTPANDA_BIN") {
        return Ok(path.into());
    }
    let bundled = env::current_exe()?.with_file_name("lightpanda");
    Ok(if bundled.is_file() {
        bundled
    } else {
        PathBuf::from("lightpanda")
    })
}
pub async fn verify(path: &std::path::Path) -> Result<(), Error> {
    let result = timeout(
        Duration::from_secs(5),
        Command::new(path)
            .args(["version", "--camopanda-capabilities"])
            .env("LIGHTPANDA_DISABLE_TELEMETRY", "true")
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| "patched Lightpanda capability check timed out")??;
    if !result.status.success() || result.stdout != b"camopanda-header-profiles-v1\n" {
        return Err("Camopanda requires its patched Lightpanda build; install the bundled release or set LIGHTPANDA_BIN to a compatible build".into());
    }
    Ok(())
}
pub fn command(path: &std::path::Path, ua: &str, args: &[String]) -> Command {
    let mut command = Command::new(path);
    command
        .args(args)
        .args([
            "--camopanda-header-profiles",
            "--camopanda-lock-user-agent",
            "--user-agent",
            ua,
        ])
        .env(
            "LIGHTPANDA_DISABLE_TELEMETRY",
            env::var("LIGHTPANDA_DISABLE_TELEMETRY").unwrap_or_else(|_| "true".into()),
        )
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    command
}
