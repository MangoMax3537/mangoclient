//! End-to-end tests of `mango-dev` over real HTTP with a throw-away data dir
//! and no network (every endpoint of the service points at a closed port).
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod ts_check;

use futures_util::StreamExt;
use mango_app::{Host, ServiceOptions};
use mango_auth::MemoryDeviceKey;
use mango_devserver::{start, RunningServer, ServerConfig};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

const ORIGIN: &str = "http://127.0.0.1:5173";
/// Nothing listens here: any accidental network call fails at once.
const DEAD: &str = "http://127.0.0.1:9";

struct Fx {
    server: RunningServer,
    http: reqwest::Client,
    _home: tempfile::TempDir,
}

impl Fx {
    async fn new() -> Fx {
        let home = tempfile::tempdir().unwrap();
        let options = ServiceOptions {
            open_folders: false,
            manifest_url: format!("{DEAD}/manifest.json"),
            modrinth_api: DEAD.into(),
            mclogs_api: format!("{DEAD}/log"),
            update_manifest_url: format!("{DEAD}/latest.json"),
            auto_update_check: false,
            fetch: mango_net::FetchOptions {
                retries: 0,
                timeout: Duration::from_secs(2),
                ..Default::default()
            },
            ..ServiceOptions::default()
        };
        let server = start(ServerConfig {
            home: home.path().to_path_buf(),
            allow_origins: vec![ORIGIN.into()],
            token: None,
            port: 0,
            options,
            host: Some(Host::system(false).with_device_keys(Arc::new(MemoryDeviceKey::new()))),
            background: false,
        })
        .await
        .unwrap();
        Fx {
            server,
            http: reqwest::Client::new(),
            _home: home,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.server.base_url())
    }

    /// POST with the token; returns status and parsed JSON body. No type checks.
    async fn call(&self, command: &str, args: Value) -> (u16, Value) {
        let res = self
            .http
            .post(self.url(&format!("/invoke/{command}")))
            .bearer_auth(&self.server.token)
            .json(&args)
            .send()
            .await
            .unwrap();
        let status = res.status().as_u16();
        (status, res.json().await.unwrap_or(Value::Null))
    }

    /// A call that must succeed: the arguments must fit the generated args
    /// type and the result the generated result type.
    async fn ok(&self, command: &str, args: Value) -> Value {
        let (args_ty, result_ty) = ts_check::command_types(command);
        let checked = if args.is_null() {
            json!({})
        } else {
            args.clone()
        };
        ts_check::check(&checked, &args_ty, "args")
            .unwrap_or_else(|e| panic!("{command} args: {e}"));
        let (status, body) = self.call(command, args).await;
        assert_eq!(status, 200, "{command} failed: {body}");
        ts_check::check(&body, &result_ty, "result")
            .unwrap_or_else(|e| panic!("{command} result: {e}"));
        body
    }
}

fn keys(v: &Value) -> BTreeSet<String> {
    v.as_object().unwrap().keys().cloned().collect()
}

