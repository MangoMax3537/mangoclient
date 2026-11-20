#![allow(clippy::unwrap_used)]

use crate::events::ProgressThrottle;
use crate::launch::handle_game_event;
use crate::service::lock;
use crate::*;
use mango_runtime::{ExitInfo, GameEvent, GameState, LogLevel};
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct MockSink(Mutex<Vec<Event>>);

impl EventSink for MockSink {
    fn emit(&self, event: Event) {
        self.0.lock().unwrap().push(event);
    }
}

impl MockSink {
    fn events(&self) -> Vec<Event> {
        self.0.lock().unwrap().clone()
    }
}

fn setup() -> (tempfile::TempDir, AppService, Arc<MockSink>) {
    let tmp = tempfile::tempdir().unwrap();
    let sink = Arc::new(MockSink::default());
    let opts = ServiceOptions {
        open_folders: false,
        ..ServiceOptions::default()
    };
    let svc = AppService::new(mango_core::Paths::new(tmp.path()), sink.clone(), opts).unwrap();
    (tmp, svc, sink)
}

fn new_profile(svc: &AppService, name: &str) -> ProfileView {
    svc.profile_create(name.into(), "1.21.1".into(), "fabric".into(), None)
        .unwrap()
}

#[test]
fn app_state_has_default_profile_and_camel_case_json() {
    let (tmp, svc, _) = setup();
    let st = svc.app_state().unwrap();
    assert_eq!(st.profiles.len(), 1);
    assert!(st.running.is_empty());
    assert_eq!(st.root_dir, tmp.path().display().to_string());
    let v = serde_json::to_value(&st).unwrap();
    assert!(v["totalRamMb"].is_number());
    assert!(v["config"]["keepLauncherOpen"].is_boolean());
    assert!(v["config"].get("theme").is_none());
    assert_eq!(v["profiles"][0]["loader"], "fabric");
    assert!(v["profiles"][0]["coverUrl"].is_null());
    assert!(v["profiles"][0]["modCount"].is_number());
}

#[test]
fn config_set_validates_and_persists() {
    let (tmp, svc, _) = setup();
    let c = svc
        .config_set(json!({"language": "en", "ram": 4096}))
        .unwrap();
    assert_eq!(c.language, "en");
    assert_eq!(c.ram, 4096);
    let err = svc.config_set(json!({"ram": 1})).unwrap_err();
    assert_eq!(err.code, "validation");
    assert!(!err.needs_relogin);
    // Survives a reopen.
    let again = AppService::new(
        mango_core::Paths::new(tmp.path()),
        Arc::new(MockSink::default()),
        ServiceOptions::default(),
    )
    .unwrap();
    assert_eq!(again.app_state().unwrap().config.language, "en");
}

#[tokio::test]
async fn profile_crud_and_select() {
    let (_t, svc, _) = setup();
    let p = new_profile(&svc, "Alpha");
    assert_eq!(p.loader_version, None);
    let u = svc
        .profile_update(
            p.id.clone(),
            json!({"name": "Beta", "ram": 3072, "loaderVersion": "0.16.0", "modCount": 99, "coverUrl": "x"}),
        ).await
        .unwrap();
    assert_eq!(u.name, "Beta");
    assert_eq!(u.ram, Some(3072));
    assert_eq!(u.loader_version.as_deref(), Some("0.16.0"));
    assert_eq!(u.mod_count, 0);
    let u = svc
        .profile_update(p.id.clone(), json!({"loaderVersion": null}))
        .await
        .unwrap();
    assert_eq!(u.loader_version, None);
    assert!(svc
        .profile_update(p.id.clone(), json!({"loader": "bogus"}))
        .await
        .is_err());
    assert_eq!(
        svc.profile_update("nope".into(), json!({}))
            .await
            .unwrap_err()
            .code,
        "invalid_profile_id"
    );
    assert_eq!(
        svc.profile_select(p.id.clone()).unwrap().selected_profile,
        Some(p.id.clone())
    );
    svc.profile_delete(p.id.clone()).unwrap();
    assert_eq!(
        svc.profile_delete(p.id.clone()).unwrap_err().code,
        "profile_not_found"
    );
    assert_eq!(
        svc.profile_select(p.id).unwrap_err().code,
        "profile_not_found"
    );
}

