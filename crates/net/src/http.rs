use crate::error::NetError;
use reqwest::{Client, Response};
use serde::de::DeserializeOwned;
use std::sync::OnceLock;
use std::time::Duration;

pub const UA: &str = concat!(
    "MangoClient/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/MangoMax3537/mangoclient)"
);

/// Which hosts a request may reach, including every redirect hop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostPolicy {
    /// Exact host names (case-insensitive). `None` = any host (tests).
    hosts: Option<Vec<String>>,
    allow_http: bool,
}

impl HostPolicy {
    /// https only, and only these exact hosts.
    pub fn https_hosts(hosts: &[&str]) -> Self {
        Self {
            hosts: Some(hosts.iter().map(|h| h.to_ascii_lowercase()).collect()),
            allow_http: false,
        }
    }

    /// Any host over http or https. For tests with local mock servers.
    pub fn allow_all() -> Self {
        Self {
            hosts: None,
            allow_http: true,
        }
    }

    /// Allow one more exact host and plain http (tests with a mock server).
    #[must_use]
    pub fn with_http_host(mut self, host: &str) -> Self {
        self.allow_http = true;
        if let Some(h) = &mut self.hosts {
            h.push(host.to_ascii_lowercase());
        }
        self
    }

    pub fn allows_url(&self, url: &reqwest::Url) -> bool {
        let scheme_ok = match url.scheme() {
            "https" => true,
            "http" => self.allow_http,
            _ => false,
        };
        if !scheme_ok || !url.username().is_empty() || url.password().is_some() {
            return false;
        }
        let Some(host) = url.host_str() else {
            return false;
        };
        match &self.hosts {
            None => true,
            Some(list) => list.iter().any(|h| h.eq_ignore_ascii_case(host)),
        }
    }

    pub fn allows(&self, url: &str) -> bool {
        reqwest::Url::parse(url).is_ok_and(|u| self.allows_url(&u))
    }
}

/// Options for [`fetch_with_retry`]. Defaults: 5 attempts, 400ms * 2^n
/// backoff, 60s timeout until response headers arrive.
#[derive(Debug, Clone)]
pub struct FetchOptions {
    /// Retries after the first attempt (4 = 5 attempts total).
    pub retries: u32,
    pub timeout: Duration,
    pub backoff: Duration,
    /// Extra headers; they override the defaults (including User-Agent).
    pub headers: Vec<(String, String)>,
    /// When set, the URL and every redirect hop must satisfy the policy.
    pub host_policy: Option<HostPolicy>,
}

impl Default for FetchOptions {
    fn default() -> Self {
        Self {
            retries: 4,
            timeout: Duration::from_secs(60),
            backoff: Duration::from_millis(400),
            headers: Vec::new(),
            host_policy: None,
        }
    }
}

/// A connection that cannot be opened within this is given up (then retried by the callers that retry).
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
/// No byte for this long ends the transfer with an error instead of hanging forever (a stalled GitHub release
/// download kept the Java stage at 14% on macOS). Large files still take as long as they need while data flows.
const STALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

fn client() -> Result<Client, NetError> {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    if let Some(c) = CLIENT.get() {
        return Ok(c.clone());
    }
    let c = Client::builder()
        .user_agent(UA)
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(STALL_TIMEOUT)
        .build()
        .map_err(NetError::Client)?;
    let _ = CLIENT.set(c.clone());
    Ok(c)
}

/// A client whose redirect policy re-checks every hop against `policy`.
pub fn restricted_client(policy: &HostPolicy) -> Result<Client, NetError> {
    let policy = policy.clone();
    Client::builder()
        .user_agent(UA)
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(STALL_TIMEOUT)
        .redirect(reqwest::redirect::Policy::custom(move |attempt| {
            if attempt.previous().len() >= 10 {
                attempt.error("too many redirects")
            } else if policy.allows_url(attempt.url()) {
                attempt.follow()
            } else {
                attempt.error("redirect to a host that is not allowed")
            }
        }))
        .build()
        .map_err(NetError::Client)
}

