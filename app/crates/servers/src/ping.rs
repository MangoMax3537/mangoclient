//! Server List Ping client.

use crate::icons::IconStore;
use serde::Serialize;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const MAX_PACKET: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct PingOptions {
    pub timeout: Duration,
    pub protocol: i32,
}

impl Default for PingOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(9),
            protocol: 767,
        }
    }
}

pub type SrvFuture<'a> = Pin<Box<dyn Future<Output = Option<(String, u16)>> + Send + 'a>>;

/// Resolves `_minecraft._tcp.<host>` SRV records. Used only when the address
/// has no explicit port.
pub trait SrvResolver: Send + Sync {
    fn resolve_srv<'a>(&'a self, host: &'a str) -> SrvFuture<'a>;
}

/// TODO: real SRV lookup needs a DNS resolver dependency (e.g. hickory); until
/// then hosts without a port use 25565.
pub struct NoSrv;

impl SrvResolver for NoSrv {
    fn resolve_srv<'a>(&'a self, _host: &'a str) -> SrvFuture<'a> {
        Box::pin(async { None })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Players {
    pub online: i64,
    pub max: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ServerStatus {
    pub online: bool,
    pub ping_ms: Option<u64>,
    pub motd: String,
    pub players: Option<Players>,
    pub version: String,
    pub protocol: Option<i64>,
    pub favicon: Option<String>,
    pub host: String,
    pub port: u16,
}

impl ServerStatus {
    fn offline(host: String, port: u16) -> Self {
        Self {
            online: false,
            ping_ms: None,
            motd: String::new(),
            players: None,
            version: String::new(),
            protocol: None,
            favicon: None,
            host,
            port,
        }
    }
}

pub fn write_varint(out: &mut Vec<u8>, value: i32) {
    let mut v = value as u32;
    loop {
        let mut byte = (v & 0x7f) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if v == 0 {
            break;
        }
    }
}

async fn read_varint<R: AsyncRead + Unpin>(r: &mut R) -> Option<i32> {
    let mut value: u32 = 0;
    for shift in (0..35).step_by(7) {
        let byte = r.read_u8().await.ok()?;
        value |= u32::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(value as i32);
        }
    }
    None
}

fn packetize(id: i32, payload: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    write_varint(&mut body, id);
    body.extend_from_slice(payload);
    let mut out = Vec::new();
    write_varint(&mut out, body.len() as i32);
    out.extend_from_slice(&body);
    out
}

fn write_string(out: &mut Vec<u8>, s: &str) {
    write_varint(out, s.len() as i32);
    out.extend_from_slice(s.as_bytes());
}

/// Flatten a chat component into plain text.
pub fn flatten_motd(desc: &Value) -> String {
    match desc {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().map(flatten_motd).collect(),
        Value::Object(o) => {
            let mut out = o
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if let Some(Value::Array(extra)) = o.get("extra") {
                out.extend(extra.iter().map(flatten_motd));
            }
            if out.is_empty() {
                if let Some(t) = o.get("translate").and_then(Value::as_str) {
                    out = t.to_string();
                }
            }
            out
        }
        _ => String::new(),
    }
}

/// Remove `§x` formatting codes.
pub fn strip_color_codes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '§'
            && chars.peek().is_some_and(|n| {
                n.is_ascii_alphanumeric() && "0123456789abcdefklmnorABCDEFKLMNOR".contains(*n)
            })
        {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}

fn split_address(address: &str) -> (String, Option<u16>) {
    let address = address.trim();
    if let Some(rest) = address.strip_prefix('[') {
        if let Some((host, tail)) = rest.split_once(']') {
            return (
                host.to_string(),
                tail.strip_prefix(':').and_then(|p| p.parse().ok()),
            );
        }
    }
    if address.matches(':').count() == 1 {
        if let Some((host, port)) = address.split_once(':') {
            return (host.to_string(), port.parse().ok().filter(|&p| p != 0));
        }
    }
    (address.to_string(), None)
}

fn parse_status(json: &str, started: Instant, host: &str, port: u16) -> Option<ServerStatus> {
    let data: Value = serde_json::from_str(json).ok()?;
    let motd = strip_color_codes(&flatten_motd(
        data.get("description").unwrap_or(&Value::Null),
    ))
    .trim()
    .to_string();
    let num = |v: Option<&Value>| v.and_then(Value::as_i64).unwrap_or(0);
    Some(ServerStatus {
        online: true,
        ping_ms: Some(started.elapsed().as_millis() as u64),
        motd,
        players: Some(Players {
            online: num(data.pointer("/players/online")),
            max: num(data.pointer("/players/max")),
        }),
        version: data
            .pointer("/version/name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        protocol: data.pointer("/version/protocol").and_then(Value::as_i64),
        favicon: data
            .get("favicon")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(String::from),
        host: host.to_string(),
        port,
    })
}

async fn exchange(
    host: &str,
    port: u16,
    advertised: &str,
    opts: &PingOptions,
) -> Option<ServerStatus> {
    let started = Instant::now();
    let mut stream = TcpStream::connect((host, port)).await.ok()?;
    let mut hs = Vec::new();
    write_varint(&mut hs, opts.protocol);
    write_string(&mut hs, advertised);
    hs.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut hs, 1);
    let mut out = packetize(0, &hs);
    out.extend_from_slice(&packetize(0, &[]));
    stream.write_all(&out).await.ok()?;

    let len = usize::try_from(read_varint(&mut stream).await?).ok()?;
    if len == 0 || len > MAX_PACKET {
        return None;
    }
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).await.ok()?;
    let mut cursor = body.as_slice();
    if read_varint(&mut cursor).await? != 0 {
        return None;
    }
    let str_len = usize::try_from(read_varint(&mut cursor).await?).ok()?;
    if str_len > cursor.len() {
        return None;
    }
    let json = std::str::from_utf8(&cursor[..str_len]).ok()?;
    parse_status(json, started, host, port)
}

/// Ping a server with an explicit SRV resolver. Any failure yields an
/// offline status.
pub async fn ping_server_with(
    address: &str,
    opts: &PingOptions,
    srv: &dyn SrvResolver,
) -> ServerStatus {
    let (raw_host, raw_port) = split_address(address);
    let (host, port) = match raw_port {
        Some(p) => (raw_host.clone(), p),
        None => srv
            .resolve_srv(&raw_host)
            .await
            .unwrap_or((raw_host.clone(), 25565)),
    };
    match tokio::time::timeout(opts.timeout, exchange(&host, port, &raw_host, opts)).await {
        Ok(Some(status)) => status,
        _ => ServerStatus::offline(host, port),
    }
}

/// Ping without SRV support (see [`NoSrv`]).
pub async fn ping_server(address: &str, opts: &PingOptions) -> ServerStatus {
    ping_server_with(address, opts, &NoSrv).await
}

/// Ping, remember the first favicon, and always report the durable cached one.
pub async fn ping_server_cached(
    address: &str,
    opts: &PingOptions,
    srv: &dyn SrvResolver,
    icons: &dyn IconStore,
) -> ServerStatus {
    let mut status = ping_server_with(address, opts, srv).await;
    if let Some(f) = &status.favicon {
        icons.remember(address, f);
    }
    if let Some(cached) = icons.get(address) {
        status.favicon = Some(cached);
    }
    status
}

/// A server together with its status.
#[derive(Debug, Clone, Serialize)]
pub struct PingedServer {
    pub id: String,
    pub name: String,
    pub address: String,
    pub status: ServerStatus,
}

/// Ping all servers concurrently; `on_result` fires as each one completes.
pub async fn ping_all(
    servers: &[crate::serverlist::FeaturedServer],
    opts: &PingOptions,
    srv: Arc<dyn SrvResolver>,
    icons: Arc<dyn IconStore>,
    mut on_result: impl FnMut(&PingedServer),
) -> Vec<PingedServer> {
    let mut set = tokio::task::JoinSet::new();
    for (i, s) in servers.iter().cloned().enumerate() {
        let (opts, srv, icons) = (opts.clone(), srv.clone(), icons.clone());
        set.spawn(async move {
            let status = ping_server_cached(&s.address, &opts, srv.as_ref(), icons.as_ref()).await;
            (
                i,
                PingedServer {
                    id: s.id,
                    name: s.name,
                    address: s.address,
                    status,
                },
            )
        });
    }
    let mut results: Vec<(usize, PingedServer)> = Vec::new();
    while let Some(joined) = set.join_next().await {
        if let Ok((i, p)) = joined {
            on_result(&p);
            results.push((i, p));
        }
    }
    results.sort_by_key(|(i, _)| *i);
    results.into_iter().map(|(_, p)| p).collect()
}
