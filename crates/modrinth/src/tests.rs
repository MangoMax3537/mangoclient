#![allow(clippy::unwrap_used)]

use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use mango_core::{Loader, ModDependency, ModRecord, ModType, Paths, Profile};
use mango_net::FetchOptions;
use serde_json::{json, Value};
use sha2::{Digest, Sha512};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::*;

const PID: &str = "00000000-0000-4000-8000-000000000000";

type Handler = Arc<dyn Fn(&str) -> (u16, Vec<u8>) + Send + Sync>;

/// Minimal HTTP server. `make` gets the base URL so bodies can link back.
async fn serve(make: impl FnOnce(&str) -> Handler) -> (Client, String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let handler = make(&base);
    let hits = Arc::new(AtomicUsize::new(0));
    let h2 = hits.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            h2.fetch_add(1, Ordering::SeqCst);
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
                let (status, body) = handler(&path);
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
    let client = Client::with_options(
        base.clone(),
        FetchOptions {
            retries: 0,
            ..FetchOptions::default()
        },
    );
    (client, base, hits)
}

fn json_body(v: Value) -> (u16, Vec<u8>) {
    (200, serde_json::to_vec(&v).unwrap())
}

fn not_found() -> (u16, Vec<u8>) {
    (404, b"{}".to_vec())
}

fn sha512(bytes: &[u8]) -> String {
    hex::encode(Sha512::digest(bytes))
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' => {
                out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap()
}

fn profile(paths: &Paths) -> Profile {
    let _ = paths;
    let mut p = Profile::new(PID.into(), 0);
    p.mc_version = "1.21".into();
    p.loader = Loader::Fabric;
    p
}

fn version_json(
    id: &str,
    project: &str,
    date: &str,
    kind: &str,
    file: Option<(&str, &str, &[u8])>,
    deps: Value,
) -> Value {
    let files = match file {
        Some((url, name, bytes)) => json!([{
            "url": url, "filename": name, "primary": true, "size": bytes.len(),
            "hashes": {"sha512": sha512(bytes)}
        }]),
        None => json!([]),
    };
    json!({
        "id": id, "project_id": project, "name": id, "version_number": id,
        "version_type": kind, "date_published": date,
        "game_versions": ["1.21"], "loaders": ["fabric"],
        "files": files, "dependencies": deps
    })
}

fn rec(dir: &Path, project: &str, deps: &[(&str, &str)]) -> ModRecord {
    let file = dir.join(format!("{project}.jar"));
    std::fs::write(&file, project).unwrap();
    ModRecord {
        project_id: Some(project.into()),
        title: Some(project.into()),
        version_id: Some(format!("{project}-compatible")),
        file: Some(file.to_string_lossy().into_owned()),
        filename: Some(format!("{project}.jar")),
        enabled: Some(true),
        required_dependencies: Some(
            deps.iter()
                .map(|(p, v)| ModDependency {
                    project_id: (*p).into(),
                    version_id: Some((*v).into()),
                })
                .collect(),
        ),
        ..ModRecord::default()
    }
}

fn ids(v: &[Affected]) -> Vec<&str> {
    v.iter().map(|a| a.project_id.as_str()).collect()
}

// --- ported from test/content-library.test.js ---

#[test]
fn instance_content_is_routed_to_minecraft_folders() {
    let paths = Paths::new("/data");
    let base = paths.instance_dir(PID).unwrap();
    assert_eq!(
        target_dir_for(&paths, ModType::Mod, PID).unwrap(),
        base.join("mods")
    );
    assert_eq!(
        target_dir_for(&paths, ModType::Resourcepack, PID).unwrap(),
        base.join("resourcepacks")
    );
    assert_eq!(
        target_dir_for(&paths, ModType::Shader, PID).unwrap(),
        base.join("shaderpacks")
    );
    assert_eq!(
        target_dir_for(&paths, ModType::Datapack, PID).unwrap(),
        base.join("datapacks")
    );
    assert_eq!(mods_dir(&paths, PID).unwrap(), base.join("mods"));
    assert_eq!(
        target_dir_for(&paths, ModType::Mod, "../evil")
            .unwrap_err()
            .code(),
        "invalid_profile_id"
    );
}