#[tokio::test]
async fn duplicate_copies_instance_and_rebases_mod_paths() {
    let (tmp, svc, _) = setup();
    let p = new_profile(&svc, "Src");
    let paths = svc.paths().clone();
    let old_dir = paths.instance_dir(&p.id).unwrap();
    std::fs::write(old_dir.join("mods").join("a.jar"), b"jar").unwrap();
    std::fs::write(old_dir.join("options.txt"), b"x").unwrap();
    let file = old_dir.join("mods").join("a.jar").display().to_string();
    svc.inner
        .with_store(|s| {
            s.update_profile(
                &p.id,
                &json!({"mods": [{"title": "A", "filename": "a.jar", "file": file}]}),
            )
            .map(|_| ())
        })
        .unwrap();

    let d = svc.profile_duplicate(p.id.clone()).await.unwrap();
    assert_ne!(d.id, p.id);
    assert_eq!(d.name, "Src (copy)");
    assert_eq!(d.mod_count, 1);
    let new_dir = paths.instance_dir(&d.id).unwrap();
    assert!(new_dir.join("options.txt").is_file());
    let rec = svc
        .inner
        .with_store(|s| s.get_profile(&d.id).unwrap().mods[0].clone());
    assert_eq!(
        rec.file.unwrap(),
        new_dir.join("mods").join("a.jar").display().to_string()
    );
    assert!(tmp.path().join("accounts.json").exists());
}

#[test]
fn accounts_offline_select_remove_and_no_secrets() {
    let (_t, svc, _) = setup();
    let a = svc.account_add_offline("Steve_1".into()).unwrap();
    assert_eq!(a.name, "Steve_1");
    assert_eq!(a.kind, mango_core::AccountType::Offline);
    assert!(!a.expired);
    let json = serde_json::to_value(&a).unwrap();
    assert_eq!(json["type"], "offline");
    assert!(json.get("accessToken").is_none());
    assert_eq!(
        svc.account_add_offline("x".into()).unwrap_err().code,
        "auth.invalid_username"
    );
    let b = svc.account_add_offline("Alex_2".into()).unwrap();
    assert_eq!(
        svc.account_select(b.uuid.clone()).unwrap().selected_account,
        Some(b.uuid.clone())
    );
    assert_eq!(
        svc.account_select("zzz".into()).unwrap_err().code,
        "account_not_found"
    );
    svc.account_remove(b.uuid).unwrap();
    let st = svc.app_state().unwrap();
    assert_eq!(st.accounts.len(), 1);
    assert_eq!(st.config.selected_account, Some(a.uuid));
}

#[tokio::test]
async fn microsoft_sign_in_without_a_window_fails_with_a_stable_code() {
    let (_t, svc, _) = setup();
    let e = svc.account_sign_in_microsoft().await.unwrap_err();
    assert_eq!(e.code, "auth.login_failed");
    assert_eq!(
        serde_json::to_value(&e).unwrap(),
        json!({"code": "auth.login_failed", "message": e.message, "needsRelogin": false})
    );
}

#[test]
fn needs_relogin_flag_follows_the_auth_code() {
    let e: AppError = mango_auth::AuthError::NeedsRelogin.into();
    assert_eq!(e.code, "auth.needs_relogin");
    assert!(e.needs_relogin);
}