#[tokio::test]
async fn real_flow_over_http_matches_the_generated_types() {
    let fx = Fx::new().await;

    let state = fx.ok("app_state", Value::Null).await;
    ts_check::check_named(&state, "AppState").unwrap();
    assert_eq!(state["profiles"].as_array().unwrap().len(), 1);
    assert_eq!(state["rootDir"], fx._home.path().display().to_string());

    let account = fx.ok("account_add_offline", json!({"name": "Dev"})).await;
    ts_check::check_named(&account, "PublicAccount").unwrap();
    assert_eq!(account["type"], "offline");
    let uuid = account["uuid"].as_str().unwrap().to_string();
    let cfg = fx.ok("account_select", json!({"uuid": uuid})).await;
    ts_check::check_named(&cfg, "ConfigView").unwrap();
    assert_eq!(cfg["selectedAccount"], uuid.as_str());

    let created = fx
        .ok(
            "profile_create",
            json!({"name": "Dev pack", "mcVersion": "1.21.1", "loader": "fabric"}),
        )
        .await;
    ts_check::check_named(&created, "ProfileView").unwrap();
    assert_eq!(created["loader"], "fabric");
    assert_eq!(created["background"], json!({"kind": "auto"}));
    let id = created["id"].as_str().unwrap().to_string();

    let state = fx.ok("app_state", json!({})).await;
    ts_check::check_named(&state, "AppState").unwrap();
    assert!(state["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["id"] == id.as_str()));

    let cfg = fx
        .ok(
            "config_set",
            json!({"patch": {"language": "en", "ram": 4096}}),
        )
        .await;
    ts_check::check_named(&cfg, "ConfigView").unwrap();
    assert_eq!(
        (cfg["language"].as_str(), cfg["ram"].as_u64()),
        (Some("en"), Some(4096))
    );

    // Background styles are tagged unions with optional fields.
    let styled = fx
        .ok(
            "profile_background_set_style",
            json!({"id": id, "dim": 0.5, "blur": 4}),
        )
        .await;
    ts_check::check_named(&styled, "ProfileView").unwrap();
    assert_eq!(
        styled["background"],
        json!({"kind": "auto", "dim": 0.5, "blur": 4.0})
    );
    let walled = fx
        .ok(
            "profile_background_set_kind",
            json!({"id": id, "kind": "wallpaper", "wallpaper": "plains"}),
        )
        .await;
    ts_check::check_named(&walled, "ProfileView").unwrap();
    assert_eq!(walled["background"]["kind"], "wallpaper");

    // Result types of commands that work offline.
    for (cmd, ty, args) in [
        ("storage_usage", "StorageUsage", json!({})),
        ("servers_list", "ServerEntry", json!({})),
        ("content_list", "ContentItem", json!({"profileId": id})),
        ("screenshots_list", "Screenshot", json!({"profileId": id})),
        ("logs_list", "LogEntry", json!({"profileId": id})),
    ] {
        let list = fx.ok(cmd, args).await;
        let items = list
            .as_array()
            .unwrap_or_else(|| panic!("{cmd} is not a list"));
        for item in items {
            ts_check::check_named(item, ty).unwrap_or_else(|e| panic!("{cmd}: {e}"));
        }
    }
    for (cmd, ty, args) in [
        ("stats_summary", "StatsSummary", json!({"days": 7})),
        ("update_status", "UpdateStatus", json!({})),
        ("block_icons_status", "BlockIconsStatus", json!({})),
    ] {
        let v = fx.ok(cmd, args).await;
        ts_check::check_named(&v, ty).unwrap_or_else(|e| panic!("{cmd}: {e}"));
    }
    assert!(fx
        .ok("panorama_for_profile", json!({"id": id}))
        .await
        .is_null());
    assert_eq!(fx.ok("game_running", json!({})).await, json!([]));
    fx.ok("profile_delete", json!({"id": id})).await;
}

#[tokio::test]
async fn errors_are_app_errors_with_stable_codes() {
    let fx = Fx::new().await;

    let (status, body) = fx.call("no_such_command", json!({})).await;
    assert_eq!(status, 404);
    assert_eq!(body["code"], "unknown_command");
    ts_check::check_named(&body, "AppError").unwrap();

    let (status, body) = fx.call("profile_delete", json!({"profileID": "x"})).await;
    assert_eq!(status, 400);
    assert_eq!(body["code"], "bad_args");
    assert!(
        body["message"].as_str().unwrap().contains("profileID"),
        "{body}"
    );

    let (status, body) = fx.call("profile_delete", json!({})).await;
    assert_eq!(status, 400);
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("missing field `id`"),
        "{body}"
    );

    let (status, body) = fx
        .call(
            "profile_create",
            json!({"name": "x", "mcVersion": "1.21", "loader": "lava"}),
        )
        .await;
    assert_eq!(status, 400, "{body}");
    // The Rust side validates the loader at runtime; the TS type is `Loader`.
    assert_eq!(body["code"], "validation");

    // A core validation error keeps its own code.
    let (status, body) = fx.call("config_set", json!({"patch": {"ram": 1}})).await;
    assert_eq!(status, 400);
    assert_eq!(body["code"], "validation");

    let (status, body) = fx
        .call(
            "profile_select",
            json!({"id": "00000000-0000-4000-8000-000000000000"}),
        )
        .await;
    assert_eq!(status, 500, "{body}");
    assert_eq!(body["code"], "profile_not_found");
    ts_check::check_named(&body, "AppError").unwrap();

    // Not JSON at all.
    let res = fx
        .http
        .post(fx.url("/invoke/app_state"))
        .bearer_auth(&fx.server.token)
        .body("{nope")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 400);
    assert_eq!(res.json::<Value>().await.unwrap()["code"], "bad_json");
}