#[test]
fn dependencies_are_reused_only_at_the_exact_version() {
    let mods = vec![ModRecord {
        project_id: Some("sodium".into()),
        version_id: Some("sodium-0.8.13".into()),
        ..ModRecord::default()
    }];
    assert!(has_exact_installed_version(
        &mods,
        "sodium",
        "sodium-0.8.13"
    ));
    assert!(!has_exact_installed_version(
        &mods,
        "sodium",
        "sodium-0.8.7"
    ));
}

#[tokio::test]
async fn dependency_trees_disable_and_enable_recursively() {
    let dir = tempfile::tempdir().unwrap();
    let mods = vec![
        rec(dir.path(), "sodium", &[]),
        rec(dir.path(), "iris", &[("sodium", "sodium-compatible")]),
        rec(dir.path(), "shader-tools", &[("iris", "iris-compatible")]),
    ];
    let dependents: Vec<_> = installed_dependents(&mods, "sodium")
        .iter()
        .map(|r| r.project_id.clone().unwrap())
        .collect();
    assert_eq!(dependents, ["iris"]);

    let disabled = set_enabled_with_dependencies(&mods, "sodium", false)
        .await
        .unwrap();
    assert_eq!(ids(&disabled.affected), ["shader-tools", "iris", "sodium"]);
    assert!(disabled
        .mods
        .iter()
        .all(|m| m.enabled == Some(false) && m.file.as_ref().unwrap().ends_with(".disabled")));
    assert!(dir.path().join("sodium.jar.disabled").exists());
    assert!(!dir.path().join("sodium.jar").exists());

    let enabled = set_enabled_with_dependencies(&disabled.mods, "shader-tools", true)
        .await
        .unwrap();
    assert_eq!(ids(&enabled.affected), ["sodium", "iris", "shader-tools"]);
    assert!(enabled
        .mods
        .iter()
        .all(|m| m.enabled == Some(true) && m.file.as_ref().unwrap().ends_with(".jar")));
}

#[tokio::test]
async fn enable_fails_on_missing_or_mismatched_dependency() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = rec(dir.path(), "a", &[("b", "b-compatible")]);
    a.enabled = Some(false);
    let err = set_enabled_with_dependencies(std::slice::from_ref(&a), "a", true)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "modrinth.dependency_not_installed");

    let b = rec(dir.path(), "b", &[]);
    let mut a2 = rec(dir.path(), "a", &[("b", "b-other")]);
    a2.enabled = Some(false);
    let err = set_enabled_with_dependencies(&[a2, b], "a", true)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "modrinth.dependency_version_mismatch");
}

// --- API client ---

