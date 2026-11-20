#![allow(clippy::unwrap_used)]
use crate::nbt::{self, Entry, Root, Value};
use crate::*;
use std::sync::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn server(name: &str, ip: &str, extra: Vec<Entry>) -> Value {
    let mut c = vec![Entry::string("name", name), Entry::string("ip", ip)];
    c.extend(extra);
    Value::Compound(c)
}

fn list_root(items: Vec<Value>) -> Root {
    empty_with(items)
}

fn empty_with(items: Vec<Value>) -> Root {
    Root {
        name: String::new(),
        entries: vec![Entry::new(
            "servers",
            Value::List {
                element_type: nbt::TAG_COMPOUND,
                items,
            },
        )],
    }
}

fn items_of(root: &Root) -> &Vec<Value> {
    match &nbt::field(&root.entries, "servers").map(|e| &e.value) {
        Some(Value::List { items, .. }) => items,
        _ => panic!("no servers list"),
    }
}

fn sfield(item: &Value, name: &str) -> Option<String> {
    match item {
        Value::Compound(c) => nbt::field(c, name)
            .and_then(|e| e.value.as_str())
            .map(String::from),
        _ => None,
    }
}

fn partner() -> Vec<FeaturedServer> {
    vec![FeaturedServer {
        id: "v".into(),
        name: "VincentVanilla".into(),
        address: "vincentvanilla.net".into(),
    }]
}

#[derive(Default)]
struct FakeIcons {
    icon: Option<String>,
    remembered: Mutex<Vec<(String, String)>>,
}

impl IconStore for FakeIcons {
    fn get(&self, _: &str) -> Option<String> {
        self.icon.clone()
    }
    fn remember(&self, a: &str, i: &str) -> Option<String> {
        self.remembered.lock().unwrap().push((a.into(), i.into()));
        None
    }
}

#[test]
fn nbt_strings_use_java_modified_utf8_bytes() {
    let hex: String = nbt::encode_modified_utf8("A\0🥭")
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(hex, "41c080eda0beedb5ad");
    assert_eq!(
        nbt::decode_modified_utf8(&nbt::encode_modified_utf8("A\0🥭")).unwrap(),
        "A\0🥭"
    );
    assert!(nbt::decode_modified_utf8(&[0xff]).is_err());
}

#[test]
fn nbt_roundtrip_preserves_order_and_list_types() {
    let root = Root {
        name: "r".into(),
        entries: vec![
            Entry::new("z", Value::Byte(1)),
            Entry::new(
                "a",
                Value::List {
                    element_type: nbt::TAG_INT,
                    items: vec![Value::Int(1), Value::Int(2)],
                },
            ),
            Entry::new(
                "empty",
                Value::List {
                    element_type: nbt::TAG_END,
                    items: vec![],
                },
            ),
            Entry::new("l", Value::LongArray(vec![1, -2])),
            Entry::new("f", Value::Float(1.5)),
        ],
    };
    let bytes = nbt::encode_nbt(&root).unwrap();
    assert_eq!(nbt::parse_nbt(&bytes).unwrap(), root);
    assert_eq!(
        nbt::parse_nbt_maybe_gzip(&nbt::encode_nbt_gzip(&root).unwrap()).unwrap(),
        root
    );
    assert!(nbt::parse_nbt(&bytes[..bytes.len() - 1]).is_err());
}

#[test]
fn partner_star_uses_mango_gold_without_colouring_name() {
    assert_eq!(featured_name("VincentVanilla"), "§6★ §rVincentVanilla");
}

#[tokio::test]
async fn partners_are_deduplicated_starred_and_first() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("servers.dat");
    write_server_list(
        &file,
        &list_root(vec![
            server("Other server", "example.org", vec![]),
            server(
                "Old title",
                "vincentvanilla.net:25565",
                vec![
                    Entry::string("icon", "data:image/png;base64,preserved"),
                    Entry::new("acceptTextures", Value::Byte(1)),
                ],
            ),
        ]),
    )
    .await
    .unwrap();
    ensure_featured_servers(dir.path(), &partner(), &NoIcons)
        .await
        .unwrap();
    ensure_featured_servers(dir.path(), &partner(), &NoIcons)
        .await
        .unwrap();
    let root = read_server_list(&file).await.unwrap();
    let items = items_of(&root);
    assert_eq!(items.len(), 2);
    assert_eq!(sfield(&items[0], "name").unwrap(), "§6★ §rVincentVanilla");
    assert_eq!(sfield(&items[0], "ip").unwrap(), "vincentvanilla.net");
    assert_eq!(
        sfield(&items[0], "icon").unwrap(),
        "data:image/png;base64,preserved"
    );
    assert_eq!(sfield(&items[1], "name").unwrap(), "Other server");
}

