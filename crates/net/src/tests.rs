#![allow(clippy::unwrap_used)]

use crate::*;
use sha2::{Digest, Sha512};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

struct Resp {
    status: u16,
    /// Header lines to send (without CRLF), e.g. "Content-Length: 5".
    headers: Vec<String>,
    body: Vec<u8>,
    chunked: bool,
}

impl Resp {
    fn ok(body: &[u8]) -> Self {
        Self {
            status: 200,
            headers: vec![format!("Content-Length: {}", body.len())],
            body: body.to_vec(),
            chunked: false,
        }
    }
    fn status(status: u16) -> Self {
        Self {
            status,
            headers: vec!["Content-Length: 0".into()],
            body: vec![],
            chunked: false,
        }
    }
}

type Handler = Arc<dyn Fn(usize, &str) -> Resp + Send + Sync>;

/// Minimal HTTP/1.1 server. The handler gets (request index, path).
async fn serve(handler: Handler) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let h2 = hits.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let idx = h2.fetch_add(1, Ordering::SeqCst);
            let handler = handler.clone();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut tmp = [0u8; 1024];
                while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    match sock.read(&mut tmp).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    }
                }
                let head = String::from_utf8_lossy(&buf).to_string();
                let path = head.split_whitespace().nth(1).unwrap_or("/").to_string();
                let r = handler(idx, &path);
                let mut out = format!("HTTP/1.1 {} X\r\nConnection: close\r\n", r.status);
                for h in &r.headers {
                    out.push_str(h);
                    out.push_str("\r\n");
                }
                if r.chunked {
                    out.push_str("Transfer-Encoding: chunked\r\n\r\n");
                    let mut bytes = out.into_bytes();
                    bytes.extend_from_slice(format!("{:x}\r\n", r.body.len()).as_bytes());
                    bytes.extend_from_slice(&r.body);
                    bytes.extend_from_slice(b"\r\n0\r\n\r\n");
                    let _ = sock.write_all(&bytes).await;
                } else {
                    out.push_str("\r\n");
                    let mut bytes = out.into_bytes();
                    bytes.extend_from_slice(&r.body);
                    let _ = sock.write_all(&bytes).await;
                }
                let _ = sock.shutdown().await;
            });
        }
    });
    (base, hits)
}

fn sha512_hex(data: &[u8]) -> String {
    hex::encode(Sha512::digest(data))
}

fn opts(expected: Expected) -> DownloadOptions {
    DownloadOptions {
        expected,
        fetch: fast_fetch(),
        ..Default::default()
    }
}

fn fast_fetch() -> FetchOptions {
    FetchOptions {
        backoff: Duration::from_millis(5),
        ..Default::default()
    }
}

fn part_files(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains(".part-"))
        .collect()
}

#[tokio::test]
async fn downloads_enforce_sha512_and_size_and_clean_temp_files() {
    let payload = b"verified jar bytes";
    let (base, _) = serve(Arc::new(move |_, _| Resp::ok(payload))).await;
    let url = format!("{base}/file.jar");
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("file.jar");

    let out = download_file(
        &url,
        &dest,
        &opts(Expected {
            sha512: Some(sha512_hex(payload)),
            size: Some(payload.len() as u64),
            ..Default::default()
        }),
    )
    .await
    .unwrap();
    assert!(!out.skipped);
    assert_eq!(std::fs::read(&dest).unwrap(), payload);

    let bad = dir.path().join("bad.jar");
    let err = download_file(
        &url,
        &bad,
        &opts(Expected {
            sha512: Some("0".repeat(128)),
            size: Some(payload.len() as u64),
            ..Default::default()
        }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, NetError::ChecksumMismatch { .. }), "{err}");
    assert!(!bad.exists());
    assert!(part_files(dir.path()).is_empty());

    let wrong = dir.path().join("wrong-size.jar");
    let err = download_file(
        &url,
        &wrong,
        &opts(Expected {
            size: Some(payload.len() as u64 + 1),
            ..Default::default()
        }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, NetError::AdvertisedSizeMismatch { .. }),
        "{err}"
    );
    assert!(!wrong.exists());
    assert!(part_files(dir.path()).is_empty());
}