#[tokio::test]
async fn every_command_exists_and_rejects_unknown_fields() {
    let fx = Fx::new().await;
    let listed: Vec<String> = fx
        .http
        .get(fx.url("/commands"))
        .bearer_auth(&fx.server.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(listed.len() > 60);
    for cmd in &listed {
        let (status, body) = fx.call(cmd, json!({"__bogus": 1})).await;
        assert_eq!(
            (status, body["code"].as_str()),
            (400, Some("bad_args")),
            "{cmd}: {body}"
        );
    }
    // The generated CommandMap names exactly these commands.
    let generated = ts_check::command_names();
    let served: BTreeSet<String> = listed.into_iter().collect();
    assert_eq!(generated, served);
}

#[tokio::test]
async fn auth_origin_and_limits() {
    let fx = Fx::new().await;
    let get = |p: &str| fx.http.get(fx.url(p));

    assert_eq!(get("/commands").send().await.unwrap().status(), 401);
    assert_eq!(
        get("/commands")
            .bearer_auth("wrong")
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(get("/nope").send().await.unwrap().status(), 401);
    let no_token = fx
        .http
        .post(fx.url("/invoke/app_state"))
        .send()
        .await
        .unwrap();
    assert_eq!(no_token.status(), 401);
    assert_eq!(
        no_token.json::<Value>().await.unwrap()["code"],
        "unauthorized"
    );
    assert_eq!(
        get(&format!("/commands?token={}", fx.server.token))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        get("/nope")
            .bearer_auth(&fx.server.token)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );

    // CORS: only the explicit origin.
    let ok = get("/commands")
        .bearer_auth(&fx.server.token)
        .header("Origin", ORIGIN)
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), 200);
    assert_eq!(ok.headers()["access-control-allow-origin"], ORIGIN);
    let evil = get("/commands")
        .bearer_auth(&fx.server.token)
        .header("Origin", "http://evil.example")
        .send()
        .await
        .unwrap();
    assert_eq!(evil.status(), 403);
    assert!(evil.headers().get("access-control-allow-origin").is_none());
    let pre = fx
        .http
        .request(reqwest::Method::OPTIONS, fx.url("/invoke/app_state"))
        .header("Origin", ORIGIN)
        .header("Access-Control-Request-Method", "POST")
        .header(
            "Access-Control-Request-Headers",
            "authorization,content-type",
        )
        .send()
        .await
        .unwrap();
    assert_eq!(pre.status(), 204);
    assert_eq!(pre.headers()["access-control-allow-origin"], ORIGIN);
    assert!(pre.headers()["access-control-allow-headers"]
        .to_str()
        .unwrap()
        .contains("authorization"));
    let bad_pre = fx
        .http
        .request(reqwest::Method::OPTIONS, fx.url("/invoke/app_state"))
        .header("Origin", "http://evil.example")
        .send()
        .await
        .unwrap();
    assert_eq!(bad_pre.status(), 403);

    // DNS rebinding: a foreign Host header is refused.
    let rebinding = get("/commands")
        .bearer_auth(&fx.server.token)
        .header("Host", "evil.example")
        .send()
        .await
        .unwrap();
    assert_eq!(rebinding.status(), 403);

    // 1 MB body limit.
    let big = fx
        .http
        .post(fx.url("/invoke/config_set"))
        .bearer_auth(&fx.server.token)
        .body(vec![b' '; 1024 * 1024 + 1])
        .send()
        .await
        .unwrap();
    assert_eq!(big.status(), 413);

    // Bound to loopback only.
    assert!(fx.server.addr.ip().is_loopback());
}