#[tokio::test]
async fn search_builds_facets_and_caches() {
    let seen = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let s2 = seen.clone();
    let (client, _, hits) = serve(move |_| {
        Arc::new(move |path| {
            s2.lock().unwrap().push(path.to_string());
            json_body(json!({"hits": [], "total_hits": 0}))
        })
    })
    .await;

    let params = SearchParams {
        query: "sod ium".into(),
        loader: "quilt".into(),
        game_version: "1.21".into(),
        categories: vec!["optimization".into()],
        open_source_only: true,
        ..SearchParams::default()
    };
    client.search(&params).await.unwrap();
    client.search(&params).await.unwrap();
    assert_eq!(hits.load(Ordering::SeqCst), 1, "second call is cached");

    let path = seen.lock().unwrap()[0].clone();
    let query = decode(path.split_once('?').unwrap().1);
    assert!(query.contains("query=sod ium"), "{query}");
    let facets = query
        .split('&')
        .find_map(|p| p.strip_prefix("facets="))
        .unwrap();
    let facets: Value = serde_json::from_str(facets).unwrap();
    assert_eq!(
        facets,
        json!([
            ["project_type:mod"],
            ["categories:quilt", "categories:fabric"],
            ["versions:1.21"],
            ["categories:optimization"],
            ["open_source:true"]
        ])
    );

    // Loader facets only apply to mods; vanilla adds none.
    let shader = SearchParams {
        project_type: "shader".into(),
        loader: "fabric".into(),
        ..SearchParams::default()
    };
    client.search(&shader).await.unwrap();
    let path = seen.lock().unwrap()[1].clone();
    assert!(decode(&path).contains(r#"[["project_type:shader"]]"#));
    let neo = SearchParams {
        loader: "neoforge".into(),
        ..SearchParams::default()
    };
    client.search(&neo).await.unwrap();
    assert!(
        decode(&seen.lock().unwrap()[2]).contains(r#"["categories:neoforge","categories:forge"]"#)
    );
}

#[tokio::test]
async fn versions_query_uses_loader_fallbacks() {
    let seen = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let s2 = seen.clone();
    let (client, _, _) = serve(move |_| {
        Arc::new(move |path| {
            s2.lock().unwrap().push(decode(path));
            json_body(json!([]))
        })
    })
    .await;
    client
        .versions("a b", Some("quilt"), Some("1.21"))
        .await
        .unwrap();
    client.versions("x", Some("vanilla"), None).await.unwrap();
    let seen = seen.lock().unwrap();
    assert_eq!(
        seen[0],
        r#"/project/a b/version?loaders=["quilt","fabric"]&game_versions=["1.21"]"#
    );
    assert_eq!(seen[1], "/project/x/version");
}

#[tokio::test]
async fn in_flight_requests_are_deduplicated() {
    let (client, _, hits) = serve(|_| {
        Arc::new(|_| {
            std::thread::sleep(Duration::from_millis(150));
            json_body(json!({"id": "p", "slug": "p", "title": "P", "project_type": "mod"}))
        })
    })
    .await;
    let (a, b, c) = tokio::join!(
        client.project("p"),
        client.project("p"),
        client.project("p")
    );
    assert!(a.is_ok() && b.is_ok() && c.is_ok());
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

fn v(id: &str, date: &str, kind: &str) -> Version {
    serde_json::from_value(version_json(id, "p", date, kind, None, json!([]))).unwrap()
}

#[test]
fn pick_version_prefers_newest_release() {
    let list = vec![
        v("old-release", "2024-01-01T00:00:00Z", "release"),
        v("new-beta", "2025-06-01T00:00:00Z", "beta"),
        v("new-release", "2025-01-01T00:00:00Z", "release"),
    ];
    assert_eq!(pick_version(&list).unwrap().id, "new-release");
    let betas = vec![
        v("a", "2024-01-01T00:00:00Z", "beta"),
        v("b", "2025-01-01T00:00:00Z", "alpha"),
    ];
    assert_eq!(pick_version(&betas).unwrap().id, "b");
    assert!(pick_version(&[]).is_none());

    let mut ver = v("x", "2025-01-01T00:00:00Z", "release");
    ver.files = vec![
        VersionFile {
            filename: "first.jar".into(),
            ..VersionFile::default()
        },
        VersionFile {
            filename: "main.jar".into(),
            primary: true,
            ..VersionFile::default()
        },
    ];
    assert_eq!(primary_file(&ver).unwrap().filename, "main.jar");
    ver.files[1].primary = false;
    assert_eq!(primary_file(&ver).unwrap().filename, "first.jar");
}

// --- install / uninstall ---

fn mod_server(iris_deps: Value) -> impl FnOnce(&str) -> Handler {
    move |base: &str| {
        let base = base.to_string();
        Arc::new(move |full: &str| {
            let path = full.split('?').next().unwrap();
            let pr = |id: &str| json!({"id": id, "slug": id, "title": id.to_uppercase(), "project_type": "mod", "icon_url": null});
            match path {
                "/project/iris" => json_body(pr("iris")),
                "/project/sodium" => json_body(pr("sodium")),
                "/projects" => json_body(json!([pr("sodium")])),
                "/project/iris/version" => json_body(json!([version_json(
                    "iris-1",
                    "iris",
                    "2025-01-01T00:00:00Z",
                    "release",
                    Some((&format!("{base}/files/iris.jar"), "iris.jar", b"IRIS")),
                    iris_deps.clone()
                )])),
                "/project/sodium/version" => json_body(json!([version_json(
                    "sodium-1",
                    "sodium",
                    "2025-01-01T00:00:00Z",
                    "release",
                    Some((&format!("{base}/files/sodium.jar"), "sodium.jar", b"SODIUM")),
                    json!([])
                )])),
                "/files/iris.jar" => (200, b"IRIS".to_vec()),
                "/files/sodium.jar" => (200, b"SODIUM".to_vec()),
                _ => not_found(),
            }
        })
    }
}

#[tokio::test]
async fn install_pulls_required_dependencies_and_verifies_hashes() {
    let (client, _, _) = serve(mod_server(json!([
        {"project_id": "sodium", "dependency_type": "required"},
        {"project_id": "other", "dependency_type": "optional"}
    ])))
    .await;
    let root = tempfile::tempdir().unwrap();
    let paths = Paths::new(root.path());
    let mut prof = profile(&paths);
    let logs = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let l2 = logs.clone();
    let log = move |l: &str| l2.lock().unwrap().push(l.to_string());

    let installed = install_project(&client, &paths, &prof, "iris", None, true, &log)
        .await
        .unwrap();
    assert_eq!(installed.len(), 2);
    let iris = &installed[0];
    let sodium = &installed[1];
    assert_eq!(iris.dependency, Some(false));
    assert_eq!(sodium.dependency, Some(true));
    assert_eq!(
        iris.required_dependencies,
        Some(vec![ModDependency {
            project_id: "sodium".into(),
            version_id: Some("sodium-1".into())
        }])
    );
    let mods_dir = paths.instance_dir(PID).unwrap().join("mods");
    assert_eq!(std::fs::read(mods_dir.join("iris.jar")).unwrap(), b"IRIS");
    assert_eq!(
        std::fs::read(sodium.file.as_ref().unwrap()).unwrap(),
        b"SODIUM"
    );
    assert!(logs.lock().unwrap()[0].contains("1 required dependency"));

    // Merge, then install again: the exact dependency build is reused.
    prof.mods = merge_installed(&prof.mods, installed).await;
    assert_eq!(prof.mods.len(), 2);
    assert!(prof.mods.iter().all(|m| m.enabled == Some(true)));
    let again = install_project(&client, &paths, &prof, "iris", None, true, &|_| {})
        .await
        .unwrap();
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].project_id.as_deref(), Some("iris"));
}

#[tokio::test]
async fn install_rejects_bad_hash_and_unsafe_filename() {
    let (client, _, _) = serve(|base| {
        let base = base.to_string();
        Arc::new(move |full: &str| {
            let path = full.split('?').next().unwrap();
            let project =
                json!({"id": "bad", "slug": "bad", "title": "Bad", "project_type": "mod"});
            match path {
                "/project/bad" => json_body(project),
                "/version/evil" => json_body(version_json(
                    "evil",
                    "bad",
                    "2025-01-01T00:00:00Z",
                    "release",
                    Some((&format!("{base}/files/x.jar"), "../escape.jar", b"X")),
                    json!([]),
                )),
                "/version/tampered" => json_body(version_json(
                    "tampered",
                    "bad",
                    "2025-01-01T00:00:00Z",
                    "release",
                    Some((&format!("{base}/files/x.jar"), "x.jar", b"EXPECTED")),
                    json!([]),
                )),
                "/files/x.jar" => (200, b"TAMPERED".to_vec()),
                _ => not_found(),
            }
        })
    })
    .await;
    let root = tempfile::tempdir().unwrap();
    let paths = Paths::new(root.path());
    let prof = profile(&paths);
    let err = install_project(&client, &paths, &prof, "bad", Some("evil"), true, &|_| {})
        .await
        .unwrap_err();
    assert_eq!(err.code(), "modrinth.invalid_filename");
    let err = install_project(
        &client,
        &paths,
        &prof,
        "bad",
        Some("tampered"),
        true,
        &|_| {},
    )
    .await
    .unwrap_err();
    assert!(err.code().starts_with("net."), "{}", err.code());
    let mods = paths.instance_dir(PID).unwrap().join("mods");
    let leftovers: Vec<_> = std::fs::read_dir(&mods)
        .map(|d| d.collect())
        .unwrap_or_default();
    assert!(leftovers.is_empty(), "no jar or temp file stays behind");
}

#[tokio::test]
async fn uninstall_is_blocked_by_dependents_and_otherwise_removes() {
    let dir = tempfile::tempdir().unwrap();
    let (client, _, hits) = serve(|_| Arc::new(|_| not_found())).await;
    let mods = vec![
        rec(dir.path(), "sodium", &[]),
        rec(dir.path(), "iris", &[("sodium", "sodium-compatible")]),
    ];

    let blocked = uninstall(&client, &mods, "sodium").await.unwrap();
    match blocked {
        UninstallOutcome::Blocked { dependents, mods } => {
            assert_eq!(
                dependents,
                [Dependent {
                    project_id: "iris".into(),
                    title: "iris".into()
                }]
            );
            assert_eq!(mods.len(), 2);
        }
        UninstallOutcome::Removed { .. } => panic!("must be blocked"),
    }
    assert!(dir.path().join("sodium.jar").exists());

    let removed = uninstall(&client, &mods, "iris").await.unwrap();
    let UninstallOutcome::Removed { mods: left } = removed else {
        panic!("iris has no dependents");
    };
    assert_eq!(left.len(), 1);
    assert!(!dir.path().join("iris.jar").exists());
    assert_eq!(hits.load(Ordering::SeqCst), 0, "graph was already known");
}

#[tokio::test]
async fn hydrate_backfills_old_records_and_flags_failures() {
    let (client, base, _) = serve(|_| {
        Arc::new(|full: &str| match full.split('?').next().unwrap() {
            "/version/iris-old" => json_body(version_json(
                "iris-old",
                "iris",
                "2025-01-01T00:00:00Z",
                "release",
                None,
                json!([
                    {"project_id": "sodium", "dependency_type": "required"},
                    {"version_id": "lib-v", "dependency_type": "required"}
                ]),
            )),
            "/version/lib-v" => json_body(version_json(
                "lib-v",
                "lib",
                "2025-01-01T00:00:00Z",
                "release",
                None,
                json!([]),
            )),
            _ => not_found(),
        })
    })
    .await;
    let _ = base;
    let old = |p: &str, v: &str| ModRecord {
        project_id: Some(p.into()),
        version_id: Some(v.into()),
        title: Some(p.into()),
        ..ModRecord::default()
    };
    let mods = vec![old("iris", "iris-old"), old("sodium", "sodium-9")];
    let h = hydrate_dependency_metadata(&client, &mods).await.unwrap();
    // sodium's own version 404s: unknown, not guessed.
    assert!(!h.complete);
    assert!(h.changed);
    assert_eq!(
        h.mods[0].required_dependencies,
        Some(vec![
            ModDependency {
                project_id: "sodium".into(),
                version_id: Some("sodium-9".into())
            },
            ModDependency {
                project_id: "lib".into(),
                version_id: Some("lib-v".into())
            },
        ])
    );
    assert!(h.mods[1].required_dependencies.is_none());

    let dir = tempfile::tempdir().unwrap();
    let mut a = rec(dir.path(), "sodium", &[]);
    a.required_dependencies = None;
    let err = toggle(&client, &[a], "sodium", false).await.unwrap_err();
    assert_eq!(err.code(), "modrinth.metadata_incomplete");
}

// --- updates ---

fn update_server() -> impl FnOnce(&str) -> Handler {
    |_| {
        Arc::new(|full: &str| match full.split('?').next().unwrap() {
            "/version/sodium-2" => json_body(version_json(
                "sodium-2",
                "sodium",
                "2025-02-01T00:00:00Z",
                "release",
                None,
                json!([]),
            )),
            "/project/iris/version" => json_body(json!([version_json(
                "iris-compatible",
                "iris",
                "2025-01-01T00:00:00Z",
                "release",
                None,
                json!([])
            )])),
            "/project/lithium/version" => json_body(json!([version_json(
                "lithium-new",
                "lithium",
                "2025-03-01T00:00:00Z",
                "release",
                None,
                json!([])
            )])),
            _ => not_found(),
        })
    }
}

#[tokio::test]
async fn updates_honour_exact_version_pins() {
    let (client, _, _) = serve(update_server()).await;
    let dir = tempfile::tempdir().unwrap();
    let mut prof = profile(&Paths::new(dir.path()));
    prof.mods = vec![
        rec(dir.path(), "sodium", &[]),
        rec(dir.path(), "iris", &[("sodium", "sodium-2")]),
        rec(dir.path(), "lithium", &[]),
    ];
    let updates = check_updates(&client, &prof).await.unwrap();
    let by = |id: &str| updates.iter().find(|u| u.project_id == id);

    // Same project id is not enough: iris pins sodium-2.
    let sodium = by("sodium").unwrap();
    assert_eq!(sodium.compatibility, Some(true));
    assert_eq!(sodium.version_id.as_deref(), Some("sodium-2"));
    assert_eq!(sodium.from.as_deref(), None);
    assert!(by("iris").is_none(), "already latest");
    let lithium = by("lithium").unwrap();
    assert_eq!(lithium.compatibility, Some(false));
    assert_eq!(lithium.to.as_deref(), Some("lithium-new"));
}

#[tokio::test]
async fn updates_report_conflicting_pins() {
    let (client, _, _) = serve(update_server()).await;
    let dir = tempfile::tempdir().unwrap();
    let mut prof = profile(&Paths::new(dir.path()));
    prof.mods = vec![
        rec(dir.path(), "sodium", &[]),
        rec(dir.path(), "iris", &[("sodium", "sodium-2")]),
        rec(dir.path(), "other", &[("sodium", "sodium-3")]),
    ];
    let all = check_all_updates(&client, std::slice::from_ref(&prof)).await;
    assert_eq!(all[0].profile_id, PID);
    let conflict = all[0]
        .updates
        .iter()
        .find(|u| u.project_id == "sodium")
        .unwrap();
    assert!(conflict.conflict);
    let owners: Vec<_> = conflict
        .required_by
        .iter()
        .map(|r| (r.project_id.as_str(), r.version_id.as_str()))
        .collect();
    assert_eq!(owners, [("iris", "sodium-2"), ("other", "sodium-3")]);
    let wire = serde_json::to_value(conflict).unwrap();
    assert_eq!(wire["conflict"], true);
    assert!(wire.get("to").is_none());

    // Disabled parents do not pin.
    prof.mods[2].enabled = Some(false);
    let updates = check_updates(&client, &prof).await.unwrap();
    assert!(updates
        .iter()
        .any(|u| u.project_id == "sodium" && !u.conflict));
}

// --- .mrpack ---

fn build_pack(index: &Value, entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("modrinth.index.json", opts).unwrap();
    zip.write_all(&serde_json::to_vec(index).unwrap()).unwrap();
    for (name, data) in entries {
        zip.start_file(*name, opts).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn pack_server(pack: Vec<u8>, extra: Vec<(&'static str, Vec<u8>)>) -> impl FnOnce(&str) -> Handler {
    move |base: &str| {
        let base = base.to_string();
        Arc::new(move |full: &str| {
            let path = full.split('?').next().unwrap();
            if path == "/version/pack-v1" {
                return json_body(version_json(
                    "pack-v1",
                    "pack",
                    "2025-01-01T00:00:00Z",
                    "release",
                    Some((&format!("{base}/files/pack.mrpack"), "pack.mrpack", &pack)),
                    json!([]),
                ));
            }
            if path == "/files/pack.mrpack" {
                return (200, pack.clone());
            }
            for (p, body) in &extra {
                if path == *p {
                    return (200, body.clone());
                }
            }
            not_found()
        })
    }
}

#[tokio::test]
async fn mrpack_installs_files_overrides_and_detects_loader() {
    let a = b"AAAA".to_vec();
    let index = |base: &str| {
        json!({
            "name": "Cool Pack",
            "dependencies": {"minecraft": "1.21", "fabric-loader": "0.16.5"},
            "files": [
                {"path": "mods/a.jar", "hashes": {"sha512": sha512(&a)},
                 "downloads": [format!("{base}/files/a.jar")], "fileSize": a.len()},
                {"path": "mods/server-only.jar", "env": {"client": "unsupported"},
                 "downloads": [format!("{base}/files/missing.jar")]}
            ]
        })
    };
    // The pack's own URL needs the base, so build it behind the server's bind.
    let listener_base = Arc::new(std::sync::Mutex::new(String::new()));
    let lb = listener_base.clone();
    let a2 = a.clone();
    let (client, base, _) = serve(move |base| {
        *lb.lock().unwrap() = base.to_string();
        let pack = build_pack(
            &index(base),
            &[
                ("overrides/config/x.toml", b"x=1"),
                ("client-overrides/config/y.toml", b"y=2"),
            ],
        );
        pack_server(pack, vec![("/files/a.jar", a2)])(base)
    })
    .await;
    assert_eq!(*listener_base.lock().unwrap(), base);

    let root = tempfile::tempdir().unwrap();
    let paths = Paths::new(root.path());
    let prof = profile(&paths);
    let progress = Arc::new(std::sync::Mutex::new(Vec::new()));
    let p2 = progress.clone();
    let cb: Arc<ProgressFn> = Arc::new(move |p| p2.lock().unwrap().push(p.clone()));
    let info = install_modpack(&client, &paths, &prof, "pack-v1", &|_| {}, Some(cb))
        .await
        .unwrap();
    assert_eq!(
        info,
        ModpackInfo {
            name: Some("Cool Pack".into()),
            mc_version: Some("1.21".into()),
            loader: Loader::Fabric,
            loader_version: "0.16.5".into(),
            file_count: 1,
        }
    );
    let game = paths.instance_dir(PID).unwrap();
    assert_eq!(std::fs::read(game.join("mods/a.jar")).unwrap(), a);
    assert!(!game.join("mods/server-only.jar").exists());
    assert_eq!(std::fs::read(game.join("config/x.toml")).unwrap(), b"x=1");
    assert_eq!(std::fs::read(game.join("config/y.toml")).unwrap(), b"y=2");
    assert!(!paths.cache().join("pack.mrpack").exists(), "pack removed");
    let p = progress.lock().unwrap();
    assert_eq!(p.len(), 1);
    assert_eq!(
        (p[0].done, p[0].total, p[0].label.as_str()),
        (1, 1, "a.jar")
    );
}

#[tokio::test]
async fn mrpack_with_traversal_entry_is_rejected() {
    let index = json!({"name": "Evil", "dependencies": {"minecraft": "1.21"}, "files": []});
    let pack = build_pack(&index, &[("overrides/../../evil.txt", b"pwned")]);
    let (client, _, _) = serve(pack_server(pack, vec![])).await;
    let root = tempfile::tempdir().unwrap();
    let paths = Paths::new(root.path());
    let prof = profile(&paths);
    let err = install_modpack(&client, &paths, &prof, "pack-v1", &|_| {}, None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "modrinth.archive", "{err}");
    assert!(!root.path().join("instances/evil.txt").exists());
    assert!(!root.path().join("evil.txt").exists());
    assert!(!paths.cache().join("pack.mrpack").exists());
}

#[tokio::test]
async fn mrpack_with_traversal_file_path_is_rejected_before_downloading() {
    let index = json!({"name": "Evil", "dependencies": {}, "files": [
        {"path": "../../escape.jar", "downloads": ["http://127.0.0.1:1/x"]}]});
    let pack = build_pack(&index, &[]);
    let (client, _, _) = serve(pack_server(pack, vec![])).await;
    let root = tempfile::tempdir().unwrap();
    let paths = Paths::new(root.path());
    let err = install_modpack(&client, &paths, &profile(&paths), "pack-v1", &|_| {}, None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "modrinth.archive");
}

#[tokio::test]
async fn mrpack_without_index_or_with_huge_index_is_invalid() {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("other.txt", opts).unwrap();
    zip.write_all(b"x").unwrap();
    let no_index = zip.finish().unwrap().into_inner();
    let (client, _, _) = serve(pack_server(no_index, vec![])).await;
    let root = tempfile::tempdir().unwrap();
    let paths = Paths::new(root.path());
    let err = install_modpack(&client, &paths, &profile(&paths), "pack-v1", &|_| {}, None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "modrinth.invalid_modpack");

    let big = json!({"name": "x".repeat(MAX_INDEX_BYTES as usize + 10)});
    let pack = build_pack(&big, &[]);
    let (client, _, _) = serve(pack_server(pack, vec![])).await;
    let err = install_modpack(&client, &paths, &profile(&paths), "pack-v1", &|_| {}, None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "modrinth.invalid_modpack");
}

#[test]
fn loader_detection_follows_dependency_priority() {
    let d = |pairs: &[(&str, &str)]| {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    };
    assert_eq!(
        detect_loader(&d(&[("quilt-loader", "0.2"), ("forge", "1")])),
        (Loader::Quilt, "0.2".into())
    );
    assert_eq!(
        detect_loader(&d(&[("neoforge", "21.1.1"), ("forge", "1")])),
        (Loader::Neoforge, "21.1.1".into())
    );
    assert_eq!(
        detect_loader(&d(&[("forge", "47")])),
        (Loader::Forge, "47".into())
    );
    assert_eq!(
        detect_loader(&d(&[("minecraft", "1.21")])),
        (Loader::Vanilla, String::new())
    );
}

/// A server that reads the whole request (head and body) and records
/// `(request line, body)`.
type Seen = Arc<std::sync::Mutex<Vec<(String, Vec<u8>)>>>;

async fn post_server(
    status: u16,
    answer: Value,
) -> (Client, Arc<std::sync::Mutex<Vec<(String, Vec<u8>)>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen: Seen = Arc::default();
    let log = seen.clone();
    let answer = serde_json::to_vec(&answer).unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let (log, answer) = (log.clone(), answer.clone());
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut tmp = [0u8; 4096];
                let (head_end, len) = loop {
                    if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&buf[..i]).to_ascii_lowercase();
                        let len = head
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length:")?.trim().parse().ok())
                            .unwrap_or(0usize);
                        break (i + 4, len);
                    }
                    match sock.read(&mut tmp).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    }
                };
                while buf.len() < head_end + len {
                    match sock.read(&mut tmp).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    }
                }
                let line = String::from_utf8_lossy(&buf)
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .to_string();
                log.lock()
                    .unwrap()
                    .push((line, buf[head_end..head_end + len].to_vec()));
                let mut out = format!(
                    "HTTP/1.1 {status} X\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
                    answer.len()
                )
                .into_bytes();
                out.extend_from_slice(&answer);
                let _ = sock.write_all(&out).await;
            });
        }
    });
    let client = Client::with_options(
        base,
        FetchOptions {
            retries: 0,
            ..FetchOptions::default()
        },
    );
    (client, seen)
}