#[tokio::test]
async fn size_mismatch_without_content_length_cleans_temp() {
    let payload = b"short body";
    let (base, _) = serve(Arc::new(move |_, _| Resp {
        status: 200,
        headers: vec![],
        body: payload.to_vec(),
        chunked: true,
    }))
    .await;
    let dir = tempfile::tempdir().unwrap();
    // Too large expected -> underrun; too small -> overrun.
    for (size, name) in [(payload.len() as u64 + 5, "under"), (3, "over")] {
        let dest = dir.path().join(name);
        let err = download_file(
            &format!("{base}/x"),
            &dest,
            &opts(Expected {
                size: Some(size),
                ..Default::default()
            }),
        )
        .await
        .unwrap_err();
        match (name, &err) {
            ("under", NetError::SizeMismatch { .. }) | ("over", NetError::Oversized { .. }) => {}
            _ => panic!("unexpected {err}"),
        }
        assert!(!dest.exists());
    }
    assert!(part_files(dir.path()).is_empty());
}

#[test]
fn content_length_zero_is_ignored_positive_is_authoritative() {
    assert!(check_advertised_length("u", Some(15), Some(0)).is_ok());
    assert!(check_advertised_length("u", Some(15), None).is_ok());
    assert!(check_advertised_length("u", None, Some(9)).is_ok());
    assert!(check_advertised_length("u", Some(15), Some(15)).is_ok());
    assert!(check_advertised_length("u", Some(15), Some(16)).is_err());
}

#[tokio::test]
async fn download_without_advertised_length_succeeds() {
    let payload = b"mojang metadata";
    let (base, _) = serve(Arc::new(move |_, _| Resp {
        status: 200,
        headers: vec![],
        body: payload.to_vec(),
        chunked: true,
    }))
    .await;
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("sub/version.json");
    download_file(
        &format!("{base}/version.json"),
        &dest,
        &opts(Expected {
            size: Some(payload.len() as u64),
            ..Default::default()
        }),
    )
    .await
    .unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), payload);
}

#[tokio::test]
async fn retries_503_then_succeeds() {
    let (base, hits) = serve(Arc::new(|i, _| {
        if i < 2 {
            Resp::status(503)
        } else {
            Resp::ok(b"fine")
        }
    }))
    .await;
    let res = fetch_with_retry(&format!("{base}/a"), &fast_fetch())
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 200);
    assert_eq!(hits.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn gives_up_after_five_attempts() {
    let (base, hits) = serve(Arc::new(|_, _| Resp::status(429))).await;
    let err = fetch_with_retry(&format!("{base}/a"), &fast_fetch())
        .await
        .unwrap_err();
    assert!(matches!(err, NetError::Status { status: 429, .. }));
    assert_eq!(hits.load(Ordering::SeqCst), 5);
}

#[tokio::test]
async fn does_not_retry_404() {
    let (base, hits) = serve(Arc::new(|_, _| Resp::status(404))).await;
    let res = fetch_with_retry(&format!("{base}/a"), &fast_fetch())
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 404);
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("f");
    let err = download_file(&format!("{base}/a"), &dest, &opts(Expected::default()))
        .await
        .unwrap_err();
    assert!(matches!(err, NetError::Status { status: 404, .. }));
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    assert!(!dest.exists() && part_files(dir.path()).is_empty());
}

