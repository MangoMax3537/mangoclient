#![allow(clippy::unwrap_used)]
use mango_java::*;
use std::path::{Path, PathBuf};

fn cand(path: &str, major: u32) -> Candidate {
    Candidate {
        path: PathBuf::from(path),
        major,
        id: String::new(),
    }
}

fn use_of(d: Decision) -> (String, JavaSource) {
    match d {
        Decision::Use { path, source, .. } => (path.to_string_lossy().into_owned(), source),
        Decision::Download => ("download".into(), JavaSource::Downloaded),
    }
}

#[test]
fn version_parsing() {
    let p = |s: &str| parse_java_version(s);
    assert_eq!(p("openjdk version \"1.8.0_392\" 2023-10-17"), Some(8));
    assert_eq!(p("java version \"17.0.9\" 2023-10-17 LTS"), Some(17));
    assert_eq!(p("openjdk version \"21\" 2023-09-19"), Some(21));
    assert_eq!(p("openjdk version \"25-ea\" 2025-09-16"), Some(25));
    assert_eq!(p("garbage"), None);
}

#[test]
fn required_major_fallbacks() {
    assert_eq!(required_major(Some(17), "1.8"), 17);
    assert_eq!(required_major(None, "26.1"), 25);
    assert_eq!(required_major(None, "1.21.4"), 21);
    assert_eq!(required_major(None, "1.20.1"), 21);
    assert_eq!(required_major(None, "1.19.4"), 17);
    assert_eq!(required_major(None, "1.18.2"), 17);
    assert_eq!(required_major(None, "1.17.1"), 16);
    assert_eq!(required_major(None, "1.16.5"), 8);
    assert_eq!(required_major(None, "1.7.10"), 8);
    assert_eq!(required_major(None, "weird"), 21);
    let v: serde_json::Value = serde_json::json!({"id":"1.16.5","javaVersion":{"majorVersion":11}});
    assert_eq!(required_major_from_json(&v), 11);
}

#[test]
fn resolution_order() {
    let managed = [cand("/m/17", 17), cand("/m/21", 21)];
    let system = [cand("/s/8", 8), cand("/s/21", 21), cand("/s/25", 25)];

    let custom_ok = CustomJava {
        path: "/c".into(),
        major: Some(21),
    };
    let r = decide(17, Some(&custom_ok), &managed, &system).unwrap();
    assert_eq!(use_of(r), ("/c".into(), JavaSource::Custom));
    let bad = CustomJava {
        path: "/c".into(),
        major: None,
    };
    let e = decide(17, Some(&bad), &managed, &system).unwrap_err();
    assert_eq!(e.code(), "java.not_runnable");
    let old = CustomJava {
        path: "/c".into(),
        major: Some(8),
    };
    let e = decide(17, Some(&old), &managed, &system).unwrap_err();
    assert_eq!(e.code(), "java.too_old");

    // managed exact beats everything, else managed greater beats system
    assert_eq!(
        use_of(decide(17, None, &managed, &system).unwrap()),
        ("/m/17".into(), JavaSource::Managed)
    );
    assert_eq!(
        use_of(decide(8, None, &managed, &system).unwrap()),
        ("/m/17".into(), JavaSource::Managed)
    );
    // system exact
    assert_eq!(
        use_of(decide(8, None, &[], &system).unwrap()),
        ("/s/8".into(), JavaSource::System)
    );
    // smallest newer system only when required >= 17
    assert_eq!(
        use_of(decide(17, None, &[], &system).unwrap()),
        ("/s/21".into(), JavaSource::System)
    );
    assert_eq!(
        decide(16, None, &[], &[cand("/s/21", 21)]).unwrap(),
        Decision::Download
    );
    assert_eq!(
        decide(25, None, &[], &[cand("/s/21", 21)]).unwrap(),
        Decision::Download
    );
}

