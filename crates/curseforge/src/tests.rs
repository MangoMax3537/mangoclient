#![allow(clippy::unwrap_used)]

use std::io::Write;
use std::sync::{Arc, Mutex};

use mango_core::Loader;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::*;

const KEY: &str = "$2a$10$abcdefghijklmnopqrstuv.wxyz0123456789ABCDEFGHIJKLMNO";

#[derive(Debug, Clone)]
struct Req {
    method: String,
    path: String,
    key: Option<String>,
    body: Vec<u8>,
}

type Handler = Arc<dyn Fn(&Req) -> (u16, Vec<u8>) + Send + Sync>;

async fn serve(make: impl FnOnce(&str) -> Handler) -> (String, Arc<Mutex<Vec<Req>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let handler = make(&base);
    let seen: Arc<Mutex<Vec<Req>>> = Arc::default();
    let log = seen.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let (handler, log) = (handler.clone(), log.clone());
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut tmp = [0u8; 4096];
                let head_end = loop {
                    if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        break i + 4;
                    }
                    match sock.read(&mut tmp).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    }
                };
                let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
                let header = |name: &str| {
                    head.lines().find_map(|l| {
                        let (k, v) = l.split_once(':')?;
                        k.eq_ignore_ascii_case(name).then(|| v.trim().to_string())
                    })
                };
                let len: usize = header("content-length")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                while buf.len() < head_end + len {
                    match sock.read(&mut tmp).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    }
                }
                let mut first = head.split_whitespace();
                let req = Req {
                    method: first.next().unwrap_or("").to_string(),
                    path: first.next().unwrap_or("/").to_string(),
                    key: header("x-api-key"),
                    body: buf[head_end..].to_vec(),
                };
                let (status, body) = handler(&req);
                log.lock().unwrap().push(req);
                let mut out = format!(
                    "HTTP/1.1 {status} X\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
                    body.len()
                )
                .into_bytes();
                out.extend_from_slice(&body);
                let _ = sock.write_all(&out).await;
            });
        }
    });
    (base, seen)
}

fn ok(v: Value) -> (u16, Vec<u8>) {
    (200, serde_json::to_vec(&v).unwrap())
}

fn client(base: &str) -> Client {
    Client::with_base(
        ApiKey::new(KEY).unwrap(),
        base,
        Options {
            retries: 0,
            strict_download_urls: false,
            ..Options::default()
        },
    )
    .unwrap()
}

fn file_json(id: u64, versions: &[&str], release: u32, date: &str) -> Value {
    json!({
        "id": id, "modId": 10, "isAvailable": true,
        "displayName": format!("f{id}"), "fileName": format!("mod-{id}.jar"),
        "releaseType": release, "fileDate": date, "fileLength": 3,
        "downloadUrl": format!("https://edge.forgecdn.net/files/{id}/mod-{id}.jar"),
        "gameVersions": versions, "hashes": [], "dependencies": []
    })
}

fn file(id: u64, versions: &[&str], release: u32, date: &str) -> File {
    serde_json::from_value(file_json(id, versions, release, date)).unwrap()
}

// ---------- key handling ----------

#[test]
fn api_key_is_validated_and_never_printed() {
    assert!(ApiKey::new(KEY).is_ok());
    for bad in [
        "",
        "short",
        "has space in it 1234567890",
        "bad\"quote1234567890",
    ] {
        assert_eq!(
            ApiKey::new(bad).unwrap_err().code(),
            "curseforge.invalid_api_key"
        );
    }
    let key = ApiKey::new(KEY).unwrap();
    let c = Client::new(key.clone()).unwrap();
    for s in [format!("{key:?}"), format!("{key}"), format!("{c:?}")] {
        assert!(!s.contains("abcdefgh"), "{s}");
        assert!(s.contains("redacted"));
    }
}

#[tokio::test]
async fn api_key_is_sent_to_the_api_and_not_leaked_in_errors() {
    let (base, seen) = serve(|_| {
        Arc::new(|r: &Req| {
            if r.path.starts_with("/mods/1") {
                ok(json!({"data": {"id": 1, "name": "A", "slug": "a"}}))
            } else {
                (500, b"{}".to_vec())
            }
        })
    })
    .await;
    let c = client(&base);
    c.project(1).await.unwrap();
    assert_eq!(seen.lock().unwrap()[0].key.as_deref(), Some(KEY));
    let err = c.project(2).await.unwrap_err();
    assert_eq!(err.code(), "curseforge.http_status");
    assert!(!format!("{err} {err:?}").contains(KEY));
}

