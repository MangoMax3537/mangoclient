//! MangoConfig jar injection (port of `src/main/mangoconfig.js`).
//!
//! The bundled jars are scanned from an injectable directory; the final bundle
//! location is an open decision. Everything here is best effort: nothing
//! returns an error to the launch path.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use mango_core::{Config, Loader, Paths, Profile};
use regex::Regex;
use serde::Serialize;

pub const NAME: &str = "MangoConfig";
pub const LOADERS: [&str; 2] = ["fabric", "quilt"];

fn own_jar_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^MangoConfig-.*\.jar(\.disabled)?$").unwrap_or_else(|_| unreachable!())
    })
}

fn versioned_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^MangoConfig-.+\+mc(.+)\.jar$").unwrap_or_else(|_| unreachable!())
    })
}

/// True for any build of ours (also `.disabled`), so the mod list can skip it.
pub fn is_own_jar(filename: &str) -> bool {
    let base = filename.rsplit(['/', '\\']).next().unwrap_or("");
    own_jar_re().is_match(base)
}

/// Natural ordering: digit runs compare numerically.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    fn chunks(s: &str) -> Vec<(bool, &str)> {
        let mut out = Vec::new();
        let mut start = 0;
        let mut prev: Option<bool> = None;
        for (i, c) in s.char_indices() {
            let d = c.is_ascii_digit();
            if prev.is_some_and(|p| p != d) {
                out.push((prev == Some(true), &s[start..i]));
                start = i;
            }
            prev = Some(d);
        }
        if prev.is_some() {
            out.push((prev == Some(true), &s[start..]));
        }
        out
    }
    let (ca, cb) = (chunks(a), chunks(b));
    for (x, y) in ca.iter().zip(cb.iter()) {
        let ord = if x.0 && y.0 {
            let (xs, ys) = (x.1.trim_start_matches('0'), y.1.trim_start_matches('0'));
            xs.len().cmp(&ys.len()).then_with(|| xs.cmp(ys))
        } else {
            x.1.cmp(y.1)
        };
        if ord != Ordering::Equal {
            return ord;
        }
    }
    ca.len().cmp(&cb.len())
}

/// The jars shipped with the app: game version -> jar path.
#[derive(Debug, Clone, Default)]
pub struct Bundle {
    jars: BTreeMap<String, PathBuf>,
}

impl Bundle {
    /// Scan `dir` for `MangoConfig-<ver>+mc<mc>.jar`. A missing or unreadable
    /// directory is an empty bundle.
    pub fn scan(dir: &Path) -> Bundle {
        let mut jars = BTreeMap::new();
        if let Ok(rd) = fs::read_dir(dir) {
            for entry in rd.flatten() {
                let name = entry.file_name();
                let Some(name) = name.to_str() else { continue };
                if let Some(m) = versioned_re().captures(name) {
                    jars.insert(m[1].to_string(), entry.path());
                }
            }
        }
        Bundle { jars }
    }

    pub fn jar_for(&self, mc_version: &str) -> Option<&Path> {
        self.jars.get(mc_version).map(PathBuf::as_path)
    }

    pub fn has_build_for(&self, mc_version: &str) -> bool {
        self.jars.contains_key(mc_version)
    }

    /// Game versions with a build, numeric-aware ascending.
    pub fn game_versions(&self) -> Vec<String> {
        let mut v: Vec<String> = self.jars.keys().cloned().collect();
        v.sort_by(|a, b| natural_cmp(a, b));
        v
    }
}

/// Profile switch wins over the global toggle only to turn it off.
pub fn enabled_for(profile: &Profile, config: &Config) -> bool {
    profile.mango_config != Some(false) && config.mango_config
}

fn loader_ok(loader: Loader) -> bool {
    matches!(loader, Loader::Fabric | Loader::Quilt)
}

/// Fabric/Quilt and an exactly matching game version with a bundled build.
pub fn supports(profile: &Profile, bundle: &Bundle) -> bool {
    loader_ok(profile.loader) && bundle.has_build_for(&profile.mc_version)
}

