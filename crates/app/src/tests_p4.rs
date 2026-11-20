#![allow(clippy::unwrap_used)]
//! Part 4 of `docs/ipc.md`: providers (Modrinth + CurseForge), install
//! planning, apply and modpacks. Mock servers and a temp data dir only: no
//! real network, no real key.

use crate::tests_p2::{json_ok, serve, Handler, Req};
use crate::*;
use mango_core::{Paths, Secret};
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use std::sync::{Arc, Mutex};

pub(crate) const KEY: &str = "$2a$10$abcdefghijklmnopqrstuv.wxyz0123456789ABCDEFGHIJKLMNO";

#[derive(Default)]
pub(crate) struct Sink(Mutex<Vec<Event>>);

impl EventSink for Sink {
    fn emit(&self, event: Event) {
        self.0.lock().unwrap().push(event);
    }
}

impl Sink {
    pub(crate) fn events(&self) -> Vec<Event> {
        self.0.lock().unwrap().clone()
    }
    pub(crate) fn warnings(&self) -> Vec<String> {
        self.events()
            .into_iter()
            .filter_map(|e| match e {
                Event::LaunchLog(l) if l.level == LogLevelName::Warn => Some(l.line),
                _ => None,
            })
            .collect()
    }
}

struct Fx {
    _tmp: tempfile::TempDir,
    svc: AppService,
    sink: Arc<Sink>,
}

fn fx(modrinth: &str, curse: &str, key: Option<&str>) -> Fx {
    let tmp = tempfile::tempdir().unwrap();
    let sink = Arc::new(Sink::default());
    let opts = ServiceOptions {
        open_folders: false,
        update_dev: true,
        auto_update_check: false,
        fetch: mango_net::FetchOptions {
            retries: 0,
            ..mango_net::FetchOptions::default()
        },
        partners: Vec::new(),
        icon_hosts: Vec::new(),
        modrinth_api: modrinth.to_string(),
        download_policy: Some(mango_net::HostPolicy::allow_all()),
        curseforge_api: curse.to_string(),
        curseforge_key: key.map(Secret::new),
        curseforge: mango_curseforge::Options {
            retries: 0,
            strict_download_urls: false,
            ..mango_curseforge::Options::default()
        },
        ..ServiceOptions::default()
    };
    let svc = AppService::new(Paths::new(tmp.path()), sink.clone(), opts).unwrap();
    Fx {
        _tmp: tmp,
        svc,
        sink,
    }
}

impl Fx {
    fn profile(&self, name: &str, mc: &str, loader: &str) -> String {
        self.svc
            .profile_create(name.into(), mc.into(), loader.into(), None)
            .unwrap()
            .id
    }
    fn dir(&self, id: &str) -> std::path::PathBuf {
        self.svc.paths().instance_dir(id).unwrap()
    }
    fn mods(&self, id: &str) -> Vec<mango_core::ModRecord> {
        self.svc.profile(id).unwrap().mods
    }
}

// ---------------------------------------------------------------- CurseForge mock

pub(crate) fn cf_bytes(name: &str) -> Vec<u8> {
    format!("cf:{name}").into_bytes()
}

pub(crate) fn cf_project(id: u64, slug: &str, class: u32, allow: Option<bool>) -> Value {
    json!({
        "id": id, "name": format!("P{id}"), "slug": slug, "classId": class,
        "summary": "sum", "downloadCount": 99, "thumbsUpCount": 7,
        "authors": [{"name": "alice"}],
        "logo": {"thumbnailUrl": "https://media.forgecdn.net/a.png", "url": "https://media.forgecdn.net/b.png"},
        "categories": [{"id": 1, "name": "Tech", "slug": "tech"}],
        "allowModDistribution": allow, "links": {"sourceUrl": "https://example.com/src"},
        "latestFilesIndexes": [{"gameVersion": "1.21.1", "fileId": 1, "releaseType": 1, "modLoader": 4}],
        "dateModified": "2024-05-01T00:00:00Z"
    })
}

pub(crate) struct CfFile {
    pub(crate) id: u64,
    pub(crate) mod_id: u64,
    pub(crate) name: &'static str,
    pub(crate) versions: Vec<&'static str>,
    pub(crate) release: u32,
    pub(crate) date: &'static str,
    pub(crate) deps: Vec<(u64, u32)>,
    /// `false` = null downloadUrl.
    pub(crate) url: bool,
    /// File content; defaults to `cf:<name>`.
    pub(crate) body: Option<Vec<u8>>,
    /// `false` = the API lists no hashes.
    pub(crate) hash: bool,
}

impl CfFile {
    fn bytes(&self) -> Vec<u8> {
        self.body.clone().unwrap_or_else(|| cf_bytes(self.name))
    }
}