#[tokio::test]
async fn a_rejected_key_is_unauthorized() {
    let (base, _) = serve(|_| Arc::new(|_| (403, b"{}".to_vec()))).await;
    assert_eq!(
        client(&base).project(1).await.unwrap_err().code(),
        "curseforge.unauthorized"
    );
}

// ---------- search ----------

#[tokio::test]
async fn search_maps_params_to_the_api() {
    let (base, seen) = serve(|_| {
        Arc::new(|_| {
            ok(json!({
                "data": [{"id": 7, "name": "Sodium", "slug": "sodium", "classId": 6,
                          "downloadCount": 1234.0, "authors": [{"name": "jelly"}]}],
                "pagination": {"index": 30, "pageSize": 30, "resultCount": 1, "totalCount": 91}
            }))
        })
    })
    .await;
    let (hits, page) = client(&base)
        .search(&SearchQuery {
            filter: "ferrite core".into(),
            class: ClassId::Mods,
            game_version: Some("1.21.1".into()),
            loader: ModLoaderType::from_name("neoforge"),
            sort: SortField::parse("follows").unwrap(),
            index: 30,
            page_size: 30,
        })
        .await
        .unwrap();
    assert_eq!(hits[0].name, "Sodium");
    assert_eq!(hits[0].authors[0].name, "jelly");
    assert_eq!(page.total_count, 91);
    let path = seen.lock().unwrap()[0].path.clone();
    for part in [
        "gameId=432",
        "classId=6",
        "searchFilter=ferrite+core",
        "gameVersion=1.21.1",
        "modLoaderType=6",
        "sortField=2",
        "sortOrder=desc",
        "index=30",
        "pageSize=30",
    ] {
        assert!(path.contains(part), "{part} missing in {path}");
    }
}

#[test]
fn class_loader_and_sort_ids_match_the_api() {
    assert_eq!(
        [
            ClassId::Mods.id(),
            ClassId::Modpacks.id(),
            ClassId::ResourcePacks.id(),
            ClassId::Shaders.id(),
            ClassId::DataPacks.id()
        ],
        [6, 4471, 12, 6552, 6945]
    );
    assert_eq!(
        ["forge", "fabric", "quilt", "neoforge"].map(|n| ModLoaderType::from_name(n).unwrap().id()),
        [1, 4, 5, 6]
    );
    assert!(ModLoaderType::from_name("vanilla").is_none());
    assert_eq!(
        ["relevance", "downloads", "follows", "newest", "updated"]
            .map(|s| SortField::parse(s).unwrap().id()),
        [1, 6, 2, 11, 3]
    );
    assert!(SortField::parse("random").is_none());
}

// ---------- description ----------

#[tokio::test]
async fn description_html_becomes_safe_text() {
    let html = r#"<h2>Intro</h2><p>Hello &amp; <b>welcome</b></p>
        <script>alert(1)</script><style>p{}</style>
        <ul><li>one</li><li>two</li></ul>
        <a href="https://example.com/x">site</a> <a href="javascript:alert(1)">bad</a>
        <img src="x" onerror="alert(1)"> &lt;img src=x onerror=alert(1)&gt;<!-- hidden -->"#;
    let (base, _) = serve(|_| {
        let body = json!({ "data": html });
        Arc::new(move |_| ok(body.clone()))
    })
    .await;
    let text = client(&base).description(5).await.unwrap();
    assert!(text.contains("## Intro"));
    assert!(text.contains("Hello & welcome"));
    assert!(text.contains("- one") && text.contains("- two"));
    assert!(text.contains("[site](https://example.com/x)"));
    assert!(!text.contains("alert(1)</script") && !text.contains("<script"));
    assert!(!text.contains("javascript:") && text.contains("bad"));
    assert!(!text.contains("hidden") && !text.contains('<'));
}

// ---------- files, dependencies, best file ----------

