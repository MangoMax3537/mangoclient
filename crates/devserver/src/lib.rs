//! `mango-dev`: a localhost HTTP bridge to one [`AppService`], so the UI can
//! be developed and tested in a plain browser against the real Rust backend
//! instead of a mock. It is a development tool: never bundled, never a
//! dependency of anything else.
//!
//! Routes (all need the bearer token, `Authorization: Bearer <t>` or `?token=<t>`):
//! * `POST /invoke/<command>`: JSON body = the camelCase argument object
//!   (empty body = no arguments). `200` + the JSON result, or an `AppError`
//!   JSON body with status `404` (`unknown_command`), `400` (`bad_args`,
//!   `bad_json`, `validation`) or `500` (everything else; rely on the body).
//! * `GET /events`: Server-Sent Events. Every service event arrives as one
//!   message `data: {"name":"launch:state","payload":{...}}`.
//! * `GET /img/<kind>/<rest>`: the `mangoimg://<kind>/<rest>` image protocol.
//! * `GET /commands`: the command names.
//!
//! Security: binds 127.0.0.1 only, checks the `Host` header (DNS rebinding),
//! CORS and `Origin` are limited to the explicit allow-list, bodies are capped
//! at 1 MB, and the service keeps its own file-system policy.

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Path, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use mango_app::image_protocol::Image;
use mango_app::{dispatch, AppError, AppService, Event, EventSink, Host, ServiceOptions};
use mango_core::Paths;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, oneshot};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

/// Request bodies above this are refused with `413`.
pub const BODY_LIMIT: usize = 1024 * 1024;

pub struct ServerConfig {
    /// The data dir of the service (a throw-away dir for tests).
    pub home: std::path::PathBuf,
    /// Exact origins (`http://127.0.0.1:5173`) allowed to call the server.
    pub allow_origins: Vec<String>,
    /// `None` = a random token.
    pub token: Option<String>,
    /// `0` = a random free port.
    pub port: u16,
    pub options: ServiceOptions,
    /// `None` = the system host (real keychain vault, no folder opening).
    pub host: Option<Host>,
    /// Also run the service's background work (telemetry, update checks, mods
    /// watcher). Off by default: a dev server must not phone home.
    pub background: bool,
}

pub struct RunningServer {
    pub addr: SocketAddr,
    pub token: String,
    pub service: AppService,
    shutdown: Option<oneshot::Sender<()>>,
    join: tokio::task::JoinHandle<()>,
}

impl RunningServer {
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Stop accepting connections and wait for the server task.
    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        let _ = (&mut self.join).await;
    }
}

/// Fans service events out to every SSE connection.
struct BroadcastSink(broadcast::Sender<Value>);

impl EventSink for BroadcastSink {
    fn emit(&self, event: Event) {
        // No subscriber is fine: the event is simply not observed.
        let _ = self
            .0
            .send(json!({"name": event.name(), "payload": event.payload()}));
    }
}

#[derive(Clone)]
struct Shared {
    service: AppService,
    token: Arc<String>,
    origins: Arc<Vec<String>>,
    events: broadcast::Sender<Value>,
    port: u16,
}

/// Why the config was refused.
pub fn validate_origin(origin: &str) -> Result<String, String> {
    let o = origin.trim().trim_end_matches('/');
    let rest = o
        .strip_prefix("http://")
        .or_else(|| o.strip_prefix("https://"))
        .ok_or_else(|| format!("origin `{origin}` must start with http:// or https://"))?;
    if rest.is_empty() || rest.contains(['/', '?', '#', '@', ' ']) {
        return Err(format!(
            "origin `{origin}` must be scheme://host[:port] only"
        ));
    }
    Ok(o.to_string())
}

fn random_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

/// Start the server on `127.0.0.1:<port>`.
pub async fn start(cfg: ServerConfig) -> Result<RunningServer, String> {
    let origins = cfg
        .allow_origins
        .iter()
        .map(|o| validate_origin(o))
        .collect::<Result<Vec<_>, _>>()?;
    let (tx, _) = broadcast::channel(1024);
    let host = cfg.host.unwrap_or_else(|| Host::system(false));
    let service = AppService::with_host(
        Paths::new(&cfg.home),
        Arc::new(BroadcastSink(tx.clone())),
        cfg.options,
        host,
    )
    .map_err(|e| format!("could not open the data dir: {e}"))?;
    if cfg.background {
        service.start_background();
    }
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", cfg.port))
        .await
        .map_err(|e| format!("could not bind 127.0.0.1:{}: {e}", cfg.port))?;
    let addr = listener.local_addr().map_err(|e| e.to_string())?;
    let token = cfg.token.unwrap_or_else(random_token);
    let shared = Shared {
        service: service.clone(),
        token: Arc::new(token.clone()),
        origins: Arc::new(origins),
        events: tx,
        port: addr.port(),
    };
    let app = router(shared);
    let (stop_tx, stop_rx) = oneshot::channel::<()>();
    let join = tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = stop_rx.await;
            })
            .await;
    });
    Ok(RunningServer {
        addr,
        token,
        service,
        shutdown: Some(stop_tx),
        join,
    })
}

fn router(shared: Shared) -> Router {
    Router::new()
        .route("/invoke/{command}", post(invoke))
        .route("/events", get(events))
        .route("/img/{kind}/{*rest}", get(image))
        .route("/commands", get(commands))
        .fallback(not_found)
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .layer(middleware::from_fn_with_state(shared.clone(), guard))
        .with_state(shared)
}