pub(crate) fn cf_file(
    id: u64,
    mod_id: u64,
    name: &'static str,
    versions: &[&'static str],
) -> CfFile {
    CfFile {
        id,
        mod_id,
        name,
        versions: versions.to_vec(),
        release: 1,
        date: "2024-01-01T00:00:00Z",
        deps: Vec::new(),
        url: true,
        body: None,
        hash: true,
    }
}

fn cf_file_json(f: &CfFile, base: &str) -> Value {
    let bytes = f.bytes();
    json!({
        "id": f.id, "modId": f.mod_id, "isAvailable": true,
        "displayName": format!("{} build", f.name), "fileName": f.name,
        "releaseType": f.release, "fileDate": f.date, "fileLength": bytes.len(),
        "downloadUrl": if f.url { json!(format!("{base}/cdn/{}", f.name)) } else { Value::Null },
        "gameVersions": f.versions,
        "hashes": if f.hash { json!([{"value": hex::encode(Sha1::digest(&bytes)), "algo": 1}]) } else { json!([]) },
        "dependencies": f.deps.iter().map(|(m, r)| json!({"modId": m, "relationType": r})).collect::<Vec<_>>(),
    })
}

fn qs(path: &str, key: &str) -> Option<String> {
    path.split('?')
        .nth(1)?
        .split('&')
        .find_map(|p| p.strip_prefix(&format!("{key}=")).map(str::to_string))
}

/// A CurseForge API + CDN over `projects` and `files`. Extra routes can be
/// added with `extra` (checked first).
pub(crate) fn cf_server(
    projects: Vec<Value>,
    files: Vec<CfFile>,
    extra: impl Fn(&str, &Req) -> Option<(u16, Vec<u8>)> + Send + Sync + 'static,
) -> impl FnOnce(&str) -> Handler {
    move |base| {
        let base = base.to_string();
        Arc::new(move |r: &Req| {
            let path = r.path.split('?').next().unwrap().to_string();
            if let Some(x) = extra(&base, r) {
                return x;
            }
            if let Some(name) = path.strip_prefix("/cdn/") {
                return match files.iter().find(|f| f.name == name) {
                    Some(f) => (200, f.bytes()),
                    None => (404, Vec::new()),
                };
            }
            let find_file = |id: u64| files.iter().find(|f| f.id == id);
            if r.method == "POST" {
                let body: Value = serde_json::from_slice(&r.body).unwrap_or(Value::Null);
                if path == "/mods/files" {
                    let data: Vec<Value> = body["fileIds"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter_map(|i| find_file(i.as_u64()?))
                        .map(|f| cf_file_json(f, &base))
                        .collect();
                    return json_ok(json!({"data": data}));
                }
                if path == "/mods" {
                    let data: Vec<Value> = projects
                        .iter()
                        .filter(|p| body["modIds"].as_array().unwrap().contains(&p["id"]))
                        .cloned()
                        .collect();
                    return json_ok(json!({"data": data}));
                }
            }
            if path == "/mods/search" {
                let class: u64 = qs(&r.path, "classId").unwrap().parse().unwrap();
                let data: Vec<&Value> = projects
                    .iter()
                    .filter(|p| p["classId"].as_u64() == Some(class))
                    .collect();
                return json_ok(json!({
                    "data": data,
                    "pagination": {"index": 0, "pageSize": 30, "resultCount": data.len(), "totalCount": 42}
                }));
            }
            let seg: Vec<&str> = path.trim_start_matches('/').split('/').collect();
            match seg.as_slice() {
                ["mods", id] => match projects
                    .iter()
                    .find(|p| p["id"].as_u64().map(|n| n.to_string()).as_deref() == Some(*id))
                {
                    Some(p) => json_ok(json!({"data": p})),
                    None => (404, b"{}".to_vec()),
                },
                ["mods", _, "description"] => json_ok(json!({
                    "data": "<h2>About</h2><p>Hello <b>world</b></p><script>x()</script>"
                })),
                ["mods", id, "files"] => {
                    let mc = qs(&r.path, "gameVersion");
                    let data: Vec<Value> = files
                        .iter()
                        .filter(|f| f.mod_id.to_string() == *id)
                        .filter(|f| mc.as_ref().is_none_or(|m| f.versions.contains(&m.as_str())))
                        .map(|f| cf_file_json(f, &base))
                        .collect();
                    let total = data.len();
                    let index: usize = qs(&r.path, "index").unwrap_or("0".into()).parse().unwrap();
                    let page: Vec<Value> = data.into_iter().skip(index).collect();
                    json_ok(json!({"data": page, "pagination": {"totalCount": total}}))
                }
                ["mods", _, "files", fid] => match fid.parse().ok().and_then(find_file) {
                    Some(f) => json_ok(json!({"data": cf_file_json(f, &base)})),
                    None => (404, b"{}".to_vec()),
                },
                ["mods", _, "files", _, "download-url"] => (403, b"{}".to_vec()),
                _ => (404, b"{}".to_vec()),
            }
        })
    }
}

/// Standard CurseForge world:
/// 100 Alpha (fabric 1.21.1) requires 200 Lib, optional 300 Opt, incompatible 400 Bad;
/// 500 is blocked by its author; 600 has no download url; 700 is a shader pack;
/// 800 is forge-only 1.20.1; 900 and 901 require each other.
pub(crate) fn cf_world() -> (Vec<Value>, Vec<CfFile>) {
    let projects = vec![
        cf_project(100, "alpha", 6, Some(true)),
        cf_project(200, "lib", 6, None),
        cf_project(300, "opt", 6, None),
        cf_project(400, "bad", 6, None),
        cf_project(500, "secret", 6, Some(false)),
        cf_project(600, "nourl", 6, Some(true)),
        cf_project(700, "pretty", 6552, None),
        cf_project(800, "forgeonly", 6, None),
        cf_project(900, "cycle-a", 6, None),
        cf_project(901, "cycle-b", 6, None),
        cf_project(1000, "the-pack", 4471, None),
    ];
    let fab = ["1.21.1", "Fabric", "Client"];
    let mut alpha = cf_file(1001, 100, "alpha-1.jar", &fab);
    alpha.deps = vec![(200, 3), (300, 2), (400, 5), (999, 1)];
    let mut alpha_old = cf_file(1000, 100, "alpha-0.jar", &fab);
    alpha_old.date = "2023-01-01T00:00:00Z";
    let mut alpha_beta = cf_file(1002, 100, "alpha-2b.jar", &fab);
    alpha_beta.release = 2;
    alpha_beta.date = "2025-01-01T00:00:00Z";
    let mut nourl = cf_file(6001, 600, "nourl.jar", &fab);
    nourl.url = false;
    let mut a = cf_file(9001, 900, "cycle-a.jar", &fab);
    a.deps = vec![(901, 3)];
    let mut b = cf_file(9011, 901, "cycle-b.jar", &fab);
    b.deps = vec![(900, 3)];
    let files = vec![
        alpha,
        alpha_old,
        alpha_beta,
        cf_file(2001, 200, "lib-1.jar", &fab),
        cf_file(3001, 300, "opt-1.jar", &fab),
        cf_file(4001, 400, "bad-1.jar", &fab),
        cf_file(5001, 500, "secret-1.jar", &fab),
        nourl,
        cf_file(7001, 700, "pretty-1.zip", &["1.21.1"]),
        cf_file(8001, 800, "forgeonly-1.jar", &["1.20.1", "Forge"]),
        a,
        b,
    ];
    (projects, files)
}

async fn cf_fx() -> (Fx, String, Arc<Mutex<Vec<Req>>>) {
    let (projects, files) = cf_world();
    cf_fx_with(projects, files).await
}

async fn cf_fx_with(
    projects: Vec<Value>,
    files: Vec<CfFile>,
) -> (Fx, String, Arc<Mutex<Vec<Req>>>) {
    let (base, seen) = serve(cf_server(projects, files, |_, _| None)).await;
    (fx("http://127.0.0.1:1", &base, Some(KEY)), base, seen)
}

fn params(provider: Provider, kind: &str) -> SearchParams {
    SearchParams {
        provider,
        query: "x".into(),
        project_type: kind.into(),
        loader: None,
        mc_version: None,
        categories: None,
        sort: "relevance".into(),
        offset: 0,
        limit: 30,
    }
}

fn target<'a>(ts: &'a [InstallTarget], id: &str) -> &'a InstallTarget {
    ts.iter().find(|t| t.profile_id == id).unwrap()
}

fn item<'a>(plan: &'a InstallPlan, id: &str) -> &'a InstallPlanItem {
    plan.items.iter().find(|i| i.project_id == id).unwrap()
}

// ---------------------------------------------------------------- status and key

#[tokio::test]
async fn providers_status_reports_availability_and_never_the_key() {
    let f = fx("http://127.0.0.1:1", "http://127.0.0.1:1", None);
    let st = serde_json::to_value(f.svc.providers_status().unwrap()).unwrap();
    assert_eq!(
        st,
        json!([
            {"provider": "modrinth", "available": true},
            {"provider": "curseforge", "available": false, "reason": "no_api_key"}
        ])
    );
    // Every CurseForge command says why instead of failing obscurely.
    let e = f
        .svc
        .search_projects(params(Provider::Curseforge, "mod"))
        .await
        .unwrap_err();
    assert_eq!(e.code, "curseforge.no_api_key");
    assert_eq!(
        f.svc
            .install_targets(Provider::Curseforge, "100".into(), None)
            .await
            .unwrap_err()
            .code,
        "curseforge.no_api_key"
    );

    // The config field enables it; the key never comes back out.
    f.svc.config_set(json!({"curseforgeApiKey": KEY})).unwrap();
    let st = f.svc.providers_status().unwrap();
    assert!(st[1].available && st[1].reason.is_none());
    let state = serde_json::to_string(&f.svc.app_state().unwrap()).unwrap();
    assert!(!state.contains("abcdefgh") && !state.to_lowercase().contains("apikey"));
    let cfg = f.svc.config_set(json!({"fullscreen": true})).unwrap();
    assert!(!serde_json::to_string(&cfg).unwrap().contains("abcdefgh"));
    // Clearing it makes the provider unavailable again.
    f.svc.config_set(json!({"curseforgeApiKey": ""})).unwrap();
    assert!(!f.svc.providers_status().unwrap()[1].available);
    // Malformed keys are refused at the boundary.
    for bad in ["short", "white space is not allowed 12345"] {
        assert!(f.svc.config_set(json!({"curseforgeApiKey": bad})).is_err());
    }
}

#[tokio::test]
async fn the_startup_key_is_used_until_the_config_overrides_it() {
    let f = fx("http://127.0.0.1:1", "http://127.0.0.1:1", Some(KEY));
    assert!(f.svc.providers_status().unwrap()[1].available);
    // A malformed startup key counts as missing.
    let g = fx("http://127.0.0.1:1", "http://127.0.0.1:1", Some("nope"));
    assert!(!g.svc.providers_status().unwrap()[1].available);
    // Debug output of the options never shows the key.
    let opts = ServiceOptions {
        curseforge_key: Some(Secret::new(KEY)),
        ..ServiceOptions::default()
    };
    assert!(!format!("{opts:?}").contains("abcdefgh"));
}

// ---------------------------------------------------------------- browsing