#[test]
fn best_file_prefers_release_then_newest_and_checks_loader_and_version() {
    let files = vec![
        file(1, &["1.21.1", "Fabric"], 1, "2024-01-01T00:00:00Z"),
        file(2, &["1.21.1", "Fabric"], 1, "2024-03-01T00:00:00Z"),
        file(3, &["1.21.1", "Fabric"], 2, "2024-09-01T00:00:00Z"),
        file(4, &["1.21.1", "Forge"], 1, "2024-12-01T00:00:00Z"),
        file(5, &["1.20.1", "Fabric"], 1, "2024-12-01T00:00:00Z"),
    ];
    let fab = Some(ModLoaderType::Fabric);
    assert_eq!(pick_best_file(&files, "1.21.1", fab).unwrap().id, 2);
    assert_eq!(pick_best_file(&files, "1.20.1", fab).unwrap().id, 5);
    assert_eq!(
        pick_best_file(&files, "1.21.1", Some(ModLoaderType::Forge))
            .unwrap()
            .id,
        4
    );
    assert!(pick_best_file(&files, "1.19", fab).is_none());
    // Beta beats alpha when there is no release.
    let only = vec![
        file(1, &["1.21", "Fabric"], 3, "2025-01-01T00:00:00Z"),
        file(2, &["1.21", "Fabric"], 2, "2024-01-01T00:00:00Z"),
    ];
    assert_eq!(pick_best_file(&only, "1.21", fab).unwrap().id, 2);
    // No loader filter (resource packs).
    let pack = vec![file(9, &["1.21.1"], 1, "2024-01-01T00:00:00Z")];
    assert_eq!(pick_best_file(&pack, "1.21.1", None).unwrap().id, 9);
    assert!(pick_best_file(&pack, "1.21.1", fab).is_none());
}

#[test]
fn quilt_falls_back_to_fabric_only_when_no_quilt_file_exists() {
    let files = vec![
        file(1, &["1.21.1", "Fabric"], 1, "2024-05-01T00:00:00Z"),
        file(2, &["1.21.1", "Quilt"], 2, "2024-01-01T00:00:00Z"),
    ];
    let q = Some(ModLoaderType::Quilt);
    assert_eq!(pick_best_file(&files, "1.21.1", q).unwrap().id, 2);
    assert_eq!(pick_best_file(&files[..1], "1.21.1", q).unwrap().id, 1);
    // NeoForge does not run Fabric files.
    assert!(pick_best_file(&files, "1.21.1", Some(ModLoaderType::NeoForge)).is_none());
}

#[test]
fn unavailable_and_server_pack_files_never_fit() {
    let mut a = file(1, &["1.21.1", "Fabric"], 1, "2024-01-01T00:00:00Z");
    a.is_available = Some(false);
    let mut b = file(2, &["1.21.1", "Fabric"], 1, "2024-01-01T00:00:00Z");
    b.is_server_pack = Some(true);
    assert!(pick_best_file(&[a, b], "1.21.1", Some(ModLoaderType::Fabric)).is_none());
}

#[test]
fn dependency_relations_are_told_apart() {
    let f: File = serde_json::from_value(json!({
        "id": 1, "dependencies": [
            {"modId": 11, "relationType": 3}, {"modId": 11, "relationType": 3},
            {"modId": 12, "relationType": 2}, {"modId": 13, "relationType": 1},
            {"modId": 14, "relationType": 5}, {"modId": 15, "relationType": 6},
            {"modId": 16, "relationType": 4}
        ]
    }))
    .unwrap();
    let ids = |r| {
        dependencies_of(&f, r)
            .iter()
            .map(|d| d.mod_id)
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(Relation::Required), [11]);
    assert_eq!(ids(Relation::Optional), [12]);
    assert_eq!(ids(Relation::Embedded), [13]);
    assert_eq!(ids(Relation::Incompatible), [14]);
    assert_eq!(ids(Relation::Include), [15]);
}