#[tokio::test]
async fn preserves_modified_utf_names_with_emoji_and_nul() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("servers.dat");
    let name = "Freunde 🥭 \0 privat";
    write_server_list(
        &file,
        &list_root(vec![server(name, "friends.example", vec![])]),
    )
    .await
    .unwrap();
    let root = read_server_list(&file).await.unwrap();
    assert_eq!(sfield(&items_of(&root)[0], "name").unwrap(), name);
}

#[tokio::test]
async fn recovers_from_sidecar_after_primary_corruption() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("servers.dat");
    write_server_list(
        &file,
        &list_root(vec![server("Keep me", "keep.example", vec![])]),
    )
    .await
    .unwrap();
    std::fs::write(&file, b"truncated").unwrap();
    ensure_featured_servers(dir.path(), &partner(), &NoIcons)
        .await
        .unwrap();
    let root = read_server_list(&file).await.unwrap();
    let items = items_of(&root);
    assert_eq!(sfield(&items[0], "name").unwrap(), "§6★ §rVincentVanilla");
    assert_eq!(sfield(&items[1], "name").unwrap(), "Keep me");
}

#[tokio::test]
async fn missing_file_reads_as_empty_and_corrupt_without_sidecar_errors() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("servers.dat");
    assert!(items_of(&read_server_list(&file).await.unwrap()).is_empty());
    std::fs::write(&file, b"junk").unwrap();
    assert!(read_server_list(&file).await.is_err());
}

#[tokio::test]
async fn seeding_never_rewrites_existing_player_list() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("servers.dat");
    let icon = "data:image/png;base64,cHJlc2VydmVk";
    write_server_list(
        &file,
        &list_root(vec![
            server("Friend one", "one.example", vec![]),
            server(
                "VincentVanilla",
                "vincentvanilla.net",
                vec![Entry::string("icon", icon)],
            ),
            server("Friend two", "two.example", vec![]),
        ]),
    )
    .await
    .unwrap();
    let before = std::fs::read(&file).unwrap();
    let store = FakeIcons::default();
    seed_featured_servers(dir.path(), &partner(), &store)
        .await
        .unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), before);
    assert_eq!(
        *store.remembered.lock().unwrap(),
        vec![("vincentvanilla.net".to_string(), icon.to_string())]
    );
}

#[tokio::test]
async fn cached_icon_is_restored_when_new_list_is_bootstrapped() {
    let dir = tempfile::tempdir().unwrap();
    let icon = "data:image/png;base64,Y2FjaGVk";
    let store = FakeIcons {
        icon: Some(icon.into()),
        ..Default::default()
    };
    seed_featured_servers(dir.path(), &partner(), &store)
        .await
        .unwrap();
    let root = read_server_list(&dir.path().join("servers.dat"))
        .await
        .unwrap();
    let first = &items_of(&root)[0];
    assert_eq!(sfield(first, "icon").unwrap(), icon);
    assert_eq!(sfield(first, "name").unwrap(), "§6★ §rVincentVanilla");
}

#[tokio::test]
async fn icon_capture_only_reads_current_list() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("servers.dat");
    let icon = "data:image/png;base64,aWNvbg==";
    write_server_list(
        &file,
        &list_root(vec![server(
            "VincentVanilla",
            "vincentvanilla.net",
            vec![Entry::string("icon", icon)],
        )]),
    )
    .await
    .unwrap();
    let store = FakeIcons::default();
    cache_featured_server_icons(dir.path(), &partner(), &store)
        .await
        .unwrap();
    assert_eq!(
        *store.remembered.lock().unwrap(),
        vec![("vincentvanilla.net".to_string(), icon.to_string())]
    );
}