#[tokio::test]
async fn curseforge_search_detail_and_versions() {
    let (f, _, seen) = cf_fx().await;
    let pid = f.profile("Fab", "1.21.1", "fabric");
    let r = f
        .svc
        .search_projects(SearchParams {
            loader: Some("fabric".into()),
            mc_version: Some("1.21.1".into()),
            sort: "follows".into(),
            ..params(Provider::Curseforge, "mod")
        })
        .await
        .unwrap();
    let h = &r.hits[0];
    assert_eq!(h.provider, Provider::Curseforge);
    assert_eq!(
        (h.project_id.as_str(), h.slug.as_str(), h.kind.as_str()),
        ("100", "alpha", "mod")
    );
    assert_eq!(h.author, "alice");
    assert_eq!((h.downloads, h.follows, r.total), (99, 7, 42));
    assert_eq!(h.loaders, ["fabric"]);
    assert!(h
        .icon_url
        .as_deref()
        .unwrap()
        .starts_with("mangoimg://icon/"));
    let path = seen
        .lock()
        .unwrap()
        .iter()
        .find(|q| q.path.contains("/mods/search"))
        .unwrap()
        .path
        .clone();
    for p in [
        "classId=6",
        "modLoaderType=4",
        "gameVersion=1.21.1",
        "sortField=2",
    ] {
        assert!(path.contains(p), "{p} in {path}");
    }
    let shaders = f
        .svc
        .search_projects(params(Provider::Curseforge, "shader"))
        .await
        .unwrap();
    assert_eq!(
        (shaders.hits.len(), shaders.hits[0].kind.as_str()),
        (1, "shader")
    );
    assert!(f
        .svc
        .search_projects(SearchParams {
            sort: "random".into(),
            ..params(Provider::Curseforge, "mod")
        })
        .await
        .is_err());

    let d = f
        .svc
        .project_detail(Provider::Curseforge, "100".into())
        .await
        .unwrap();
    assert_eq!(d.hit.provider, Provider::Curseforge);
    assert!(d.body.contains("## About") && d.body.contains("Hello world"));
    assert!(!d.body.contains("<script") && !d.body.contains("x()"));
    assert_eq!(d.source_url.as_deref(), Some("https://example.com/src"));
    assert!(f
        .svc
        .project_detail(Provider::Curseforge, "../x".into())
        .await
        .is_err());

    let v = f
        .svc
        .project_versions(Provider::Curseforge, "100".into(), Some(pid))
        .await
        .unwrap();
    assert_eq!(v.len(), 3);
    assert!(v
        .iter()
        .all(|x| x.provider == Provider::Curseforge && x.compatible));
    // Newest first; the beta is reported as such.
    assert_eq!(v[0].version_id, "1002");
    assert_eq!(v[0].kind, "beta");
    let forge = f.profile("Forge", "1.20.1", "forge");
    let v = f
        .svc
        .project_versions(Provider::Curseforge, "100".into(), Some(forge))
        .await
        .unwrap();
    assert!(v.iter().all(|x| !x.compatible));
}

// ---------------------------------------------------------------- targets

#[tokio::test]
async fn install_targets_cover_every_profile_state() {
    let (f, _, _) = cf_fx().await;
    let fab = f.profile("Fab", "1.21.1", "fabric");
    let fab_old = f.profile("FabOld", "1.20.1", "fabric");
    let forge = f.profile("Forge", "1.21.1", "forge");
    let forge_old = f.profile("ForgeOld", "1.20.1", "forge");
    let quilt = f.profile("Quilt", "1.21.1", "quilt");
    let vanilla = f.profile("Vanilla", "1.21.1", "vanilla");
    let ts = f
        .svc
        .install_targets(Provider::Curseforge, "100".into(), None)
        .await
        .unwrap();

    let t = target(&ts, &fab);
    assert_eq!(
        (t.state, t.reason, t.installed_version.as_deref()),
        (TargetState::Available, None, None)
    );
    assert_eq!(t.required_deps, 1, "Lib is the only required dependency");
    // Quilt runs Fabric builds.
    assert_eq!(target(&ts, &quilt).state, TargetState::Available);
    let t = target(&ts, &fab_old);
    assert_eq!(
        (t.state, t.reason),
        (TargetState::Incompatible, Some(TargetReason::McVersion))
    );
    let t = target(&ts, &forge);
    assert_eq!(t.reason, Some(TargetReason::Loader));
    assert_eq!(
        t.detail.as_deref(),
        Some("needs fabric, instance uses forge")
    );
    assert_eq!(
        target(&ts, &forge_old).reason,
        Some(TargetReason::NoCompatibleVersion)
    );
    assert_eq!(target(&ts, &vanilla).reason, Some(TargetReason::Loader));

    // Installed: the version is reported, the others stay as they were.
    f.svc
        .install_apply(
            Provider::Curseforge,
            "100".into(),
            None,
            fab.clone(),
            vec![],
        )
        .await
        .unwrap();
    let ts = f
        .svc
        .install_targets(Provider::Curseforge, "100".into(), None)
        .await
        .unwrap();
    let t = target(&ts, &fab);
    assert_eq!(t.state, TargetState::Installed);
    assert_eq!(t.installed_version.as_deref(), Some("alpha-1.jar build"));
    assert_eq!(target(&ts, &quilt).state, TargetState::Available);

    // Content types: a shader pack fits every loader, a modpack no instance.
    let ts = f
        .svc
        .install_targets(Provider::Curseforge, "700".into(), None)
        .await
        .unwrap();
    assert_eq!(target(&ts, &vanilla).state, TargetState::Available);
    assert_eq!(target(&ts, &forge).state, TargetState::Available);
    assert_eq!(target(&ts, &fab_old).reason, Some(TargetReason::McVersion));
    let ts = f
        .svc
        .install_targets(Provider::Curseforge, "1000".into(), None)
        .await
        .unwrap();
    assert!(ts.iter().all(
        |t| t.state == TargetState::Incompatible && t.reason == Some(TargetReason::ContentType)
    ));

    // A pinned version is judged against each profile.
    let ts = f
        .svc
        .install_targets(Provider::Curseforge, "800".into(), Some("8001".into()))
        .await
        .unwrap();
    assert_eq!(target(&ts, &forge_old).state, TargetState::Available);
    assert_eq!(target(&ts, &forge).reason, Some(TargetReason::McVersion));
    assert_eq!(target(&ts, &fab_old).reason, Some(TargetReason::Loader));
    assert_eq!(
        target(&ts, &fab).reason,
        Some(TargetReason::NoCompatibleVersion)
    );
}

#[tokio::test]
async fn blocked_distribution_is_reported_per_profile_and_never_downloaded() {
    let (f, _, seen) = cf_fx().await;
    let fab = f.profile("Fab", "1.21.1", "fabric");
    for id in ["500", "600"] {
        let ts = f
            .svc
            .install_targets(Provider::Curseforge, id.into(), None)
            .await
            .unwrap();
        let t = target(&ts, &fab);
        assert_eq!(
            (t.state, t.reason),
            (
                TargetState::Incompatible,
                Some(TargetReason::DistributionBlocked)
            ),
            "{id}"
        );
    }
    // The plan shows the website instead of a download.
    let plan = f
        .svc
        .install_plan(Provider::Curseforge, "500".into(), None, fab.clone())
        .await
        .unwrap();
    let it = item(&plan, "500");
    assert!(it.distribution_blocked);
    assert_eq!(
        it.download_url.as_deref(),
        Some("https://www.curseforge.com/minecraft/mc-mods/secret")
    );
    assert_eq!(plan.total_bytes, 0);
    let plan = f
        .svc
        .install_plan(Provider::Curseforge, "600".into(), None, fab.clone())
        .await
        .unwrap();
    assert!(
        item(&plan, "600").distribution_blocked,
        "no url and the endpoint refuses"
    );
    // Applying refuses, with a stable code, and fetches nothing.
    for id in ["500", "600"] {
        let e = f
            .svc
            .install_apply(Provider::Curseforge, id.into(), None, fab.clone(), vec![])
            .await
            .unwrap_err();
        assert_eq!(e.code, "distribution_blocked");
    }
    assert!(f.mods(&fab).is_empty());
    assert!(!seen
        .lock()
        .unwrap()
        .iter()
        .any(|r| r.path.starts_with("/cdn/")));
}

// ---------------------------------------------------------------- plan