#[cfg(unix)]
mod unix {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    pub fn fake_java(dir: &Path, line: &str) -> PathBuf {
        let bin = dir.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let p = bin.join("java");
        let mut f = std::fs::File::create(&p).unwrap();
        writeln!(f, "#!/bin/sh\necho '{line}' >&2").unwrap();
        f.sync_all().unwrap();
        drop(f);
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    #[tokio::test]
    async fn probe_fake_java() {
        let t = tempfile::tempdir().unwrap();
        let p = fake_java(t.path(), "openjdk version \"1.8.0_392\" 2023");
        let info = probe(&p).await.unwrap();
        assert_eq!(info.major, 8);
        assert!(info.raw.starts_with("openjdk version"));
        assert!(probe(&t.path().join("nope")).await.is_none());
    }

    #[tokio::test]
    async fn discovery_with_injected_roots() {
        let t = tempfile::tempdir().unwrap();
        let jvm = t.path().join("jvm");
        fake_java(&jvm.join("java-17-openjdk"), "openjdk version \"17.0.9\"");
        fake_java(
            &jvm.join("mac.jdk").join("Contents").join("Home"),
            "openjdk version \"21.0.1\"",
        );
        let home = fake_java(&t.path().join("home"), "openjdk version \"11.0.2\"");
        let _ = home;
        let roots = SearchRoots {
            java_home: Some(t.path().join("home")),
            path_dirs: vec![t.path().join("emptypath")],
            jvm_dirs: vec![jvm],
        };
        let mut majors: Vec<u32> = find_system_javas(&roots, Platform::current())
            .await
            .iter()
            .map(|j| j.major)
            .collect();
        majors.sort();
        assert_eq!(majors, vec![11, 17, 21]);
    }

    #[tokio::test]
    async fn managed_runtimes_and_resolve_without_network() {
        let t = tempfile::tempdir().unwrap();
        let rt = t.path().join("runtimes");
        fake_java(
            &rt.join("jre-21").join("jdk-21.0.1+12-jre"),
            "openjdk version \"21.0.1\"",
        );
        let platform = Platform::current();
        let managed = list_managed_runtimes(&rt, platform).await;
        assert_eq!(managed.len(), 1);
        assert_eq!(managed[0].id, "jre-21");
        assert!(java_executable(&rt.join("jre-21"), platform, true).is_some());

        let mut o = ResolveOptions::new(&rt, t.path().join("cache"));
        o.roots = SearchRoots::default();
        let r = resolve_java(21, &o).await.unwrap();
        assert_eq!(r.source, JavaSource::Managed);
        assert_eq!(r.major, 21);

        o.custom_path = Some(t.path().join("missing"));
        assert_eq!(
            resolve_java(21, &o).await.unwrap_err().code(),
            "java.not_runnable"
        );
    }

    fn tar_gz_with_java(version_line: &str) -> Vec<u8> {
        let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        let mut b = tar::Builder::new(gz);
        let body = format!("#!/bin/sh\necho '{version_line}' >&2\n");
        let mut h = tar::Header::new_gnu();
        h.set_size(body.len() as u64);
        h.set_mode(0o755);
        h.set_cksum();
        b.append_data(&mut h, "jdk-21.0.1+12-jre/bin/java", body.as_bytes())
            .unwrap();
        b.into_inner().unwrap().finish().unwrap()
    }