fn error_response(status: StatusCode, code: &str, message: &str) -> Response {
    let err = AppError::new(code, message);
    (status, Json(err)).into_response()
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut diff = a.len() ^ b.len();
    for i in 0..a.len().max(b.len()) {
        diff |= usize::from(a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0));
    }
    diff == 0
}

fn query_token(query: Option<&str>) -> Option<String> {
    query?
        .split('&')
        .find_map(|kv| kv.strip_prefix("token="))
        .map(str::to_string)
}

fn host_allowed(headers: &HeaderMap, port: u16) -> bool {
    let Some(host) = headers.get(header::HOST).and_then(|h| h.to_str().ok()) else {
        return false;
    };
    [
        format!("127.0.0.1:{port}"),
        format!("localhost:{port}"),
        format!("[::1]:{port}"),
    ]
    .iter()
    .any(|h| h.eq_ignore_ascii_case(host))
}

fn cors_headers(res: &mut Response, origin: &str) {
    let h = res.headers_mut();
    if let Ok(v) = HeaderValue::from_str(origin) {
        h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, v);
    }
    h.insert(header::VARY, HeaderValue::from_static("Origin"));
}

/// Host check, CORS, token. Runs before routing, so unknown paths are guarded too.
async fn guard(State(s): State<Shared>, req: Request, next: Next) -> Response {
    if !host_allowed(req.headers(), s.port) {
        return error_response(
            StatusCode::FORBIDDEN,
            "host_forbidden",
            "Unexpected Host header",
        );
    }
    let origin = req
        .headers()
        .get(header::ORIGIN)
        .and_then(|o| o.to_str().ok())
        .map(str::to_string);
    if let Some(o) = &origin {
        if !s.origins.iter().any(|a| a == o) {
            return error_response(
                StatusCode::FORBIDDEN,
                "origin_forbidden",
                "Origin is not in --allow-origin",
            );
        }
    }
    // Preflight carries no credentials and no data; answer it for allowed origins only.
    if req.method() == Method::OPTIONS {
        let Some(o) = origin else {
            return error_response(StatusCode::FORBIDDEN, "origin_forbidden", "Missing Origin");
        };
        let mut res = StatusCode::NO_CONTENT.into_response();
        cors_headers(&mut res, &o);
        let h = res.headers_mut();
        h.insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static("GET, POST, OPTIONS"),
        );
        h.insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static("authorization, content-type"),
        );
        h.insert(
            header::ACCESS_CONTROL_MAX_AGE,
            HeaderValue::from_static("600"),
        );
        return res;
    }
    let supplied = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(str::to_string)
        .or_else(|| query_token(req.uri().query()));
    if !supplied.is_some_and(|t| constant_time_eq(&t, &s.token)) {
        let mut res = error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Missing or wrong bearer token",
        );
        if let Some(o) = &origin {
            cors_headers(&mut res, o);
        }
        return res;
    }
    let mut res = next.run(req).await;
    if let Some(o) = &origin {
        cors_headers(&mut res, o);
    }
    res
}

async fn not_found() -> Response {
    error_response(StatusCode::NOT_FOUND, "not_found", "No such route")
}

fn status_for(e: &AppError) -> StatusCode {
    match e.code.as_str() {
        "unknown_command" => StatusCode::NOT_FOUND,
        "bad_args" | "bad_json" | "validation" => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn invoke(
    State(s): State<Shared>,
    Path(command): Path<String>,
    body: axum::body::Bytes,
) -> Response {
    let args = if body.iter().all(u8::is_ascii_whitespace) {
        Value::Null
    } else {
        match serde_json::from_slice::<Value>(&body) {
            Ok(v) => v,
            Err(e) => {
                return error_response(
                    StatusCode::BAD_REQUEST,
                    "bad_json",
                    &format!("Body is not valid JSON: {e}"),
                )
            }
        }
    };
    match dispatch::invoke(&s.service, &command, args).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => (status_for(&e), Json(e)).into_response(),
    }
}

async fn events(State(s): State<Shared>) -> impl IntoResponse {
    let stream = BroadcastStream::new(s.events.subscribe()).filter_map(|m| match m {
        Ok(v) => Some(Ok::<_, std::convert::Infallible>(
            SseEvent::default().data(v.to_string()),
        )),
        // A slow reader loses the oldest events; the stream itself goes on.
        Err(_) => None,
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

async fn commands() -> Json<Vec<&'static str>> {
    Json(dispatch::COMMANDS.to_vec())
}

async fn image(State(s): State<Shared>, Path((kind, rest)): Path<(String, String)>) -> Response {
    let url = format!("mangoimg://{kind}/{rest}");
    let (data, mime) = match s.service.image(&url).await {
        Ok(Image::Bytes { data, mime }) => (data, mime),
        Ok(Image::File { path, mime }) => match tokio::fs::read(path).await {
            Ok(data) => (data, mime),
            Err(_) => return StatusCode::NOT_FOUND.into_response(),
        },
        Err(e) => {
            return StatusCode::from_u16(e.status())
                .unwrap_or(StatusCode::NOT_FOUND)
                .into_response()
        }
    };
    let mut res = Response::new(Body::from(data));
    let h = res.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    h.insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("cross-origin"),
    );
    res
}