#[tokio::test]
async fn plan_resolves_required_optional_installed_and_conflicting_items() {
    let (f, _, _) = cf_fx().await;
    let fab = f.profile("Fab", "1.21.1", "fabric");
    let plan = f
        .svc
        .install_plan(Provider::Curseforge, "100".into(), None, fab.clone())
        .await
        .unwrap();
    let roles: Vec<(&str, PlanRole)> = plan
        .items
        .iter()
        .map(|i| (i.project_id.as_str(), i.role))
        .collect();
    // Release beats the newer beta; embedded (999) is ignored.
    assert_eq!(
        roles,
        [
            ("100", PlanRole::Requested),
            ("200", PlanRole::Required),
            ("300", PlanRole::Optional)
        ]
    );
    assert_eq!(item(&plan, "100").version_id, "1001");
    assert!(plan.conflicts.is_empty());
    let size = |n: &str| cf_bytes(n).len() as u64;
    assert_eq!(
        plan.total_bytes,
        size("alpha-1.jar") + size("lib-1.jar"),
        "optional items are not in the default size"
    );
    assert!(plan
        .items
        .iter()
        .all(|i| !i.already_installed && !i.distribution_blocked && i.download_url.is_none()));
    assert!(item(&plan, "100").icon_url.is_some());

    // Install the lib alone first: it is then reported as installed (exact version).
    f.svc
        .install_apply(
            Provider::Curseforge,
            "200".into(),
            None,
            fab.clone(),
            vec![],
        )
        .await
        .unwrap();
    let plan = f
        .svc
        .install_plan(Provider::Curseforge, "100".into(), None, fab.clone())
        .await
        .unwrap();
    let lib = item(&plan, "200");
    assert!(lib.already_installed);
    assert_eq!(lib.installed_version.as_deref(), Some("lib-1.jar build"));
    assert_eq!(plan.total_bytes, size("alpha-1.jar"));

    // The incompatible project, once installed, is a conflict.
    f.svc
        .install_apply(
            Provider::Curseforge,
            "400".into(),
            None,
            fab.clone(),
            vec![],
        )
        .await
        .unwrap();
    let plan = f
        .svc
        .install_plan(Provider::Curseforge, "100".into(), None, fab.clone())
        .await
        .unwrap();
    assert_eq!(
        plan.conflicts,
        [PlanConflict {
            project_id: "400".into(),
            title: "P400".into(),
            with: "P100".into()
        }]
    );
    let bad = item(&plan, "400");
    assert_eq!(
        (bad.role, bad.already_installed),
        (PlanRole::Incompatible, true)
    );
    assert_eq!(plan.items.last().unwrap().project_id, "400");
}

#[tokio::test]
async fn plan_terminates_on_dependency_cycles() {
    let (f, _, _) = cf_fx().await;
    let fab = f.profile("Fab", "1.21.1", "fabric");
    let plan = f
        .svc
        .install_plan(Provider::Curseforge, "900".into(), None, fab.clone())
        .await
        .unwrap();
    let ids: Vec<&str> = plan.items.iter().map(|i| i.project_id.as_str()).collect();
    assert_eq!(ids, ["900", "901"]);
    let items = f
        .svc
        .install_apply(
            Provider::Curseforge,
            "900".into(),
            None,
            fab.clone(),
            vec![],
        )
        .await
        .unwrap();
    assert_eq!(items.len(), 2);
    assert!(
        !items
            .iter()
            .find(|i| i.project_id == "900")
            .unwrap()
            .dependency
    );
    assert!(
        items
            .iter()
            .find(|i| i.project_id == "901")
            .unwrap()
            .dependency
    );
}

#[tokio::test]
async fn plan_errors_are_stable_codes() {
    let (f, _, _) = cf_fx().await;
    let fab = f.profile("Fab", "1.21.1", "fabric");
    let forge = f.profile("Forge", "1.20.1", "forge");
    let vanilla = f.profile("Van", "1.21.1", "vanilla");
    let code = |r: Res<InstallPlan>| r.unwrap_err().code;
    assert_eq!(
        code(
            f.svc
                .install_plan(Provider::Curseforge, "1000".into(), None, fab.clone())
                .await
        ),
        "use_modpack_install"
    );
    assert_eq!(
        code(
            f.svc
                .install_plan(Provider::Curseforge, "100".into(), None, forge)
                .await
        ),
        "no_compatible_version"
    );
    assert_eq!(
        code(
            f.svc
                .install_plan(Provider::Curseforge, "100".into(), None, vanilla)
                .await
        ),
        "no_compatible_version"
    );
    assert_eq!(
        code(
            f.svc
                .install_plan(Provider::Curseforge, "abc".into(), None, fab.clone())
                .await
        ),
        "validation"
    );
    assert_eq!(
        code(
            f.svc
                .install_plan(Provider::Curseforge, "100".into(), None, "nope".into())
                .await
        ),
        "invalid_profile_id"
    );
}

// ---------------------------------------------------------------- apply, toggle, update, uninstall

#[tokio::test]
async fn apply_installs_requested_required_and_selected_optionals() {
    let (f, _, seen) = cf_fx().await;
    let fab = f.profile("Fab", "1.21.1", "fabric");
    let items = f
        .svc
        .install_apply(
            Provider::Curseforge,
            "100".into(),
            None,
            fab.clone(),
            vec!["300".into()],
        )
        .await
        .unwrap();
    assert_eq!(items.len(), 3);
    for (id, file, dep) in [
        ("100", "alpha-1.jar", false),
        ("200", "lib-1.jar", true),
        ("300", "opt-1.jar", true),
    ] {
        let it = items.iter().find(|i| i.project_id == id).unwrap();
        assert_eq!(it.provider, Some(Provider::Curseforge));
        assert_eq!(
            (it.filename.as_str(), it.dependency, it.enabled, it.local),
            (file, dep, true, false)
        );
        assert_eq!(
            std::fs::read(f.dir(&fab).join("mods").join(file)).unwrap(),
            cf_bytes(file)
        );
    }
    assert_eq!(
        items
            .iter()
            .find(|i| i.project_id == "200")
            .unwrap()
            .required_by,
        ["100"]
    );
    // Unselected optionals and the incompatible project were not touched.
    let without = f.profile("Fab2", "1.21.1", "fabric");
    let items = f
        .svc
        .install_apply(
            Provider::Curseforge,
            "100".into(),
            None,
            without.clone(),
            vec![],
        )
        .await
        .unwrap();
    assert_eq!(items.len(), 2);
    assert!(!f.dir(&without).join("mods/opt-1.jar").exists());
    assert!(!f.dir(&without).join("mods/bad-1.jar").exists());

    // Records carry provider and ids and survive in accounts.json.
    let rec = f
        .mods(&fab)
        .into_iter()
        .find(|r| r.project_id.as_deref() == Some("100"))
        .unwrap();
    assert_eq!(rec.provider.as_deref(), Some("curseforge"));
    assert_eq!(rec.version_id.as_deref(), Some("1001"));
    assert_eq!(rec.kind, Some(mango_core::ModType::Mod));
    assert_eq!(rec.slug.as_deref(), Some("alpha"));
    let deps = rec.required_dependencies.unwrap();
    assert_eq!(
        deps.iter()
            .map(|d| d.project_id.as_str())
            .collect::<Vec<_>>(),
        ["200"]
    );
    assert!(deps[0].version_id.is_none(), "CurseForge never pins");
    let raw = std::fs::read_to_string(f.svc.paths().root().join("accounts.json")).unwrap();
    assert!(raw.contains("\"provider\""));

    // Progress events: start at 0, end at done == total.
    let progress: Vec<_> = f
        .sink
        .events()
        .into_iter()
        .filter_map(|e| match e {
            Event::InstallProgress(p) if p.profile_id == fab => Some(p),
            _ => None,
        })
        .collect();
    assert_eq!(progress.first().unwrap().done, 0);
    let last = progress.last().unwrap();
    assert_eq!(
        (last.done, last.total, last.project_id.as_str()),
        (3, 3, "100")
    );

    // Applying again downloads nothing new.
    let cdn = || {
        seen.lock()
            .unwrap()
            .iter()
            .filter(|r| r.path.starts_with("/cdn/"))
            .count()
    };
    let before = cdn();
    assert!(before >= 5);
    f.svc
        .install_apply(
            Provider::Curseforge,
            "100".into(),
            None,
            fab.clone(),
            vec!["300".into()],
        )
        .await
        .unwrap();
    assert_eq!(cdn(), before);
}

