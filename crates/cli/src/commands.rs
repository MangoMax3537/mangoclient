//! Non-launch commands: profile, account, versions, java.

use crate::args::Args;
use crate::error::{fail, usage, CliError, Res};
use mango_core::{Paths, Store};
use mango_java::{find_system_javas, list_managed_runtimes, Platform, SearchRoots};
use mango_minecraft::{get_version_manifest, VERSION_MANIFEST_URL};
use mango_net::FetchOptions;
use serde_json::{json, Value};

pub fn open_store(paths: &Paths) -> Res<Store> {
    Store::open(paths.clone()).map_err(fail("opening the data directory"))
}

pub fn run_profile(paths: &Paths, mut args: Args) -> Res<()> {
    match args.positional().as_deref() {
        Some("create") => {
            let name = args.positional();
            let mc = args.value("--mc")?;
            let loader = args.value("--loader")?.unwrap_or_else(|| "vanilla".into());
            let lv = args.value("--loader-version")?;
            args.finish()?;
            let (Some(name), Some(mc)) = (name, mc) else {
                return usage(
                    "usage: profile create <name> --mc <version> [--loader L] [--loader-version V]",
                );
            };
            if !["vanilla", "fabric", "quilt", "forge", "neoforge"].contains(&loader.as_str()) {
                return usage(format!("unknown loader {loader:?}"));
            }
            let mut data = json!({"name": name, "mcVersion": mc, "loader": loader});
            if let Some(lv) = lv {
                data["loaderVersion"] = Value::String(lv);
            }
            let mut store = open_store(paths)?;
            let p = store
                .add_profile(&data)
                .map_err(fail("creating the profile"))?;
            println!("Created profile {:?} ({} {})", p.name, p.mc_version, loader);
            println!("RESULT profile id={}", p.id);
            Ok(())
        }
        Some("list") => {
            args.finish()?;
            let store = open_store(paths)?;
            for p in &store.profiles {
                let lv = if p.loader_version.is_empty() {
                    String::new()
                } else {
                    format!(" {}", p.loader_version)
                };
                let loader = serde_json::to_value(p.loader)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default();
                println!(
                    "{}  {}  mc {}  {}{}  played {}s",
                    p.id,
                    p.name,
                    p.mc_version,
                    loader,
                    lv,
                    p.play_time_ms / 1000
                );
            }
            Ok(())
        }
        _ => usage("usage: profile <create|list> ..."),
    }
}

pub async fn run_versions(paths: &Paths, mut args: Args) -> Res<()> {
    let snapshots = args.flag("--snapshots");
    args.finish()?;
    let manifest =
        get_version_manifest(paths, VERSION_MANIFEST_URL, false, &FetchOptions::default())
            .await
            .map_err(fail("fetching the version manifest"))?;
    let list = manifest
        .get("versions")
        .and_then(Value::as_array)
        .ok_or_else(|| CliError::Failure("the version manifest has no versions".into()))?;
    for v in list {
        let kind = v.get("type").and_then(Value::as_str).unwrap_or("");
        if !snapshots && kind != "release" {
            continue;
        }
        println!(
            "{}\t{kind}",
            v.get("id").and_then(Value::as_str).unwrap_or("?")
        );
    }
    Ok(())
}

pub async fn run_java(paths: &Paths, mut args: Args) -> Res<()> {
    if args.positional().as_deref() != Some("list") {
        return usage("usage: java list");
    }
    args.finish()?;
    let platform = Platform::current();
    let managed = list_managed_runtimes(&paths.runtimes(), platform).await;
    for c in &managed {
        println!("managed\tjava {}\t{}", c.major, c.path.display());
    }
    let system = find_system_javas(&SearchRoots::from_env(platform), platform).await;
    for j in &system {
        println!("system\tjava {}\t{}", j.major, j.path.display());
    }
    if managed.is_empty() && system.is_empty() {
        println!("no Java runtimes found");
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Args {
        Args::new(v.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn create_profile_and_offline_account() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        run_profile(
            &paths,
            args(&[
                "create",
                "Test Pack",
                "--mc",
                "1.21.1",
                "--loader",
                "fabric",
            ]),
        )
        .unwrap();
        let acc = mango_auth::offline_account("Steve_1").unwrap();
        let mut store = open_store(&paths).unwrap();
        store
            .upsert_account(serde_json::from_value(serde_json::to_value(&acc).unwrap()).unwrap())
            .unwrap();
        drop(store);
        let store = open_store(&paths).unwrap();
        let p = store
            .profiles
            .iter()
            .find(|p| p.name == "Test Pack")
            .unwrap();
        assert_eq!(p.mc_version, "1.21.1");
        let acc = store.selected_account().unwrap();
        assert_eq!(acc.name, "Steve_1");
        assert_eq!(acc.kind, mango_core::AccountType::Offline);
        assert!(run_profile(
            &paths,
            args(&["create", "x", "--mc", "1.21", "--loader", "bogus"])
        )
        .is_err());
    }
}