/// A vanilla profile switches to Fabric only when the user explicitly turned
/// MangoConfig on for it (`mangoConfig === true`), the global switch is on and
/// a jar exists for its exact version. The default (`null`) never converts a
/// loader. Forge/NeoForge/Quilt never switch.
pub fn should_switch_vanilla_to_fabric(
    profile: &Profile,
    config: &Config,
    bundle: &Bundle,
) -> bool {
    profile.loader == Loader::Vanilla
        && profile.mango_config == Some(true)
        && config.mango_config
        && bundle.has_build_for(&profile.mc_version)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Disabled by profile or global switch; `removed` jars were cleared.
    Disabled {
        removed: usize,
    },
    /// No build for this loader/version; own jars were cleared.
    Unsupported {
        reason: String,
    },
    /// The right jar was already in place.
    Current,
    Installed,
    Failed {
        error: String,
    },
}

fn mods_dir(paths: &Paths, profile: &Profile) -> Option<PathBuf> {
    paths.instance_dir(&profile.id).ok().map(|d| d.join("mods"))
}

/// Delete our jars in `dir` except the one named `except`. Returns the count
/// removed; failures (file in use) are skipped, the next launch retries.
fn remove_own_jars(dir: &Path, except: Option<&str>) -> usize {
    let Ok(rd) = fs::read_dir(dir) else { return 0 };
    let mut removed = 0;
    for entry in rd.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !is_own_jar(name) || Some(name) == except {
            continue;
        }
        if fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Put MangoConfig into the instance or take it out. Never fails.
pub fn ensure(paths: &Paths, profile: &Profile, config: &Config, bundle_dir: &Path) -> Outcome {
    ensure_with_log(paths, profile, config, bundle_dir, &|_| {})
}

pub fn ensure_with_log(
    paths: &Paths,
    profile: &Profile,
    config: &Config,
    bundle_dir: &Path,
    on_log: &dyn Fn(&str),
) -> Outcome {
    let Some(dir) = mods_dir(paths, profile) else {
        return Outcome::Failed {
            error: "invalid profile id".into(),
        };
    };

    if !enabled_for(profile, config) {
        let removed = remove_own_jars(&dir, None);
        if removed > 0 {
            on_log(&format!("{NAME} removed from this profile"));
        }
        return Outcome::Disabled { removed };
    }

    let bundle = Bundle::scan(bundle_dir);
    let source = match bundle.jar_for(&profile.mc_version) {
        Some(s) if loader_ok(profile.loader) => s,
        _ => {
            // Leave nothing behind, or the game may refuse to start.
            remove_own_jars(&dir, None);
            return Outcome::Unsupported {
                reason: format!("{:?} {}", profile.loader, profile.mc_version).to_lowercase(),
            };
        }
    };
    let Some(jar_name) = source.file_name().and_then(|n| n.to_str()) else {
        return Outcome::Failed {
            error: "bad jar name".into(),
        };
    };
    let target = dir.join(jar_name);

    let same_size = || match (fs::metadata(&target), fs::metadata(source)) {
        (Ok(a), Ok(b)) => a.is_file() && a.len() == b.len(),
        _ => false,
    };
    if same_size() {
        remove_own_jars(&dir, Some(jar_name));
        return Outcome::Current;
    }
    let result = fs::create_dir_all(&dir).and_then(|_| {
        // An older build has to go first: two jars, one mod id, no launch.
        remove_own_jars(&dir, Some(jar_name));
        fs::copy(source, &target).map(|_| ())
    });
    match result {
        Ok(()) => {
            on_log(&format!("{NAME} added to this profile"));
            Outcome::Installed
        }
        Err(e) => Outcome::Failed {
            error: e.to_string(),
        },
    }
}

/// A versioned jar of ours (not `.disabled`) sits in the instance.
pub fn present(paths: &Paths, profile_id: &str) -> bool {
    let Ok(dir) = paths.instance_dir(profile_id) else {
        return false;
    };
    fs::read_dir(dir.join("mods"))
        .map(|rd| {
            rd.flatten().any(|e| {
                e.file_name()
                    .to_str()
                    .is_some_and(|n| versioned_re().is_match(n))
            })
        })
        .unwrap_or(false)
}

/// What the `mangoconfig:info` IPC returns.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub name: &'static str,
    pub loaders: Vec<&'static str>,
    pub game_versions: Vec<String>,
    pub global_enabled: bool,
    pub enabled: bool,
    pub supported: bool,
    pub installed: bool,
}