#[tokio::test]
async fn a_blocked_dependency_is_skipped_with_a_warning() {
    let (mut projects, mut files) = cf_world();
    projects.push(cf_project(110, "needs-secret", 6, None));
    let mut m = cf_file(1101, 110, "needs-secret-1.jar", &["1.21.1", "Fabric"]);
    m.deps = vec![(500, 3)];
    files.push(m);
    let (f, _, seen) = cf_fx_with(projects, files).await;
    let fab = f.profile("Fab", "1.21.1", "fabric");
    let plan = f
        .svc
        .install_plan(Provider::Curseforge, "110".into(), None, fab.clone())
        .await
        .unwrap();
    assert!(item(&plan, "500").distribution_blocked);
    assert!(!item(&plan, "110").distribution_blocked);
    let items = f
        .svc
        .install_apply(
            Provider::Curseforge,
            "110".into(),
            None,
            fab.clone(),
            vec![],
        )
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    assert!(!f.dir(&fab).join("mods/secret-1.jar").exists());
    assert!(!seen
        .lock()
        .unwrap()
        .iter()
        .any(|r| r.path.contains("/cdn/secret-1")));
    let w = f.sink.warnings();
    assert!(
        w.iter()
            .any(|l| l.contains("P500") && l.contains("curseforge.com/minecraft/mc-mods/secret")),
        "{w:?}"
    );
    // The record lists only dependencies that are present, so toggling works.
    let rec = &f.mods(&fab)[0];
    assert!(rec.required_dependencies.as_ref().unwrap().is_empty());
    f.svc
        .content_toggle(fab.clone(), "110".into(), false)
        .await
        .unwrap();
}

#[tokio::test]
async fn null_download_url_is_fetched_from_the_download_endpoint_when_allowed() {
    let (projects, files) = cf_world();
    let (base, _) = serve(cf_server(projects, files, |base, r| {
        r.path
            .ends_with("/mods/600/files/6001/download-url")
            .then(|| json_ok(json!({"data": format!("{base}/cdn/nourl.jar")})))
    }))
    .await;
    let f = fx("http://127.0.0.1:1", &base, Some(KEY));
    let fab = f.profile("Fab", "1.21.1", "fabric");
    let items = f
        .svc
        .install_apply(
            Provider::Curseforge,
            "600".into(),
            None,
            fab.clone(),
            vec![],
        )
        .await
        .unwrap();
    assert_eq!(items[0].filename, "nourl.jar");
    assert_eq!(
        std::fs::read(f.dir(&fab).join("mods/nourl.jar")).unwrap(),
        cf_bytes("nourl.jar")
    );
}

#[tokio::test]
async fn updating_installed_content_takes_an_automatic_snapshot_first() {
    use crate::backups::{BackupKind, BackupReason};
    let (f, _, _) = cf_fx().await;
    let fab = f.profile("Fab", "1.21.1", "fabric");
    let first = || {
        f.svc.install_apply(
            Provider::Curseforge,
            "100".into(),
            Some("1000".into()),
            fab.clone(),
            vec![],
        )
    };
    // A fresh install replaces nothing: no snapshot.
    first().await.unwrap();
    assert!(f.svc.backups_list(fab.clone()).await.unwrap().is_empty());

    // The update does: the snapshot still holds the old jar.
    f.svc
        .install_apply(
            Provider::Curseforge,
            "100".into(),
            Some("1001".into()),
            fab.clone(),
            vec![],
        )
        .await
        .unwrap();
    let mods = f.dir(&fab).join("mods");
    assert!(!mods.join("alpha-0.jar").exists() && mods.join("alpha-1.jar").exists());
    let list = f.svc.backups_list(fab.clone()).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].reason, BackupReason::BeforeUpdate);
    assert_eq!(list[0].kind, BackupKind::Snapshot);
    f.svc
        .backup_restore(fab.clone(), list[0].id.clone(), None)
        .await
        .unwrap();
    assert!(mods.join("alpha-0.jar").exists() && !mods.join("alpha-1.jar").exists());
    assert_eq!(f.mods(&fab)[0].version_id.as_deref(), Some("1000"));

    // Failure path (the backup folder cannot be created): the update still
    // goes through and warns.
    let backups = f.svc.paths().backups();
    std::fs::remove_dir_all(&backups).unwrap();
    std::fs::write(&backups, b"x").unwrap();
    f.svc
        .install_apply(
            Provider::Curseforge,
            "100".into(),
            Some("1001".into()),
            fab.clone(),
            vec![],
        )
        .await
        .unwrap();
    assert!(mods.join("alpha-1.jar").exists());
    assert!(
        f.sink
            .warnings()
            .iter()
            .any(|w| w.contains("automatic snapshot")),
        "{:?}",
        f.sink.warnings()
    );
}

#[tokio::test]
async fn curseforge_items_toggle_update_and_uninstall_without_asking_modrinth() {
    let (f, _, _) = cf_fx().await;
    let fab = f.profile("Fab", "1.21.1", "fabric");
    // Pin the older build, then check for updates.
    f.svc
        .install_apply(
            Provider::Curseforge,
            "100".into(),
            Some("1000".into()),
            fab.clone(),
            vec![],
        )
        .await
        .unwrap();
    let mods = f.dir(&fab).join("mods");
    assert!(mods.join("alpha-0.jar").exists());

    // Modrinth points at a dead port: any request to it would fail the call.
    let r = f
        .svc
        .content_toggle(fab.clone(), "100".into(), false)
        .await
        .unwrap();
    assert_eq!(r.affected, ["100"]);
    assert!(mods.join("alpha-0.jar.disabled").exists());
    f.svc
        .content_toggle(fab.clone(), "100".into(), true)
        .await
        .unwrap();
    assert!(mods.join("alpha-0.jar").exists());

    let updates = f.svc.content_check_updates(fab.clone()).await.unwrap();
    assert_eq!(updates.len(), 1);
    let u = &updates[0];
    assert_eq!(
        (u.provider, u.project_id.as_str(), u.version_id.as_str()),
        (Some(Provider::Curseforge), "100", "1001")
    );
    assert_eq!(
        (u.from.as_deref(), u.to.as_str()),
        (Some("alpha-0.jar build"), "alpha-1.jar build")
    );
    let listed = f.svc.content_list(fab.clone()).await.unwrap();
    assert_eq!(
        listed[0].update_available.as_deref(),
        Some("alpha-1.jar build")
    );

    // Updating replaces the old jar and clears the hint.
    let items = f
        .svc
        .install_apply(
            Provider::Curseforge,
            "100".into(),
            Some("1001".into()),
            fab.clone(),
            vec![],
        )
        .await
        .unwrap();
    assert!(!mods.join("alpha-0.jar").exists() && mods.join("alpha-1.jar").exists());
    let alpha = items.iter().find(|i| i.project_id == "100").unwrap();
    assert!(alpha.update_available.is_none());
    assert!(f
        .svc
        .content_check_updates(fab.clone())
        .await
        .unwrap()
        .is_empty());

    // Dependents block uninstall of the library.
    let r = f
        .svc
        .content_uninstall(fab.clone(), "200".into())
        .await
        .unwrap();
    assert!(r.blocked);
    assert_eq!(r.dependents, ["100"]);
    let r = f
        .svc
        .content_uninstall(fab.clone(), "100".into())
        .await
        .unwrap();
    assert!(!r.blocked);
    assert!(!mods.join("alpha-1.jar").exists());
    let r = f
        .svc
        .content_uninstall(fab.clone(), "200".into())
        .await
        .unwrap();
    assert!(!r.blocked && r.items.is_empty());
}

#[tokio::test]
async fn content_items_carry_their_provider() {
    let (f, _, _) = cf_fx().await;
    let fab = f.profile("Fab", "1.21.1", "fabric");
    std::fs::write(f.dir(&fab).join("mods/hand.jar"), b"x").unwrap();
    f.svc
        .install_apply(
            Provider::Curseforge,
            "200".into(),
            None,
            fab.clone(),
            vec![],
        )
        .await
        .unwrap();
    // A record written before providers existed means Modrinth.
    let mut mods = f.mods(&fab);
    mods.push(mango_core::ModRecord {
        project_id: Some("OLD".into()),
        title: Some("Old".into()),
        filename: Some("old.jar".into()),
        file: Some(
            f.dir(&fab)
                .join("mods/old.jar")
                .to_string_lossy()
                .into_owned(),
        ),
        ..Default::default()
    });
    std::fs::write(f.dir(&fab).join("mods/old.jar"), b"x").unwrap();
    f.svc
        .inner
        .with_store(|s| s.update_profile(&fab, &json!({"mods": mods})).map(|_| ()))
        .unwrap();
    let items = f.svc.content_list(fab.clone()).await.unwrap();
    let by = |id: &str| items.iter().find(|i| i.project_id == id).unwrap().provider;
    assert_eq!(by("200"), Some(Provider::Curseforge));
    assert_eq!(by("OLD"), Some(Provider::Modrinth));
    assert_eq!(by("local:hand.jar"), None);
    let json = serde_json::to_value(&items).unwrap();
    assert!(json
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["provider"].is_null()));
}

