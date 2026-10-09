#![cfg(unix)]
use std::{fs, os::unix::fs::PermissionsExt, process::Command};
fn fixture(name: &str, body: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("camopanda-{name}-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("lightpanda");
    fs::write(&path,format!("#!/bin/sh\nif [ \"$1\" = version ]; then echo camopanda-header-profiles-v1; exit 0; fi\n{body}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    (dir, path)
}
#[test]
fn forwards_arguments_exit_code_and_creates_no_ca() {
    let (dir, path) = fixture("forward", "printf '%s\\n' \"$@\"\nexit 7");
    let state = dir.join("old-state");
    let output = Command::new(env!("CARGO_BIN_EXE_camopanda"))
        .args([
            "--lightpanda",
            path.to_str().unwrap(),
            "--state-dir",
            state.to_str().unwrap(),
            "--user-agent",
            "Test/1",
            "fetch",
            "--dump",
            "markdown",
            "https://example.com",
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(7),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "fetch\n--dump\nmarkdown\nhttps://example.com\n--camopanda-header-profiles\n--camopanda-lock-user-agent\n--user-agent\nTest/1\n"
    );
    assert!(
        !state.exists(),
        "direct backend must not create certificate storage"
    );
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn rejects_stock_browser_instead_of_silently_using_wrong_headers() {
    let (dir, path) = fixture("stock", "exit 0");
    fs::write(&path, "#!/bin/sh\necho Lightpanda/1.0\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_camopanda"))
        .args([
            "--lightpanda",
            path.to_str().unwrap(),
            "fetch",
            "https://example.com",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("requires its patched Lightpanda"));
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn rejects_profile_override_before_starting_browser() {
    let output = Command::new(env!("CARGO_BIN_EXE_camopanda"))
        .args(["fetch", "https://example.com", "--user-agent=forged"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("managed by Camopanda"));
}
#[test]
fn version_reports_camopanda_without_requiring_browser() {
    let output = Command::new(env!("CARGO_BIN_EXE_camopanda"))
        .args(["--lightpanda", "missing-browser", "--version"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("camopanda 0.2.0"));
}
#[test]
fn sigterm_and_sigint_reap_browser_and_preserve_signal_exit_codes() {
    use std::{thread, time::Duration};
    for (signal, code) in [("-TERM", 143), ("-INT", 130)] {
        let (dir, path) = fixture(signal, "echo $$ > \"$TEST_DIR/pid\"\nexec sleep 60");
        let mut child = Command::new(env!("CARGO_BIN_EXE_camopanda"))
            .env("TEST_DIR", &dir)
            .args(["--lightpanda", path.to_str().unwrap(), "serve"])
            .spawn()
            .unwrap();
        for _ in 0..100 {
            if dir.join("pid").exists() {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        let pid = fs::read_to_string(dir.join("pid")).unwrap();
        assert!(
            Command::new("kill")
                .args([signal, &child.id().to_string()])
                .status()
                .unwrap()
                .success()
        );
        for _ in 0..100 {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        let status = child.try_wait().unwrap();
        if status.is_none() {
            child.kill().unwrap();
        }
        assert_eq!(status.unwrap().code(), Some(code));
        assert!(
            !Command::new("kill")
                .args(["-0", pid.trim()])
                .stderr(std::process::Stdio::null())
                .status()
                .unwrap()
                .success()
        );
        fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn healthcheck_detects_listener_without_launching_browser() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_camopanda"))
            .env("CAMOPANDA_HEALTH_ADDR", &address)
            .args(["--lightpanda", "missing-browser", "--healthcheck"])
            .status()
            .unwrap()
    };
    assert!(run().success());
    drop(listener);
    assert_eq!(run().code(), Some(1));
}
