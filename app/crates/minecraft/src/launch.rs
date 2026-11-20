//! Pure launch command construction.

use crate::host::{HostEnv, HostOs};
use crate::install::InstallResult;
use crate::rules::{rules_allow, Features};
use mango_core::Paths;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

const POTATO: &[&str] = &[
    "-XX:+UseSerialGC",
    "-XX:TieredStopAtLevel=1",
    "-Dsun.rmi.dgc.server.gcInterval=2147483646",
];
const BALANCED: &[&str] = &[
    "-XX:+UseG1GC",
    "-XX:+ParallelRefProcEnabled",
    "-XX:MaxGCPauseMillis=200",
    "-XX:+UnlockExperimentalVMOptions",
    "-XX:+DisableExplicitGC",
    "-XX:+AlwaysPreTouch",
    "-XX:G1NewSizePercent=30",
    "-XX:G1MaxNewSizePercent=40",
    "-XX:G1HeapRegionSize=8M",
    "-XX:G1ReservePercent=20",
    "-XX:G1HeapWastePercent=5",
    "-XX:G1MixedGCCountTarget=4",
    "-XX:InitiatingHeapOccupancyPercent=15",
    "-XX:G1MixedGCLiveThresholdPercent=90",
    "-XX:G1RSetUpdatingPauseTimePercent=5",
    "-XX:SurvivorRatio=32",
    "-XX:+PerfDisableSharedMem",
    "-XX:MaxTenuringThreshold=1",
];
const QUALITY: &[&str] = &[
    "-XX:+UseZGC",
    "-XX:+ZGenerational",
    "-XX:+AlwaysPreTouch",
    "-XX:+DisableExplicitGC",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Preset {
    Potato,
    #[default]
    Balanced,
    Quality,
}

impl Preset {
    /// Unknown names fall back to balanced, like the legacy launcher.
    pub fn from_name(name: &str) -> Self {
        match name {
            "potato" => Self::Potato,
            "quality" => Self::Quality,
            _ => Self::Balanced,
        }
    }

    pub fn flags(self) -> &'static [&'static str] {
        match self {
            Self::Potato => POTATO,
            Self::Balanced => BALANCED,
            Self::Quality => QUALITY,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LaunchAccount {
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    pub xuid: Option<String>,
    pub offline: bool,
}

#[derive(Debug, Clone)]
pub struct LaunchInput {
    pub java_path: PathBuf,
    pub paths: Paths,
    /// Flattened version JSON.
    pub version: Value,
    pub classpath: Vec<PathBuf>,
    pub client_jar: PathBuf,
    pub natives_dir: PathBuf,
    pub assets_index_id: String,
    pub is_legacy_assets: bool,
    pub logging_arg: Option<String>,
    pub game_dir: PathBuf,
    pub profile_id: String,
    pub profile_name: String,
    pub ram_mb: u32,
    pub preset: Preset,
    pub global_java_args: String,
    pub profile_java_args: String,
    pub fullscreen: bool,
    /// 0 means the default (1280).
    pub width: u32,
    /// 0 means the default (720).
    pub height: u32,
    /// `host` or `host:port`.
    pub quick_join: Option<String>,
    pub account: LaunchAccount,
    pub launcher_version: String,
    pub env: HostEnv,
}

impl LaunchInput {
    /// Fill the install-derived fields from an [`InstallResult`].
    pub fn apply_install(&mut self, install: &InstallResult) {
        self.version = install.version.clone();
        self.classpath = install.classpath.clone();
        self.client_jar = install.client_jar.clone();
        self.natives_dir = install.natives_dir.clone();
        self.assets_index_id = install.assets_index_id.clone();
        self.is_legacy_assets = install.is_legacy_assets;
        self.logging_arg = install.logging_arg.clone();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    /// Variables to add to the inherited environment.
    pub env: Vec<(String, String)>,
    pub cwd: PathBuf,
}

/// Replace `${key}`; unknown keys are left untouched.
pub fn substitute(s: &str, vars: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) if end > 0 => {
                let key = &after[..end];
                match vars.get(key) {
                    Some(v) => out.push_str(v),
                    None => {
                        out.push_str("${");
                        out.push_str(key);
                        out.push('}');
                    }
                }
                rest = &after[end + 1..];
            }
            _ => {
                out.push_str("${");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Flatten an `arguments` array, honouring rules and features.
pub fn expand_args(
    args: &[Value],
    vars: &HashMap<String, String>,
    features: &Features,
    env: &HostEnv,
) -> Vec<String> {
    let mut out = Vec::new();
    for arg in args {
        if let Some(s) = arg.as_str() {
            out.push(substitute(s, vars));
            continue;
        }
        if !rules_allow(arg.get("rules"), features, env) {
            continue;
        }
        match arg.get("value") {
            Some(Value::Array(vs)) => out.extend(
                vs.iter()
                    .filter_map(Value::as_str)
                    .map(|v| substitute(v, vars)),
            ),
            Some(Value::String(v)) => out.push(substitute(v, vars)),
            _ => {}
        }
    }
    out
}

fn lossy(p: &std::path::Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Build the JVM command line for a prepared install. Does no I/O.
pub fn build_launch_command(input: &LaunchInput) -> LaunchCommand {
    let sep = input.env.os.classpath_separator().to_string();
    let mut cp: Vec<String> = input.classpath.iter().map(|p| lossy(p)).collect();
    cp.push(lossy(&input.client_jar));
    let classpath = cp.join(&sep);

    let quick = input.quick_join.as_deref().filter(|q| !q.is_empty());
    let width = if input.width == 0 { 1280 } else { input.width };
    let height = if input.height == 0 { 720 } else { input.height };
    let version = &input.version;
    let version_id = version.get("id").and_then(Value::as_str).unwrap_or("");

    let features: Features = [
        ("is_demo_user", false),
        ("has_custom_resolution", !input.fullscreen),
        ("has_quick_plays_support", quick.is_some()),
        ("is_quick_play_multiplayer", quick.is_some()),
        ("is_quick_play_singleplayer", false),
        ("is_quick_play_realms", false),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();

    let acc = &input.account;
    let game_assets = if input.is_legacy_assets {
        lossy(&input.paths.asset_virtual().join(&input.assets_index_id))
    } else {
        lossy(&input.paths.assets())
    };
    let pairs: Vec<(&str, String)> = vec![
        ("auth_player_name", acc.name.clone()),
        ("auth_uuid", acc.uuid.clone()),
        ("auth_access_token", acc.access_token.clone()),
        (
            "auth_session",
            format!("token:{}:{}", acc.access_token, acc.uuid),
        ),
        (
            "auth_xuid",
            acc.xuid
                .clone()
                .filter(|x| !x.is_empty())
                .unwrap_or_else(|| "0".into()),
        ),
        (
            "user_type",
            if acc.offline { "legacy" } else { "msa" }.into(),
        ),
        ("user_properties", "{}".into()),
        ("clientid", String::new()),
        ("version_name", version_id.to_string()),
        (
            "version_type",
            version
                .get("type")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .unwrap_or("release")
                .to_string(),
        ),
        ("game_directory", lossy(&input.game_dir)),
        ("assets_root", lossy(&input.paths.assets())),
        ("game_assets", game_assets),
        ("assets_index_name", input.assets_index_id.clone()),
        ("natives_directory", lossy(&input.natives_dir)),
        ("launcher_name", "MangoClient".into()),
        ("launcher_version", input.launcher_version.clone()),
        ("classpath", classpath.clone()),
        ("classpath_separator", sep),
        ("library_directory", lossy(&input.paths.libraries())),
        ("resolution_width", width.to_string()),
        ("resolution_height", height.to_string()),
        ("quickPlayMultiplayer", quick.unwrap_or("").to_string()),
        ("quickPlayPath", String::new()),
        ("quickPlaySingleplayer", String::new()),
        ("quickPlayRealms", String::new()),
    ];
    let vars: HashMap<String, String> =
        pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect();

    let mut jvm: Vec<String> = Vec::new();
    jvm.push(format!("-Xmx{}M", input.ram_mb));
    jvm.push(format!("-Xms{}M", input.ram_mb.min(1024)));
    jvm.extend(input.preset.flags().iter().map(|s| s.to_string()));
    jvm.extend(
        [
            "-Dfile.encoding=UTF-8",
            "-Dstdout.encoding=UTF-8",
            "-Dstderr.encoding=UTF-8",
            "-Dminecraft.launcher.brand=MangoClient",
        ]
        .map(String::from),
    );
    jvm.push(format!(
        "-Dminecraft.launcher.version={}",
        input.launcher_version
    ));
    if input.env.os == HostOs::Osx {
        jvm.push("-XstartOnFirstThread".into());
    }
    if let Some(arg) = &input.logging_arg {
        jvm.push(arg.clone());
    }

    match version
        .get("arguments")
        .and_then(|a| a.get("jvm"))
        .and_then(Value::as_array)
    {
        Some(list) => jvm.extend(expand_args(list, &vars, &features, &input.env)),
        None => {
            // Pre-1.13 versions have no jvm argument list at all.
            jvm.push(format!("-Djava.library.path={}", lossy(&input.natives_dir)));
            jvm.push("-cp".into());
            jvm.push(classpath);
        }
    }
    for extra in [&input.global_java_args, &input.profile_java_args] {
        jvm.extend(extra.split_whitespace().map(String::from));
    }

    let mut game: Vec<String> = match version
        .get("arguments")
        .and_then(|a| a.get("game"))
        .and_then(Value::as_array)
    {
        Some(list) => expand_args(list, &vars, &features, &input.env),
        None => substitute(
            version
                .get("minecraftArguments")
                .and_then(Value::as_str)
                .unwrap_or(""),
            &vars,
        )
        .split_whitespace()
        .map(String::from)
        .collect(),
    };
    if input.fullscreen {
        game.push("--fullscreen".into());
    } else if !game.iter().any(|a| a == "--width") {
        game.extend([
            "--width".into(),
            width.to_string(),
            "--height".into(),
            height.to_string(),
        ]);
    }
    if let Some(q) = quick {
        if !game.iter().any(|a| a == "--quickPlayMultiplayer") {
            let mut parts = q.split(':');
            let host = parts.next().unwrap_or("");
            let port = parts.next().unwrap_or("25565");
            game.extend([
                "--server".into(),
                host.to_string(),
                "--port".into(),
                port.to_string(),
            ]);
        }
    }

    let main_class = version
        .get("mainClass")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut args = jvm;
    args.push(main_class);
    args.extend(game);

    LaunchCommand {
        program: input.java_path.clone(),
        args,
        env: vec![
            ("INST_NAME".into(), input.profile_name.clone()),
            ("INST_ID".into(), input.profile_id.clone()),
            ("INST_DIR".into(), lossy(&input.game_dir)),
        ],
        cwd: input.game_dir.clone(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::host::HostArch;
    use serde_json::json;

    fn input(version: Value) -> LaunchInput {
        LaunchInput {
            java_path: "/jre/bin/java".into(),
            paths: Paths::new("/r"),
            version,
            classpath: vec!["/r/libraries/a.jar".into(), "/r/libraries/b.jar".into()],
            client_jar: "/r/versions/v/v.jar".into(),
            natives_dir: "/r/natives/v".into(),
            assets_index_id: "17".into(),
            is_legacy_assets: false,
            logging_arg: Some("-Dlog4j.configurationFile=/r/assets/log_configs/x.xml".into()),
            game_dir: "/r/instances/i".into(),
            profile_id: "pid".into(),
            profile_name: "My Pack".into(),
            ram_mb: 4096,
            preset: Preset::Balanced,
            global_java_args: "-Dg=1  -Dh=2".into(),
            profile_java_args: " -Dp=3 ".into(),
            fullscreen: false,
            width: 0,
            height: 0,
            quick_join: None,
            account: LaunchAccount {
                name: "Steve".into(),
                uuid: "u-1".into(),
                access_token: "tok".into(),
                xuid: None,
                offline: false,
            },
            launcher_version: "2.0.0".into(),
            env: HostEnv::new(HostOs::Linux, HostArch::X64),
        }
    }

    fn modern() -> Value {
        json!({
            "id": "1.21", "type": "release", "mainClass": "net.minecraft.Main",
            "arguments": {
                "jvm": [
                    "-Djava.library.path=${natives_directory}",
                    {"rules":[{"action":"allow","os":{"name":"osx"}}], "value": "-XstartOnFirstThread"},
                    {"rules":[{"action":"allow","os":{"name":"linux"}}], "value": ["-Da=1", "-Db=${launcher_name}"]},
                    "-cp", "${classpath}"
                ],
                "game": [
                    "--username", "${auth_player_name}", "--uuid", "${auth_uuid}",
                    "--accessToken", "${auth_access_token}", "--userType", "${user_type}",
                    "--xuid", "${auth_xuid}", "--unknown", "${nope}",
                    {"rules":[{"action":"allow","features":{"has_custom_resolution":true}}],
                     "value": ["--width", "${resolution_width}", "--height", "${resolution_height}"]},
                    {"rules":[{"action":"allow","features":{"is_quick_play_multiplayer":true}}],
                     "value": ["--quickPlayMultiplayer", "${quickPlayMultiplayer}"]}
                ]
            }
        })
    }

    #[test]
    fn substitution_leaves_unknown_untouched() {
        let mut v = HashMap::new();
        v.insert("a".to_string(), "1".to_string());
        assert_eq!(substitute("x${a}y${b}z${", &v), "x1y${b}z${");
        assert_eq!(substitute("${}${a}", &v), "${}1");
    }

    #[test]
    fn jvm_arg_order() {
        let cmd = build_launch_command(&input(modern()));
        let a = &cmd.args;
        assert_eq!(a[0], "-Xmx4096M");
        assert_eq!(a[1], "-Xms1024M");
        assert_eq!(a[2], "-XX:+UseG1GC");
        let enc = a.iter().position(|x| x == "-Dfile.encoding=UTF-8").unwrap();
        assert_eq!(enc, 2 + BALANCED.len());
        assert_eq!(a[enc + 3], "-Dminecraft.launcher.brand=MangoClient");
        assert_eq!(a[enc + 4], "-Dminecraft.launcher.version=2.0.0");
        assert_eq!(
            a[enc + 5],
            "-Dlog4j.configurationFile=/r/assets/log_configs/x.xml"
        );
        assert_eq!(a[enc + 6], "-Djava.library.path=/r/natives/v");
        assert!(!a.contains(&"-XstartOnFirstThread".to_string()));
        assert_eq!(a[enc + 7], "-Da=1");
        assert_eq!(a[enc + 8], "-Db=MangoClient");
        assert_eq!(a[enc + 9], "-cp");
        assert_eq!(
            a[enc + 10],
            "/r/libraries/a.jar:/r/libraries/b.jar:/r/versions/v/v.jar"
        );
        assert_eq!(&a[enc + 11..enc + 14], ["-Dg=1", "-Dh=2", "-Dp=3"]);
        assert_eq!(a[enc + 14], "net.minecraft.Main");
    }

    #[test]
    fn mac_flag_potato_and_ram_floor() {
        let mut i = input(modern());
        i.env = HostEnv::new(HostOs::Osx, HostArch::Arm64);
        i.preset = Preset::Potato;
        i.ram_mb = 512;
        let a = build_launch_command(&i).args;
        assert_eq!(a[1], "-Xms512M");
        assert_eq!(a[2], "-XX:+UseSerialGC");
        assert!(a.contains(&"-XstartOnFirstThread".to_string()));
        assert_eq!(Preset::from_name("???"), Preset::Balanced);
        assert_eq!(Preset::from_name("quality").flags()[0], "-XX:+UseZGC");
    }

    #[test]
    fn game_args_substitution_and_resolution() {
        let cmd = build_launch_command(&input(modern()));
        let a = &cmd.args;
        let main = a.iter().position(|x| x == "net.minecraft.Main").unwrap();
        let game = &a[main + 1..];
        assert_eq!(
            game,
            [
                "--username",
                "Steve",
                "--uuid",
                "u-1",
                "--accessToken",
                "tok",
                "--userType",
                "msa",
                "--xuid",
                "0",
                "--unknown",
                "${nope}",
                "--width",
                "1280",
                "--height",
                "720"
            ]
        );
        assert_eq!(cmd.cwd, PathBuf::from("/r/instances/i"));
        assert!(cmd.env.contains(&("INST_ID".into(), "pid".into())));
    }

    #[test]
    fn fullscreen_and_quick_join() {
        let mut i = input(modern());
        i.fullscreen = true;
        i.quick_join = Some("mc.example.com:25570".into());
        let a = build_launch_command(&i).args;
        assert!(a.ends_with(&[
            "--quickPlayMultiplayer".to_string(),
            "mc.example.com:25570".to_string(),
            "--fullscreen".to_string()
        ]));
        assert!(!a.contains(&"--server".to_string()));

        // legacy version without quick-play support falls back to --server/--port
        let mut i = input(
            json!({"id":"1.8.9","mainClass":"M","minecraftArguments":"--username ${auth_player_name} --session ${auth_session}"}),
        );
        i.quick_join = Some("host".into());
        i.account.offline = true;
        let a = build_launch_command(&i).args;
        let m = a.iter().position(|x| x == "M").unwrap();
        assert_eq!(
            &a[m + 1..],
            [
                "--username",
                "Steve",
                "--session",
                "token:tok:u-1",
                "--width",
                "1280",
                "--height",
                "720",
                "--server",
                "host",
                "--port",
                "25565"
            ]
        );
    }

    #[test]
    fn pre_1_13_gets_library_path_and_cp() {
        let i =
            input(json!({"id":"1.8.9","mainClass":"M","minecraftArguments":"--v ${user_type}"}));
        let a = build_launch_command(&i).args;
        let p = a
            .iter()
            .position(|x| x == "-Djava.library.path=/r/natives/v")
            .unwrap();
        assert_eq!(a[p + 1], "-cp");
        assert!(a[p + 2].ends_with("/r/versions/v/v.jar"));
        let m = a.iter().position(|x| x == "M").unwrap();
        assert_eq!(a[m + 1], "--v");
        assert_eq!(a[m + 2], "msa");
    }

    #[test]
    fn legacy_assets_use_virtual_dir_and_windows_separator() {
        let mut i = input(
            json!({"id":"1.6","mainClass":"M","minecraftArguments":"--assetsDir ${game_assets} --cp ${classpath_separator}"}),
        );
        i.is_legacy_assets = true;
        i.assets_index_id = "legacy".into();
        i.env = HostEnv::new(HostOs::Windows, HostArch::X64);
        let a = build_launch_command(&i).args;
        let m = a.iter().position(|x| x == "M").unwrap();
        assert!(a[m + 2].contains("virtual") && a[m + 2].ends_with("legacy"));
        assert_eq!(a[m + 4], ";");
    }
}
