//! Anonymous presence beat (port of `src/main/telemetry.js`).
//!
//! What leaves the machine is exactly: random install id, launcher version,
//! platform string. The opt-out is checked before every beat.

use std::sync::Arc;
use std::time::Duration;

use mango_core::Paths;
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

use crate::error::Result;
use crate::security::validated_api_base;

pub const BEAT_INTERVAL: Duration = Duration::from_secs(60);
pub const TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Serialize, Deserialize)]
struct InstallFile {
    id: String,
    #[serde(default)]
    created: String,
}

/// The id for this copy: read from `install.json`, or made once and kept.
/// A read-only data root only means the id changes per run.
pub fn install_id(paths: &Paths) -> String {
    let file = paths.install_file();
    if let Ok(raw) = std::fs::read_to_string(&file) {
        if let Ok(saved) = serde_json::from_str::<InstallFile>(&raw) {
            if saved.id.len() >= 8 {
                return saved.id;
            }
        }
    }
    let id = uuid::Uuid::new_v4().to_string();
    let created = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let saved = InstallFile {
        id: id.clone(),
        created,
    };
    if let Ok(mut json) = serde_json::to_string_pretty(&saved) {
        json.push('\n');
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = mango_core::persistence::write_file_atomic(&file, json.as_bytes(), 0o600);
    }
    id
}

/// Platform string as Node's `process.platform` reports it.
pub fn os_string() -> &'static str {
    match std::env::consts::OS {
        "windows" => "win32",
        "macos" => "darwin",
        other => other,
    }
}

/// The wire payload. Exactly these three fields.
#[derive(Debug, Clone, Serialize)]
pub struct Beat {
    pub id: String,
    pub version: String,
    pub os: &'static str,
}

/// Base URL from `MANGO_API_BASE`; an invalid value falls back to the legacy
/// endpoint.
pub fn base_from_env() -> String {
    let raw = std::env::var("MANGO_API_BASE").ok();
    validated_api_base(raw.as_deref())
        .or_else(|_| validated_api_base(None))
        .unwrap_or_else(|_| crate::security::LEGACY_API.to_string())
}

/// POST one beat. Errors are swallowed (offline is normal); returns whether
/// the request completed.
pub async fn beat(base: &str, payload: &Beat) -> bool {
    let Ok(client) = reqwest::Client::builder().timeout(TIMEOUT).build() else {
        return false;
    };
    client
        .post(format!("{base}/v1/launcher"))
        .json(payload)
        .send()
        .await
        .is_ok()
}

/// Cancel handle; dropping it also stops the task.
pub struct Telemetry {
    task: JoinHandle<()>,
}

impl Telemetry {
    pub fn stop(&self) {
        self.task.abort();
    }
}

impl Drop for Telemetry {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Start beating every 60s. `is_enabled` is asked before every beat, so the
/// opt-out takes effect on the next one without a restart. `base` must
/// already be validated.
pub fn start<F>(paths: &Paths, base: String, version: String, is_enabled: F) -> Telemetry
where
    F: Fn() -> bool + Send + Sync + 'static,
{
    start_with_interval(paths, base, version, is_enabled, BEAT_INTERVAL)
}

pub fn start_with_interval<F>(
    paths: &Paths,
    base: String,
    version: String,
    is_enabled: F,
    interval: Duration,
) -> Telemetry
where
    F: Fn() -> bool + Send + Sync + 'static,
{
    let payload = Beat {
        id: install_id(paths),
        version,
        os: os_string(),
    };
    let is_enabled = Arc::new(is_enabled);
    let task = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await; // first tick fires immediately
            if is_enabled() {
                beat(&base, &payload).await;
            }
        }
    });
    Telemetry { task }
}