#[tokio::test]
async fn versions_by_sha1_posts_the_batch_and_maps_hashes_to_versions() {
    let sha = "ab".repeat(20);
    let (client, seen) = post_server(
        200,
        json!({
            sha.to_uppercase(): {"id": "v1", "project_id": "p1", "loaders": ["fabric"],
                "files": [{"url": "u", "filename": "a.jar", "primary": true, "hashes": {"sha1": sha}}]}
        }),
    )
    .await;
    let other = "cd".repeat(20);
    let got = client
        .versions_by_sha1(&[sha.to_uppercase(), sha.clone(), other.clone()])
        .await
        .unwrap();
    assert_eq!(got.len(), 1, "unknown hashes are absent");
    assert_eq!(got[&sha].id, "v1");
    assert_eq!(got[&sha].loaders, vec!["fabric".to_string()]);

    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "one request for the whole batch");
    assert!(seen[0].0.starts_with("POST /version_files "));
    let body: Value = serde_json::from_slice(&seen[0].1).unwrap();
    assert_eq!(body, json!({"hashes": [sha, other], "algorithm": "sha1"}));
}

#[tokio::test]
async fn versions_by_sha1_fails_on_http_errors_and_skips_empty_input() {
    let (client, seen) = post_server(500, json!({})).await;
    assert!(client.versions_by_sha1(&["ab".repeat(20)]).await.is_err());
    assert!(client.versions_by_sha1(&[]).await.unwrap().is_empty());
    assert_eq!(seen.lock().unwrap().len(), 1);
}