// ---------------------------------------------------------------- Modrinth through the same commands

fn sha512(b: &[u8]) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha512::digest(b))
}

fn mr_version(
    base: &str,
    project: &str,
    id: &str,
    date: &str,
    file: &str,
    deps: Value,
    versions: &[&str],
) -> Value {
    let bytes = format!("mr:{file}").into_bytes();
    json!({
        "id": id, "project_id": project, "name": id, "version_number": format!("n-{id}"),
        "version_type": "release", "date_published": date, "downloads": 3,
        "game_versions": versions, "loaders": ["fabric"],
        "files": [{"url": format!("{base}/files/{file}"), "filename": file, "primary": true,
                   "size": bytes.len(), "hashes": {"sha512": sha512(&bytes)}}],
        "dependencies": deps
    })
}

fn mr_project(id: &str, kind: &str) -> Value {
    json!({
        "id": id, "slug": id, "title": format!("T-{id}"), "project_type": kind,
        "icon_url": "https://cdn.modrinth.com/data/x/icon.png", "description": "d", "body": "b",
        "downloads": 1, "followers": 2, "categories": [], "game_versions": ["1.21.1"],
        "loaders": ["fabric"], "updated": "2024-01-01T00:00:00Z"
    })
}

/// mrmod 2.0 requires mrlib (pinned build), suggests mropt, conflicts with mrbad.
async fn mr_server() -> (String, Arc<Mutex<Vec<Req>>>) {
    serve(move |base| {
        let base = base.to_string();
        Arc::new(move |r: &Req| {
            let path = r.path.split('?').next().unwrap();
            let v = |p: &str, id: &str, date: &str, file: &str, deps: Value| {
                mr_version(&base, p, id, date, file, deps, &["1.21.1"])
            };
            let new_deps = json!([
                {"project_id": "mrlib", "version_id": "lib-v1", "dependency_type": "required"},
                {"project_id": "mropt", "dependency_type": "optional"},
                {"project_id": "mrbad", "dependency_type": "incompatible"},
                {"project_id": "mremb", "dependency_type": "embedded"}
            ]);
            if let Some(name) = path.strip_prefix("/files/") {
                return (200, format!("mr:{name}").into_bytes());
            }
            match path {
                "/project/mrmod" => json_ok(mr_project("mrmod", "mod")),
                "/project/mrlib" => json_ok(mr_project("mrlib", "mod")),
                "/project/mropt" => json_ok(mr_project("mropt", "mod")),
                "/project/mrbad" => json_ok(mr_project("mrbad", "mod")),
                p if p.ends_with("/members") => json_ok(json!([])),
                "/project/mrmod/version" => json_ok(json!([
                    v(
                        "mrmod",
                        "mod-new",
                        "2024-03-01T00:00:00Z",
                        "mod-new.jar",
                        new_deps.clone()
                    ),
                    v(
                        "mrmod",
                        "mod-old",
                        "2024-01-01T00:00:00Z",
                        "mod-old.jar",
                        json!([])
                    ),
                ])),
                "/project/mrlib/version" => json_ok(json!([v(
                    "mrlib",
                    "lib-v1",
                    "2024-01-01T00:00:00Z",
                    "lib.jar",
                    json!([])
                )])),
                "/project/mropt/version" => json_ok(json!([v(
                    "mropt",
                    "opt-v1",
                    "2024-01-01T00:00:00Z",
                    "opt.jar",
                    json!([])
                )])),
                "/project/mrbad/version" => json_ok(json!([v(
                    "mrbad",
                    "bad-v1",
                    "2024-01-01T00:00:00Z",
                    "bad.jar",
                    json!([])
                )])),
                "/version/mod-new" => json_ok(v(
                    "mrmod",
                    "mod-new",
                    "2024-03-01T00:00:00Z",
                    "mod-new.jar",
                    new_deps,
                )),
                "/version/mod-old" => json_ok(v(
                    "mrmod",
                    "mod-old",
                    "2024-01-01T00:00:00Z",
                    "mod-old.jar",
                    json!([]),
                )),
                "/version/opt-v1" => json_ok(v(
                    "mropt",
                    "opt-v1",
                    "2024-01-01T00:00:00Z",
                    "opt.jar",
                    json!([]),
                )),
                "/version/bad-v1" => json_ok(v(
                    "mrbad",
                    "bad-v1",
                    "2024-01-01T00:00:00Z",
                    "bad.jar",
                    json!([]),
                )),
                "/version/lib-v1" => json_ok(v(
                    "mrlib",
                    "lib-v1",
                    "2024-01-01T00:00:00Z",
                    "lib.jar",
                    json!([]),
                )),
                "/search" => json_ok(json!({"hits": [{
                    "project_id": "mrmod", "slug": "mrmod", "title": "T", "author": "a",
                    "description": "d", "project_type": "mod", "downloads": 1, "follows": 2,
                    "categories": ["fabric"], "versions": ["1.21.1"], "date_modified": "2024"
                }], "total_hits": 1, "offset": 0})),
                _ => (404, b"{}".to_vec()),
            }
        })
    })
    .await
}

#[tokio::test]
async fn modrinth_uses_the_same_plan_and_apply_with_pinned_dependencies() {
    let (base, _) = mr_server().await;
    let f = fx(&base, "http://127.0.0.1:1", None);
    let fab = f.profile("Fab", "1.21.1", "fabric");
    let r = f
        .svc
        .search_projects(params(Provider::Modrinth, "mod"))
        .await
        .unwrap();
    assert_eq!(r.hits[0].provider, Provider::Modrinth);

    let plan = f
        .svc
        .install_plan(Provider::Modrinth, "mrmod".into(), None, fab.clone())
        .await
        .unwrap();
    let roles: Vec<(&str, PlanRole)> = plan
        .items
        .iter()
        .map(|i| (i.project_id.as_str(), i.role))
        .collect();
    assert_eq!(
        roles,
        [
            ("mrmod", PlanRole::Requested),
            ("mrlib", PlanRole::Required),
            ("mropt", PlanRole::Optional)
        ]
    );
    assert_eq!(item(&plan, "mrmod").version_id, "mod-new");
    assert_eq!(
        item(&plan, "mrlib").version_id,
        "lib-v1",
        "the pinned build"
    );
    assert!(plan.total_bytes > 0 && plan.conflicts.is_empty());

    let items = f
        .svc
        .install_apply(
            Provider::Modrinth,
            "mrmod".into(),
            None,
            fab.clone(),
            vec!["mropt".into()],
        )
        .await
        .unwrap();
    assert_eq!(items.len(), 3);
    assert!(items.iter().all(|i| i.provider == Some(Provider::Modrinth)));
    for file in ["mod-new.jar", "lib.jar", "opt.jar"] {
        assert_eq!(
            std::fs::read(f.dir(&fab).join("mods").join(file)).unwrap(),
            format!("mr:{file}").into_bytes()
        );
    }
    let rec = f
        .mods(&fab)
        .into_iter()
        .find(|r| r.project_id.as_deref() == Some("mrmod"))
        .unwrap();
    assert_eq!(rec.provider.as_deref(), Some("modrinth"));
    let deps = rec.required_dependencies.unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(
        (deps[0].project_id.as_str(), deps[0].version_id.as_deref()),
        ("mrlib", Some("lib-v1"))
    );

    // Targets: installed here, available elsewhere.
    let other = f.profile("Other", "1.21.1", "quilt");
    let ts = f
        .svc
        .install_targets(Provider::Modrinth, "mrmod".into(), None)
        .await
        .unwrap();
    assert_eq!(target(&ts, &fab).state, TargetState::Installed);
    assert_eq!(target(&ts, &other).state, TargetState::Available);
    assert_eq!(target(&ts, &other).required_deps, 1);
    // A second plan sees everything installed, including the conflict check.
    let plan = f
        .svc
        .install_plan(Provider::Modrinth, "mrmod".into(), None, fab.clone())
        .await
        .unwrap();
    assert!(plan.items.iter().all(|i| i.already_installed));
    assert_eq!(plan.total_bytes, 0);
}

