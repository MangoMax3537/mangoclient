#![allow(clippy::unwrap_used)]
//! Part 9 (first group): worlds, backups, snapshots, automatic snapshots,
//! `pick_save_file` and the new config fields.

use crate::backups::{self, BackupKind, BackupReason, MANIFEST};
use crate::image_protocol::{self, ImageError};
use crate::service::lock;
use crate::tests_p4::Sink;
use crate::worlds::{self, dir_size, parse_level, LevelInfo};
use crate::*;
use mango_core::Paths;
use mango_servers::nbt::{encode_nbt, encode_nbt_gzip, Entry, Root, Value};
use serde_json::json;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

struct Fx {
    tmp: tempfile::TempDir,
    svc: AppService,
    sink: Arc<Sink>,
    pid: String,
}

fn fx_with(host: Option<Host>) -> Fx {
    let tmp = tempfile::tempdir().unwrap();
    let sink = Arc::new(Sink::default());
    let opts = ServiceOptions {
        open_folders: false,
        update_dev: true,
        auto_update_check: false,
        partners: Vec::new(),
        ..ServiceOptions::default()
    };
    let paths = Paths::new(tmp.path());
    let svc = match host {
        Some(h) => AppService::with_host(paths, sink.clone(), opts, h).unwrap(),
        None => AppService::new(paths, sink.clone(), opts).unwrap(),
    };
    let p = svc
        .profile_create("T".into(), "1.21.1".into(), "fabric".into(), None)
        .unwrap();
    Fx {
        tmp,
        svc,
        sink,
        pid: p.id,
    }
}

fn fx() -> Fx {
    fx_with(None)
}

impl Fx {
    fn paths(&self) -> Paths {
        Paths::new(self.tmp.path())
    }
    fn inst(&self) -> PathBuf {
        self.paths().instance_dir(&self.pid).unwrap()
    }
    fn saves(&self) -> PathBuf {
        self.inst().join("saves")
    }
}

fn level_dat(name: Option<&str>, game_type: Option<i32>, hardcore: bool, played: i64) -> Vec<u8> {
    let mut data = Vec::new();
    if let Some(n) = name {
        data.push(Entry::string("LevelName", n));
    }
    if let Some(g) = game_type {
        data.push(Entry::new("GameType", Value::Int(g)));
    }
    data.push(Entry::new("hardcore", Value::Byte(i8::from(hardcore))));
    data.push(Entry::new("LastPlayed", Value::Long(played)));
    data.push(Entry::new(
        "Version",
        Value::Compound(vec![
            Entry::string("Name", "1.21.1"),
            Entry::new("Id", Value::Int(3955)),
        ]),
    ));
    encode_nbt_gzip(&Root {
        name: String::new(),
        entries: vec![Entry::new("Data", Value::Compound(data))],
    })
    .unwrap()
}

fn write(p: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap();
}

/// A world folder with a level.dat, a region file, nested data and an icon.
fn make_world(saves: &Path, folder: &str, level: &str, played: i64) {
    let w = saves.join(folder);
    write(
        &w.join("level.dat"),
        &level_dat(Some(level), Some(1), false, played),
    );
    write(&w.join("region/r.0.0.mca"), &vec![7u8; 5000]);
    write(&w.join("data/raids.dat"), b"abc");
    write(&w.join("icon.png"), b"\x89PNG\r\n\x1a\nfake");
}

/// Relative path -> bytes of every file below `root`.
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let t = e.file_type().unwrap();
            if t.is_dir() {
                walk(&e.path(), root, out);
            } else if t.is_file() {
                let rel = e.path().strip_prefix(root).unwrap().display().to_string();
                out.insert(rel, std::fs::read(e.path()).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zw = zip::ZipWriter::new(&mut buf);
        for (name, data) in entries {
            zw.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zw.write_all(data).unwrap();
        }
        zw.finish().unwrap();
    }
    buf.into_inner()
}

/// Put a hand-made backup (zip + sidecar) into the backup folder.
fn plant(paths: &Paths, pid: &str, id: &str, world: Option<&str>, zip: &[u8]) {
    let dir = paths.backup_dir(pid).unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{id}.zip")), zip).unwrap();
    let kind = if world.is_some() { "world" } else { "snapshot" };
    let side = json!({
        "id": id, "kind": kind, "profileId": pid, "worldId": world,
        "createdAt": 1, "sizeBytes": 0, "label": "planted", "reason": "manual",
    });
    std::fs::write(dir.join(format!("{id}.json")), side.to_string()).unwrap();
}

fn warn_lines(f: &Fx) -> Vec<String> {
    f.sink.warnings()
}

// ---------- level.dat ----------

#[test]
fn level_dat_fields_and_missing_fields() {
    let full = parse_level(&level_dat(
        Some("  My World "),
        Some(2),
        true,
        1_700_000_000_123,
    ))
    .unwrap();
    assert_eq!(full.name.as_deref(), Some("My World"));
    assert_eq!(full.last_played, Some(1_700_000_000_123));
    assert_eq!(full.game_mode, Some(GameMode::Adventure));
    assert!(full.hardcore);
    assert_eq!(full.version.as_deref(), Some("1.21.1"));

    // Every field missing or out of range.
    let sparse = parse_level(&level_dat(None, Some(9), false, -5)).unwrap();
    assert_eq!(sparse.name, None);
    assert_eq!(sparse.game_mode, None);
    assert_eq!(sparse.last_played, None);
    assert!(!sparse.hardcore);

    // Raw (uncompressed) NBT is accepted, an empty Data compound is all-empty.
    let raw = encode_nbt(&Root {
        name: String::new(),
        entries: vec![Entry::new("Data", Value::Compound(vec![]))],
    })
    .unwrap();
    assert_eq!(parse_level(&raw), Some(LevelInfo::default()));
    // No Data at all, garbage, and mistyped fields do not parse / fill in.
    let no_data = encode_nbt(&Root {
        name: String::new(),
        entries: vec![Entry::string("x", "y")],
    })
    .unwrap();
    assert_eq!(parse_level(&no_data), None);
    assert_eq!(parse_level(b"not nbt"), None);
    let mistyped = encode_nbt(&Root {
        name: String::new(),
        entries: vec![Entry::new(
            "Data",
            Value::Compound(vec![
                Entry::new("LevelName", Value::Int(1)),
                Entry::string("GameType", "1"),
                Entry::string("Version", "x"),
            ]),
        )],
    })
    .unwrap();
    assert_eq!(parse_level(&mistyped), Some(LevelInfo::default()));
    for (id, mode) in [
        (0, GameMode::Survival),
        (1, GameMode::Creative),
        (3, GameMode::Spectator),
    ] {
        assert_eq!(
            parse_level(&level_dat(Some("x"), Some(id), false, 1))
                .unwrap()
                .game_mode,
            Some(mode)
        );
    }
}