#[tokio::test]
async fn launch_refuses_unknown_profile_missing_account_and_unusable_microsoft() {
    let (_t, svc, sink) = setup();
    let p = new_profile(&svc, "L");
    assert_eq!(
        svc.game_launch("bad".into(), None).await.unwrap_err().code,
        "invalid_profile_id"
    );
    assert_eq!(
        svc.game_launch("12345678-1234-1234-1234-123456789abc".into(), None)
            .await
            .unwrap_err()
            .code,
        "profile_not_found"
    );
    assert_eq!(
        svc.game_launch(p.id.clone(), None).await.unwrap_err().code,
        "no_account"
    );
    // The guard is released: a second attempt reaches the same error.
    assert!(!svc.is_busy(&p.id));
    // The failed launch ends in `stopped` with an error log line.
    let ev = sink.events();
    assert!(ev
        .iter()
        .any(|e| matches!(e, Event::LaunchState(s) if s.state == LaunchState::Stopped)));
    assert!(ev
        .iter()
        .any(|e| matches!(e, Event::LaunchLog(l) if l.level == LogLevelName::Error)));

    svc.inner
        .with_store(|s| {
            s.upsert_account(
                serde_json::from_value(json!({"type": "microsoft", "uuid": "u1", "name": "Ms",
                    "skins": [], "capes": []}))
                .unwrap(),
            )
        })
        .unwrap();
    assert_eq!(
        svc.game_launch(p.id, None).await.unwrap_err().code,
        "auth.needs_relogin"
    );
}

#[tokio::test]
async fn double_launch_is_refused() {
    let (_t, svc, _) = setup();
    let p = new_profile(&svc, "D");
    lock(&svc.inner.launching).insert(p.id.clone());
    let e = svc.game_launch(p.id.clone(), None).await.unwrap_err();
    assert_eq!(e.code, "already_running");
    // The refusal must not clear the other launch's marker.
    assert!(svc.is_busy(&p.id));
    assert_eq!(
        svc.profile_delete(p.id).unwrap_err().code,
        "already_running"
    );
}

#[test]
fn game_exit_records_playtime_and_ends_in_stopped_or_crashed() {
    let (_t, svc, sink) = setup();
    let p = new_profile(&svc, "G");
    svc.inner
        .patch_profile(
            &p.id,
            &json!({"playTimeMs": 1000, "session": {"pid": 4242, "versionId": "v", "startedAt": 1}}),
        )
        .unwrap();
    let weak = Arc::downgrade(&svc.inner);
    handle_game_event(&weak, &p.id, GameEvent::Running);
    handle_game_event(
        &weak,
        &p.id,
        GameEvent::Log {
            line: "[10:00:00] [main/WARN]: hm".into(),
            level: LogLevel::Info,
        },
    );
    handle_game_event(
        &weak,
        &p.id,
        GameEvent::Exited(ExitInfo {
            code: Some(1),
            state: GameState::Crashed,
            played_ms: Some(5000),
        }),
    );
    let prof = svc
        .inner
        .with_store(|s| s.get_profile(&p.id).unwrap().clone());
    assert_eq!(prof.play_time_ms, 6000);
    assert!(prof.session.is_none());
    assert!(prof.last_played.is_some());

    let ev = sink.events();
    let states: Vec<_> = ev
        .iter()
        .filter_map(|e| match e {
            Event::LaunchState(s) => Some(s.state),
            _ => None,
        })
        .collect();
    assert_eq!(states, vec![LaunchState::Running, LaunchState::Crashed]);
    assert!(ev
        .iter()
        .any(|e| matches!(e, Event::LaunchLog(l) if l.level == LogLevelName::Warn)));

    // Already-flushed play time (None) must not be added twice.
    handle_game_event(
        &weak,
        &p.id,
        GameEvent::Exited(ExitInfo {
            code: Some(0),
            state: GameState::Stopped,
            played_ms: None,
        }),
    );
    let t = svc
        .inner
        .with_store(|s| s.get_profile(&p.id).unwrap().play_time_ms);
    assert_eq!(t, 6000);
}