#[tokio::test]
async fn files_are_paged_and_batched() {
    let (base, seen) = serve(|_| {
        Arc::new(|r: &Req| {
            if r.method == "POST" {
                let body: Value = serde_json::from_slice(&r.body).unwrap();
                let data: Vec<Value> = body["fileIds"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|i| file_json(i.as_u64().unwrap(), &["1.21"], 1, "2024-01-01T00:00:00Z"))
                    .collect();
                return ok(json!({ "data": data }));
            }
            let index: u64 = r
                .path
                .split("index=")
                .nth(1)
                .and_then(|s| s.split('&').next())
                .unwrap()
                .parse()
                .unwrap();
            let data: Vec<Value> = (index..(index + 50).min(70))
                .map(|i| file_json(i + 1, &["1.21"], 1, "2024-01-01T00:00:00Z"))
                .collect();
            ok(json!({"data": data, "pagination": {"totalCount": 70}}))
        })
    })
    .await;
    let c = client(&base);
    assert_eq!(c.files(10, Some("1.21")).await.unwrap().len(), 70);
    assert!(seen
        .lock()
        .unwrap()
        .iter()
        .all(|r| r.path.contains("gameVersion=1.21")));
    assert_eq!(c.files_by_ids(&[3, 4, 5]).await.unwrap().len(), 3);
}

// ---------- download urls and distribution ----------