async fn attempt(
    client: &Client,
    url: &str,
    opts: &FetchOptions,
    body: Option<&[u8]>,
) -> Result<Response, NetError> {
    let mut req = match body {
        Some(b) => client
            .post(url)
            .header("Content-Type", "application/json")
            .body(b.to_vec()),
        None => client.get(url),
    };
    for (k, v) in &opts.headers {
        req = req.header(k.as_str(), v.as_str());
    }
    let res = match tokio::time::timeout(opts.timeout, req.send()).await {
        Err(_) => return Err(NetError::Timeout { url: url.into() }),
        Ok(Err(source)) => {
            return Err(NetError::Request {
                url: url.into(),
                source,
            })
        }
        Ok(Ok(res)) => res,
    };
    let status = res.status();
    // 5xx and 429 are worth retrying; other non-OK codes are returned as is.
    if status.is_server_error() || status.as_u16() == 429 {
        return Err(NetError::Status {
            status: status.as_u16(),
            url: url.into(),
        });
    }
    Ok(res)
}

/// GET with retry on network errors, timeouts, 5xx and 429. Other non-success
/// statuses (e.g. 404) are returned to the caller without retry.
pub async fn fetch_with_retry(url: &str, opts: &FetchOptions) -> Result<Response, NetError> {
    send_with_retry(url, opts, None).await
}

async fn send_with_retry(
    url: &str,
    opts: &FetchOptions,
    body: Option<&[u8]>,
) -> Result<Response, NetError> {
    let client = match &opts.host_policy {
        Some(policy) => {
            if !policy.allows(url) {
                return Err(NetError::HostNotAllowed { url: url.into() });
            }
            restricted_client(policy)?
        }
        None => client()?,
    };
    let mut n: u32 = 0;
    loop {
        match attempt(&client, url, opts, body).await {
            Ok(res) => return Ok(res),
            Err(err) => {
                if n >= opts.retries {
                    return Err(err);
                }
            }
        }
        tokio::time::sleep(opts.backoff.saturating_mul(1u32 << n.min(16))).await;
        n += 1;
    }
}

async fn ok_response(url: &str, opts: &FetchOptions) -> Result<Response, NetError> {
    let res = fetch_with_retry(url, opts).await?;
    if !res.status().is_success() {
        return Err(NetError::Status {
            status: res.status().as_u16(),
            url: url.into(),
        });
    }
    Ok(res)
}

pub async fn get_json<T: DeserializeOwned>(url: &str, opts: &FetchOptions) -> Result<T, NetError> {
    let bytes = get_bytes(url, opts).await?;
    serde_json::from_slice(&bytes).map_err(|source| NetError::Json {
        url: url.into(),
        source,
    })
}

pub async fn get_bytes(url: &str, opts: &FetchOptions) -> Result<Vec<u8>, NetError> {
    let res = ok_response(url, opts).await?;
    res.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|source| NetError::Request {
            url: url.into(),
            source,
        })
}

/// POST a JSON body (same retry policy as GET) and parse the JSON answer.
/// Only for idempotent lookups: a retry sends the request again.
pub async fn post_json<B: serde::Serialize, T: DeserializeOwned>(
    url: &str,
    body: &B,
    opts: &FetchOptions,
) -> Result<T, NetError> {
    let payload = serde_json::to_vec(body).map_err(|source| NetError::Json {
        url: url.into(),
        source,
    })?;
    let res = send_with_retry(url, opts, Some(&payload)).await?;
    if !res.status().is_success() {
        return Err(NetError::Status {
            status: res.status().as_u16(),
            url: url.into(),
        });
    }
    let bytes = res.bytes().await.map_err(|source| NetError::Request {
        url: url.into(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| NetError::Json {
        url: url.into(),
        source,
    })
}