#[tokio::test]
async fn sse_delivers_launch_state_events_of_a_failed_launch() {
    let fx = Fx::new().await;
    let account = fx.ok("account_add_offline", json!({"name": "Dev"})).await;
    fx.ok("account_select", json!({"uuid": account["uuid"]}))
        .await;
    // No Modrinth lookups on launch.
    fx.ok("config_set", json!({"patch": {"performanceMods": false}}))
        .await;
    let profile = fx
        .ok(
            "profile_create",
            json!({"name": "P", "mcVersion": "1.21.1", "loader": "vanilla"}),
        )
        .await;
    let id = profile["id"].as_str().unwrap().to_string();

    let res = fx
        .http
        .get(fx.url(&format!("/events?token={}", fx.server.token)))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(res.headers()["content-type"], "text/event-stream");

    // The launch fails offline (manifest endpoint is dead) but announces its states.
    let (http, url, token) = (
        fx.http.clone(),
        fx.url("/invoke/game_launch"),
        fx.server.token.clone(),
    );
    let launch_id = id.clone();
    let launch = tokio::spawn(async move {
        http.post(url)
            .bearer_auth(token)
            .json(&json!({"profileId": launch_id}))
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()
    });

    let mut stream = res.bytes_stream();
    let mut buf = String::new();
    let mut states: Vec<String> = Vec::new();
    let mut saw_error_log = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    'outer: while tokio::time::Instant::now() < deadline {
        let chunk = tokio::time::timeout(Duration::from_secs(30), stream.next())
            .await
            .expect("SSE stalled")
            .expect("SSE closed")
            .unwrap();
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(end) = buf.find("\n\n") {
            let block: String = buf.drain(..end + 2).collect();
            let Some(data) = block.lines().find_map(|l| l.strip_prefix("data:")) else {
                continue;
            };
            let msg: Value = serde_json::from_str(data.trim()).unwrap();
            assert_eq!(
                keys(&msg),
                BTreeSet::from(["name".to_string(), "payload".to_string()])
            );
            let name = msg["name"].as_str().unwrap();
            match name {
                "launch:state" => {
                    ts_check::check_named(&msg["payload"], "LaunchStateEvent").unwrap();
                    assert_eq!(msg["payload"]["profileId"], id.as_str());
                    let st = msg["payload"]["state"].as_str().unwrap().to_string();
                    let done = st == "stopped" || st == "crashed";
                    states.push(st);
                    if done {
                        break 'outer;
                    }
                }
                "launch:log" => {
                    ts_check::check_named(&msg["payload"], "LaunchLogEvent").unwrap();
                    saw_error_log |= msg["payload"]["level"] == "error";
                }
                _ => {}
            }
        }
    }
    assert_eq!(
        states.first().map(String::as_str),
        Some("preparing"),
        "{states:?}"
    );
    assert_eq!(
        states.last().map(String::as_str),
        Some("stopped"),
        "{states:?}"
    );
    assert!(saw_error_log, "the failure is logged as an error line");
    let outcome = launch.await.unwrap();
    ts_check::check_named(&outcome, "AppError").unwrap();
    assert_ne!(outcome["code"], "unknown_command");
}

#[tokio::test]
async fn images_are_served_through_the_protocol_policy() {
    let fx = Fx::new().await;
    let account = fx.ok("account_add_offline", json!({"name": "Dev"})).await;
    let head = account["headUrl"].as_str().unwrap();
    let path = head.strip_prefix("mangoimg://").unwrap();
    assert!(path.starts_with("head/"));

    let res = fx
        .http
        .get(fx.url(&format!("/img/{path}")))
        .bearer_auth(&fx.server.token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(res.headers()["content-type"], "image/png");
    assert!(res
        .bytes()
        .await
        .unwrap()
        .starts_with(&[0x89, b'P', b'N', b'G']));
    // `<img>` cannot send headers: the token works in the query too.
    let res = fx
        .http
        .get(fx.url(&format!("/img/{path}?token={}", fx.server.token)))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    for bad in [
        "/img/nope/x",
        "/img/cover/..%2F..%2Fconfig",
        "/img/f/AAAA",
        "/img/block/x/top",
    ] {
        let res = fx
            .http
            .get(fx.url(bad))
            .bearer_auth(&fx.server.token)
            .send()
            .await
            .unwrap();
        assert!(res.status().is_client_error(), "{bad} -> {}", res.status());
    }
}

#[test]
fn the_ts_checker_catches_drift() {
    let good = json!({"id": "x", "color": null});
    let ty = ts_check::parse_ty("{ id: string, color: string | null, dim?: number, }");
    assert!(ts_check::check(&good, &ty, "v").is_ok());
    let extra = json!({"id": "x", "color": null, "surprise": 1});
    assert!(ts_check::check(&extra, &ty, "v")
        .unwrap_err()
        .contains("surprise"));
    let missing = json!({"id": "x"});
    assert!(ts_check::check(&missing, &ty, "v")
        .unwrap_err()
        .contains("color"));
    let wrong = json!({"id": 1, "color": null});
    assert!(ts_check::check(&wrong, &ty, "v").is_err());
}