pub fn info(paths: &Paths, profile: Option<&Profile>, config: &Config, bundle_dir: &Path) -> Info {
    let bundle = Bundle::scan(bundle_dir);
    Info {
        name: NAME,
        loaders: LOADERS.to_vec(),
        game_versions: bundle.game_versions(),
        global_enabled: config.mango_config,
        enabled: profile.is_some_and(|p| enabled_for(p, config)),
        supported: profile.is_some_and(|p| supports(p, &bundle)),
        installed: profile.is_some_and(|p| present(paths, &p.id)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "12345678-1234-4234-8234-123456789abc";

    struct Fx {
        _tmp: tempfile::TempDir,
        paths: Paths,
        bundle: PathBuf,
        mods: PathBuf,
    }

    fn fx() -> Fx {
        let tmp = tempfile::tempdir().unwrap();
        let bundle = tmp.path().join("bundle");
        fs::create_dir_all(&bundle).unwrap();
        for mc in ["1.21.10", "1.21.4", "1.21", "1.20.5"] {
            fs::write(bundle.join(format!("MangoConfig-1.10.1+mc{mc}.jar")), mc).unwrap();
        }
        fs::write(bundle.join("readme.txt"), "x").unwrap();
        let paths = Paths::new(tmp.path().join("root"));
        let mods = paths.instance_dir(ID).unwrap().join("mods");
        Fx {
            _tmp: tmp,
            paths,
            bundle,
            mods,
        }
    }

    fn profile(loader: Loader, mc: &str) -> Profile {
        let mut p = Profile::new(ID.into(), 0);
        p.loader = loader;
        p.mc_version = mc.into();
        p
    }

    fn config(on: bool) -> Config {
        let mut c = Config::with_total_ram(8192);
        c.mango_config = on;
        c
    }

    #[test]
    fn own_jar_matching() {
        assert!(is_own_jar("MangoConfig-1.10.1+mc1.21.jar"));
        assert!(is_own_jar("mangoconfig-old.jar.disabled"));
        assert!(is_own_jar("/x/mods/MangoConfig-1.jar"));
        assert!(!is_own_jar("Sodium.jar"));
        assert!(!is_own_jar("MangoConfigExtra.jar"));
        assert!(!is_own_jar("MangoConfig-1.jar.bak"));
    }

    #[test]
    fn versions_sorted_numerically() {
        let f = fx();
        assert_eq!(
            Bundle::scan(&f.bundle).game_versions(),
            ["1.20.5", "1.21", "1.21.4", "1.21.10"]
        );
        assert!(Bundle::scan(&f.bundle.join("nope"))
            .game_versions()
            .is_empty());
    }

    #[test]
    fn installs_then_current_and_replaces_stale() {
        let f = fx();
        let p = profile(Loader::Fabric, "1.21.4");
        fs::create_dir_all(&f.mods).unwrap();
        fs::write(f.mods.join("MangoConfig-1.0.0+mc1.21.4.jar"), "old").unwrap();
        fs::write(
            f.mods.join("MangoConfig-1.0.0+mc1.20.5.jar.disabled"),
            "old",
        )
        .unwrap();
        fs::write(f.mods.join("Sodium.jar"), "keep").unwrap();

        assert_eq!(
            ensure(&f.paths, &p, &config(true), &f.bundle),
            Outcome::Installed
        );
        let mut names: Vec<_> = fs::read_dir(&f.mods)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, ["MangoConfig-1.10.1+mc1.21.4.jar", "Sodium.jar"]);
        assert_eq!(
            ensure(&f.paths, &p, &config(true), &f.bundle),
            Outcome::Current
        );
        assert!(present(&f.paths, ID));
    }

    #[test]
    fn same_size_skips_copy_and_clears_others() {
        let f = fx();
        let p = profile(Loader::Quilt, "1.21");
        fs::create_dir_all(&f.mods).unwrap();
        // Same size (4 bytes) but different content: must be left alone.
        fs::write(f.mods.join("MangoConfig-1.10.1+mc1.21.jar"), "ZZZZ").unwrap();
        fs::write(f.mods.join("MangoConfig-0.9+mc1.21.jar"), "o").unwrap();
        assert_eq!(
            ensure(&f.paths, &p, &config(true), &f.bundle),
            Outcome::Current
        );
        assert_eq!(
            fs::read(f.mods.join("MangoConfig-1.10.1+mc1.21.jar")).unwrap(),
            b"ZZZZ"
        );
        assert!(!f.mods.join("MangoConfig-0.9+mc1.21.jar").exists());
    }

    #[test]
    fn unsupported_cases_clear_jars() {
        let f = fx();
        for (loader, mc) in [
            (Loader::Forge, "1.21.4"),
            (Loader::Neoforge, "1.21.4"),
            (Loader::Vanilla, "1.21.4"),
            (Loader::Fabric, "1.21.99"),
            (Loader::Fabric, "1.21.4.1"),
        ] {
            fs::create_dir_all(&f.mods).unwrap();
            fs::write(f.mods.join("MangoConfig-1+mc1.21.4.jar"), "x").unwrap();
            let out = ensure(&f.paths, &profile(loader, mc), &config(true), &f.bundle);
            assert!(
                matches!(out, Outcome::Unsupported { .. }),
                "{loader:?} {mc}"
            );
            assert!(!f.mods.join("MangoConfig-1+mc1.21.4.jar").exists());
        }
    }

    #[test]
    fn disabled_by_profile_or_global() {
        let f = fx();
        let mut p = profile(Loader::Fabric, "1.21.4");
        fs::create_dir_all(&f.mods).unwrap();
        fs::write(f.mods.join("MangoConfig-1+mc1.21.4.jar"), "x").unwrap();
        p.mango_config = Some(false);
        assert_eq!(
            ensure(&f.paths, &p, &config(true), &f.bundle),
            Outcome::Disabled { removed: 1 }
        );
        p.mango_config = None;
        assert_eq!(
            ensure(&f.paths, &p, &config(false), &f.bundle),
            Outcome::Disabled { removed: 0 }
        );
        // Profile `true` does not override a global off.
        p.mango_config = Some(true);
        assert!(!enabled_for(&p, &config(false)));
    }

    #[test]
    fn failure_is_reported_not_thrown() {
        let f = fx();
        let p = profile(Loader::Fabric, "1.21.4");
        // `mods` is a file, so the directory cannot be created.
        fs::create_dir_all(f.mods.parent().unwrap()).unwrap();
        fs::write(&f.mods, "file").unwrap();
        assert!(matches!(
            ensure(&f.paths, &p, &config(true), &f.bundle),
            Outcome::Failed { .. }
        ));
        let mut bad = p.clone();
        bad.id = "../x".into();
        assert!(matches!(
            ensure(&f.paths, &bad, &config(true), &f.bundle),
            Outcome::Failed { .. }
        ));
    }

    #[test]
    fn auto_switch_rules() {
        let f = fx();
        let b = Bundle::scan(&f.bundle);
        let c = config(true);
        let mut opted_in = profile(Loader::Vanilla, "1.21.4");
        opted_in.mango_config = Some(true);
        assert!(should_switch_vanilla_to_fabric(&opted_in, &c, &b));
        // The default (null) never converts a loader.
        assert!(!should_switch_vanilla_to_fabric(
            &profile(Loader::Vanilla, "1.21.4"),
            &c,
            &b
        ));
        // An unsupported version never switches, even when opted in.
        let mut unsupported = profile(Loader::Vanilla, "1.19");
        unsupported.mango_config = Some(true);
        assert!(!should_switch_vanilla_to_fabric(&unsupported, &c, &b));
        let mut future = profile(Loader::Vanilla, "26.3");
        future.mango_config = Some(true);
        assert!(!should_switch_vanilla_to_fabric(&future, &c, &b));
        for l in [
            Loader::Quilt,
            Loader::Forge,
            Loader::Neoforge,
            Loader::Fabric,
        ] {
            assert!(!should_switch_vanilla_to_fabric(
                &profile(l, "1.21.4"),
                &c,
                &b
            ));
        }
        assert!(!should_switch_vanilla_to_fabric(
            &opted_in,
            &config(false),
            &b
        ));
        let mut off = profile(Loader::Vanilla, "1.21.4");
        off.mango_config = Some(false);
        assert!(!should_switch_vanilla_to_fabric(&off, &c, &b));
    }

    #[test]
    fn info_shape() {
        let f = fx();
        let p = profile(Loader::Fabric, "1.21");
        let i = info(&f.paths, Some(&p), &config(true), &f.bundle);
        assert!(i.enabled && i.supported && !i.installed && i.global_enabled);
        assert_eq!(i.loaders, ["fabric", "quilt"]);
        ensure(&f.paths, &p, &config(true), &f.bundle);
        assert!(info(&f.paths, Some(&p), &config(true), &f.bundle).installed);
        let v = serde_json::to_value(&i).unwrap();
        assert!(v.get("gameVersions").is_some() && v.get("globalEnabled").is_some());
        assert!(!info(&f.paths, None, &config(false), &f.bundle).enabled);
    }
}
