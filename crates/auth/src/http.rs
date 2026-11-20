use crate::endpoints::Endpoints;
use crate::error::AuthError;
use mango_net::UA;
use reqwest::{Client, RequestBuilder};
use serde_json::{Map, Value};
use std::sync::OnceLock;
use std::time::Duration;

/// One pooled client for every auth and skin request. Only the connect
/// timeout lives here; the total time of a request is `Endpoints::request_timeout`
/// (per request, so tests can shorten it).
fn client() -> Result<Client, AuthError> {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    if let Some(c) = CLIENT.get() {
        return Ok(c.clone());
    }
    let c = Client::builder()
        .user_agent(UA)
        .connect_timeout(Duration::from_secs(5))
        .pool_idle_timeout(Duration::from_secs(60))
        .build()?;
    let _ = CLIENT.set(c.clone());
    Ok(c)
}

/// `Retry-After` as whole seconds (the HTTP-date form is not used by Mojang's
/// services and counts as "unknown").
fn retry_after_secs(res: &reqwest::Response) -> Option<u64> {
    res.headers()
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn map_send_error(e: reqwest::Error) -> AuthError {
    if e.is_timeout() {
        AuthError::Timeout
    } else {
        AuthError::Request(e)
    }
}

/// Send with a bounded total time per attempt, retry (with backoff) on
/// network errors, timeouts and 5xx, and on 429 honoring `Retry-After`
/// (waits at most `Endpoints::retry_after_cap`; a longer demand returns
/// [`AuthError::RateLimited`] at once so the caller can show it). The last
/// response is returned as is. Nothing here logs URLs, headers or bodies.
pub(crate) async fn send_raw(
    ep: &Endpoints,
    build: impl Fn(&Client) -> RequestBuilder,
) -> Result<(u16, Vec<u8>), AuthError> {
    let client = client()?;
    let started = std::time::Instant::now();
    let mut n: u32 = 0;
    loop {
        let sent = build(&client).timeout(ep.request_timeout).send().await;
        let wait = match sent {
            Err(e) => {
                let e = map_send_error(e);
                if n >= ep.retries {
                    tracing::debug!(
                        attempts = n + 1,
                        ms = elapsed_ms(started),
                        "request failed: {}",
                        e.code()
                    );
                    return Err(e);
                }
                ep.backoff.saturating_mul(1u32 << n.min(16))
            }
            Ok(res) => {
                let status = res.status().as_u16();
                if status == 429 {
                    let asked = retry_after_secs(&res);
                    let secs = asked.unwrap_or_else(|| ep.backoff.as_secs().max(1));
                    let wait = Duration::from_secs(secs);
                    if wait > ep.retry_after_cap || n >= ep.retries {
                        tracing::debug!(
                            attempts = n + 1,
                            ms = elapsed_ms(started),
                            "rate limited, retry after {secs} s"
                        );
                        return Err(AuthError::RateLimited(secs));
                    }
                    // A server that sends 0 still gets a short pause.
                    wait.max(Duration::from_millis(50))
                } else if res.status().is_server_error() && n < ep.retries {
                    ep.backoff.saturating_mul(1u32 << n.min(16))
                } else {
                    let bytes = tokio::time::timeout(ep.request_timeout, res.bytes())
                        .await
                        .map_err(|_| AuthError::Timeout)?
                        .map_err(map_send_error)?;
                    tracing::debug!(
                        status,
                        attempts = n + 1,
                        ms = elapsed_ms(started),
                        "request done"
                    );
                    return Ok((status, bytes.to_vec()));
                }
            }
        };
        tokio::time::sleep(wait).await;
        n += 1;
    }
}

fn elapsed_ms(since: std::time::Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// [`send_raw`] with the body parsed leniently (non-JSON becomes `{}`).
pub(crate) async fn send_json(
    ep: &Endpoints,
    build: impl Fn(&Client) -> RequestBuilder,
) -> Result<(u16, Value), AuthError> {
    let (status, bytes) = send_raw(ep, build).await?;
    let json = serde_json::from_slice(&bytes).unwrap_or_else(|_| Value::Object(Map::new()));
    Ok((status, json))
}

pub(crate) async fn post_form(
    ep: &Endpoints,
    url: &str,
    params: &[(&str, &str)],
) -> Result<(u16, Value), AuthError> {
    send_json(ep, |c| c.post(url).form(params)).await
}

/// JSON POST; non-2xx becomes [`AuthError::Http`] carrying the Xbox `XErr`.
pub(crate) async fn post_json(ep: &Endpoints, url: &str, body: &Value) -> Result<Value, AuthError> {
    let (status, json) = send_json(ep, |c| {
        c.post(url).header("Accept", "application/json").json(body)
    })
    .await?;
    if (200..300).contains(&status) {
        return Ok(json);
    }
    let message = ["Message", "error_description"]
        .iter()
        .find_map(|k| json.get(*k).and_then(Value::as_str))
        .map(str::to_string)
        .unwrap_or_else(|| "request rejected".to_string());
    let xerr = json.get("XErr").map(|v| match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    });
    Err(AuthError::Http {
        status,
        message,
        xerr,
    })
}