#[tokio::test]
async fn null_download_url_uses_the_endpoint_and_a_refusal_means_blocked() {
    let (base, seen) = serve(|base| {
        let base = base.to_string();
        Arc::new(move |r: &Req| {
            if r.path.ends_with("/files/1/download-url") {
                ok(json!({"data": format!("{base}/cdn/ok.jar")}))
            } else {
                (403, b"{}".to_vec())
            }
        })
    })
    .await;
    let c = client(&base);
    let mut f = file(1, &["1.21"], 1, "2024-01-01T00:00:00Z");
    f.download_url = None;
    let url = c.resolve_download_url(None, &f).await.unwrap();
    assert_eq!(url, Some(format!("{base}/cdn/ok.jar")));
    // The endpoint refuses: blocked, not an error.
    let mut g = file(2, &["1.21"], 1, "2024-01-01T00:00:00Z");
    g.download_url = None;
    assert_eq!(c.resolve_download_url(None, &g).await.unwrap(), None);
    assert_eq!(seen.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn distribution_blocked_never_asks_for_or_builds_a_url() {
    let (base, seen) = serve(|_| Arc::new(|_| (500, b"{}".to_vec()))).await;
    let c = client(&base);
    let project: Project = serde_json::from_value(json!({
        "id": 10, "slug": "secret-mod", "classId": 6, "allowModDistribution": false
    }))
    .unwrap();
    let f = file(1, &["1.21"], 1, "2024-01-01T00:00:00Z");
    // Even a file that carries a URL stays blocked.
    assert_eq!(
        c.resolve_download_url(Some(&project), &f).await.unwrap(),
        None
    );
    assert!(seen.lock().unwrap().is_empty());
    assert_eq!(
        project.website_url().unwrap(),
        "https://www.curseforge.com/minecraft/mc-mods/secret-mod"
    );
    let dest = tempfile::tempdir().unwrap();
    let err = download_cf_file(&c, Some(&project), &f, &dest.path().join("mod-1.jar"), None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "curseforge.distribution_blocked");
    assert!(!dest.path().join("mod-1.jar").exists());
}

#[test]
fn strict_mode_only_trusts_the_curseforge_cdn() {
    for ok in [
        "https://edge.forgecdn.net/files/1/2/a.jar",
        "https://mediafilez.forgecdn.net/files/1/2/a.jar",
        "https://www.curseforge.com/api/v1/mods/1/files/2/download",
    ] {
        assert!(is_trusted_cdn_url(ok), "{ok}");
    }
    for bad in [
        "http://edge.forgecdn.net/a.jar",
        "https://evil.example/forgecdn.net",
        "https://forgecdn.net.evil.example/a.jar",
        "https://user@edge.forgecdn.net/a.jar",
        "https://edge.forgecdn.net:444/a.jar",
        "file:///etc/passwd",
        // Backslash is a path separator to the HTTP client: the host is evil.example.
        "https://evil.example\\.forgecdn.net/x.jar",
        "https://evil.example\\@edge.forgecdn.net/x.jar",
        "https://evil.example%5c.forgecdn.net/x.jar",
        "https://evil.example/\\.forgecdn.net/x.jar",
        "not a url",
    ] {
        assert!(!is_trusted_cdn_url(bad), "{bad}");
    }
}

#[test]
fn website_urls_need_plain_slugs() {
    assert_eq!(
        website_url(ClassId::Shaders, "bsl-shaders").unwrap(),
        "https://www.curseforge.com/minecraft/shaders/bsl-shaders"
    );
    assert_eq!(
        website_url(ClassId::ResourcePacks, "faithful").unwrap(),
        "https://www.curseforge.com/minecraft/texture-packs/faithful"
    );
    assert!(website_url(ClassId::Mods, "../x").is_none());
    assert!(website_url(ClassId::Mods, "").is_none());
}

#[tokio::test]
async fn download_verifies_sha1_and_size() {
    use sha1::{Digest, Sha1};
    let body = b"jar".to_vec();
    let good = hex::encode(Sha1::digest(&body));
    let b2 = body.clone();
    let (base, _) = serve(move |_| {
        let b = b2.clone();
        Arc::new(move |_| (200, b.clone()))
    })
    .await;
    let c = client(&base);
    let dir = tempfile::tempdir().unwrap();
    let mut f = file(1, &["1.21"], 1, "2024-01-01T00:00:00Z");
    f.download_url = Some(format!("{base}/x.jar"));
    f.hashes = vec![FileHash {
        value: good.to_uppercase(),
        algo: 1,
    }];
    download_cf_file(&c, None, &f, &dir.path().join("mod-1.jar"), None)
        .await
        .unwrap();
    assert_eq!(std::fs::read(dir.path().join("mod-1.jar")).unwrap(), body);
    // A wrong hash is rejected and leaves nothing behind.
    f.hashes = vec![FileHash {
        value: "0".repeat(40),
        algo: 1,
    }];
    let err = download_cf_file(&c, None, &f, &dir.path().join("mod-2.jar"), None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "net.checksum_mismatch");
    assert!(!dir.path().join("mod-2.jar").exists());
    // A traversal filename is refused before any request.
    f.file_name = "../evil.jar".into();
    assert_eq!(
        download_cf_file(&c, None, &f, &dir.path().join("x.jar"), None)
            .await
            .unwrap_err()
            .code(),
        "curseforge.invalid_filename"
    );
}

// ---------- manifest ----------

#[test]
fn manifest_loader_ids_parse_in_every_format() {
    for (id, loader, version) in [
        ("fabric-0.16.9", Loader::Fabric, "0.16.9"),
        ("neoforge-21.1.1", Loader::Neoforge, "21.1.1"),
        ("forge-47.2.0", Loader::Forge, "47.2.0"),
        ("quilt-0.26.0-beta.1", Loader::Quilt, "0.26.0-beta.1"),
    ] {
        assert_eq!(
            parse_loader_id(id).unwrap(),
            (loader, version.to_string()),
            "{id}"
        );
    }
    for bad in ["fabric", "fabric-", "liteloader-1.0", ""] {
        assert!(parse_loader_id(bad).is_err(), "{bad}");
    }
}

fn manifest_json(loaders: Value, overrides: Option<&str>) -> Vec<u8> {
    let mut m = json!({
        "minecraft": {"version": "1.21.1", "modLoaders": loaders},
        "manifestType": "minecraftModpack", "name": "Pack", "version": "1.2",
        "files": [
            {"projectID": 10, "fileID": 1, "required": true},
            {"projectID": 11, "fileID": 2},
            {"projectID": 12, "fileID": 3, "required": false}
        ]
    });
    if let Some(o) = overrides {
        m["overrides"] = json!(o);
    }
    serde_json::to_vec(&m).unwrap()
}

#[test]
fn manifest_parsing_picks_the_primary_loader() {
    let m = parse_manifest(&manifest_json(
        json!([{"id": "forge-47.2.0", "primary": false}, {"id": "neoforge-21.1.1", "primary": true}]),
        None,
    ))
    .unwrap();
    assert_eq!(
        (m.mc_version.as_str(), m.loader, m.loader_version.as_str()),
        ("1.21.1", Loader::Neoforge, "21.1.1")
    );
    assert_eq!(m.overrides, "overrides");
    assert_eq!(m.files.len(), 3);
    assert!(m.files[1].required && !m.files[2].required);
    assert_eq!(m.name.as_deref(), Some("Pack"));
    // No primary flag: the first wins. No loader: vanilla.
    let m = parse_manifest(&manifest_json(json!([{"id": "fabric-0.16.9"}]), Some("ov"))).unwrap();
    assert_eq!((m.loader, m.overrides.as_str()), (Loader::Fabric, "ov"));
    let m = parse_manifest(&manifest_json(json!([]), None)).unwrap();
    assert_eq!((m.loader, m.loader_version.as_str()), (Loader::Vanilla, ""));
    // Garbage and unsafe overrides are refused.
    assert!(parse_manifest(b"{").is_err());
    assert!(parse_manifest(br#"{"minecraft": {}}"#).is_err());
    for bad in ["../x", "a/b", "/abs", ".."] {
        assert!(
            parse_manifest(&manifest_json(json!([{"id": "fabric-1"}]), Some(bad))).is_err(),
            "{bad}"
        );
    }
}

// ---------- modpack import ----------

fn make_zip(path: &std::path::Path, entries: &[(&str, &[u8])]) {
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let opts = zip::write::SimpleFileOptions::default();
    for (name, data) in entries {
        zip.start_file(*name, opts).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn read_manifest_finds_manifest_json_in_the_zip() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("pack.zip");
    make_zip(
        &p,
        &[(
            "manifest.json",
            &manifest_json(json!([{"id": "fabric-0.16.9"}]), None),
        )],
    );
    assert_eq!(read_manifest(&p).unwrap().loader, Loader::Fabric);
    make_zip(&p, &[("other.json", b"{}")]);
    assert_eq!(
        read_manifest(&p).unwrap_err().code(),
        "curseforge.invalid_modpack"
    );
}

/// Mock API + CDN for a three-mod pack: 10 ok, 11 author opt-out, 12 shader.
async fn pack_server() -> (String, Arc<Mutex<Vec<Req>>>) {
    serve(|base| {
        let base = base.to_string();
        Arc::new(move |r: &Req| {
            let path = r.path.split('?').next().unwrap();
            if r.method == "POST" && path == "/mods/files" {
                let mut a = file_json(1, &["1.21.1", "Fabric"], 1, "2024-01-01T00:00:00Z");
                a["modId"] = json!(10);
                a["downloadUrl"] = json!(format!("{base}/cdn/a.jar"));
                a["fileName"] = json!("a.jar");
                a["fileLength"] = json!(5);
                let mut b = file_json(2, &["1.21.1", "Fabric"], 1, "2024-01-01T00:00:00Z");
                b["modId"] = json!(11);
                b["downloadUrl"] = Value::Null;
                b["fileName"] = json!("b.jar");
                return ok(json!({"data": [a, b]}));
            }
            if r.method == "POST" && path == "/mods" {
                return ok(json!({"data": [
                    {"id": 10, "slug": "alpha", "classId": 6},
                    {"id": 11, "slug": "bravo", "classId": 6, "allowModDistribution": false}
                ]}));
            }
            match path {
                "/cdn/a.jar" => (200, b"AAAAA".to_vec()),
                _ => (404, b"{}".to_vec()),
            }
        })
    })
    .await
}

fn pack_manifest() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "minecraft": {"version": "1.21.1", "modLoaders": [{"id": "fabric-0.16.9", "primary": true}]},
        "name": "Pack", "overrides": "overrides",
        "files": [
            {"projectID": 10, "fileID": 1, "required": true},
            {"projectID": 11, "fileID": 2, "required": true},
            {"projectID": 12, "fileID": 3, "required": false}
        ]
    }))
    .unwrap()
}

#[tokio::test]
async fn resolve_reports_blocked_files_instead_of_failing() {
    let (base, seen) = pack_server().await;
    let c = client(&base);
    let manifest = parse_manifest(&pack_manifest()).unwrap();
    let pack = resolve_pack(&c, &manifest).await.unwrap();
    assert_eq!(pack.skipped_optional, 1);
    assert_eq!(pack.entries.len(), 2);
    assert_eq!(pack.downloadable().count(), 1);
    assert_eq!(pack.download_bytes(), 5);
    assert_eq!(
        pack.blocked(),
        vec![BlockedFile {
            name: "b.jar".into(),
            url: "https://www.curseforge.com/minecraft/mc-mods/bravo".into()
        }]
    );
    // The blocked file never hit a download endpoint.
    assert!(!seen
        .lock()
        .unwrap()
        .iter()
        .any(|r| r.path.contains("download-url")));
}

#[tokio::test]
async fn install_downloads_files_and_extracts_overrides() {
    let (base, seen) = pack_server().await;
    let c = client(&base);
    let tmp = tempfile::tempdir().unwrap();
    let zip = tmp.path().join("pack.zip");
    make_zip(
        &zip,
        &[
            ("manifest.json", &pack_manifest()),
            ("overrides/config/x.toml", b"x = 1"),
            ("overrides/options.txt", b"lang:de"),
            ("overrides/mods/.mangoclient-performance.json", b"{}"),
            ("overrides/MODS/.MANGOCLIENT-x", b"{}"),
        ],
    );
    let manifest = read_manifest(&zip).unwrap();
    let pack = resolve_pack(&c, &manifest).await.unwrap();
    let game = tmp.path().join("game");
    let steps = Arc::new(Mutex::new(Vec::new()));
    let s2 = steps.clone();
    install_resolved(
        &pack,
        &zip,
        &game,
        Some(Arc::new(move |p: &PackProgress| {
            s2.lock().unwrap().push((p.done, p.total))
        })),
    )
    .await
    .unwrap();
    assert_eq!(std::fs::read(game.join("mods/a.jar")).unwrap(), b"AAAAA");
    assert!(!game.join("mods/b.jar").exists());
    assert_eq!(std::fs::read(game.join("config/x.toml")).unwrap(), b"x = 1");
    assert_eq!(std::fs::read(game.join("options.txt")).unwrap(), b"lang:de");
    // The launcher's bookkeeping is skipped at any depth and in any letter case.
    assert!(!game.join("mods/.mangoclient-performance.json").exists());
    assert!(!game.join("MODS/.MANGOCLIENT-x").exists());
    assert_eq!(*steps.lock().unwrap(), [(1, 1)]);
    // The API key never went to the CDN.
    assert!(seen
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.path.starts_with("/cdn/"))
        .all(|r| r.key.is_none()));
}