#[test]
fn startup_clears_sessions_of_dead_processes() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = mango_core::Paths::new(tmp.path());
    {
        let mut store = mango_core::Store::open(paths.clone()).unwrap();
        let id = store.profiles[0].id.clone();
        store
            .update_profile(
                &id,
                &json!({"session": {"pid": 2_000_000_000u64, "versionId": "v", "startedAt": 1}}),
            )
            .unwrap();
    }
    let svc = AppService::new(
        paths,
        Arc::new(MockSink::default()),
        ServiceOptions::default(),
    )
    .unwrap();
    assert!(svc.game_running().is_empty());
    assert!(svc.inner.with_store(|s| s.profiles[0].session.is_none()));
}

#[test]
fn progress_is_throttled_per_profile() {
    let mut t = ProgressThrottle::default();
    let t0 = Instant::now();
    assert!(t.ready("a", false, t0));
    assert!(!t.ready("a", false, t0 + Duration::from_millis(50)));
    assert!(t.ready("b", false, t0 + Duration::from_millis(50)));
    assert!(t.ready("a", true, t0 + Duration::from_millis(60)));
    assert!(t.ready("a", false, t0 + Duration::from_millis(170)));
}

#[test]
fn event_names_and_payloads_match_the_contract() {
    let e = Event::LaunchProgress(LaunchProgressEvent {
        profile_id: "p".into(),
        phase: ProgressPhase::Libraries,
        done: 1,
        total: 2,
        label: "x".into(),
    });
    assert_eq!(e.name(), "launch:progress");
    assert_eq!(
        e.payload(),
        json!({"profileId": "p", "phase": "libraries", "done": 1, "total": 2, "label": "x"})
    );
    let e = Event::LaunchState(LaunchStateEvent {
        profile_id: "p".into(),
        state: LaunchState::Downloading,
    });
    assert_eq!(e.name(), "launch:state");
    assert_eq!(
        e.payload(),
        json!({"profileId": "p", "state": "downloading"})
    );
    let e = Event::LaunchLog(LaunchLogEvent {
        profile_id: "p".into(),
        line: "l".into(),
        level: LogLevelName::Warn,
    });
    assert_eq!(e.name(), "launch:log");
    assert_eq!(e.payload()["level"], "warn");
}

#[test]
fn manifest_is_mapped_to_the_contract_shape() {
    let raw = json!({
        "latest": {"release": "1.21.11", "snapshot": "26w01a"},
        "versions": [{"id": "1.21.11", "type": "release", "releaseTime": "2025-12-09T00:00:00+00:00", "url": "u"}]
    });
    let m = crate::service::parse_manifest_for_test(&raw);
    assert_eq!(
        serde_json::to_value(&m).unwrap(),
        json!({"latest": {"release": "1.21.11", "snapshot": "26w01a"},
               "versions": [{"id": "1.21.11", "type": "release", "releaseTime": "2025-12-09T00:00:00+00:00"}],
               "installed": []})
    );
}

#[tokio::test]
async fn loader_versions_vanilla_is_empty_and_bad_loader_rejected() {
    let (_t, svc, _) = setup();
    assert!(svc
        .loader_versions("vanilla".into(), "1.21".into())
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        svc.loader_versions("x".into(), "1.21".into())
            .await
            .unwrap_err()
            .code,
        "validation"
    );
}

#[test]
fn open_folder_returns_paths_and_rejects_bad_targets() {
    let (tmp, svc, _) = setup();
    assert_eq!(
        svc.open_folder("root".into()).unwrap(),
        tmp.path().display().to_string()
    );
    assert!(svc.open_folder("logs".into()).unwrap().ends_with("logs"));
    let p = new_profile(&svc, "F");
    assert!(svc.open_folder(p.id).unwrap().contains("instances"));
    assert_eq!(
        svc.open_folder("../etc".into()).unwrap_err().code,
        "invalid_profile_id"
    );
    assert!(svc
        .game_stop("12345678-1234-1234-1234-123456789abc".into())
        .is_ok_and(|b| !b));
}
