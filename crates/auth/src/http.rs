use crate::endpoints::Endpoints;
use crate::error::AuthError;
use mango_net::UA;
use reqwest::{Client, RequestBuilder};
use serde_json::{Map, Value};
use std::sync::OnceLock;
use std::time::Duration;

fn client() -> Result<Client, AuthError> {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    if let Some(c) = CLIENT.get() {
        return Ok(c.clone());
    }
    let c = Client::builder().user_agent(UA).build()?;
    let _ = CLIENT.set(c.clone());
    Ok(c)
}

/// Send with retry on network errors, 5xx and 429; the last response is
/// returned as is. The body is parsed leniently (non-JSON becomes `{}`).
pub(crate) async fn send_json(
    ep: &Endpoints,
    build: impl Fn(&Client) -> RequestBuilder,
) -> Result<(u16, Value), AuthError> {
    let client = client()?;
    let mut n: u32 = 0;
    loop {
        let sent = tokio::time::timeout(Duration::from_secs(60), build(&client).send()).await;
        let retry = match sent {
            Err(_) => Err(AuthError::Timeout),
            Ok(Err(e)) => Err(AuthError::Request(e)),
            Ok(Ok(res)) => {
                let status = res.status().as_u16();
                if (res.status().is_server_error() || status == 429) && n < ep.retries {
                    Ok(())
                } else {
                    let bytes = res.bytes().await?;
                    let json = serde_json::from_slice(&bytes)
                        .unwrap_or_else(|_| Value::Object(Map::new()));
                    return Ok((status, json));
                }
            }
        };
        if let Err(e) = retry {
            if n >= ep.retries {
                return Err(e);
            }
        }
        tokio::time::sleep(ep.backoff.saturating_mul(1u32 << n.min(16))).await;
        n += 1;
    }
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