#[tokio::test]
async fn an_installed_incompatible_modrinth_project_is_a_conflict() {
    let (base, _) = mr_server().await;
    let f = fx(&base, "http://127.0.0.1:1", None);
    let fab = f.profile("Fab", "1.21.1", "fabric");
    f.svc
        .install_apply(
            Provider::Modrinth,
            "mrbad".into(),
            None,
            fab.clone(),
            vec![],
        )
        .await
        .unwrap();
    let plan = f
        .svc
        .install_plan(Provider::Modrinth, "mrmod".into(), None, fab.clone())
        .await
        .unwrap();
    assert_eq!(
        plan.conflicts,
        [PlanConflict {
            project_id: "mrbad".into(),
            title: "T-mrbad".into(),
            with: "T-mrmod".into()
        }]
    );
    assert_eq!(item(&plan, "mrbad").role, PlanRole::Incompatible);
}

#[tokio::test]
async fn legacy_records_without_a_provider_count_as_modrinth() {
    let (base, _) = mr_server().await;
    let f = fx(&base, "http://127.0.0.1:1", None);
    let fab = f.profile("Fab", "1.21.1", "fabric");
    std::fs::write(f.dir(&fab).join("mods/mod-old.jar"), b"mr:mod-old.jar").unwrap();
    let rec = mango_core::ModRecord {
        project_id: Some("mrmod".into()),
        version_id: Some("mod-old".into()),
        version_number: Some("n-mod-old".into()),
        title: Some("T-mrmod".into()),
        filename: Some("mod-old.jar".into()),
        file: Some(
            f.dir(&fab)
                .join("mods/mod-old.jar")
                .to_string_lossy()
                .into_owned(),
        ),
        kind: Some(mango_core::ModType::Mod),
        required_dependencies: Some(vec![]),
        ..Default::default()
    };
    f.svc
        .inner
        .with_store(|s| s.update_profile(&fab, &json!({"mods": [rec]})).map(|_| ()))
        .unwrap();
    let ts = f
        .svc
        .install_targets(Provider::Modrinth, "mrmod".into(), None)
        .await
        .unwrap();
    let t = target(&ts, &fab);
    assert_eq!(
        (t.state, t.installed_version.as_deref()),
        (TargetState::Installed, Some("n-mod-old"))
    );
    let hits = f
        .svc
        .search_projects(params(Provider::Modrinth, "mod"))
        .await
        .unwrap()
        .hits;
    assert_eq!(hits[0].installed_in, std::slice::from_ref(&fab));
    let updates = f.svc.content_check_updates(fab.clone()).await.unwrap();
    assert_eq!(updates[0].provider, Some(Provider::Modrinth));
    assert_eq!(updates[0].version_id, "mod-new");
}

#[tokio::test]
async fn both_providers_live_side_by_side_in_one_profile() {
    let (mr_base, _) = mr_server().await;
    let (projects, files) = cf_world();
    let (cf_base, _) = serve(cf_server(projects, files, |_, _| None)).await;
    let f = fx(&mr_base, &cf_base, Some(KEY));
    let fab = f.profile("Fab", "1.21.1", "fabric");
    f.svc
        .install_apply(
            Provider::Modrinth,
            "mrmod".into(),
            Some("mod-old".into()),
            fab.clone(),
            vec![],
        )
        .await
        .unwrap();
    f.svc
        .install_apply(
            Provider::Curseforge,
            "100".into(),
            Some("1000".into()),
            fab.clone(),
            vec![],
        )
        .await
        .unwrap();
    let list = f.svc.content_list(fab.clone()).await.unwrap();
    assert_eq!(
        list.iter()
            .filter(|i| i.provider == Some(Provider::Modrinth))
            .count(),
        1
    );
    assert_eq!(
        list.iter()
            .filter(|i| i.provider == Some(Provider::Curseforge))
            .count(),
        1
    );

    let mut updates = f.svc.content_check_updates(fab.clone()).await.unwrap();
    updates.sort_by_key(|u| u.project_id.clone());
    assert_eq!(
        updates
            .iter()
            .map(|u| (u.provider.unwrap(), u.project_id.as_str()))
            .collect::<Vec<_>>(),
        [(Provider::Curseforge, "100"), (Provider::Modrinth, "mrmod")]
    );
    // Toggle and uninstall each in turn.
    f.svc
        .content_toggle(fab.clone(), "mrmod".into(), false)
        .await
        .unwrap();
    f.svc
        .content_toggle(fab.clone(), "100".into(), false)
        .await
        .unwrap();
    let mods = f.dir(&fab).join("mods");
    assert!(
        mods.join("mod-old.jar.disabled").exists() && mods.join("alpha-0.jar.disabled").exists()
    );
    let r = f
        .svc
        .content_uninstall(fab.clone(), "mrmod".into())
        .await
        .unwrap();
    assert!(!r.blocked);
    assert!(r
        .items
        .iter()
        .all(|i| i.provider == Some(Provider::Curseforge)));
}

// ---------------------------------------------------------------- modpacks

pub(crate) fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    use std::io::Write;
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        for (name, data) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }
    buf.into_inner()
}

fn cf_pack_world(entries: &[(&str, &[u8])]) -> (Vec<Value>, Vec<CfFile>) {
    let (projects, mut files) = cf_world();
    let mut pack = cf_file(10001, 1000, "the-pack-1.zip", &["1.21.1", "Fabric"]);
    pack.body = Some(zip_bytes(entries));
    files.push(pack);
    (projects, files)
}

fn cf_manifest() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "minecraft": {"version": "1.21.1", "modLoaders": [{"id": "fabric-0.16.9", "primary": true}]},
        "name": "The Pack", "overrides": "overrides",
        "files": [
            {"projectID": 100, "fileID": 1001, "required": true},
            {"projectID": 500, "fileID": 5001, "required": true},
            {"projectID": 200, "fileID": 2001, "required": false}
        ]
    }))
    .unwrap()
}

#[tokio::test]
async fn curseforge_modpack_plan_and_install() {
    let manifest = cf_manifest();
    let (projects, files) = cf_pack_world(&[
        ("manifest.json", &manifest),
        ("overrides/config/x.toml", b"x = 1"),
    ]);
    let (base, seen) = serve(cf_server(projects, files, |_, _| None)).await;
    let f = fx("http://127.0.0.1:1", &base, Some(KEY));

    let plan = f
        .svc
        .modpack_plan(Provider::Curseforge, "1000".into(), None)
        .await
        .unwrap();
    let alpha = cf_bytes("alpha-1.jar").len() as u64;
    assert_eq!(
        serde_json::to_value(&plan).unwrap(),
        json!({
            "provider": "curseforge", "projectId": "1000", "versionId": "10001", "name": "P1000",
            "mcVersion": "1.21.1", "loader": "fabric", "loaderVersion": "0.16.9",
            "fileCount": 2, "sizeBytes": alpha,
            "iconUrl": plan.icon_url,
            "blockedFiles": [{
                "name": "P500", "fileName": "secret-1.jar",
                "url": "https://www.curseforge.com/minecraft/mc-mods/secret",
                "sha1": hex::encode(Sha1::digest(cf_bytes("secret-1.jar"))),
                "size": cf_bytes("secret-1.jar").len(), "type": "mod", "state": "pending",
                "modrinth": null, "foundPath": null
            }]
        })
    );
    assert!(plan.icon_url.is_some());
    let cdn = |n: &str| {
        seen.lock()
            .unwrap()
            .iter()
            .filter(|r| r.path == format!("/cdn/{n}"))
            .count()
    };
    assert_eq!(cdn("the-pack-1.zip"), 1);

    let before = f.svc.app_state().unwrap().profiles.len();
    let view = f
        .svc
        .modpack_install(
            Provider::Curseforge,
            "1000".into(),
            Some("10001".into()),
            Some(" Mine ".into()),
        )
        .await
        .unwrap();
    assert_eq!(cdn("the-pack-1.zip"), 1, "the planned download is reused");
    assert_eq!(f.svc.app_state().unwrap().profiles.len(), before + 1);
    assert_eq!(
        (
            view.name.as_str(),
            view.mc_version.as_str(),
            view.loader,
            view.loader_version.as_deref()
        ),
        ("Mine", "1.21.1", mango_core::Loader::Fabric, Some("0.16.9"))
    );
    let dir = f.dir(&view.id);
    assert_eq!(
        std::fs::read(dir.join("mods/alpha-1.jar")).unwrap(),
        cf_bytes("alpha-1.jar")
    );
    assert!(!dir.join("mods/secret-1.jar").exists());
    assert!(
        !dir.join("mods/lib-1.jar").exists(),
        "optional files are not installed"
    );
    assert_eq!(cdn("secret-1.jar"), 0);
    assert_eq!(std::fs::read(dir.join("config/x.toml")).unwrap(), b"x = 1");
    assert_eq!(view.mod_count, 1);
    // The cached pack is gone and the blocked file was reported.
    assert!(!f.svc.paths().cache().join("the-pack-1.zip").exists());
    assert!(f
        .sink
        .warnings()
        .iter()
        .any(|w| w.contains("secret-1.jar") && w.contains("curseforge.com")));
    let last = f
        .sink
        .events()
        .into_iter()
        .filter_map(|e| match e {
            Event::InstallProgress(p) if p.profile_id == view.id => Some(p),
            _ => None,
        })
        .next_back()
        .unwrap();
    assert_eq!((last.done, last.total), (1, 1));
}