#[test]
fn folder_name_check_and_segment_encoding() {
    for ok in ["New World", "Welt (2)", "世界", "a.b", "x-y_z"] {
        assert!(worlds::safe_folder_name(ok), "{ok}");
        assert_eq!(
            worlds::decode_segment(&worlds::encode_segment(ok)).as_deref(),
            Some(ok)
        );
    }
    for bad in [
        "", ".", "..", ".hidden", "a/b", "a\\b", "a\0b", "C:x", "a*b", "a?b", "a\nb", "a|b",
    ] {
        assert!(!worlds::safe_folder_name(bad), "{bad:?}");
    }
    assert!(!worlds::safe_folder_name(&"x".repeat(256)));
    assert_eq!(worlds::decode_segment("%zz"), None);
    assert_eq!(worlds::decode_segment("%2"), None);
    assert_eq!(worlds::decode_segment("%FF"), None);
}

// ---------- listing ----------

#[tokio::test]
async fn worlds_are_listed_with_metadata_newest_first() {
    let f = fx();
    let saves = f.saves();
    make_world(&saves, "old", "Old One", 1_000);
    make_world(&saves, "new world", "Fresh", 9_000);
    // Damaged level.dat: still a world, named after its folder.
    write(&saves.join("broken/level.dat"), b"garbage");
    // Not worlds.
    write(&saves.join("plain/readme.txt"), b"x");
    write(&saves.join(".mango-restore-1/level.dat"), b"x");
    write(&saves.join("file.txt"), b"x");

    let list = f.svc.worlds_list(f.pid.clone()).await.unwrap();
    let ids: Vec<_> = list.iter().map(|w| w.id.as_str()).collect();
    assert_eq!(ids, ["new world", "old", "broken"]);
    let fresh = &list[0];
    assert_eq!(fresh.name, "Fresh");
    assert_eq!(fresh.last_played, Some(9_000));
    assert_eq!(fresh.game_mode, Some(GameMode::Creative));
    assert_eq!(fresh.version.as_deref(), Some("1.21.1"));
    assert!(!fresh.hardcore);
    assert!(fresh.size_bytes >= 5000 + 3);
    assert_eq!(
        fresh.icon_url.as_deref(),
        Some(format!("mangoimg://world/{}/new%20world", f.pid).as_str())
    );
    let broken = &list[2];
    assert_eq!(broken.name, "broken");
    assert_eq!(broken.last_played, None);
    assert_eq!(broken.icon_url, None);
    let json = serde_json::to_value(fresh).unwrap();
    assert_eq!(json["gameMode"], "creative");
    assert_eq!(json["lastPlayed"], 9000);

    // No saves folder: empty, and unknown profiles still fail.
    let other = f
        .svc
        .profile_create("O".into(), "1.21.1".into(), "vanilla".into(), None)
        .unwrap();
    assert!(f.svc.worlds_list(other.id).await.unwrap().is_empty());
    assert_eq!(
        f.svc
            .worlds_list("12345678-1234-1234-1234-123456789abc".into())
            .await
            .unwrap_err()
            .code,
        "profile_not_found"
    );
    assert_eq!(
        f.svc.worlds_list("nope".into()).await.unwrap_err().code,
        "invalid_profile_id"
    );
}

#[test]
fn open_folder_returns_and_creates_saves() {
    let f = fx();
    let p = f.svc.world_open_folder(f.pid.clone()).unwrap();
    assert_eq!(PathBuf::from(&p), f.saves());
    assert!(f.saves().is_dir());
}

#[test]
fn size_walk_counts_files_skips_symlinks_and_is_capped() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("w");
    write(&root.join("a.bin"), &[0u8; 100]);
    write(&root.join("d/e/b.bin"), &[0u8; 23]);
    let outside = tmp.path().join("outside");
    write(&outside.join("big.bin"), &[0u8; 10_000]);
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
        std::os::unix::fs::symlink(outside.join("big.bin"), root.join("big-link")).unwrap();
    }
    let w = dir_size(&root, 1000, Duration::from_secs(5));
    assert_eq!((w.bytes, w.files, w.truncated), (123, 2, false));
    // Entry cap: a partial lower bound, flagged.
    let capped = dir_size(&root, 2, Duration::from_secs(5));
    assert!(capped.truncated && capped.bytes <= 123);
    // Missing folder: zero, no panic.
    assert_eq!(
        dir_size(&tmp.path().join("nope"), 10, Duration::from_secs(1)).bytes,
        0
    );
}

// ---------- world backups ----------