#[test]
fn first_valid_favicon_is_durable_and_cannot_be_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("server-icons.json");
    let first = "data:image/png;base64,Zmlyc3Q=";
    let later = "data:image/png;base64,bGF0ZXI=";
    let icons = ServerIcons::new(&file);
    assert_eq!(
        icons.remember("VincentVanilla.net:25565", first).as_deref(),
        Some(first)
    );
    assert_eq!(
        icons.remember("vincentvanilla.net", later).as_deref(),
        Some(first)
    );
    assert_eq!(icons.get("VINCENTVANILLA.NET.").as_deref(), Some(first));
    let reloaded = ServerIcons::new(&file);
    assert_eq!(reloaded.get("vincentvanilla.net").as_deref(), Some(first));
}

#[test]
fn icon_validation_is_strict() {
    assert!(valid_icon("data:image/png;base64,Zmlyc3Q="));
    assert!(!valid_icon("data:image/jpeg;base64,Zmlyc3Q="));
    assert!(!valid_icon("data:image/png;base64,"));
    assert!(!valid_icon("data:image/png;base64,abc==="));
    assert!(!valid_icon("data:image/png;base64,ab cd"));
    assert!(!valid_icon(&format!(
        "data:image/png;base64,{}",
        "A".repeat(2 * 1024 * 1024)
    )));
    let dir = tempfile::tempdir().unwrap();
    let icons = ServerIcons::in_root(dir.path());
    assert_eq!(icons.remember("x.example", "not an icon"), None);
    assert!(!dir.path().join("server-icons.json").exists());
}

#[test]
fn motd_flattening_and_color_stripping() {
    let v: serde_json::Value =
        serde_json::from_str(r#"{"text":"§aHello ","extra":[{"text":"§lWorld"},"!"]}"#).unwrap();
    assert_eq!(strip_color_codes(&flatten_motd(&v)), "Hello World!");
}

async fn read_varint(s: &mut tokio::net::TcpStream) -> i32 {
    let mut v = 0u32;
    for shift in (0..35).step_by(7) {
        let b = s.read_u8().await.unwrap();
        v |= u32::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            break;
        }
    }
    v as i32
}

#[tokio::test]
async fn ping_parses_status_from_a_mock_server() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let json = r#"{"version":{"name":"1.21","protocol":767},"players":{"online":3,"max":20},"description":{"text":"§6Hi"},"favicon":"data:image/png;base64,Zg=="}"#;
    let handle = tokio::spawn(async move {
        let (mut s, _) = listener.accept().await.unwrap();
        let len = read_varint(&mut s).await;
        let mut hs = vec![0u8; len as usize];
        s.read_exact(&mut hs).await.unwrap();
        let status_len = read_varint(&mut s).await;
        let mut sr = vec![0u8; status_len as usize];
        s.read_exact(&mut sr).await.unwrap();
        let mut body = Vec::new();
        ping::write_varint(&mut body, 0);
        ping::write_varint(&mut body, json.len() as i32);
        body.extend_from_slice(json.as_bytes());
        let mut out = Vec::new();
        ping::write_varint(&mut out, body.len() as i32);
        out.extend_from_slice(&body);
        s.write_all(&out).await.unwrap();
        hs
    });
    let status = ping_server(&format!("127.0.0.1:{port}"), &PingOptions::default()).await;
    let hs = handle.await.unwrap();
    // protocol 767 = ff 05, then host string, port, next state 1
    assert_eq!(&hs[..3], &[0x00, 0xff, 0x05]);
    assert_eq!(*hs.last().unwrap(), 1);
    assert!(status.online);
    assert_eq!(status.motd, "Hi");
    assert_eq!(status.players, Some(Players { online: 3, max: 20 }));
    assert_eq!(status.version, "1.21");
    assert_eq!(status.protocol, Some(767));
    assert_eq!(
        status.favicon.as_deref(),
        Some("data:image/png;base64,Zg==")
    );
    assert_eq!((status.host.as_str(), status.port), ("127.0.0.1", port));
}

#[tokio::test]
async fn ping_is_offline_on_any_error() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let status = ping_server(&format!("127.0.0.1:{port}"), &PingOptions::default()).await;
    assert!(!status.online);
    assert_eq!(status.port, port);
}
