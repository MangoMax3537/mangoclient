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
}

impl Default for FetchOptions {
    fn default() -> Self {
        Self {
            retries: 4,
            timeout: Duration::from_secs(60),
            backoff: Duration::from_millis(400),
            headers: Vec::new(),
        }
    }
}

fn client() -> Result<Client, NetError> {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    if let Some(c) = CLIENT.get() {
        return Ok(c.clone());
    }
    let c = Client::builder()
        .user_agent(UA)
        .build()
        .map_err(NetError::Client)?;
    let _ = CLIENT.set(c.clone());
    Ok(c)
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
    let client = client()?;
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