#[tokio::test]
async fn world_backup_delete_restore_round_trip_is_identical() {
    let f = fx();
    make_world(&f.saves(), "Alpha", "Alpha", 5);
    let before = tree(&f.saves().join("Alpha"));

    let b = f
        .svc
        .world_backup(f.pid.clone(), "Alpha".into(), Some("My: backup/1".into()))
        .await
        .unwrap();
    assert_eq!(b.kind, BackupKind::World);
    assert_eq!(b.reason, BackupReason::Manual);
    assert_eq!(b.world_id.as_deref(), Some("Alpha"));
    assert_eq!(b.label, "My: backup/1");
    assert!(b.size_bytes > 0);
    assert!(b.id.ends_with("-world-My-backup-1"), "{}", b.id);
    assert!(b.id.starts_with(&b.created_at.to_string()));
    let dir = f.paths().backup_dir(&f.pid).unwrap();
    assert!(dir.join(format!("{}.zip", b.id)).is_file());
    assert!(dir.join(format!("{}.json", b.id)).is_file());
    // Stored outside the instance.
    assert!(!f.inst().join("backups").exists());
    assert!(dir.starts_with(f.tmp.path().join("backups")));

    // Delete with a backup of its own, then restore the manual one.
    let auto = f
        .svc
        .world_delete(f.pid.clone(), "Alpha".into(), true)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(auto.reason, BackupReason::BeforeDelete);
    assert!(!f.saves().join("Alpha").exists());

    let list = f
        .svc
        .backup_restore(f.pid.clone(), b.id.clone(), None)
        .await
        .unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(tree(&f.saves().join("Alpha")), before);
    // The world is back under its own name, the second restore gets a suffix.
    f.svc
        .backup_restore(f.pid.clone(), auto.id.clone(), None)
        .await
        .unwrap();
    assert_eq!(tree(&f.saves().join("Alpha (restored)")), before);
    // Staging folders are gone.
    let names: Vec<String> = std::fs::read_dir(f.saves())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().into_string().unwrap())
        .collect();
    assert!(names.iter().all(|n| !n.starts_with('.')), "{names:?}");
}

#[tokio::test]
async fn restore_never_overwrites_and_numbers_new_folders() {
    let f = fx();
    make_world(&f.saves(), "W", "W", 5);
    let b = f
        .svc
        .world_backup(f.pid.clone(), "W".into(), None)
        .await
        .unwrap();
    assert_eq!(b.label, "W");
    // Change the live world; it must survive every restore untouched.
    write(&f.saves().join("W/level.dat"), b"changed");
    for expect in ["W (restored)", "W (restored 2)", "W (restored 3)"] {
        f.svc
            .backup_restore(f.pid.clone(), b.id.clone(), None)
            .await
            .unwrap();
        assert!(
            f.saves().join(expect).join("level.dat").is_file(),
            "{expect}"
        );
    }
    assert_eq!(
        std::fs::read(f.saves().join("W/level.dat")).unwrap(),
        b"changed"
    );
    // asNew is a new name even when the original is gone.
    std::fs::remove_dir_all(f.saves().join("W")).unwrap();
    f.svc
        .backup_restore(f.pid.clone(), b.id.clone(), Some(true))
        .await
        .unwrap();
    assert!(f.saves().join("W (restored 4)").is_dir());
    assert!(!f.saves().join("W").exists());
    // Without asNew and a free name it comes back under the original name.
    f.svc
        .backup_restore(f.pid.clone(), b.id.clone(), Some(false))
        .await
        .unwrap();
    assert!(f.saves().join("W/region/r.0.0.mca").is_file());
}

#[tokio::test]
async fn destructive_actions_are_refused_while_the_game_runs() {
    let f = fx();
    make_world(&f.saves(), "W", "W", 5);
    let b = f
        .svc
        .world_backup(f.pid.clone(), "W".into(), None)
        .await
        .unwrap();
    let snap = f.svc.snapshot_create(f.pid.clone(), None).await.unwrap();
    // A launch in flight counts as running (same set the launcher uses).
    lock(&f.svc.inner.launching).insert(f.pid.clone());

    for r in [
        f.svc
            .world_delete(f.pid.clone(), "W".into(), true)
            .await
            .err(),
        f.svc
            .world_delete(f.pid.clone(), "W".into(), false)
            .await
            .err(),
        f.svc
            .backup_restore(f.pid.clone(), b.id.clone(), None)
            .await
            .err(),
        f.svc
            .backup_restore(f.pid.clone(), snap.id.clone(), None)
            .await
            .err(),
    ] {
        assert_eq!(r.unwrap().code, "game_running");
    }
    // Nothing happened: world there, no extra backup, no extra folder.
    assert!(f.saves().join("W/level.dat").is_file());
    assert_eq!(f.svc.backups_list(f.pid.clone()).await.unwrap().len(), 2);
    assert_eq!(std::fs::read_dir(f.saves()).unwrap().count(), 1);
    // Copies are fine while it runs.
    f.svc
        .world_backup(f.pid.clone(), "W".into(), None)
        .await
        .unwrap();
    f.svc
        .backup_delete(f.pid.clone(), b.id.clone())
        .await
        .unwrap();

    lock(&f.svc.inner.launching).remove(&f.pid);
    // The operations themselves hold the profile only while they run.
    f.svc
        .world_delete(f.pid.clone(), "W".into(), false)
        .await
        .unwrap();
    assert!(lock(&f.svc.inner.launching).is_empty());
}

