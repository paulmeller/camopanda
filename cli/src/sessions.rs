//! Authenticated session API with process-isolated browser profiles.
use crate::{Body, browser, validate_user_agent};
use futures_util::{SinkExt, StreamExt};
use http_body_util::{BodyExt, Limited};
use hyper::{Method, Request, Response, StatusCode, header::HeaderValue};
use hyper_util::rt::TokioIo;
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::HashMap,
    convert::Infallible,
    env,
    path::PathBuf,
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    net::{TcpListener, TcpStream},
    process::Child,
    sync::{Mutex as AsyncMutex, watch},
    time::{sleep, timeout},
};
use tokio_tungstenite::{connect_async_with_config, tungstenite::protocol::WebSocketConfig};
use uuid::Uuid;

type Error = Box<dyn std::error::Error + Send + Sync>;

pub struct Config {
    pub bind: std::net::SocketAddr,
    pub public_url: String,
    pub api_key: String,
    pub max_sessions: usize,
    pub idle_secs: u64,
    pub browser: PathBuf,
}
impl Config {
    pub fn from_env() -> Result<Self, Error> {
        fn number<T: std::str::FromStr>(name: &str, default: &str) -> Result<T, Error> {
            env::var(name)
                .unwrap_or_else(|_| default.into())
                .parse()
                .map_err(|_| format!("invalid {name}").into())
        }
        Ok(Self {
            bind: number("SESSION_BIND", "127.0.0.1:9223")?,
            public_url: env::var("SESSION_PUBLIC_URL")
                .unwrap_or_else(|_| "ws://127.0.0.1:9223".into()),
            api_key: env::var("SESSION_API_KEY").unwrap_or_default(),
            max_sessions: number("SESSION_MAX_SESSIONS", "4")?,
            idle_secs: number("SESSION_IDLE_SECS", "300")?,
            browser: browser::executable()?,
        })
    }
}
struct Session {
    id: String,
    token: String,
    user_agent: String,
    port: u16,
    child: AsyncMutex<Child>,
    process_group: u32,
    last_activity: Mutex<Instant>,
    active: AtomicBool,
    ready: AtomicBool,
    stop: watch::Sender<bool>,
}
impl Session {
    fn touch(&self) {
        *self.last_activity.lock().unwrap() = Instant::now();
    }
    async fn close(&self) {
        self.stop.send_replace(true);
        let mut child = self.child.lock().await;
        let pid = self.process_group;
        // Descendants inherit this private group, including when the wrapper has
        // already exited. The group id is captured before any child can be reaped.
        unsafe {
            libc::kill(-(pid as i32), libc::SIGTERM);
        }
        let exited = timeout(Duration::from_secs(2), child.wait()).await;
        // Reap descendants even if the wrapper crashed before running its cleanup.
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
        if !matches!(exited, Ok(Ok(_))) {
            let _ = child.wait().await;
        }
    }
}
#[derive(Clone)]
pub struct Gateway {
    config: Arc<Config>,
    sessions: Arc<Mutex<HashMap<String, Arc<Session>>>>,
    stopping: Arc<AtomicBool>,
}
struct Active(Arc<Session>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.active.store(false, Ordering::SeqCst);
        self.0.touch();
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Create {
    user_agent: String,
}
fn response(status: StatusCode, value: serde_json::Value) -> Response<Body> {
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .header("Cache-Control", "no-store")
        .body(Body::new(value.to_string().into()))
        .unwrap()
}
fn error(status: StatusCode, message: &str) -> Response<Body> {
    response(status, json!({"error":message}))
}
fn secret_eq(actual: &str, expected: &str) -> bool {
    actual.len() == expected.len()
        && actual
            .bytes()
            .zip(expected.bytes())
            .fold(0u8, |diff, (a, b)| diff | (a ^ b))
            == 0
}
impl Gateway {
    pub fn new(config: Config) -> Result<Self, Error> {
        if config.api_key.len() < 32 || HeaderValue::from_str(&config.api_key).is_err() {
            return Err("SESSION_API_KEY must be a valid header value of at least 32 bytes".into());
        }
        if !(1..=32).contains(&config.max_sessions) || !(1..=86400).contains(&config.idle_secs) {
            return Err("session limit must be 1..32 and idle seconds 1..86400".into());
        }
        let uri: hyper::Uri = config.public_url.parse()?;
        if !matches!(uri.scheme_str(), Some("ws" | "wss"))
            || uri.authority().is_none()
            || uri.authority().unwrap().as_str().contains('@')
            || uri.query().is_some()
            || !matches!(uri.path(), "" | "/")
        {
            return Err("SESSION_PUBLIC_URL must be a ws:// or wss:// origin".into());
        }
        Ok(Self {
            config: Arc::new(config),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            stopping: Arc::new(AtomicBool::new(false)),
        })
    }
    fn authorized<B>(&self, request: &Request<B>) -> bool {
        request
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .is_some_and(|key| secret_eq(key, &self.config.api_key))
    }
    async fn remove(&self, id: &str) -> bool {
        let session = self.sessions.lock().unwrap().remove(id);
        if let Some(session) = session {
            session.close().await;
            true
        } else {
            false
        }
    }
    async fn create(&self, user_agent: String) -> Result<Arc<Session>, StatusCode> {
        browser::verify(&self.config.browser)
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let reservation = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let port = reservation
            .local_addr()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
            .port();
        let session = {
            let mut sessions = self.sessions.lock().unwrap();
            if self.stopping.load(Ordering::SeqCst) {
                return Err(StatusCode::SERVICE_UNAVAILABLE);
            }
            if sessions.len() >= self.config.max_sessions {
                return Err(StatusCode::TOO_MANY_REQUESTS);
            }
            drop(reservation);
            let child = browser::command(
                &self.config.browser,
                &user_agent,
                &[
                    "serve".into(),
                    "--host".into(),
                    "127.0.0.1".into(),
                    "--port".into(),
                    port.to_string(),
                ],
            )
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
            let (stop, _) = watch::channel(false);
            let session = Arc::new(Session {
                id: Uuid::new_v4().simple().to_string(),
                token: Uuid::new_v4().simple().to_string(),
                user_agent,
                port,
                process_group: child.id().expect("new child must have a pid"),
                child: AsyncMutex::new(child),
                last_activity: Mutex::new(Instant::now()),
                active: AtomicBool::new(false),
                ready: AtomicBool::new(false),
                stop,
            });
            sessions.insert(session.id.clone(), session.clone());
            session
        };
        let mut ready = false;
        for _ in 0..100 {
            if self.stopping.load(Ordering::SeqCst) || *session.stop.borrow() {
                break;
            }
            let alive = session
                .child
                .lock()
                .await
                .try_wait()
                .is_ok_and(|status| status.is_none());
            if !alive {
                break;
            }
            if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
                ready = true;
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
        if !ready {
            self.remove(&session.id).await;
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        session.ready.store(true, Ordering::SeqCst);
        session.touch();
        Ok(session)
    }
    pub async fn handle<B>(&self, mut request: Request<B>) -> Response<Body>
    where
        B: hyper::body::Body<Data = bytes::Bytes> + Send,
        B::Error: Into<browser::Error>,
    {
        let path = request.uri().path().to_string();
        if path == "/health" && request.method() == Method::GET {
            return response(StatusCode::OK, json!({"status":"ok"}));
        }
        if path == "/v1/sessions" && request.method() == Method::POST {
            if !self.authorized(&request) {
                return error(StatusCode::UNAUTHORIZED, "unauthorized");
            }
            if request
                .headers()
                .get("content-type")
                .and_then(|h| h.to_str().ok())
                .is_none_or(|v| v.split(';').next() != Some("application/json"))
            {
                return error(
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "application/json required",
                );
            }
            let body = match timeout(
                Duration::from_secs(5),
                Limited::new(request.into_body(), 4096).collect(),
            )
            .await
            {
                Ok(Ok(body)) => body.to_bytes(),
                Ok(Err(_)) => {
                    return error(StatusCode::PAYLOAD_TOO_LARGE, "request body limit exceeded");
                }
                Err(_) => return error(StatusCode::REQUEST_TIMEOUT, "request body timed out"),
            };
            let input: Create = match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(_) => return error(StatusCode::BAD_REQUEST, "invalid session request"),
            };
            if let Err(message) = validate_user_agent(&input.user_agent) {
                return error(StatusCode::BAD_REQUEST, message);
            }
            return match self.create(input.user_agent).await {
                Ok(session) => response(
                    StatusCode::CREATED,
                    json!({"id":session.id,"user_agent":session.user_agent,
                    "cdp_url":format!("{}/v1/sessions/{}/cdp?token={}",self.config.public_url.trim_end_matches('/'),session.id,session.token),
                    "idle_timeout_secs":self.config.idle_secs}),
                ),
                Err(status) => error(
                    status,
                    if status == StatusCode::TOO_MANY_REQUESTS {
                        "session capacity reached"
                    } else {
                        "browser startup failed"
                    },
                ),
            };
        }
        let Some(rest) = path.strip_prefix("/v1/sessions/") else {
            return error(StatusCode::NOT_FOUND, "not found");
        };
        let (id, cdp) = rest
            .strip_suffix("/cdp")
            .map_or((rest, false), |id| (id, true));
        if id.is_empty() || id.contains('/') {
            return error(StatusCode::NOT_FOUND, "not found");
        }
        if !cdp && !self.authorized(&request) {
            return error(StatusCode::UNAUTHORIZED, "unauthorized");
        }
        let session = self.sessions.lock().unwrap().get(id).cloned();
        let Some(session) = session else {
            return error(StatusCode::NOT_FOUND, "session not found");
        };
        if !cdp {
            if request.method() == Method::DELETE {
                self.remove(id).await;
                return Response::builder()
                    .status(StatusCode::NO_CONTENT)
                    .header("Cache-Control", "no-store")
                    .body(Body::new(bytes::Bytes::new()))
                    .unwrap();
            }
            if request.method() == Method::GET {
                return response(
                    StatusCode::OK,
                    json!({"id":id,"user_agent":session.user_agent,"connected":session.active.load(Ordering::SeqCst)}),
                );
            }
            return error(StatusCode::METHOD_NOT_ALLOWED, "method not allowed");
        }
        let token = request
            .uri()
            .query()
            .and_then(|v| v.strip_prefix("token="))
            .unwrap_or("");
        if !secret_eq(token, &session.token) {
            return error(StatusCode::NOT_FOUND, "session not found");
        }
        if request.method() != Method::GET || !hyper_tungstenite::is_upgrade_request(&request) {
            return error(StatusCode::BAD_REQUEST, "WebSocket upgrade required");
        }
        if !session.ready.load(Ordering::SeqCst) || *session.stop.borrow() {
            return error(StatusCode::SERVICE_UNAVAILABLE, "session unavailable");
        }
        if session
            .active
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return error(StatusCode::CONFLICT, "session already connected");
        }
        let active = Active(session.clone());
        let (response, websocket) = match hyper_tungstenite::upgrade(
            &mut request,
            Some(
                WebSocketConfig::default()
                    .max_message_size(Some(16 * 1024 * 1024))
                    .max_frame_size(Some(16 * 1024 * 1024)),
            ),
        ) {
            Ok(v) => v,
            Err(_) => return error(StatusCode::BAD_REQUEST, "invalid WebSocket upgrade"),
        };
        let upstream = match timeout(
            Duration::from_secs(5),
            connect_async_with_config(
                format!("ws://127.0.0.1:{}/", session.port),
                Some(
                    WebSocketConfig::default()
                        .max_message_size(Some(16 * 1024 * 1024))
                        .max_frame_size(Some(16 * 1024 * 1024)),
                ),
                false,
            ),
        )
        .await
        {
            Ok(Ok((socket, _))) => socket,
            _ => return error(StatusCode::BAD_GATEWAY, "browser connection failed"),
        };
        let mut stopped = session.stop.subscribe();
        session.touch();
        tokio::spawn(async move {
            let _active = active;
            let Ok(mut client) = websocket.await else {
                return;
            };
            let mut upstream = upstream;
            if *stopped.borrow() {
                return;
            }
            loop {
                tokio::select! {
                    _ = stopped.changed() => break,
                    message = client.next() => {
                        let Some(Ok(message)) = message else { break; };
                        session.touch();
                        if !matches!(timeout(Duration::from_secs(5), upstream.send(message)).await, Ok(Ok(()))) { break; }
                    }
                    message = upstream.next() => {
                        let Some(Ok(message)) = message else { break; };
                        session.touch();
                        if !matches!(timeout(Duration::from_secs(5), client.send(message)).await, Ok(Ok(()))) { break; }
                    }
                }
            }
        });
        response
    }
    async fn sweep(&self) {
        let sessions: Vec<_> = self.sessions.lock().unwrap().values().cloned().collect();
        for session in sessions {
            let dead = session
                .child
                .lock()
                .await
                .try_wait()
                .is_ok_and(|s| s.is_some());
            let retired = {
                let mut sessions = self.sessions.lock().unwrap();
                let activity = session.last_activity.lock().unwrap();
                if dead || activity.elapsed() >= Duration::from_secs(self.config.idle_secs) {
                    sessions.remove(&session.id)
                } else {
                    None
                }
            };
            if let Some(retired) = retired {
                retired.close().await;
            }
        }
    }
    pub async fn serve(self) -> Result<(), Error> {
        let listener = TcpListener::bind(self.config.bind).await?;
        let sweep_gateway = self.clone();
        let sweeper = tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(1)).await;
                sweep_gateway.sweep().await;
            }
        });
        let connections = Arc::new(tokio::sync::Semaphore::new(64));
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => break,
                _ = terminate.recv() => break,
                accepted = listener.accept() => {
                    let (stream,_) = accepted?;
                    let Ok(permit) = connections.clone().try_acquire_owned() else { continue; };
                    let gateway = self.clone();
                    tokio::spawn(async move {
                        let _permit = permit;
                        let service = hyper::service::service_fn(move |request| {
                            let gateway = gateway.clone();
                            async move { Ok::<_,Infallible>(gateway.handle(request).await) }
                        });
                        let _ = hyper::server::conn::http1::Builder::new()
                            .timer(hyper_util::rt::TokioTimer::new())
                            .header_read_timeout(Duration::from_secs(10))
                            .serve_connection(TokioIo::new(stream), service).with_upgrades().await;
                    });
                }
            }
        }
        self.stopping.store(true, Ordering::SeqCst);
        sweeper.abort();
        let sessions: Vec<_> = self
            .sessions
            .lock()
            .unwrap()
            .drain()
            .map(|(_, s)| s)
            .collect();
        futures_util::future::join_all(sessions.iter().map(|session| session.close())).await;
        Ok(())
    }
}