#[tokio::test]
async fn a_traversal_override_aborts_before_anything_is_downloaded() {
    let (base, seen) = pack_server().await;
    let c = client(&base);
    let tmp = tempfile::tempdir().unwrap();
    let zip = tmp.path().join("evil.zip");
    make_zip(
        &zip,
        &[
            ("manifest.json", &pack_manifest()),
            ("overrides/../../escape.txt", b"nope"),
        ],
    );
    let manifest = read_manifest(&zip).unwrap();
    let pack = resolve_pack(&c, &manifest).await.unwrap();
    let game = tmp.path().join("game");
    let before = seen.lock().unwrap().len();
    let err = install_resolved(&pack, &zip, &game, None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "curseforge.archive");
    assert!(!tmp.path().join("escape.txt").exists());
    assert!(!game.join("mods/a.jar").exists());
    assert_eq!(seen.lock().unwrap().len(), before, "no CDN request");
}

#[tokio::test]
async fn a_hostile_file_name_is_refused() {
    let (base, _) = serve(|base| {
        let base = base.to_string();
        Arc::new(move |r: &Req| {
            let path = r.path.split('?').next().unwrap();
            match (r.method.as_str(), path) {
                ("POST", "/mods/files") => {
                    let mut a = file_json(1, &["1.21.1"], 1, "2024-01-01T00:00:00Z");
                    a["downloadUrl"] = json!(format!("{base}/cdn/a.jar"));
                    a["fileName"] = json!("../../evil.jar");
                    ok(json!({"data": [a]}))
                }
                ("POST", "/mods") => ok(json!({"data": [{"id": 10, "slug": "a", "classId": 6}]})),
                _ => (200, b"AAA".to_vec()),
            }
        })
    })
    .await;
    let c = client(&base);
    let tmp = tempfile::tempdir().unwrap();
    let zip = tmp.path().join("p.zip");
    let manifest_bytes = serde_json::to_vec(&json!({
        "minecraft": {"version": "1.21.1", "modLoaders": []},
        "files": [{"projectID": 10, "fileID": 1, "required": true}]
    }))
    .unwrap();
    make_zip(&zip, &[("manifest.json", &manifest_bytes)]);
    let pack = resolve_pack(&c, &read_manifest(&zip).unwrap())
        .await
        .unwrap();
    let game = tmp.path().join("game");
    assert_eq!(
        install_resolved(&pack, &zip, &game, None)
            .await
            .unwrap_err()
            .code(),
        "curseforge.invalid_filename"
    );
    assert!(!tmp.path().join("evil.jar").exists());
}
