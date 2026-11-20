#![allow(clippy::unwrap_used)]
//! Part 4 of `docs/ipc.md`: Modrinth browsing, install planning, apply and
//! modpacks. Mock servers and a temp data dir only: no real network.

use crate::tests_p2::{json_ok, serve, Req};
use crate::*;
use mango_core::Paths;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

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
}

fn fx(modrinth: &str) -> Fx {
    let tmp = tempfile::tempdir().unwrap();
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
        ..ServiceOptions::default()
    };
    let svc = AppService::new(Paths::new(tmp.path()), Arc::new(Sink::default()), opts).unwrap();
    Fx { _tmp: tmp, svc }
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

fn params(kind: &str) -> SearchParams {
    SearchParams {
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

// ---------------------------------------------------------------- Modrinth

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
    let f = fx(&base);
    let fab = f.profile("Fab", "1.21.1", "fabric");
    let r = f.svc.search_projects(params("mod")).await.unwrap();
    assert_eq!(r.hits[0].project_id, "mrmod");

    let plan = f
        .svc
        .install_plan("mrmod".into(), None, fab.clone())
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
        .install_apply("mrmod".into(), None, fab.clone(), vec!["mropt".into()])
        .await
        .unwrap();
    assert_eq!(items.len(), 3);
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
    assert_eq!(rec.provider, None, "new records carry no provider");
    let deps = rec.required_dependencies.unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(
        (deps[0].project_id.as_str(), deps[0].version_id.as_deref()),
        ("mrlib", Some("lib-v1"))
    );

    // Targets: installed here, available elsewhere.
    let other = f.profile("Other", "1.21.1", "quilt");
    let ts = f.svc.install_targets("mrmod".into(), None).await.unwrap();
    assert_eq!(target(&ts, &fab).state, TargetState::Installed);
    assert_eq!(target(&ts, &other).state, TargetState::Available);
    assert_eq!(target(&ts, &other).required_deps, 1);
    // A second plan sees everything installed, including the conflict check.
    let plan = f
        .svc
        .install_plan("mrmod".into(), None, fab.clone())
        .await
        .unwrap();
    assert!(plan.items.iter().all(|i| i.already_installed));
    assert_eq!(plan.total_bytes, 0);
}

#[tokio::test]
async fn an_installed_incompatible_modrinth_project_is_a_conflict() {
    let (base, _) = mr_server().await;
    let f = fx(&base);
    let fab = f.profile("Fab", "1.21.1", "fabric");
    f.svc
        .install_apply("mrbad".into(), None, fab.clone(), vec![])
        .await
        .unwrap();
    let plan = f
        .svc
        .install_plan("mrmod".into(), None, fab.clone())
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
    let f = fx(&base);
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
    let ts = f.svc.install_targets("mrmod".into(), None).await.unwrap();
    let t = target(&ts, &fab);
    assert_eq!(
        (t.state, t.installed_version.as_deref()),
        (TargetState::Installed, Some("n-mod-old"))
    );
    let hits = f.svc.search_projects(params("mod")).await.unwrap().hits;
    assert_eq!(hits[0].installed_in, std::slice::from_ref(&fab));
    let updates = f.svc.content_check_updates(fab.clone()).await.unwrap();
    assert_eq!(updates[0].version_id, "mod-new");
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
    let f = fx(&base);
    let plan = f.svc.modpack_plan("mrpack".into(), None).await.unwrap();
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
    let view = f
        .svc
        .modpack_install("mrpack".into(), Some(plan.version_id.clone()), None)
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
            .modpack_plan("mrmod".into(), None)
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
        reason: Some(TargetReason::Loader),
        detail: None,
        required_deps: 2,
    };
    assert_eq!(
        serde_json::to_value(&t).unwrap(),
        json!({"profileId": "p", "state": "incompatible", "installedVersion": null,
               "reason": "loader", "detail": null, "requiredDeps": 2})
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
        project_id: "1".into(),
        version_id: "2".into(),
        title: "T".into(),
        kind: mango_core::ModType::Shader,
        icon_url: None,
        role: PlanRole::Optional,
        already_installed: false,
        installed_version: None,
        size_bytes: None,
    };
    assert_eq!(
        serde_json::to_value(&it).unwrap(),
        json!({"projectId": "1", "versionId": "2", "title": "T", "type": "shader",
               "iconUrl": null, "role": "optional", "alreadyInstalled": false,
               "installedVersion": null, "sizeBytes": null})
    );
    let sp: SearchParams = serde_json::from_value(json!({"projectType": "mod"})).unwrap();
    assert_eq!(
        (sp.query.as_str(), sp.sort.as_str(), sp.limit),
        ("", "relevance", 30)
    );
}