#[tokio::test]
async fn delete_waits_for_the_backup_and_keeps_the_world_when_it_fails() {
    let f = fx();
    make_world(&f.saves(), "W", "W", 5);
    // `backups` is a file: no backup can be written.
    std::fs::write(f.tmp.path().join("backups"), b"in the way").unwrap();
    assert!(f
        .svc
        .world_delete(f.pid.clone(), "W".into(), true)
        .await
        .is_err());
    assert!(f.saves().join("W/level.dat").is_file());
    // Without a backup the delete goes through and answers null.
    assert_eq!(
        f.svc
            .world_delete(f.pid.clone(), "W".into(), false)
            .await
            .unwrap(),
        None
    );
    assert!(!f.saves().join("W").exists());
    assert_eq!(
        f.svc
            .world_delete(f.pid.clone(), "W".into(), false)
            .await
            .unwrap_err()
            .code,
        "world_not_found"
    );
    for bad in ["../x", "a/b", "", ".."] {
        let e = f
            .svc
            .world_delete(f.pid.clone(), bad.into(), false)
            .await
            .unwrap_err();
        assert_eq!(e.code, "invalid_world", "{bad:?}");
        assert_eq!(
            f.svc
                .world_backup(f.pid.clone(), bad.into(), None)
                .await
                .unwrap_err()
                .code,
            "invalid_world"
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn symlinks_inside_a_world_are_not_followed() {
    let f = fx();
    make_world(&f.saves(), "W", "W", 5);
    let secret = f.tmp.path().join("secret");
    write(&secret.join("passwd"), b"top secret");
    std::os::unix::fs::symlink(&secret, f.saves().join("W/linked-dir")).unwrap();
    std::os::unix::fs::symlink(secret.join("passwd"), f.saves().join("W/linked-file")).unwrap();

    let b = f
        .svc
        .world_backup(f.pid.clone(), "W".into(), None)
        .await
        .unwrap();
    let zip_file = f
        .paths()
        .backup_dir(&f.pid)
        .unwrap()
        .join(format!("{}.zip", b.id));
    let mut zip = zip::ZipArchive::new(std::fs::File::open(zip_file).unwrap()).unwrap();
    let names: Vec<String> = (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().to_string())
        .collect();
    assert!(names.contains(&"W/level.dat".to_string()));
    assert!(
        names
            .iter()
            .all(|n| !n.contains("linked") && !n.contains("passwd")),
        "{names:?}"
    );
    // The world's size ignores the links too.
    let w = &f.svc.worlds_list(f.pid.clone()).await.unwrap()[0];
    assert!(w.size_bytes < 6000);
    // A world folder that is itself a link is neither listed nor backed up.
    std::os::unix::fs::symlink(f.saves().join("W"), f.saves().join("L")).unwrap();
    let ids: Vec<String> = f
        .svc
        .worlds_list(f.pid.clone())
        .await
        .unwrap()
        .into_iter()
        .map(|w| w.id)
        .collect();
    assert_eq!(ids, ["W"]);
    assert_eq!(
        f.svc
            .world_backup(f.pid.clone(), "L".into(), None)
            .await
            .unwrap_err()
            .code,
        "world_not_found"
    );
    assert_eq!(
        f.svc
            .world_delete(f.pid.clone(), "L".into(), false)
            .await
            .unwrap_err()
            .code,
        "world_not_found"
    );
}

// ---------- listing, deleting, corrupt entries ----------

#[tokio::test]
async fn listing_ignores_corrupt_entries_and_sorts_newest_first() {
    let f = fx();
    make_world(&f.saves(), "W", "W", 5);
    let a = f
        .svc
        .world_backup(f.pid.clone(), "W".into(), Some("a".into()))
        .await
        .unwrap();
    let b = f
        .svc
        .snapshot_create(f.pid.clone(), Some("b".into()))
        .await
        .unwrap();
    let dir = f.paths().backup_dir(&f.pid).unwrap();
    let zip = zip_of(&[("W/level.dat", b"x")]);

    // Garbage sidecar, sidecar without zip, zip without sidecar, id/profile
    // mismatches, a half-written .part, a wrong kind.
    plant(&f.paths(), &f.pid, "5-world-ok", Some("W"), &zip);
    std::fs::write(dir.join("6-world-garbage.json"), b"{nope").unwrap();
    std::fs::write(dir.join("6-world-garbage.zip"), &zip).unwrap();
    std::fs::write(
        dir.join("7-world-nozip.json"),
        json!({"id":"7-world-nozip","kind":"world","profileId":f.pid,"worldId":"W",
               "createdAt":7,"sizeBytes":1,"label":"x","reason":"manual"})
        .to_string(),
    )
    .unwrap();
    std::fs::write(dir.join("8-world-nosidecar.zip"), &zip).unwrap();
    std::fs::write(
        dir.join("9-world-liar.json"),
        json!({"id":"1-world-other","kind":"world","profileId":f.pid,"worldId":"W",
               "createdAt":9,"sizeBytes":1,"label":"x","reason":"manual"})
        .to_string(),
    )
    .unwrap();
    std::fs::write(dir.join("9-world-liar.zip"), &zip).unwrap();
    plant(&f.paths(), &f.pid, "10-world-evil", Some("../x"), &zip);
    plant(&f.paths(), &f.pid, "11-snapshot-kind", Some("W"), &zip);
    std::fs::write(dir.join("12-world-part.zip.part"), b"half").unwrap();
    std::fs::write(dir.join("notes.json"), b"{}").unwrap();

    let list = f.svc.backups_list(f.pid.clone()).await.unwrap();
    let ids: Vec<&str> = list.iter().map(|b| b.id.as_str()).collect();
    assert_eq!(ids.len(), 3, "{ids:?}");
    assert!(ids.contains(&a.id.as_str()) && ids.contains(&b.id.as_str()));
    assert_eq!(*ids.last().unwrap(), "5-world-ok");
    // Newest first.
    assert!(list.windows(2).all(|w| w[0].created_at >= w[1].created_at));
    // The real size comes from the zip, not the sidecar.
    assert_eq!(list.last().unwrap().size_bytes, zip.len() as u64);

    // No backup folder at all is an empty list.
    let other = f
        .svc
        .profile_create("O".into(), "1.21.1".into(), "vanilla".into(), None)
        .unwrap();
    assert!(f.svc.backups_list(other.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn backup_delete_validates_ids() {
    let f = fx();
    make_world(&f.saves(), "W", "W", 5);
    let b = f
        .svc
        .world_backup(f.pid.clone(), "W".into(), None)
        .await
        .unwrap();
    let outside = f.tmp.path().join("backups/precious.zip");
    write(&outside, b"keep");
    for bad in [
        "../precious",
        "..",
        "",
        "x",
        "1-world-a/../../precious",
        "9-world-none",
    ] {
        assert_eq!(
            f.svc
                .backup_delete(f.pid.clone(), bad.into())
                .await
                .unwrap_err()
                .code,
            "backup_not_found",
            "{bad:?}"
        );
        assert_eq!(
            f.svc
                .backup_restore(f.pid.clone(), bad.into(), None)
                .await
                .unwrap_err()
                .code,
            "backup_not_found"
        );
    }
    assert!(outside.is_file());
    let left = f
        .svc
        .backup_delete(f.pid.clone(), b.id.clone())
        .await
        .unwrap();
    assert!(left.is_empty());
    let dir = f.paths().backup_dir(&f.pid).unwrap();
    assert!(!dir.join(format!("{}.zip", b.id)).exists());
    assert!(!dir.join(format!("{}.json", b.id)).exists());
}

// ---------- hostile archives ----------

#[tokio::test]
async fn restoring_archives_with_bad_entries_is_refused_and_leaves_nothing() {
    let f = fx();
    let evil_target = f.tmp.path().join("evil.txt");
    type Case<'a> = (&'a str, Vec<(&'a str, &'a [u8])>);
    let cases: Vec<Case> = vec![
        (
            "1-world-dotdot",
            vec![("W/level.dat", b"x"), ("../../evil.txt", b"pwn")],
        ),
        (
            "2-world-inner",
            vec![("W/level.dat", b"x"), ("W/../../../evil.txt", b"pwn")],
        ),
        (
            "3-world-abs",
            vec![("W/level.dat", b"x"), ("/tmp/evil.txt", b"pwn")],
        ),
        (
            "4-world-back",
            vec![("W/level.dat", b"x"), ("W\\..\\evil.txt", b"pwn")],
        ),
        (
            "5-world-other",
            vec![("W/level.dat", b"x"), ("Other/level.dat", b"y")],
        ),
        ("6-world-dot", vec![("W/./level.dat", b"x")]),
        ("7-world-empty", vec![("W//level.dat", b"x")]),
    ];
    for (id, entries) in &cases {
        plant(&f.paths(), &f.pid, id, Some("W"), &zip_of(entries));
        let e = f
            .svc
            .backup_restore(f.pid.clone(), (*id).into(), None)
            .await
            .unwrap_err();
        assert_eq!(e.code, "backup_corrupt", "{id}");
        assert!(!evil_target.exists());
        // No world, no staging folder.
        assert!(
            !f.saves().exists() || std::fs::read_dir(f.saves()).unwrap().count() == 0,
            "{id}"
        );
    }
    // A snapshot may only hold its four folders and the manifest.
    let manifest = json!({
        "version": 1, "profileId": f.pid, "name": "x", "mcVersion": "1.21.1",
        "loader": "fabric", "mods": []
    })
    .to_string();
    plant(
        &f.paths(),
        &f.pid,
        "8-snapshot-bad",
        None,
        &zip_of(&[
            (MANIFEST, manifest.as_bytes()),
            ("mods/a.jar", b"a"),
            ("saves/W/level.dat", b"x"),
        ]),
    );
    let e = f
        .svc
        .backup_restore(f.pid.clone(), "8-snapshot-bad".into(), None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "backup_corrupt");
    assert!(!f.inst().join("saves").exists());
    plant(
        &f.paths(),
        &f.pid,
        "9-snapshot-nomanifest",
        None,
        &zip_of(&[("mods/a.jar", b"a")]),
    );
    assert_eq!(
        f.svc
            .backup_restore(f.pid.clone(), "9-snapshot-nomanifest".into(), None)
            .await
            .unwrap_err()
            .code,
        "backup_corrupt"
    );
}

// ---------- snapshots ----------

fn local_record(f: &Fx, filename: &str) -> serde_json::Value {
    json!({
        "projectId": format!("local:{filename}"), "title": filename, "filename": filename,
        "file": f.inst().join("mods").join(filename).display().to_string(),
        "type": "mod", "enabled": true, "local": true,
    })
}

#[tokio::test]
async fn snapshot_restore_puts_back_folders_and_the_mod_list() {
    let f = fx();
    let inst = f.inst();
    write(&inst.join("mods/a.jar"), b"jar-a");
    write(&inst.join("mods/b.jar"), b"jar-b");
    write(&inst.join("config/x.toml"), b"x = 1");
    write(&inst.join("resourcepacks/p.zip"), b"pack");
    write(&inst.join("saves/W/level.dat"), b"not part of a snapshot");
    f.svc
        .inner
        .with_store(|s| {
            s.update_profile(
                &f.pid,
                &json!({"mods": [local_record(&f, "a.jar"), local_record(&f, "b.jar")]}),
            )
            .map(|_| ())
        })
        .unwrap();
    let before = (
        tree(&inst.join("mods")),
        tree(&inst.join("config")),
        tree(&inst.join("resourcepacks")),
    );

    let snap = f
        .svc
        .snapshot_create(f.pid.clone(), Some("Before experiments".into()))
        .await
        .unwrap();
    assert_eq!(snap.kind, BackupKind::Snapshot);
    assert_eq!(snap.reason, BackupReason::Manual);
    assert_eq!(snap.world_id, None);
    assert_eq!(snap.label, "Before experiments");
    // The zip holds the folders and the profile record, but not the saves.
    let zip_file = f
        .paths()
        .backup_dir(&f.pid)
        .unwrap()
        .join(format!("{}.zip", snap.id));
    let mut zip = zip::ZipArchive::new(std::fs::File::open(zip_file).unwrap()).unwrap();
    let names: Vec<String> = (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().to_string())
        .collect();
    assert!(names.contains(&MANIFEST.to_string()));
    assert!(names.contains(&"mods/a.jar".to_string()));
    assert!(names
        .iter()
        .all(|n| !n.starts_with("saves") && !n.starts_with("shaderpacks")));

    // Wreck the instance: new mod, deleted mod, new folder, changed config.
    std::fs::remove_file(inst.join("mods/a.jar")).unwrap();
    write(&inst.join("mods/new.jar"), b"new");
    write(&inst.join("config/x.toml"), b"x = 2");
    write(&inst.join("shaderpacks/s.zip"), b"shader");
    std::fs::remove_dir_all(inst.join("resourcepacks")).unwrap();
    f.svc
        .inner
        .with_store(|s| s.update_profile(&f.pid, &json!({"mods": []})).map(|_| ()))
        .unwrap();

    let list = f
        .svc
        .backup_restore(f.pid.clone(), snap.id.clone(), None)
        .await
        .unwrap();
    assert_eq!(
        (
            tree(&inst.join("mods")),
            tree(&inst.join("config")),
            tree(&inst.join("resourcepacks")),
        ),
        before
    );
    // A folder that did not exist in the snapshot is gone again; saves stay.
    assert!(!inst.join("shaderpacks").exists());
    assert!(inst.join("saves/W/level.dat").is_file());
    // No staging leftovers.
    let hidden: Vec<_> = std::fs::read_dir(&inst)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with('.'))
        .collect();
    assert!(hidden.is_empty());
    // The mod list is back (through the store) and the UI was told.
    let p = f.svc.app_state().unwrap();
    let p = p.profiles.iter().find(|p| p.id == f.pid).unwrap();
    assert_eq!(p.mod_count, 2);
    let stored = f.svc.profile(&f.pid).unwrap().mods;
    assert_eq!(stored.len(), 2);
    assert_eq!(
        stored[0].file.as_deref(),
        Some(
            inst.join("mods")
                .join("a.jar")
                .display()
                .to_string()
                .as_str()
        )
    );
    assert!(f
        .sink
        .events()
        .iter()
        .any(|e| matches!(e, Event::ContentChanged(c) if c.profile_id == f.pid)));
    // The state before the restore was snapshotted automatically.
    assert_eq!(list.len(), 2);
    let pre = list.iter().find(|b| b.id != snap.id).unwrap();
    assert_eq!(pre.reason, BackupReason::Auto);
    assert_eq!(pre.kind, BackupKind::Snapshot);
    // ... so the restore can itself be undone.
    f.svc
        .backup_restore(f.pid.clone(), pre.id.clone(), None)
        .await
        .unwrap();
    assert!(inst.join("mods/new.jar").is_file() && !inst.join("mods/a.jar").exists());
    assert!(inst.join("shaderpacks/s.zip").is_file());
    assert!(f.svc.profile(&f.pid).unwrap().mods.is_empty());
}

#[tokio::test]
async fn failed_pre_restore_snapshot_aborts_the_restore() {
    let f = fx();
    write(&f.inst().join("mods/a.jar"), b"a");
    let snap = f.svc.snapshot_create(f.pid.clone(), None).await.unwrap();
    write(&f.inst().join("mods/b.jar"), b"b");
    // Corrupt the archive: restoring must not touch the instance.
    let zip_file = f
        .paths()
        .backup_dir(&f.pid)
        .unwrap()
        .join(format!("{}.zip", snap.id));
    std::fs::write(&zip_file, b"not a zip").unwrap();
    let e = f
        .svc
        .backup_restore(f.pid.clone(), snap.id.clone(), None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "backup_corrupt");
    assert!(f.inst().join("mods/b.jar").is_file());
}

// ---------- pruning ----------

#[tokio::test]
async fn pruning_keeps_the_newest_automatic_backups_and_all_manual_ones() {
    let f = fx();
    write(&f.inst().join("mods/a.jar"), b"a");
    assert_eq!(
        f.svc
            .config_set(json!({"backupKeep": 2}))
            .unwrap()
            .backup_keep,
        2
    );
    let manual = f
        .svc
        .snapshot_create(f.pid.clone(), Some("keep me".into()))
        .await
        .unwrap();
    make_world(&f.saves(), "W", "W", 5);
    let world_auto = f
        .svc
        .world_delete(f.pid.clone(), "W".into(), true)
        .await
        .unwrap()
        .unwrap();

    let mut made = Vec::new();
    for i in 0..5 {
        let b = f
            .svc
            .auto_snapshot(&f.pid, BackupReason::BeforeUpdate, format!("auto {i}"))
            .await
            .unwrap();
        made.push(b.id);
    }
    let list = f.svc.backups_list(f.pid.clone()).await.unwrap();
    let ids: Vec<&str> = list.iter().map(|b| b.id.as_str()).collect();
    // The manual one, the world backup (its own bucket) and the 2 newest.
    assert!(ids.contains(&manual.id.as_str()));
    assert!(ids.contains(&world_auto.id.as_str()));
    assert!(ids.contains(&made[4].as_str()) && ids.contains(&made[3].as_str()));
    assert!(!ids.contains(&made[2].as_str()));
    assert_eq!(list.len(), 4);
    // The zip and sidecar of pruned ones are gone from disk.
    let dir = f.paths().backup_dir(&f.pid).unwrap();
    assert!(!dir.join(format!("{}.zip", made[0])).exists());
    assert!(!dir.join(format!("{}.json", made[0])).exists());
    // Direct call: protect spares one id.
    backups::prune_automatic(&f.paths(), &f.pid, 1, Some(&made[3]));
    let ids: Vec<String> = f
        .svc
        .backups_list(f.pid.clone())
        .await
        .unwrap()
        .into_iter()
        .map(|b| b.id)
        .collect();
    assert!(ids.contains(&made[3]) && ids.contains(&made[4]) && ids.contains(&manual.id));
}

// ---------- automatic snapshots ----------

async fn local_fixture(f: &Fx) {
    write(&f.inst().join("mods/foo.jar"), b"foo");
    write(&f.inst().join("config/foo.toml"), b"c");
    f.svc.content_list(f.pid.clone()).await.unwrap();
}

#[tokio::test]
async fn uninstall_takes_a_snapshot_first() {
    let f = fx();
    local_fixture(&f).await;
    let r = f
        .svc
        .content_uninstall(f.pid.clone(), "local:foo.jar".into())
        .await
        .unwrap();
    assert!(!r.blocked && !f.inst().join("mods/foo.jar").exists());
    let list = f.svc.backups_list(f.pid.clone()).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].reason, BackupReason::BeforeDelete);
    assert_eq!(list[0].kind, BackupKind::Snapshot);
    assert!(
        list[0].label.starts_with("Before removing"),
        "{}",
        list[0].label
    );
    assert!(warn_lines(&f).is_empty());
    // It holds the jar that was just removed: restoring brings it back.
    f.svc
        .backup_restore(f.pid.clone(), list[0].id.clone(), None)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(f.inst().join("mods/foo.jar")).unwrap(),
        b"foo"
    );
    assert_eq!(f.svc.profile(&f.pid).unwrap().mods.len(), 1);
}

#[tokio::test]
async fn switching_auto_snapshots_off_takes_none() {
    let f = fx();
    assert!(f.svc.app_state().unwrap().config.auto_snapshot);
    assert_eq!(f.svc.app_state().unwrap().config.backup_keep, 5);
    f.svc.config_set(json!({"autoSnapshot": false})).unwrap();
    local_fixture(&f).await;
    f.svc
        .content_uninstall(f.pid.clone(), "local:foo.jar".into())
        .await
        .unwrap();
    assert!(f.svc.backups_list(f.pid.clone()).await.unwrap().is_empty());
    assert!(!f.paths().backups().exists());
}

#[tokio::test]
async fn a_failing_snapshot_warns_but_does_not_block_the_action() {
    let f = fx();
    local_fixture(&f).await;
    std::fs::write(f.tmp.path().join("backups"), b"in the way").unwrap();
    let r = f
        .svc
        .content_uninstall(f.pid.clone(), "local:foo.jar".into())
        .await
        .unwrap();
    assert!(!r.blocked);
    assert!(!f.inst().join("mods/foo.jar").exists());
    let w = warn_lines(&f);
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("automatic snapshot"), "{w:?}");
    // Directly: None, and a warning each time.
    assert!(f
        .svc
        .auto_snapshot(&f.pid, BackupReason::BeforeUpdate, "x".into())
        .await
        .is_none());
}

