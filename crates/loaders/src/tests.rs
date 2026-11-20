#![allow(clippy::unwrap_used)]

use crate::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn serve(routes: HashMap<String, Vec<u8>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let routes = Arc::new(routes);
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let routes = routes.clone();
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
                let (status, body) = match routes.get(&path) {
                    Some(b) => (200, b.clone()),
                    None => (404, Vec::new()),
                };
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
    base
}

fn loaders(base: &str) -> Loaders {
    let mut l = Loaders::new(Endpoints {
        fabric_meta: format!("{base}/fabric"),
        quilt_meta: format!("{base}/quilt"),
        neoforge_maven: format!("{base}/neo"),
        forge_maven: format!("{base}/forge"),
    });
    l.fetch.retries = 0;
    l
}

fn noop(_: &str) {}

#[test]
fn neoforge_prefixes() {
    assert_eq!(neoforge_prefix("1.21.4"), "21.4.");
    assert_eq!(neoforge_prefix("1.21"), "21.0.");
    assert_eq!(neoforge_prefix("26.1"), "26.1.");
    assert_eq!(neoforge_prefix("26.1.2"), "26.1.");
}

#[test]
fn stable_flags() {
    assert!(quilt_stable("0.26.0"));
    assert!(!quilt_stable("0.27.0-beta.1"));
    assert!(!quilt_stable("0.27.0-Pre.2"));
    assert!(neoforge_stable("21.4.100"));
    assert!(!neoforge_stable("21.4.100-beta"));
    let list = vec![
        LoaderVersion {
            version: "2-beta".into(),
            stable: false,
            full: None,
        },
        LoaderVersion {
            version: "1".into(),
            stable: true,
            full: None,
        },
    ];
    assert_eq!(default_version(&list).unwrap().version, "1");
    assert_eq!(default_version(&list[..1]).unwrap().version, "2-beta");
    assert!(default_version(&[]).is_none());
}

#[test]
fn ids() {
    assert_eq!(
        version_id_for(Loader::Fabric, "1.21", "0.16"),
        "fabric-loader-0.16-1.21"
    );
    assert_eq!(
        version_id_for(Loader::Quilt, "1.21", "0.2"),
        "quilt-loader-0.2-1.21"
    );
    assert_eq!(
        version_id_for(Loader::NeoForge, "1.21", "21.0.1"),
        "neoforge-21.0.1"
    );
    assert_eq!(
        version_id_for(Loader::Forge, "1.20.1", "47.3.0"),
        "1.20.1-forge-47.3.0"
    );
    assert_eq!(version_id_for(Loader::Vanilla, "1.20.1", ""), "1.20.1");
}

#[test]
fn maven_parse() {
    let xml = "<metadata><versioning><versions>\n<version>21.4.1</version>\n<version> 21.4.2-beta </version></versions></versioning></metadata>";
    assert_eq!(parse_maven_versions(xml), vec!["21.4.1", "21.4.2-beta"]);
    assert!(parse_maven_versions("nope").is_empty());
}

