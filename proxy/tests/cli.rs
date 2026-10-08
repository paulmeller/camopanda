#![cfg(unix)]
use std::{fs, os::unix::fs::PermissionsExt, process::Command};

#[test]
fn forwards_arguments_and_exit_code_and_persists_private_ca() {
    let dir = std::env::temp_dir().join(format!("camopanda-test-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let script = dir.join("lightpanda");
    fs::write(&script, "#!/bin/sh\nprintf '%s\\n' \"$@\"\nexit 7\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_camopanda"))
            .args([
                "--lightpanda",
                script.to_str().unwrap(),
                "--state-dir",
                dir.to_str().unwrap(),
                "--user-agent",
                "Test/1",
                "fetch",
                "--dump",
                "markdown",
                "https://example.com",
            ])
            .output()
            .unwrap()
    };
    let first = run();
    assert_eq!(
        first.status.code(),
        Some(7),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let output = String::from_utf8(first.stdout).unwrap();
    assert!(output.starts_with("fetch\n--dump\nmarkdown\nhttps://example.com\n"));
    assert!(output.contains("--http-proxy\nhttp://127.0.0.1:"));
    assert!(output.contains("--ca-cert\n"));
    let ca = fs::read(dir.join("ca.pem")).unwrap();
    assert_eq!(
        fs::metadata(dir.join("ca.pem"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(run().status.code(), Some(7));
    assert_eq!(fs::read(dir.join("ca.pem")).unwrap(), ca);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn rejects_proxy_override_before_starting_browser() {
    let result = Command::new(env!("CARGO_BIN_EXE_camopanda"))
        .args([
            "fetch",
            "https://example.com",
            "--http-proxy=http://other:8080",
        ])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("managed by Camopanda"));
}

#[test]
fn sigterm_stops_browser_and_closes_proxy_listener() {
    use std::{net::TcpStream, thread, time::Duration};
    let dir = std::env::temp_dir().join(format!("camopanda-signal-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let script = dir.join("lightpanda");
    fs::write(&script, "#!/bin/sh\necho $$ > \"$TEST_DIR/pid\"\nprintf '%s\\n' \"$@\" > \"$TEST_DIR/args\"\nexec sleep 60\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
    let mut process = Command::new(env!("CARGO_BIN_EXE_camopanda"))
        .env("TEST_DIR", &dir)
        .args([
            "--lightpanda",
            script.to_str().unwrap(),
            "--state-dir",
            dir.to_str().unwrap(),
            "serve",
        ])
        .spawn()
        .unwrap();
    for _ in 0..100 {
        if dir.join("args").exists() {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    let args = fs::read_to_string(dir.join("args")).unwrap();
    let address = args
        .lines()
        .find_map(|s| s.strip_prefix("http://"))
        .unwrap();
    assert!(TcpStream::connect(address).is_ok());
    let pid = fs::read_to_string(dir.join("pid")).unwrap();
    assert!(
        Command::new("kill")
            .args(["-TERM", &process.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    for _ in 0..100 {
        if process.try_wait().unwrap().is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    let status = process.try_wait().unwrap();
    if status.is_none() {
        process.kill().unwrap();
    }
    assert_eq!(status.unwrap().code(), Some(143));
    assert!(
        !Command::new("kill")
            .args(["-0", pid.trim()])
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success()
    );
    assert!(TcpStream::connect(address).is_err());
    fs::remove_dir_all(dir).unwrap();
}