/// Convenience for callers that want the validated base up front.
pub fn validated_base(raw: Option<&str>) -> Result<String> {
    validated_api_base(raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::mpsc;

    async fn mock() -> (String, mpsc::UnboundedReceiver<(String, String)>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let tx = tx.clone();
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 1024];
                    loop {
                        let Ok(n) = sock.read(&mut chunk).await else {
                            return;
                        };
                        if n == 0 {
                            return;
                        }
                        buf.extend_from_slice(&chunk[..n]);
                        let text = String::from_utf8_lossy(&buf).to_string();
                        let Some(pos) = text.find("\r\n\r\n") else {
                            continue;
                        };
                        let head = text[..pos].to_ascii_lowercase();
                        let len = head
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length:"))
                            .and_then(|v| v.trim().parse::<usize>().ok())
                            .unwrap_or(0);
                        if buf.len() >= pos + 4 + len {
                            let line = text.lines().next().unwrap_or("").to_string();
                            let body = text[pos + 4..].to_string();
                            let _ = tx.send((line, body));
                            let _ = sock
                                .write_all(
                                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                                )
                                .await;
                            return;
                        }
                    }
                });
            }
        });
        (base, rx)
    }

    #[test]
    fn install_id_created_once_and_persisted() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().join("root"));
        let a = install_id(&paths);
        assert_eq!(a.len(), 36);
        assert_eq!(install_id(&paths), a);
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(paths.install_file()).unwrap()).unwrap();
        assert_eq!(v["id"], a.as_str());
        assert!(v["created"].as_str().unwrap().ends_with('Z'));
        // Garbage is replaced.
        std::fs::write(paths.install_file(), "{\"id\":\"short\"}").unwrap();
        assert_ne!(install_id(&paths), "short");
    }

    #[test]
    fn os_mapping() {
        assert!(["win32", "darwin", "linux"].contains(&os_string()) || !os_string().is_empty());
        #[cfg(target_os = "linux")]
        assert_eq!(os_string(), "linux");
    }

    #[tokio::test]
    async fn beat_posts_exactly_three_fields() {
        let (base, mut rx) = mock().await;
        let payload = Beat {
            id: "abcdefgh-1".into(),
            version: "2.0.0".into(),
            os: os_string(),
        };
        assert!(beat(&base, &payload).await);
        let (line, body) = rx.recv().await.unwrap();
        assert_eq!(line, "POST /v1/launcher HTTP/1.1");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        let obj = v.as_object().unwrap();
        let mut keys: Vec<_> = obj.keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["id", "os", "version"]);
        assert_eq!(obj["id"], "abcdefgh-1");
        assert_eq!(obj["version"], "2.0.0");
    }

    #[tokio::test]
    async fn offline_beat_is_silent() {
        let payload = Beat {
            id: "x".into(),
            version: "1".into(),
            os: "linux",
        };
        assert!(!beat("http://127.0.0.1:1", &payload).await);
    }

    #[tokio::test]
    async fn loop_checks_opt_out_before_every_beat() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        let (base, mut rx) = mock().await;
        let on = Arc::new(AtomicBool::new(true));
        let flag = on.clone();
        let t = start_with_interval(
            &paths,
            base,
            "9.9.9".into(),
            move || flag.load(Ordering::SeqCst),
            Duration::from_millis(40),
        );
        let (_, first) = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(first.contains(&install_id(&paths)));
        assert!(first.contains("9.9.9"));
        tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .unwrap()
            .unwrap();

        on.store(false, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(150)).await; // let in-flight settle
        while rx.try_recv().is_ok() {}
        tokio::time::sleep(Duration::from_millis(250)).await;
        assert!(rx.try_recv().is_err(), "beat sent while disabled");

        on.store(true, Ordering::SeqCst);
        tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .unwrap()
            .unwrap();
        t.stop();
    }

    #[test]
    fn base_validation_via_env_helper() {
        assert!(validated_base(Some("http://example.com")).is_err());
        assert_eq!(validated_base(None).unwrap(), crate::security::LEGACY_API);
    }
}