#[tokio::test]
async fn a_hostile_curseforge_pack_leaves_no_profile_and_no_cache() {
    let manifest = cf_manifest();
    let (projects, files) = cf_pack_world(&[
        ("manifest.json", &manifest),
        ("overrides/../../escape.txt", b"nope"),
    ]);
    let (base, seen) = serve(cf_server(projects, files, |_, _| None)).await;
    let f = fx("http://127.0.0.1:1", &base, Some(KEY));
    let before = f.svc.app_state().unwrap().profiles.len();
    let e = f
        .svc
        .modpack_install(Provider::Curseforge, "1000".into(), None, None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "curseforge.archive");
    assert_eq!(f.svc.app_state().unwrap().profiles.len(), before);
    assert!(!f.svc.paths().cache().join("the-pack-1.zip").exists());
    assert!(!seen
        .lock()
        .unwrap()
        .iter()
        .any(|r| r.path == "/cdn/alpha-1.jar"));
    // A mod is not a modpack and vice versa.
    assert_eq!(
        f.svc
            .modpack_plan(Provider::Curseforge, "100".into(), None)
            .await
            .unwrap_err()
            .code,
        "not_a_modpack"
    );
}

#[tokio::test]
async fn modrinth_modpack_plan_and_install_download_the_pack_once() {
    let index = serde_json::to_vec(&json!({
        "name": "MR Pack", "formatVersion": 1, "game": "minecraft",
        "dependencies": {"minecraft": "1.21.1", "fabric-loader": "0.16.9"},
        "files": [{
            "path": "mods/a.jar", "hashes": {"sha512": sha512(b"mr:a.jar")},
            "downloads": ["PLACEHOLDER/files/a.jar"], "fileSize": 8
        }]
    }))
    .unwrap();
    let (base, seen) = {
        // The index must name the mock server, which is only known once it
        // is bound: build the pack inside the handler factory.
        let index = index.clone();
        serve(move |base| {
            let index = String::from_utf8(index.clone()).unwrap().replace("PLACEHOLDER", base);
            let pack = zip_bytes(&[
                ("modrinth.index.json", index.as_bytes()),
                ("overrides/options.txt", b"lang:de"),
            ]);
            let base = base.to_string();
            Arc::new(move |r: &Req| {
                let path = r.path.split('?').next().unwrap();
                let version = json!({
                    "id": "pack-v1", "project_id": "mrpack", "name": "pack", "version_number": "1",
                    "version_type": "release", "date_published": "2024-01-01T00:00:00Z",
                    "game_versions": ["1.21.1"], "loaders": ["fabric"],
                    "files": [{"url": format!("{base}/files/pack.mrpack"), "filename": "pack.mrpack",
                               "primary": true, "size": pack.len(), "hashes": {"sha512": sha512(&pack)}}],
                    "dependencies": []
                });
                match path {
                    "/project/mrpack" => json_ok(mr_project("mrpack", "modpack")),
                    "/project/mrmod" => json_ok(mr_project("mrmod", "mod")),
                    "/project/mrpack/version" => json_ok(json!([version])),
                    "/version/pack-v1" => json_ok(version),
                    "/files/pack.mrpack" => (200, pack.clone()),
                    "/files/a.jar" => (200, b"mr:a.jar".to_vec()),
                    _ => (404, b"{}".to_vec()),
                }
            })
        })
        .await
    };
    let f = fx(&base, "http://127.0.0.1:1", None);
    let plan = f
        .svc
        .modpack_plan(Provider::Modrinth, "mrpack".into(), None)
        .await
        .unwrap();
    assert_eq!(
        (
            plan.version_id.as_str(),
            plan.mc_version.as_str(),
            plan.loader,
            plan.loader_version.as_deref()
        ),
        (
            "pack-v1",
            "1.21.1",
            mango_core::Loader::Fabric,
            Some("0.16.9")
        )
    );
    assert_eq!(
        (plan.file_count, plan.size_bytes, plan.name.as_str()),
        (1, Some(8), "T-mrpack")
    );
    assert!(plan.blocked_files.is_empty());
    let view = f
        .svc
        .modpack_install(
            Provider::Modrinth,
            "mrpack".into(),
            Some(plan.version_id.clone()),
            None,
        )
        .await
        .unwrap();
    assert_eq!(view.loader, mango_core::Loader::Fabric);
    let dir = f.dir(&view.id);
    assert_eq!(std::fs::read(dir.join("mods/a.jar")).unwrap(), b"mr:a.jar");
    assert_eq!(std::fs::read(dir.join("options.txt")).unwrap(), b"lang:de");
    let pack_gets = seen
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.path == "/files/pack.mrpack")
        .count();
    assert_eq!(pack_gets, 1, "plan and install share one download");
    assert!(!f.svc.paths().cache().join("pack.mrpack").exists());
    assert_eq!(
        f.svc
            .modpack_plan(Provider::Modrinth, "mrmod".into(), None)
            .await
            .unwrap_err()
            .code,
        "not_a_modpack"
    );
}

#[test]
fn wire_shapes_match_the_contract() {
    let t = InstallTarget {
        profile_id: "p".into(),
        state: TargetState::Incompatible,
        installed_version: None,
        reason: Some(TargetReason::DistributionBlocked),
        detail: None,
        required_deps: 2,
    };
    assert_eq!(
        serde_json::to_value(&t).unwrap(),
        json!({"profileId": "p", "state": "incompatible", "installedVersion": null,
               "reason": "distribution_blocked", "detail": null, "requiredDeps": 2})
    );
    let reasons: Vec<Value> = [
        TargetReason::Loader,
        TargetReason::McVersion,
        TargetReason::ContentType,
        TargetReason::NoCompatibleVersion,
    ]
    .iter()
    .map(|r| serde_json::to_value(r).unwrap())
    .collect();
    assert_eq!(
        reasons,
        [
            "loader",
            "mc_version",
            "content_type",
            "no_compatible_version"
        ]
    );
    let it = InstallPlanItem {
        provider: Provider::Curseforge,
        project_id: "1".into(),
        version_id: "2".into(),
        title: "T".into(),
        kind: mango_core::ModType::Shader,
        icon_url: None,
        role: PlanRole::Optional,
        already_installed: false,
        installed_version: None,
        distribution_blocked: true,
        size_bytes: None,
        download_url: Some("https://x".into()),
    };
    assert_eq!(
        serde_json::to_value(&it).unwrap(),
        json!({"provider": "curseforge", "projectId": "1", "versionId": "2", "title": "T", "type": "shader",
               "iconUrl": null, "role": "optional", "alreadyInstalled": false, "installedVersion": null,
               "distributionBlocked": true, "sizeBytes": null, "downloadUrl": "https://x"})
    );
    let sp: SearchParams = serde_json::from_value(json!({"projectType": "mod"})).unwrap();
    assert_eq!(
        sp.provider,
        Provider::Modrinth,
        "legacy callers omit the provider"
    );
    let sp: SearchParams =
        serde_json::from_value(json!({"projectType": "mod", "provider": "curseforge"})).unwrap();
    assert_eq!(sp.provider, Provider::Curseforge);
}