#[tokio::test]
async fn sends_user_agent_and_get_json() {
    let seen = Arc::new(Mutex::new(String::new()));
    let seen2 = seen.clone();
    let (base, _) = serve(Arc::new(move |_, path| {
        *seen2.lock().unwrap() = path.to_string();
        Resp::ok(br#"{"a":1}"#)
    }))
    .await;
    let v: serde_json::Value = get_json(&format!("{base}/j"), &fast_fetch()).await.unwrap();
    assert_eq!(v["a"], 1);
    assert_eq!(*seen.lock().unwrap(), "/j");
    assert_eq!(
        UA,
        format!(
            "MangoClient/{} (+https://github.com/MangoMax3537/mangoclient)",
            env!("CARGO_PKG_VERSION")
        )
    );
}

#[tokio::test]
async fn skips_valid_existing_file_and_uses_integrity_cache() {
    let payload = b"cached payload";
    let (base, hits) = serve(Arc::new(move |_, _| Resp::ok(payload))).await;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let cache_file = root.path().join("cache/integrity-cache.json");
    let cache = Arc::new(IntegrityCache::load(&cache_file, root.path()).await);
    let expected = Expected {
        sha512: Some(sha512_hex(payload)),
        size: Some(payload.len() as u64),
        ..Default::default()
    };
    let o = DownloadOptions {
        expected: expected.clone(),
        cache: Some(cache.clone()),
        fetch: fast_fetch(),
        progress: None,
    };

    let dest = root.path().join("lib/a.jar");
    assert!(
        !download_file(&format!("{base}/a"), &dest, &o)
            .await
            .unwrap()
            .skipped
    );
    assert!(
        download_file(&format!("{base}/a"), &dest, &o)
            .await
            .unwrap()
            .skipped
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    // Persisted and reloadable.
    cache.flush().await.unwrap();
    let reloaded = IntegrityCache::load(&cache_file, root.path()).await;
    let meta = std::fs::metadata(&dest).unwrap();
    assert!(reloaded.is_verified(&dest, &meta, &expected_key(&expected)));

    // Paths outside the root are never remembered.
    let out_dest = outside.path().join("b.jar");
    let o2 = DownloadOptions {
        cache: Some(cache.clone()),
        ..o.clone()
    };
    download_file(&format!("{base}/b"), &out_dest, &o2)
        .await
        .unwrap();
    let meta = std::fs::metadata(&out_dest).unwrap();
    assert!(!cache.is_verified(&out_dest, &meta, &expected_key(&expected)));

    // A corrupted file is re-downloaded.
    std::fs::write(&dest, b"cached payloaX").unwrap();
    assert!(
        !download_file(&format!("{base}/a"), &dest, &o)
            .await
            .unwrap()
            .skipped
    );
    assert_eq!(std::fs::read(&dest).unwrap(), payload);
}

fn expected_key(e: &Expected) -> String {
    format!(
        "{}|{}|{}|{}",
        e.sha512.clone().unwrap_or_default(),
        e.sha256.clone().unwrap_or_default(),
        e.sha1.clone().unwrap_or_default(),
        e.size.map(|s| s.to_string()).unwrap_or_default()
    )
}

#[tokio::test]
async fn is_valid_checks_size_hash_and_non_empty() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("f");
    std::fs::write(&f, b"abc").unwrap();
    let sha1 = "a9993e364706816aba3e25717850c26c9cd0d89d".to_string();
    let ok = Expected {
        sha1: Some(sha1.to_uppercase()),
        size: Some(3),
        ..Default::default()
    };
    assert!(is_valid(&f, &ok, None).await);
    assert!(
        !is_valid(
            &f,
            &Expected {
                size: Some(4),
                ..ok.clone()
            },
            None
        )
        .await
    );
    assert!(
        !is_valid(
            &f,
            &Expected {
                sha1: Some("00".into()),
                ..ok
            },
            None
        )
        .await
    );
    assert!(is_valid(&f, &Expected::default(), None).await);
    std::fs::write(&f, b"").unwrap();
    assert!(!is_valid(&f, &Expected::default(), None).await);
    assert!(!is_valid(&dir.path().join("missing"), &Expected::default(), None).await);
}

#[tokio::test]
async fn pool_preserves_order_and_bounds_concurrency() {
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let (a, p) = (active.clone(), peak.clone());
    let out = pool((0..20u64).collect(), 3, move |n, i| {
        let (a, p) = (a.clone(), p.clone());
        async move {
            let now = a.fetch_add(1, Ordering::SeqCst) + 1;
            p.fetch_max(now, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(20 - n)).await;
            a.fetch_sub(1, Ordering::SeqCst);
            (i, n * 2)
        }
    })
    .await
    .unwrap();
    assert_eq!(out.len(), 20);
    for (i, (idx, v)) in out.iter().enumerate() {
        assert_eq!((*idx, *v), (i, i as u64 * 2));
    }
    assert!(peak.load(Ordering::SeqCst) <= 3);
    assert!(peak.load(Ordering::SeqCst) >= 2);
    let empty: Vec<u8> = pool(Vec::<u8>::new(), 4, |n, _| async move { n })
        .await
        .unwrap();
    assert!(empty.is_empty());
}

#[tokio::test]
async fn progress_reports_done_and_total() {
    let payload = vec![7u8; 100_000];
    let p2 = payload.clone();
    let (base, _) = serve(Arc::new(move |_, _| Resp::ok(&p2))).await;
    let last = Arc::new(Mutex::new((0u64, 0u64)));
    let l2 = last.clone();
    let dir = tempfile::tempdir().unwrap();
    let o = DownloadOptions {
        progress: Some(Arc::new(move |d, t| *l2.lock().unwrap() = (d, t))),
        fetch: fast_fetch(),
        ..Default::default()
    };
    download_file(&format!("{base}/p"), &dir.path().join("p"), &o)
        .await
        .unwrap();
    assert_eq!(*last.lock().unwrap(), (100_000, 100_000));
}