    /// Minimal HTTP server: `/v3/assets/...` returns `assets_json` (with
    /// `{BASE}` replaced), `/pkg` returns `pkg`.
    async fn serve(assets_json: String, pkg: Vec<u8>) -> String {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        let json = assets_json.replace("{BASE}", &base);
        tokio::spawn(async move {
            loop {
                let Ok((mut s, _)) = l.accept().await else {
                    return;
                };
                let (json, pkg) = (json.clone(), pkg.clone());
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 4096];
                    let n = s.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                    let (ct, body) = if req.starts_with("GET /v3/assets/latest/21/hotspot?") {
                        assert!(req.contains("image_type=jre") && req.contains("vendor=eclipse"));
                        ("application/json", json.into_bytes())
                    } else if req.starts_with("GET /pkg/") {
                        ("application/octet-stream", pkg)
                    } else {
                        ("text/plain", Vec::new())
                    };
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = s.write_all(head.as_bytes()).await;
                    let _ = s.write_all(&body).await;
                    let _ = s.shutdown().await;
                });
            }
        });
        format!("{base}/v3")
    }

    fn assets(name: &str, sha: &str, size: usize) -> String {
        serde_json::json!([{"binary": {"package": {
            "name": name, "link": format!("{{BASE}}/pkg/{name}"),
            "checksum": sha, "size": size
        }}}])
        .to_string()
    }

    #[tokio::test]
    async fn adoptium_download_and_extract() {
        use sha2::{Digest, Sha256};
        let pkg = tar_gz_with_java("openjdk version \"21.0.1\" 2023");
        let sha = hex::encode(Sha256::digest(&pkg));
        let name = "OpenJDK21U-jre_x64_linux_hotspot.tar.gz";
        let base = serve(assets(name, &sha, pkg.len()), pkg.clone()).await;

        let t = tempfile::tempdir().unwrap();
        let mut o = ResolveOptions::new(t.path().join("runtimes"), t.path().join("cache"));
        o.adoptium_base = base;
        o.roots = SearchRoots::default();
        let r = resolve_java(21, &o).await.unwrap();
        assert_eq!(r.source, JavaSource::Downloaded);
        assert!(r.path.starts_with(t.path().join("runtimes").join("jre-21")));
        assert_eq!(probe(&r.path).await.unwrap().major, 21);
        assert!(
            !t.path().join("cache").join(name).exists(),
            "archive deleted"
        );

        // Second resolve reuses the managed runtime (server not needed).
        o.adoptium_base = "http://127.0.0.1:1/v3".into();
        assert_eq!(
            resolve_java(21, &o).await.unwrap().source,
            JavaSource::Managed
        );
    }

    #[tokio::test]
    async fn adoptium_rejects_bad_names_and_checksums() {
        use sha2::{Digest, Sha256};
        let pkg = tar_gz_with_java("openjdk version \"21.0.1\"");
        let sha = hex::encode(Sha256::digest(&pkg));
        for bad in ["../evil.tar.gz", "a/b.tar.gz", ".."] {
            let base = serve(assets(bad, &sha, pkg.len()), pkg.clone()).await;
            let t = tempfile::tempdir().unwrap();
            let mut o = ResolveOptions::new(t.path().join("runtimes"), t.path().join("cache"));
            o.adoptium_base = base;
            o.roots = SearchRoots::default();
            let e = resolve_java(21, &o).await.unwrap_err();
            assert_eq!(e.code(), "java.invalid_package_name", "{bad}");
            assert!(!t.path().join("evil.tar.gz").exists());
        }
        // wrong checksum
        let base = serve(assets("x.tar.gz", &"0".repeat(64), pkg.len()), pkg.clone()).await;
        let t = tempfile::tempdir().unwrap();
        let mut o = ResolveOptions::new(t.path().join("runtimes"), t.path().join("cache"));
        o.adoptium_base = base;
        o.roots = SearchRoots::default();
        assert_eq!(
            resolve_java(21, &o).await.unwrap_err().code(),
            "net.checksum_mismatch"
        );
    }

    #[tokio::test]
    async fn adoptium_empty_asset_list() {
        let base = serve("[]".into(), Vec::new()).await;
        let t = tempfile::tempdir().unwrap();
        let mut o = ResolveOptions::new(t.path().join("runtimes"), t.path().join("cache"));
        o.adoptium_base = base;
        o.roots = SearchRoots::default();
        assert_eq!(
            resolve_java(21, &o).await.unwrap_err().code(),
            "java.no_runtime"
        );
    }
}