/// Records and files written by the removed CurseForge integration still
/// load, show up as local jars and are never sent to Modrinth.
#[tokio::test]
async fn legacy_curseforge_records_load_as_local_jars_and_are_left_alone() {
    let (base, seen) = mr_server().await;
    let f = fx(&base);
    let fab = f.profile("Fab", "1.21.1", "fabric");
    let mods = f.dir(&fab).join("mods");
    std::fs::write(mods.join("cf-mod.jar"), b"cf").unwrap();
    // The sidecar the removed blocked-files feature wrote.
    let sidecar = f.dir(&fab).join(".mangoclient-blocked.json");
    std::fs::write(
        &sidecar,
        br#"{"version":1,"files":[{"name":"X","fileName":"x.jar","url":"https://www.curseforge.com/minecraft/mc-mods/x","sha1":null,"size":null,"type":"mod","state":"pending","modrinth":null,"foundPath":null}]}"#,
    )
    .unwrap();
    let rec = json!({
        "projectId": "238222", "title": "CF Mod", "type": "mod", "provider": "curseforge",
        "versionId": "5000", "versionNumber": "1.0", "filename": "cf-mod.jar",
        "file": mods.join("cf-mod.jar").to_string_lossy(), "installedAt": 1
    });
    f.svc
        .inner
        .with_store(|s| s.update_profile(&fab, &json!({"mods": [rec]})).map(|_| ()))
        .unwrap();

    // A restart: everything is read from disk again.
    let reopened = AppService::new(
        Paths::new(f.svc.paths().root()),
        Arc::new(Sink::default()),
        ServiceOptions {
            open_folders: false,
            update_dev: true,
            auto_update_check: false,
            partners: Vec::new(),
            modrinth_api: base.clone(),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    let list = reopened.content_list(fab.clone()).await.unwrap();
    assert_eq!(list.len(), 1);
    assert!(list[0].local, "unknown source = local jar");
    assert_eq!(list[0].filename, "cf-mod.jar");
    let state = reopened.app_state().unwrap();
    assert!(state.profiles.iter().any(|p| p.id == fab));
    // Modrinth is never asked about the foreign id; nothing to update.
    let updates = reopened.content_check_updates(fab.clone()).await.unwrap();
    assert!(updates.is_empty());
    assert!(!seen
        .lock()
        .unwrap()
        .iter()
        .any(|r| r.path.contains("238222")));
    // Neither the jar nor the sidecar was touched.
    assert_eq!(std::fs::read(mods.join("cf-mod.jar")).unwrap(), b"cf");
    assert!(sidecar.exists());
    // Search results do not claim the foreign project is installed.
    let hits = reopened.search_projects(params("mod")).await.unwrap().hits;
    assert!(hits[0].installed_in.is_empty());
    // The stored record is carried along unchanged.
    assert!(reopened.profile(&fab).unwrap().mods[0].provider.as_deref() == Some("curseforge"));
}

#[tokio::test]
async fn install_targets_cover_the_incompatible_reasons() {
    let (base, _) = mr_server().await;
    let f = fx(&base);
    let fab = f.profile("Fab", "1.21.1", "fabric");
    let old = f.profile("Old", "1.20.1", "fabric");
    let forge = f.profile("Forge", "1.21.1", "forge");
    let vanilla = f.profile("Van", "1.21.1", "vanilla");
    let ts = f.svc.install_targets("mrmod".into(), None).await.unwrap();
    assert_eq!(target(&ts, &fab).state, TargetState::Available);
    let reason = |id: &str| {
        let t = target(&ts, id);
        assert_eq!(t.state, TargetState::Incompatible, "{id}");
        t.reason.unwrap()
    };
    assert_eq!(reason(&old), TargetReason::McVersion);
    assert_eq!(reason(&forge), TargetReason::Loader);
    assert_eq!(reason(&vanilla), TargetReason::Loader);
}

#[tokio::test]
async fn plan_errors_are_stable_codes() {
    let (base, _) = mr_server().await;
    let f = fx(&base);
    let fab = f.profile("Fab", "1.21.1", "fabric");
    let forge = f.profile("Forge", "1.20.1", "forge");
    let vanilla = f.profile("Van", "1.21.1", "vanilla");
    let code = |r: Res<InstallPlan>| r.unwrap_err().code;
    assert_eq!(
        code(f.svc.install_plan("mrmod".into(), None, forge).await),
        "no_compatible_version"
    );
    assert_eq!(
        code(f.svc.install_plan("mrmod".into(), None, vanilla).await),
        "no_compatible_version"
    );
    assert_eq!(
        code(f.svc.install_plan("a b".into(), None, fab.clone()).await),
        "validation"
    );
    assert_eq!(
        code(
            f.svc
                .install_plan("mrmod".into(), None, "nope".into())
                .await
        ),
        "invalid_profile_id"
    );
}