#[tokio::test]
async fn an_empty_instance_gets_no_automatic_snapshot() {
    let f = fx();
    assert!(f
        .svc
        .auto_snapshot(&f.pid, BackupReason::BeforeUpdate, "x".into())
        .await
        .is_none());
    assert!(warn_lines(&f).is_empty());
    assert!(f.svc.backups_list(f.pid.clone()).await.unwrap().is_empty());
}

// ---------- config ----------

#[test]
fn config_fields_validate_and_persist() {
    let f = fx();
    let c = f.svc.app_state().unwrap().config;
    assert!(c.auto_snapshot);
    assert_eq!(c.backup_keep, 5);
    for bad in [
        json!(0),
        json!(51),
        json!(-1),
        json!("5"),
        json!(2.5),
        json!(null),
    ] {
        assert_eq!(
            f.svc
                .config_set(json!({"backupKeep": bad}))
                .unwrap_err()
                .code,
            "validation",
            "{bad}"
        );
    }
    for good in [1, 50, 7] {
        assert_eq!(
            f.svc
                .config_set(json!({"backupKeep": good}))
                .unwrap()
                .backup_keep,
            good
        );
    }
    assert!(f.svc.config_set(json!({"autoSnapshot": "yes"})).is_err());
    let c = f
        .svc
        .config_set(json!({"autoSnapshot": false, "backupKeep": 9}))
        .unwrap();
    assert!(!c.auto_snapshot);
    // Survives a reopen; old config files without the fields get defaults.
    let again = AppService::new(
        f.paths(),
        Arc::new(Sink::default()),
        ServiceOptions {
            open_folders: false,
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    let c = again.app_state().unwrap().config;
    assert!(!c.auto_snapshot);
    assert_eq!(c.backup_keep, 9);
    let json = serde_json::to_value(&c).unwrap();
    assert_eq!(json["autoSnapshot"], false);
    assert_eq!(json["backupKeep"], 9);
    let old = mango_core::Config::from_value(&json!({"ram": 4096}), 16_384);
    assert!(old.auto_snapshot);
    assert_eq!(old.backup_keep, 5);
    let clamped = mango_core::Config::from_value(&json!({"backupKeep": 500}), 16_384);
    assert_eq!(clamped.backup_keep, 5);
}

// ---------- pick_save_file ----------

#[tokio::test]
async fn pick_save_file_uses_the_dialog_and_cleans_the_name() {
    let tmp = tempfile::tempdir().unwrap();
    let chosen = tmp.path().join("out.zip");
    let dialog = Arc::new(CannedFileDialog::new(None, None).with_save(Some(chosen.clone())));
    let host = Host::system(false).with_dialog(dialog.clone());
    let f = fx_with(Some(host));
    let got = f
        .svc
        .pick_save_file(Some("Export".into()), "../../etc/My Pack.mrpack".into())
        .await
        .unwrap();
    assert_eq!(got, Some(chosen.display().to_string()));
    f.svc.pick_save_file(None, "   ".into()).await.unwrap();
    f.svc.pick_save_file(None, "a\nb.zip".into()).await.unwrap();
    assert_eq!(dialog.save_names(), ["My Pack.mrpack", "export", "ab.zip"]);
    assert_eq!(dialog.calls()[0], ("save", Some("Export".into())));
    // Cancelled and the default (no dialog) both answer null.
    let none = fx();
    assert_eq!(
        none.svc.pick_save_file(None, "x.zip".into()).await.unwrap(),
        None
    );
    let v = crate::dispatch::invoke(
        &f.svc,
        "pick_save_file",
        json!({"title": null, "defaultName": "x.zip"}),
    )
    .await
    .unwrap();
    assert_eq!(v, json!(chosen.display().to_string()));
}

// ---------- the command table ----------

#[tokio::test]
async fn commands_are_reachable_through_dispatch() {
    let f = fx();
    make_world(&f.saves(), "W", "W", 5);
    use crate::dispatch::invoke;
    let w = invoke(&f.svc, "worlds_list", json!({"profileId": f.pid}))
        .await
        .unwrap();
    assert_eq!(w[0]["id"], "W");
    let b = invoke(
        &f.svc,
        "world_backup",
        json!({"profileId": f.pid, "worldId": "W", "label": null}),
    )
    .await
    .unwrap();
    assert_eq!(b["kind"], "world");
    assert_eq!(b["reason"], "manual");
    assert_eq!(b["worldId"], "W");
    let s = invoke(&f.svc, "snapshot_create", json!({"profileId": f.pid}))
        .await
        .unwrap();
    assert_eq!(s["kind"], "snapshot");
    assert!(s["worldId"].is_null());
    let l = invoke(&f.svc, "backups_list", json!({"profileId": f.pid}))
        .await
        .unwrap();
    assert_eq!(l.as_array().unwrap().len(), 2);
    let d = invoke(
        &f.svc,
        "world_delete",
        json!({"profileId": f.pid, "worldId": "W", "makeBackup": false}),
    )
    .await
    .unwrap();
    assert!(d.is_null());
    let r = invoke(
        &f.svc,
        "backup_restore",
        json!({"profileId": f.pid, "backupId": b["id"], "asNew": false}),
    )
    .await
    .unwrap();
    assert_eq!(r.as_array().unwrap().len(), 2);
    let r = invoke(
        &f.svc,
        "backup_delete",
        json!({"profileId": f.pid, "backupId": b["id"]}),
    )
    .await
    .unwrap();
    assert_eq!(r.as_array().unwrap().len(), 1);
    let o = invoke(&f.svc, "world_open_folder", json!({"profileId": f.pid}))
        .await
        .unwrap();
    assert!(o.as_str().unwrap().ends_with("saves"));
    let e = invoke(
        &f.svc,
        "world_delete",
        json!({"profileId": f.pid, "worldId": "W"}),
    )
    .await
    .unwrap_err();
    assert_eq!(e.code, "bad_args");
}

// ---------- the image protocol ----------

#[test]
fn world_icon_protocol_accepts_and_rejects() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = Paths::new(tmp.path());
    let pid = "12345678-1234-1234-1234-123456789abc";
    let saves = paths.instance_dir(pid).unwrap().join("saves");
    write(&saves.join("Plain/icon.png"), b"png");
    write(&saves.join("My World (2)/icon.png"), b"png");
    write(&saves.join("NoIcon/level.dat"), b"x");
    write(&saves.join("Big/icon.png"), &vec![0u8; 2 * 1024 * 1024 + 1]);
    let url = |folder: &str| format!("mangoimg://world/{pid}/{folder}");

    // Accepted, both spellings, the URL builder round-trips.
    for u in [
        url("Plain"),
        url("My%20World%20%282%29"),
        worlds::world_icon_url(pid, "My World (2)"),
        format!("http://mangoimg.localhost/world/{pid}/Plain"),
    ] {
        match image_protocol::resolve(&paths, &u) {
            Ok(image_protocol::Image::File { path, mime }) => {
                assert_eq!(mime, "image/png", "{u}");
                assert_eq!(path.file_name().unwrap(), "icon.png");
            }
            other => panic!("{u}: {other:?}"),
        }
    }
    // Rejected.
    let table: Vec<(String, u16)> = vec![
        (url("NoIcon"), 404),
        (url("Missing"), 404),
        (url("Big"), 403),
        (url("..%2F..%2Fcovers"), 400),
        (url("%2E%2E"), 400),
        (url("%2E"), 400),
        (url("a%2Fb"), 400),
        (url("a%5Cb"), 400),
        (url("a%00b"), 400),
        (url(".hidden"), 400),
        (url("%2e%2e"), 400),
        (url("%zz"), 400),
        (url("%FF"), 400),
        (url("C%3Ax"), 400),
        ("mangoimg://world/not-a-uuid/Plain".to_string(), 400),
        ("mangoimg://world/../Plain".to_string(), 400),
        (format!("mangoimg://world/{pid}"), 400),
        (format!("mangoimg://world/{pid}/"), 400),
        (format!("mangoimg://world/{pid}/Plain/icon.png"), 400),
        (format!("mangoimg://world/{pid}/Plain?x=1"), 400),
        (format!("mangoimg://world/{pid}/Plain#x"), 400),
        (format!("mangoimg://world/{pid}/Plain%5Cx"), 400),
    ];
    for (u, status) in table {
        let err = image_protocol::resolve(&paths, &u).unwrap_err();
        assert_eq!(err.status(), status, "{u}: {err:?}");
        assert!(!matches!(err, ImageError::NeedsFetch(_)));
    }
}

#[cfg(unix)]
#[test]
fn world_icon_symlinks_are_rejected() {
    use std::os::unix::fs::symlink;
    let tmp = tempfile::tempdir().unwrap();
    let paths = Paths::new(tmp.path());
    let pid = "12345678-1234-1234-1234-123456789abc";
    let inst = paths.instance_dir(pid).unwrap();
    let saves = inst.join("saves");
    let outside = tmp.path().join("outside");
    write(&outside.join("secret.png"), b"png");
    write(&outside.join("World/icon.png"), b"png");
    let url = |folder: &str| format!("mangoimg://world/{pid}/{folder}");

    // icon.png is a link to a file outside.
    std::fs::create_dir_all(saves.join("LinkedIcon")).unwrap();
    symlink(
        outside.join("secret.png"),
        saves.join("LinkedIcon/icon.png"),
    )
    .unwrap();
    // The world folder is a link to a folder outside.
    symlink(outside.join("World"), saves.join("LinkedWorld")).unwrap();
    // A link to a file inside the same world is a link too.
    write(&saves.join("Inner/real.png"), b"png");
    symlink(saves.join("Inner/real.png"), saves.join("Inner/icon.png")).unwrap();
    for f in ["LinkedIcon", "LinkedWorld", "Inner"] {
        assert_eq!(
            image_protocol::resolve(&paths, &url(f))
                .unwrap_err()
                .status(),
            403,
            "{f}"
        );
    }
    // The whole saves folder is a link.
    let paths2 = Paths::new(tmp.path().join("root2"));
    let inst2 = paths2.instance_dir(pid).unwrap();
    std::fs::create_dir_all(&inst2).unwrap();
    symlink(&outside, inst2.join("saves")).unwrap();
    assert_eq!(
        image_protocol::resolve(&paths2, &url("World"))
            .unwrap_err()
            .status(),
        403
    );
}