#[tokio::test]
async fn list_all_loaders() {
    let mut r = HashMap::new();
    r.insert(
        "/fabric/versions/loader/1.21".to_string(),
        br#"[{"loader":{"version":"0.16.1","stable":false}},{"loader":{"version":"0.16.0","stable":true}}]"#.to_vec(),
    );
    r.insert(
        "/quilt/versions/loader/1.21".to_string(),
        br#"[{"loader":{"version":"0.27.0-beta.1"}},{"loader":{"version":"0.26.0"}}]"#.to_vec(),
    );
    r.insert(
        "/neo/maven-metadata.xml".to_string(),
        b"<v><version>20.4.1</version><version>21.0.5</version><version>21.0.6-beta</version></v>"
            .to_vec(),
    );
    r.insert(
        "/forge/maven-metadata.xml".to_string(),
        b"<v><version>1.21-51.0.1</version><version>1.20.1-47.3.0</version><version>1.20.1-47.3.1</version></v>".to_vec(),
    );
    r.insert(
        "/fabric/versions/game".to_string(),
        br#"[{"version":"1.21","stable":true}]"#.to_vec(),
    );
    let l = loaders(&serve(r).await);

    let f = l.list_versions(Loader::Fabric, "1.21").await.unwrap();
    assert_eq!(f.len(), 2);
    assert!(!f[0].stable && f[1].stable);
    assert_eq!(default_version(&f).unwrap().version, "0.16.0");

    let q = l.list_versions(Loader::Quilt, "1.21").await.unwrap();
    assert!(!q[0].stable && q[1].stable);

    let n = l.list_versions(Loader::NeoForge, "1.21").await.unwrap();
    let nv: Vec<_> = n.iter().map(|v| v.version.as_str()).collect();
    assert_eq!(nv, vec!["21.0.6-beta", "21.0.5"]);
    assert!(!n[0].stable);

    let fo = l.list_versions(Loader::Forge, "1.20.1").await.unwrap();
    assert_eq!(fo[0].version, "47.3.1");
    assert_eq!(fo[0].full.as_deref(), Some("1.20.1-47.3.1"));
    assert!(l
        .list_versions(Loader::Vanilla, "1.21")
        .await
        .unwrap()
        .is_empty());

    let g = l.list_game_versions(Loader::Fabric).await.unwrap();
    assert_eq!(g[0].version, "1.21");
    assert!(l
        .list_game_versions(Loader::Forge)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn fabric_profile_write() {
    let mut r = HashMap::new();
    r.insert(
        "/fabric/versions/loader/1.21/0.16.0/profile/json".to_string(),
        br#"{"id":"fabric-loader-0.16.0-1.21","inheritsFrom":"1.21","libraries":[]}"#.to_vec(),
    );
    r.insert(
        "/quilt/versions/loader/1.21/0.26.0/profile/json".to_string(),
        br#"{"inheritsFrom":"1.21"}"#.to_vec(),
    );
    r.insert(
        "/fabric/versions/loader/1.21".to_string(),
        br#"[{"loader":{"version":"0.16.0","stable":true}}]"#.to_vec(),
    );
    let l = loaders(&serve(r).await);
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());

    let out = l
        .ensure(&paths, Loader::Fabric, "1.21", None, None, &noop)
        .await
        .unwrap();
    assert_eq!(out.version_id, "fabric-loader-0.16.0-1.21");
    assert_eq!(out.loader_version, "0.16.0");
    let raw = std::fs::read(version_json_path(&paths, &out.version_id)).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(v["inheritsFrom"], "1.21");

    // Second call hits the "already installed" path (server has no more need).
    let again = l
        .install_json_loader(&paths, Loader::Fabric, "1.21", "0.16.0", &noop)
        .await
        .unwrap();
    assert_eq!(again, out.version_id);

    // Missing id is filled in.
    let q = l
        .ensure(&paths, Loader::Quilt, "1.21", Some("0.26.0"), None, &noop)
        .await
        .unwrap();
    assert_eq!(q.version_id, "quilt-loader-0.26.0-1.21");
    assert!(version_json_path(&paths, &q.version_id).exists());

    // Vanilla passthrough.
    let v = l
        .ensure(&paths, Loader::Vanilla, "1.21", None, None, &noop)
        .await
        .unwrap();
    assert_eq!(v.version_id, "1.21");
}

#[tokio::test]
async fn no_versions_error() {
    let mut r = HashMap::new();
    r.insert("/fabric/versions/loader/9.9".to_string(), b"[]".to_vec());
    let l = loaders(&serve(r).await);
    let dir = tempfile::tempdir().unwrap();
    let e = l
        .ensure(
            &Paths::new(dir.path()),
            Loader::Fabric,
            "9.9",
            None,
            None,
            &noop,
        )
        .await
        .unwrap_err();
    assert_eq!(e.code(), "loaders.no_versions");
}

#[cfg(unix)]
mod forge {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn fake_java(dir: &std::path::Path, body: &str) -> JavaInfo {
        let p = dir.join("fake-java");
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        JavaInfo {
            path: p,
            major: 21,
            raw: String::new(),
        }
    }

    const JAR_SHA1: &str = "f92e777f4341930bad9b2422283c4680d00dbc06";

    async fn setup() -> (Loaders, tempfile::TempDir, Paths) {
        let mut r = HashMap::new();
        r.insert(
            "/neo/21.0.5/neoforge-21.0.5-installer.jar.sha1".to_string(),
            JAR_SHA1.as_bytes().to_vec(),
        );
        r.insert(
            "/forge/1.20.1-47.3.0/forge-1.20.1-47.3.0-installer.jar.sha1".to_string(),
            format!("{JAR_SHA1}  forge-installer.jar\n").into_bytes(),
        );
        r.insert(
            "/neo/21.0.5/neoforge-21.0.5-installer.jar".to_string(),
            b"jar".to_vec(),
        );
        r.insert(
            "/forge/1.20.1-47.3.0/forge-1.20.1-47.3.0-installer.jar".to_string(),
            b"jar".to_vec(),
        );
        let l = loaders(&serve(r).await);
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().join("root"));
        (l, dir, paths)
    }

    #[tokio::test]
    async fn neoforge_flow() {
        let (l, dir, paths) = setup().await;
        let java = fake_java(
            dir.path(),
            r#"echo "args: $@"; echo oops >&2
root="$4"
mkdir -p "$root/versions/neoforge-21.0.5"
echo '{}' > "$root/versions/neoforge-21.0.5/neoforge-21.0.5.json""#,
        );
        let logs = Arc::new(Mutex::new(Vec::<String>::new()));
        let l2 = logs.clone();
        let cb = move |s: &str| l2.lock().unwrap().push(s.to_string());
        let out = l
            .ensure(
                &paths,
                Loader::NeoForge,
                "1.21",
                Some("21.0.5"),
                Some(&java),
                &cb,
            )
            .await
            .unwrap();
        assert_eq!(out.version_id, "neoforge-21.0.5");
        let logs = logs.lock().unwrap().join("\n");
        assert!(logs.contains("--installClient"), "{logs}");
        assert!(logs.contains("oops"));
        let pf: serde_json::Value = serde_json::from_slice(
            &std::fs::read(paths.root().join("launcher_profiles.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(pf["version"], 3);
        assert!(paths
            .root()
            .join("launcher_profiles_microsoft_store.json")
            .exists());
        assert!(paths.cache().join("neoforge-21.0.5-installer.jar").exists());
    }

    fn marker_java(dir: &std::path::Path) -> JavaInfo {
        // Leaves a marker when (wrongly) executed.
        let marker = dir.join("ran");
        fake_java(dir, &format!("touch {}; exit 1", marker.display()))
    }

    #[tokio::test]
    async fn installer_with_mismatching_sidecar_is_refused_and_never_run() {
        let mut r = HashMap::new();
        r.insert(
            "/neo/21.0.5/neoforge-21.0.5-installer.jar".to_string(),
            b"evil".to_vec(),
        );
        r.insert(
            "/neo/21.0.5/neoforge-21.0.5-installer.jar.sha1".to_string(),
            JAR_SHA1.as_bytes().to_vec(),
        );
        let l = loaders(&serve(r).await);
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().join("root"));
        let java = marker_java(dir.path());
        let e = l
            .ensure(
                &paths,
                Loader::NeoForge,
                "1.21",
                Some("21.0.5"),
                Some(&java),
                &noop,
            )
            .await
            .unwrap_err();
        assert_eq!(e.code(), "net.checksum_mismatch", "{e}");
        assert!(!dir.path().join("ran").exists());
        assert!(!paths.cache().join("neoforge-21.0.5-installer.jar").exists());
    }

    #[tokio::test]
    async fn installer_without_sidecar_is_refused_and_never_run() {
        let mut r = HashMap::new();
        r.insert(
            "/neo/21.0.5/neoforge-21.0.5-installer.jar".to_string(),
            b"jar".to_vec(),
        );
        let l = loaders(&serve(r).await);
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().join("root"));
        let java = marker_java(dir.path());
        let e = l
            .ensure(
                &paths,
                Loader::NeoForge,
                "1.21",
                Some("21.0.5"),
                Some(&java),
                &noop,
            )
            .await
            .unwrap_err();
        assert_eq!(e.code(), "loaders.installer_unverified");
        assert!(!dir.path().join("ran").exists());
    }

    #[tokio::test]
    async fn preplanted_cached_installer_is_not_trusted_without_matching_hash() {
        let (l, dir, paths) = setup().await;
        std::fs::create_dir_all(paths.cache()).unwrap();
        let cached = paths.cache().join("neoforge-21.0.5-installer.jar");
        std::fs::write(&cached, b"planted payload").unwrap();
        let java = fake_java(
            dir.path(),
            r#"root="$4"
mkdir -p "$root/versions/neoforge-21.0.5"
echo '{}' > "$root/versions/neoforge-21.0.5/neoforge-21.0.5.json""#,
        );
        l.ensure(
            &paths,
            Loader::NeoForge,
            "1.21",
            Some("21.0.5"),
            Some(&java),
            &noop,
        )
        .await
        .unwrap();
        // The planted file was replaced by the verified download.
        assert_eq!(std::fs::read(&cached).unwrap(), b"jar");
    }

    #[tokio::test]
    async fn forge_scan_fallback_and_failure() {
        let (l, dir, paths) = setup().await;
        // Produces a differently named dir.
        let java = fake_java(
            dir.path(),
            r#"root="$4"
mkdir -p "$root/versions/1.20.1-Forge-47.3.0"
echo '{}' > "$root/versions/1.20.1-Forge-47.3.0/x.json""#,
        );
        let out = l
            .ensure(
                &paths,
                Loader::Forge,
                "1.20.1",
                Some("47.3.0"),
                Some(&java),
                &noop,
            )
            .await
            .unwrap();
        assert_eq!(out.version_id, "1.20.1-Forge-47.3.0");

        let (l, dir, paths) = setup().await;
        let java = fake_java(dir.path(), "exit 3");
        let e = l
            .ensure(
                &paths,
                Loader::Forge,
                "1.20.1",
                Some("47.3.0"),
                Some(&java),
                &noop,
            )
            .await
            .unwrap_err();
        assert_eq!(e.code(), "loaders.installer_failed");
        assert!(e.to_string().contains('3'));

        let java = fake_java(dir.path(), "exit 0");
        let e = l
            .ensure(
                &paths,
                Loader::Forge,
                "1.20.1",
                Some("47.3.0"),
                Some(&java),
                &noop,
            )
            .await
            .unwrap_err();
        assert_eq!(e.code(), "loaders.no_profile");

        let e = l
            .ensure(&paths, Loader::Forge, "1.20.1", Some("47.3.0"), None, &noop)
            .await
            .unwrap_err();
        assert_eq!(e.code(), "loaders.java_required");
    }
}
