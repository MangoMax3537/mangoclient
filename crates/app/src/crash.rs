//! Part 9 of `docs/ipc.md`: crash analysis.
//!
//! The engine is a table of [`Rule`]s. Each rule is a pure function from the
//! [`Facts`] (the log text plus what the launcher knows about the instance)
//! to at most one [`Finding`]; nothing here touches the disk or the network,
//! so every rule has a unit test on a small log snippet. The service
//! (`crash_analyze`) reads the log, runs [`analyze`], scrubs the result and
//! maps mod ids to installed titles.

use mango_core::{Loader, ModRecord};
use regex::Regex;
use serde::Serialize;
use std::sync::OnceLock;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CrashCause {
    /// Stable key the UI localizes (`java_version`, `out_of_memory`, ...).
    pub code: String,
    pub title: String,
    pub detail: String,
    pub confidence: Confidence,
    pub suggestions: Vec<String>,
    /// Titles of installed mods when they could be matched, else mod ids or
    /// file names as the log names them.
    pub related_mods: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CrashReport {
    pub summary: String,
    pub causes: Vec<CrashCause>,
    pub java_version: Option<String>,
    pub mc_version: Option<String>,
    /// At most 40 lines around the most likely cause.
    pub excerpt: String,
    /// The log id (`crash:<name>`, `game:latest.log`, ...), `null` when the
    /// instance has no log.
    pub source_log: Option<String>,
}

pub(crate) const EXCERPT_LINES: usize = 40;
const MAX_LINE_CHARS: usize = 300;

/// What the rules may look at.
pub(crate) struct Facts<'a> {
    pub text: &'a str,
    pub lines: Vec<&'a str>,
    /// Memory the instance is configured with.
    pub ram_mb: u32,
    pub total_ram_mb: u64,
    pub mc_version: &'a str,
    pub loader: Loader,
    pub java_args: &'a str,
    pub os: &'a str,
    pub offline_account: bool,
    pub mods: &'a [ModRecord],
}

impl<'a> Facts<'a> {
    pub(crate) fn new(text: &'a str, base: &Base<'a>) -> Self {
        Facts {
            text,
            lines: text.lines().collect(),
            ram_mb: base.ram_mb,
            total_ram_mb: base.total_ram_mb,
            mc_version: base.mc_version,
            loader: base.loader,
            java_args: base.java_args,
            os: base.os,
            offline_account: base.offline_account,
            mods: base.mods,
        }
    }

    /// First line (index, text) containing any of `needles`, case-insensitively.
    fn find_any(&self, needles: &[&str]) -> Option<(usize, &'a str)> {
        self.lines.iter().enumerate().find_map(|(i, l)| {
            let lower = l.to_lowercase();
            needles
                .iter()
                .any(|n| lower.contains(&n.to_lowercase()))
                .then_some((i, *l))
        })
    }
}

/// The instance facts, without the text.
pub(crate) struct Base<'a> {
    pub ram_mb: u32,
    pub total_ram_mb: u64,
    pub mc_version: &'a str,
    pub loader: Loader,
    pub java_args: &'a str,
    pub os: &'a str,
    pub offline_account: bool,
    pub mods: &'a [ModRecord],
}

struct Finding {
    detail: String,
    /// Mod ids, jar names or titles as the log names them.
    related: Vec<String>,
    /// Line of the log that shows it (the excerpt is cut around it).
    line: usize,
    confidence: Confidence,
    suggestions: Vec<String>,
}

struct Rule {
    code: &'static str,
    title: &'static str,
    run: fn(&Facts) -> Option<Finding>,
}

macro_rules! re {
    ($pat:expr) => {{
        static R: OnceLock<Option<Regex>> = OnceLock::new();
        R.get_or_init(|| Regex::new($pat).ok()).as_ref()
    }};
}

/// In order of specificity; ties in confidence keep this order.
const RULES: &[Rule] = &[
    Rule {
        code: "java_version",
        title: "The Java version does not fit",
        run: rule_java_version,
    },
    Rule {
        code: "lwjgl_natives",
        title: "The game's graphics libraries are missing",
        run: rule_lwjgl_natives,
    },
    Rule {
        code: "jvm_heap_reserve",
        title: "The game could not reserve its memory",
        run: rule_heap_reserve,
    },
    Rule {
        code: "out_of_memory",
        title: "The game ran out of memory",
        run: rule_out_of_memory,
    },
    Rule {
        code: "wrong_mc_version",
        title: "A mod is made for another Minecraft version",
        run: rule_wrong_mc_version,
    },
    Rule {
        code: "mod_dependency",
        title: "A mod is missing a dependency or needs a newer one",
        run: rule_mod_dependency,
    },
    Rule {
        code: "loader_mismatch",
        title: "A mod is made for another mod loader",
        run: rule_loader_mismatch,
    },
    Rule {
        code: "duplicate_mods",
        title: "Mods are duplicated or in conflict",
        run: rule_duplicate_mods,
    },
    Rule {
        code: "mixin_failure",
        title: "A mod could not be applied to the game",
        run: rule_mixin_failure,
    },
    Rule {
        code: "mod_entrypoint",
        title: "A mod failed while starting",
        run: rule_mod_entrypoint,
    },
    Rule {
        code: "gl_driver",
        title: "The graphics driver or OpenGL failed",
        run: rule_gl_driver,
    },
    Rule {
        code: "shader_failure",
        title: "A shader pack failed",
        run: rule_shader_failure,
    },
    Rule {
        code: "corrupt_world",
        title: "A world or chunk data is damaged",
        run: rule_corrupt_world,
    },
    Rule {
        code: "disk_full",
        title: "The disk is full",
        run: rule_disk_full,
    },
    Rule {
        code: "java_path",
        title: "The Java program cannot be started",
        run: rule_java_path,
    },
    Rule {
        code: "jvm_args",
        title: "A Java argument is not accepted",
        run: rule_jvm_args,
    },
    Rule {
        code: "account_session",
        title: "The account session is not valid",
        run: rule_account_session,
    },
];

/// The result of running every rule.
pub(crate) struct Analysis {
    pub causes: Vec<CrashCause>,
    /// Line to cut the excerpt around (the best cause's).
    pub anchor: Option<usize>,
}

/// Run all rules; most confident cause first, ties in rule order.
pub(crate) fn analyze(facts: &Facts) -> Analysis {
    let mut found: Vec<(Confidence, usize, CrashCause, usize)> = Vec::new();
    for (order, rule) in RULES.iter().enumerate() {
        if let Some(f) = (rule.run)(facts) {
            found.push((
                f.confidence,
                order,
                CrashCause {
                    code: rule.code.to_string(),
                    title: rule.title.to_string(),
                    detail: f.detail,
                    confidence: f.confidence,
                    suggestions: f.suggestions,
                    related_mods: map_related(facts.mods, &f.related),
                },
                f.line,
            ));
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let anchor = found.first().map(|f| f.3);
    Analysis {
        causes: found.into_iter().map(|f| f.2).collect(),
        anchor,
    }
}

// ---------------------------------------------------------------- helpers

fn norm(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Installed titles for ids and jar names the log mentions; what cannot be
/// matched stays as written. No duplicates, order kept.
pub(crate) fn map_related(mods: &[ModRecord], raw: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for token in raw {
        let token = token.trim().trim_matches(['\'', '"']);
        let key = norm(token);
        if key.len() < 2 {
            continue;
        }
        let hit = mods.iter().find(|m| {
            let file = m.filename.as_deref().map(mango_content::jar_name);
            let slug = m.slug.as_deref().map(norm);
            let title = m.title.as_deref().map(norm);
            let file_norm = file.as_deref().map(norm);
            file.as_deref().is_some_and(|f| f.eq_ignore_ascii_case(token))
                || slug.as_deref() == Some(key.as_str())
                || title.as_deref() == Some(key.as_str())
                || m.project_id.as_deref().map(norm).as_deref() == Some(key.as_str())
                // `sodium-fabric-0.5.8+mc1.21.jar` for the id `sodium`.
                || (key.len() >= 4
                    && file_norm
                        .as_deref()
                        .is_some_and(|f| f.starts_with(key.as_str())))
        });
        let shown = hit
            .and_then(|m| m.title.clone().or_else(|| m.filename.clone()))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| token.to_string());
        if !out.contains(&shown) {
            out.push(shown);
        }
    }
    out
}

fn java_for_class_version(class_major: u32) -> u32 {
    class_major.saturating_sub(44)
}

/// The recommended memory after `ram_mb` ran out: 50 % more in 512 MB steps,
/// at most 3/4 of the machine. `None` when that would not be more.
pub(crate) fn suggested_ram(ram_mb: u32, total_mb: u64) -> Option<u32> {
    let bumped = (u64::from(ram_mb) * 3 / 2).div_ceil(512) * 512;
    let cap = total_mb * 3 / 4 / 512 * 512;
    let pick = bumped.min(cap);
    (pick > u64::from(ram_mb)).then(|| u32::try_from(pick).unwrap_or(u32::MAX))
}

fn finding(
    line: usize,
    confidence: Confidence,
    detail: impl Into<String>,
    suggestions: Vec<String>,
    related: Vec<String>,
) -> Option<Finding> {
    Some(Finding {
        detail: detail.into(),
        related,
        line,
        confidence,
        suggestions,
    })
}

fn s(x: &str) -> String {
    x.to_string()
}

// ---------------------------------------------------------------- dependencies

/// One "Mod X needs Y" statement of a loader's error output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Dep {
    pub requester: String,
    pub target: String,
    pub expected: String,
    /// `None` = missing, else the version that is present.
    pub actual: Option<String>,
    pub line: usize,
}

/// Fabric/Quilt and Forge/NeoForge dependency errors:
///
/// `Mod 'A' (a) 1.0 requires version 0.5 or later of mod 'B' (b), which is missing!`
/// `Mod ID: 'b', Requested by: 'a', Expected range: '[0.5,)', Actual version: '[MISSING]'`
pub(crate) fn parse_deps(lines: &[&str]) -> Vec<Dep> {
    let (Some(fabric), Some(forge)) = (
        re!(
            r"Mod '([^']*)' \(([^)]*)\) \S+ requires (.+?) of (.+?), (?:which is missing|but only the wrong version is present: ([^\s!]+))"
        ),
        re!(
            r"Mod ID: '([^']+)', Requested by: '([^']+)', Expected range: '([^']*)', Actual version: '([^']*)'"
        ),
    ) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if let Some(c) = fabric.captures(line) {
            let target_text = c.get(4).map_or("", |m| m.as_str());
            // `mod 'Sodium' (sodium)` -> sodium; a bare id stays.
            let target = target_text
                .rsplit_once('(')
                .and_then(|(_, r)| r.split_once(')'))
                .map_or_else(
                    || {
                        target_text
                            .trim_start_matches("mod ")
                            .trim_matches(['\'', '"', ' '])
                            .to_string()
                    },
                    |(id, _)| id.to_string(),
                );
            out.push(Dep {
                requester: c.get(2).map_or(String::new(), |m| m.as_str().to_string()),
                target: target.to_ascii_lowercase(),
                expected: c.get(3).map_or(String::new(), |m| {
                    m.as_str().trim_start_matches("version ").to_string()
                }),
                actual: c.get(5).map(|m| m.as_str().to_string()),
                line: i,
            });
        } else if let Some(c) = forge.captures(line) {
            let actual = c.get(4).map_or("", |m| m.as_str());
            out.push(Dep {
                requester: c.get(2).map_or(String::new(), |m| m.as_str().to_string()),
                target: c
                    .get(1)
                    .map_or(String::new(), |m| m.as_str().to_ascii_lowercase()),
                expected: c.get(3).map_or(String::new(), |m| m.as_str().to_string()),
                actual: (!actual.to_ascii_uppercase().contains("MISSING"))
                    .then(|| actual.to_string()),
                line: i,
            });
        }
    }
    out
}

const LOADER_TARGETS: [&str; 7] = [
    "fabricloader",
    "fabric-loader",
    "quilt_loader",
    "quilt-loader",
    "forge",
    "neoforge",
    "fml",
];

fn dep_sentence(d: &Dep) -> String {
    let wanted = if d.expected.is_empty() || d.expected.eq_ignore_ascii_case("any version") {
        String::new()
    } else {
        format!(" {}", d.expected)
    };
    match &d.actual {
        None => format!(
            "{} needs {}{wanted}, which is missing",
            d.requester, d.target
        ),
        Some(v) => format!(
            "{} needs {}{wanted}, but version {v} is installed",
            d.requester, d.target
        ),
    }
}

/// The solutions Fabric prints under "A potential solution has been determined".
fn fabric_solutions(lines: &[&str]) -> Vec<String> {
    let Some(start) = lines
        .iter()
        .position(|l| l.contains("potential solution has been determined"))
    else {
        return Vec::new();
    };
    lines[start + 1..]
        .iter()
        .map(|l| l.trim())
        .take_while(|l| l.starts_with("- "))
        .map(|l| l.trim_start_matches("- ").trim().to_string())
        .take(5)
        .collect()
}

fn rule_mod_dependency(f: &Facts) -> Option<Finding> {
    let deps: Vec<Dep> = parse_deps(&f.lines)
        .into_iter()
        .filter(|d| d.target != "minecraft" && d.target != "java")
        .collect();
    let first = deps.first()?;
    let mut related = Vec::new();
    let mut sentences = Vec::new();
    let mut suggestions = fabric_solutions(&f.lines);
    for d in deps.iter().take(5) {
        sentences.push(dep_sentence(d));
        related.push(d.requester.clone());
        if !LOADER_TARGETS.contains(&d.target.as_str()) {
            related.push(d.target.clone());
        }
    }
    if suggestions.is_empty() {
        for d in deps.iter().take(3) {
            suggestions.push(if LOADER_TARGETS.contains(&d.target.as_str()) {
                format!("Update the mod loader ({}) of this instance", d.target)
            } else {
                format!(
                    "Install or update {} for Minecraft {} ({})",
                    d.target,
                    f.mc_version,
                    modrinth_loader(f.loader)
                )
            });
        }
    }
    suggestions.push(s("Or remove the mod that asks for it"));
    let more = deps.len().saturating_sub(5);
    let mut detail = sentences.join("; ");
    if more > 0 {
        detail.push_str(&format!("; and {more} more"));
    }
    finding(first.line, Confidence::High, detail, suggestions, related)
}

fn modrinth_loader(l: Loader) -> &'static str {
    match l {
        Loader::Vanilla => "vanilla",
        Loader::Fabric => "Fabric",
        Loader::Quilt => "Quilt",
        Loader::Forge => "Forge",
        Loader::Neoforge => "NeoForge",
    }
}

fn rule_wrong_mc_version(f: &Facts) -> Option<Finding> {
    let deps: Vec<Dep> = parse_deps(&f.lines)
        .into_iter()
        .filter(|d| d.target == "minecraft")
        .collect();
    if let Some(first) = deps.first() {
        let who: Vec<String> = deps.iter().take(5).map(|d| d.requester.clone()).collect();
        let want = if first.expected.is_empty() {
            "another Minecraft version".to_string()
        } else {
            format!("Minecraft {}", first.expected)
        };
        return finding(
            first.line,
            Confidence::High,
            format!(
                "{} needs {want}, but this instance runs Minecraft {}",
                who.join(", "),
                f.mc_version
            ),
            vec![
                format!("Get the build of the mod for Minecraft {}", f.mc_version),
                s("Or change the Minecraft version of the instance to the one the mod needs"),
            ],
            who,
        );
    }
    let re = re!(
        r"(?i)(?:was|is|were) (?:designed|built|made|compiled) for Minecraft (\d+\.\d+(?:\.\d+)?)"
    )?;
    let (i, line) = f.lines.iter().enumerate().find(|(_, l)| re.is_match(l))?;
    let want = re.captures(line)?.get(1)?.as_str().to_string();
    if want == f.mc_version {
        return None;
    }
    finding(
        i,
        Confidence::Medium,
        format!(
            "A mod was built for Minecraft {want}, but this instance runs Minecraft {}",
            f.mc_version
        ),
        vec![format!(
            "Get the build of the mod for Minecraft {}",
            f.mc_version
        )],
        Vec::new(),
    )
}

// ---------------------------------------------------------------- java

fn rule_java_version(f: &Facts) -> Option<Finding> {
    let suggestions = |need: u32| {
        vec![
            format!("Use Java {need} or newer for this instance"),
            s("Clear the Java path in the settings so the launcher picks or downloads the right Java"),
        ]
    };
    if let Some(re) = re!(
        r"class file version (\d+)\.\d+\).*?(?:versions? up to|recognizes class file versions up to) (\d+)\.\d+"
    ) {
        for (i, l) in f.lines.iter().enumerate() {
            if let Some(c) = re.captures(l) {
                let need = java_for_class_version(c[1].parse().ok()?);
                let have = java_for_class_version(c[2].parse().ok()?);
                return finding(
                    i,
                    Confidence::High,
                    format!("This needs Java {need} or newer, but Java {have} is running"),
                    suggestions(need),
                    Vec::new(),
                );
            }
        }
    }
    if let Some(re) = re!(r"(?i)requires\s+(?:at least\s+)?java\s+(?:version\s+)?(\d+)") {
        for (i, l) in f.lines.iter().enumerate() {
            if let Some(c) = re.captures(l) {
                let need: u32 = c[1].parse().ok()?;
                return finding(
                    i,
                    Confidence::High,
                    format!("This needs Java {need} or newer"),
                    suggestions(need),
                    Vec::new(),
                );
            }
        }
    }
    if let Some(d) = parse_deps(&f.lines)
        .into_iter()
        .find(|d| d.target == "java")
    {
        return finding(
            d.line,
            Confidence::High,
            format!("{} needs Java {}", d.requester, d.expected),
            suggestions(
                d.expected
                    .chars()
                    .filter(char::is_ascii_digit)
                    .collect::<String>()
                    .parse()
                    .unwrap_or(21),
            ),
            vec![d.requester],
        );
    }
    let re = re!(r"Unsupported class file major version (\d+)")?;
    let (i, l) = f.lines.iter().enumerate().find(|(_, l)| re.is_match(l))?;
    let major: u32 = re.captures(l)?[1].parse().ok()?;
    finding(
        i,
        Confidence::Medium,
        format!(
            "A mod or the game contains code for Java {} that the running Java or a mod loader library cannot read",
            java_for_class_version(major)
        ),
        vec![
            format!("Use Java {} or newer", java_for_class_version(major)),
            s("Update the mod loader and the mods"),
        ],
        Vec::new(),
    )
}

fn rule_heap_reserve(f: &Facts) -> Option<Finding> {
    let (i, _) = f.find_any(&[
        "Could not reserve enough space for",
        "Invalid maximum heap size",
        "The specified size exceeds the maximum representable size",
        "Initial heap size set to a larger value than the maximum heap size",
        // HotSpot on Linux when the machine cannot back the heap.
        "There is insufficient memory for the Java Runtime Environment to continue",
    ])?;
    // `Native memory allocation (mmap) failed to map 858993459200 bytes`
    let asked_mb = re!(r"failed to map (\d+) bytes")
        .and_then(|re| re.captures(f.text))
        .and_then(|c| c[1].parse::<u64>().ok())
        .map(|b| b / (1024 * 1024))
        .filter(|mb| *mb > 0);
    let wanted = asked_mb.unwrap_or_else(|| u64::from(f.ram_mb));
    let free = if f.total_ram_mb > 0 {
        format!(" This computer has {} MB in total.", f.total_ram_mb)
    } else {
        String::new()
    };
    finding(
        i,
        Confidence::High,
        format!(
            "Java could not set aside the requested {wanted} MB.{free}"
        ),
        vec![
            s("Lower the memory of this instance (a 32-bit Java cannot use more than about 1.5 GB)"),
            s("Close other programs, or restart the computer, to free memory"),
            s("Use a 64-bit Java"),
        ],
        Vec::new(),
    )
}

fn rule_out_of_memory(f: &Facts) -> Option<Finding> {
    if let Some((i, l)) = f.find_any(&[
        "OutOfMemoryError: Java heap space",
        "OutOfMemoryError: GC overhead limit exceeded",
        "Requested array size exceeds VM limit",
        "OutOfMemoryError: Out of Memory Error",
    ]) {
        // `-Xmx` in the Java arguments comes after the launcher's own and wins.
        let own_xmx = re!(r"-Xmx(\d+[kKmMgG]?)")
            .and_then(|re| re.captures_iter(f.java_args).last())
            .map(|c| c[1].to_string());
        let advice = match suggested_ram(f.ram_mb, f.total_ram_mb) {
            _ if own_xmx.is_some() => format!(
                "The Java arguments set -Xmx{}, which overrides the memory of the instance: remove it or raise it",
                own_xmx.as_deref().unwrap_or_default()
            ),
            Some(r) => format!(
                "Raise the memory of this instance from {} MB to about {r} MB",
                f.ram_mb
            ),
            None => format!(
                "The instance already has {} MB, about as much as this computer can spare; \
                 close other programs or play with fewer mods",
                f.ram_mb
            ),
        };
        let _ = l;
        return finding(
            i,
            Confidence::High,
            match &own_xmx {
                Some(x) => format!("The game used up the -Xmx{x} of its Java arguments"),
                None => format!("The game used up its {} MB of memory", f.ram_mb),
            },
            vec![
                advice,
                s("Lower the render distance"),
                s("Remove heavy mods, high-resolution resource packs or shaders"),
            ],
            Vec::new(),
        );
    }
    if let Some((i, _)) = f.find_any(&[
        "OutOfMemoryError: Metaspace",
        "OutOfMemoryError: Compressed class space",
    ]) {
        return finding(
            i,
            Confidence::High,
            "Java ran out of space for class data (too many or too large mods)",
            vec![
                s("Add -XX:MaxMetaspaceSize=512m to the Java arguments"),
                s("Remove mods you do not need"),
            ],
            Vec::new(),
        );
    }
    let (i, _) = f.find_any(&[
        "unable to create native thread",
        "unable to create new native thread",
    ])?;
    finding(
        i,
        Confidence::Medium,
        "Java could not start another thread: the system is out of memory or thread slots",
        vec![
            s("Close other programs"),
            match suggested_ram(f.ram_mb, f.total_ram_mb) {
                Some(_) => {
                    s("Lower the memory of this instance a little; thread stacks live outside it")
                }
                None => s("Play with fewer mods"),
            },
        ],
        Vec::new(),
    )
}

fn rule_java_path(f: &Facts) -> Option<Finding> {
    if let Some(re) =
        re!(r#"Cannot run program "([^"]+)".*(?:error=2|No such file|not found|cannot find)"#)
    {
        for (i, l) in f.lines.iter().enumerate() {
            if let Some(c) = re.captures(l) {
                return finding(
                    i,
                    Confidence::High,
                    format!("The Java program {} does not exist", &c[1]),
                    vec![
                        s("Clear the Java path in the settings to let the launcher manage Java"),
                        s("Or choose a Java that is installed"),
                    ],
                    Vec::new(),
                );
            }
        }
    }
    let (i, _) = f.find_any(&[
        "'java' is not recognized",
        "could not open `",
        "Invalid or corrupt jarfile",
        "java: command not found",
        "JAVA_HOME is not set",
    ])?;
    finding(
        i,
        Confidence::High,
        "The configured Java cannot be started or is damaged",
        vec![
            s("Clear the Java path in the settings to let the launcher manage Java"),
            s("Reinstall the Java you chose"),
        ],
        Vec::new(),
    )
}

fn rule_jvm_args(f: &Facts) -> Option<Finding> {
    let re = re!(
        r"(?i)(?:Unrecognized (?:VM )?option:? '?([^'\s]+)|Improperly specified VM option '([^']+)'|Invalid VM option '?([^'\s]+))"
    )?;
    let Some((i, l)) = f.lines.iter().enumerate().find(|(_, l)| re.is_match(l)) else {
        return rule_jvm_conflict(f);
    };
    let c = re.captures(l)?;
    let arg = (1..=3).find_map(|n| c.get(n)).map_or("", |m| m.as_str());
    let own = if f.java_args.contains(arg) && !arg.is_empty() {
        " It is in this instance's Java arguments."
    } else {
        ""
    };
    finding(
        i,
        Confidence::High,
        format!("Java does not accept the option {arg}.{own}"),
        vec![
            format!("Remove {arg} from the Java arguments of the instance and of the settings"),
            s("Options for another Java version (for example -XX:+UseConcMarkSweepGC on Java 17+) are not accepted"),
        ],
        Vec::new(),
    )
}

/// `SurvivorRatio (32) must be less than or equal to ergonomic SurvivorRatio
/// maximum (16)` / `Error: Could not create the Java Virtual Machine.`: the
/// JVM refuses a combination of options (typically a small `-Xmx` next to
/// the launcher's G1 tuning); the reason is the line before the error.
fn rule_jvm_conflict(f: &Facts) -> Option<Finding> {
    let (i, _) = f.find_any(&["Could not create the Java Virtual Machine"])?;
    let reason = f.lines[..i]
        .iter()
        .rev()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .map(|l| l.chars().take(160).collect::<String>())
        .unwrap_or_default();
    // Without a stated reason this says nothing about the cause.
    if reason.is_empty() || reason.starts_with("Error:") {
        return None;
    }
    let detail = format!("Java refused to start with the configured options: {reason}");
    finding(
        i,
        Confidence::High,
        detail,
        vec![
            s("Raise the memory of the instance (very small values do not work with the G1 settings)"),
            s("Remove custom Java arguments from the instance and the settings, then add them back one by one"),
        ],
        Vec::new(),
    )
}

// ---------------------------------------------------------------- mods

fn rule_loader_mismatch(f: &Facts) -> Option<Finding> {
    let loader = modrinth_loader(f.loader);
    if let Some(re) = re!(
        r"(?i)(?P<jar>[\w.+\-() ]+\.jar)\W.{0,60}?(?:is missing|does not contain|has no|no valid|could not find)\W.{0,20}?(?P<what>fabric\.mod\.json|quilt\.mod\.json|mods\.toml)"
    ) {
        for (i, l) in f.lines.iter().enumerate() {
            let Some(c) = re.captures(l) else { continue };
            let what = c["what"].to_ascii_lowercase();
            let theirs = if what.starts_with("fabric") || what.starts_with("quilt") {
                // They look for the Fabric/Quilt descriptor, so this is no Fabric mod.
                "Forge or NeoForge"
            } else {
                "Fabric or Quilt"
            };
            let jar = c["jar"].trim().to_string();
            return finding(
                i,
                Confidence::High,
                format!("{jar} is made for {theirs}, but this instance uses {loader}"),
                vec![
                    format!("Remove {jar}, or get its {loader} build"),
                    s("Mods only work with the loader they were made for"),
                ],
                vec![jar],
            );
        }
    }
    if let Some(re) = re!(
        r"(?i)(?:Mod )?file (?P<jar>\S+\.jar) is missing (?:the )?(?:META-INF/)?(?:neoforge\.)?mods\.toml"
    ) {
        for (i, l) in f.lines.iter().enumerate() {
            if let Some(c) = re.captures(l) {
                let jar = c["jar"]
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or("")
                    .to_string();
                return finding(
                    i,
                    Confidence::High,
                    format!("{jar} is not a {loader} mod (it has no mods.toml); it is probably made for Fabric or Quilt"),
                    vec![
                        format!("Remove {jar}, or get its {loader} build"),
                        s("Mods only work with the loader they were made for"),
                    ],
                    vec![jar],
                );
            }
        }
    }
    // No explicit message: a failed mod load plus mods built for other loaders.
    let (i, _) = f.find_any(&[
        "Incompatible mod set",
        "Mod resolution failed",
        "ModResolutionException",
        "Mod loading has failed",
        "LoadingFailedException",
    ])?;
    if f.loader == Loader::Vanilla {
        return None;
    }
    let ours = match f.loader {
        Loader::Fabric => "fabric",
        Loader::Quilt => "quilt",
        Loader::Forge => "forge",
        Loader::Neoforge => "neoforge",
        Loader::Vanilla => "vanilla",
    };
    let wrong: Vec<String> = f
        .mods
        .iter()
        .filter(|m| m.enabled != Some(false))
        .filter(|m| {
            m.loaders
                .as_ref()
                .is_some_and(|l| !l.is_empty() && !crate::content::loader_accepts(ours, l))
        })
        .map(|m| {
            m.title
                .clone()
                .or_else(|| m.filename.clone())
                .unwrap_or_default()
        })
        .filter(|t| !t.is_empty())
        .collect();
    if wrong.is_empty() {
        return None;
    }
    finding(
        i,
        Confidence::Medium,
        format!(
            "{} {} built for another mod loader than {loader}",
            wrong.iter().take(5).cloned().collect::<Vec<_>>().join(", "),
            if wrong.len() == 1 { "is" } else { "are" }
        ),
        vec![
            s("Remove them, or get their builds for this loader"),
            s("Mods only work with the loader they were made for"),
        ],
        wrong.into_iter().take(5).collect(),
    )
}

fn quoted_ids(text: &str) -> Vec<String> {
    let Some(re) = re!(r#"['"`]([\w.\-]+)['"`]"#) else {
        return Vec::new();
    };
    re.captures_iter(text).map(|c| c[1].to_string()).collect()
}

fn jar_names(text: &str) -> Vec<String> {
    let Some(re) = re!(r"([\w.+\-]+\.jar)") else {
        return Vec::new();
    };
    re.captures_iter(text).map(|c| c[1].to_string()).collect()
}

fn rule_duplicate_mods(f: &Facts) -> Option<Finding> {
    let dup = re!(r"(?i)\bduplicate (?:mods?|mod ids?)\b[:\s]*(.*)")?;
    for (i, l) in f.lines.iter().enumerate() {
        let Some(c) = dup.captures(l) else { continue };
        let tail = c[1].to_string();
        let mut ids = quoted_ids(&tail);
        if ids.is_empty() {
            let word = tail
                .split(|ch: char| !(ch.is_alphanumeric() || ch == '_' || ch == '-' || ch == '.'))
                .find(|w| !w.is_empty() && !w.eq_ignore_ascii_case("found"));
            ids.extend(word.map(str::to_string));
        }
        // Jar names on this and the next few lines.
        let nearby = f.lines[i..f.lines.len().min(i + 4)].join("\n");
        let jars = jar_names(&nearby);
        let mut related = ids.clone();
        related.extend(jars.clone());
        let what = ids.first().cloned().unwrap_or_else(|| s("a mod"));
        let files = if jars.is_empty() {
            String::new()
        } else {
            format!(" ({})", jars.join(", "))
        };
        return finding(
            i,
            Confidence::High,
            format!("{what} is installed more than once{files}"),
            vec![
                s("Keep only one file of each mod in the mods list"),
                s("Check for an old copy you added by hand next to the installed one"),
            ],
            related,
        );
    }
    if let Some(found) = rule_unique_id(f) {
        return Some(found);
    }
    let conflict = re!(
        r"(?i)Mod '([^']+)' \(([^)]+)\) \S+ (?:is incompatible with|conflicts with|breaks) (.+?)(?:, but|,? which is|!|$)"
    )?;
    let (i, c) = f
        .lines
        .iter()
        .enumerate()
        .find_map(|(i, l)| conflict.captures(l).map(|c| (i, c)))?;
    let other = c[3].to_string();
    let mut related = vec![c[2].to_string()];
    related.extend(quoted_ids(&other));
    related.extend(
        other
            .rsplit_once('(')
            .and_then(|(_, r)| r.split_once(')'))
            .map(|(id, _)| id.to_string()),
    );
    finding(
        i,
        Confidence::High,
        format!("{} cannot be used together with {}", &c[1], other.trim()),
        vec![s("Remove one of the two mods")],
        related,
    )
}

/// Fabric's solver output when two mods claim one id (also through
/// `provides`): `Immediate reason: [ROOT_FORCELOAD_SINGLE a 1.0, UNIQUE_ID a,
/// ROOT_FORCELOAD_SINGLE b 1.0]` and, in the error, `Remove mod 'A' (a) 1.0
/// (/path/a-1.0.jar).` (the "More details" list stays empty).
fn rule_unique_id(f: &Facts) -> Option<Finding> {
    let (i, l) = f
        .lines
        .iter()
        .enumerate()
        .find(|(_, l)| l.contains("UNIQUE_ID"))?;
    let mods: Vec<String> = re!(r"ROOT_FORCELOAD_SINGLE ([\w.\-]+)")
        .map(|re| re.captures_iter(l).map(|c| c[1].to_string()).collect())
        .unwrap_or_default();
    let mut related = mods.clone();
    let jars: Vec<String> = f
        .lines
        .iter()
        .filter(|l| l.contains("Remove mod '"))
        .flat_map(|l| jar_names(l))
        .collect();
    related.extend(jars.iter().cloned());
    let what = if mods.is_empty() {
        s("Two mods")
    } else {
        format!("{} use", mods.join(" and "))
    };
    let detail = format!(
        "{what} the same mod id, so the loader cannot load both{}",
        if jars.is_empty() {
            String::new()
        } else {
            format!(" (the loader suggests removing {})", jars.join(", "))
        }
    );
    finding(
        i,
        Confidence::High,
        detail,
        vec![
            s("Remove one of the two mods"),
            s("Check for an old copy you added by hand next to the installed one"),
        ],
        related,
    )
}

/// `sodium.mixins.json` -> `sodium`, `mixins.create.json` -> `create`.
fn mod_of_mixin_config(name: &str) -> Option<String> {
    let re = re!(
        r"(?i)^(?:mixins?[.\-_])?([\w\-]+?)(?:[.\-_]mixins?)?(?:[.\-_](?:common|fabric|forge|neoforge|quilt|client|core))?\.json$"
    )?;
    let id = re.captures(name)?[1].to_string();
    (id.len() >= 2 && !id.eq_ignore_ascii_case("mixins")).then_some(id)
}

fn rule_mixin_failure(f: &Facts) -> Option<Finding> {
    let (i, _) = f.find_any(&[
        "Mixin apply failed",
        "Critical injection failure",
        "InjectionError",
        "MixinApplyError",
        "Mixin transformation of",
        "MixinTransformerError",
        "Mixin prepare failed",
        "Mixin prepare for mod",
    ])?;
    // Look for the config and the mod around the first failure.
    let window = f.lines[i.saturating_sub(2)..f.lines.len().min(i + 12)].join("\n");
    let from_mod = re!(r"from mod ([\w.\-]+)")
        .and_then(|r| r.captures(&window))
        .map(|c| c[1].to_string());
    let config = re!(r"([\w.\-]+\.json):[\w.$]*").and_then(|r| {
        r.captures_iter(&window)
            .map(|c| c[1].to_string())
            .find(|n| n.to_lowercase().contains("mixin"))
    });
    let modid = from_mod.or_else(|| config.as_deref().and_then(mod_of_mixin_config));
    let (confidence, detail, suggestions, related) = match &modid {
        Some(m) => (
            Confidence::High,
            format!(
                "The mod {m} could not change the game code it targets (a Mixin failed){}",
                config
                    .as_deref()
                    .map(|c| format!(", config {c}"))
                    .unwrap_or_default()
            ),
            vec![
                format!("Update {m}, or install the build made for Minecraft {} and {}", f.mc_version, modrinth_loader(f.loader)),
                s("Another mod that changes the same code may conflict: remove recently added mods"),
                format!("As a test, disable {m}"),
            ],
            vec![m.clone()],
        ),
        None => (
            Confidence::Medium,
            s("A mod could not change the game code it targets (a Mixin failed), but the log does not say which"),
            vec![
                s("Remove the mods you added last"),
                s("Update all mods and the loader"),
            ],
            Vec::new(),
        ),
    };
    finding(i, confidence, detail, suggestions, related)
}

// ---------------------------------------------------------------- graphics, shaders

/// Fabric: `Could not execute entrypoint stage 'main' due to errors, provided
/// by 'badentry' at 'does.not.Exist'!`, with the root cause further down
/// (`Caused by: java.lang.ClassNotFoundException: does.not.Exist`).
fn rule_mod_entrypoint(f: &Facts) -> Option<Finding> {
    let re = re!(
        r"Could not execute entrypoint stage '([^']+)' due to errors, provided by '([^']+)'(?: at '([^']+)')?"
    )?;
    let (i, c) = f
        .lines
        .iter()
        .enumerate()
        .find_map(|(i, l)| re.captures(l).map(|c| (i, c)))?;
    let (stage, modid) = (c[1].to_string(), c[2].to_string());
    let class = c.get(3).map(|m| m.as_str().to_string());
    let missing_class = f.lines[i..]
        .iter()
        .take(40)
        .any(|l| l.contains("ClassNotFoundException") || l.contains("NoClassDefFoundError"));
    let detail = match (&class, missing_class) {
        (Some(cl), true) => {
            format!("The mod {modid} points at the class {cl} for its {stage} entrypoint, but that class does not exist in it")
        }
        _ => format!(
            "The mod {modid} threw an error in its {stage} entrypoint while the game was starting"
        ),
    };
    finding(
        i,
        Confidence::High,
        detail,
        vec![
            format!(
                "Update {modid}, or install the build made for Minecraft {} and {}",
                f.mc_version,
                modrinth_loader(f.loader)
            ),
            format!("As a test, disable {modid}"),
            s("Tell the mod's author if it keeps happening, with the crash report"),
        ],
        vec![modid],
    )
}

fn rule_gl_driver(f: &Facts) -> Option<Finding> {
    let (i, _) = f.find_any(&[
        "GLFW error 65542",
        "GLFW error 65543",
        "GLFW error 65544",
        "GLFW_VERSION_UNAVAILABLE",
        "WGL: ",
        "GLXBadFBConfig",
        "Pixel format not accelerated",
        "The driver does not appear to support OpenGL",
        "Failed to find a suitable pixel format",
        "OpenGL 3.2 is not supported",
        "Failed to create the GLFW window",
    ])?;
    let mut suggestions = vec![s("Update the graphics driver")];
    match f.os {
        "windows" => {
            suggestions.push(s(
                "Install the driver from NVIDIA, AMD or Intel directly, not through Windows Update",
            ));
            suggestions.push(s("Do not run the game through Remote Desktop or in a virtual machine without 3D acceleration"));
        }
        "linux" => {
            suggestions.push(s(
                "Install or update Mesa (open drivers) or the proprietary NVIDIA driver",
            ));
            suggestions.push(s("Check with `glxinfo | grep OpenGL` that a driver with OpenGL 3.3 or newer is active"));
        }
        "macos" => suggestions.push(s("Update macOS; very old Macs are limited to OpenGL 2.1")),
        _ => {}
    }
    suggestions.push(s(
        "On a laptop with two graphics chips, run the game on the dedicated one",
    ));
    finding(
        i,
        Confidence::High,
        "The window or the OpenGL context could not be created: the graphics driver is missing, too old or not accelerated",
        suggestions,
        Vec::new(),
    )
}

fn rule_shader_failure(f: &Facts) -> Option<Finding> {
    let explicit = f.find_any(&[
        "Failed to compile shader",
        "Shader compilation failed",
        "Could not compile shader",
        "ShaderCompileException",
        "ProgramLinkException",
        "Failed to create shader program",
        "Shader compile error",
    ]);
    let (i, confidence) = match explicit {
        Some((i, _)) => (i, Confidence::High),
        None => {
            // Iris/Oculus/OptiFine problems that are not about one shader.
            let re =
                re!(r"(?i)(?:iris|oculus|optifine|shaderpack).*(?:exception|failed to|could not)")?;
            let (i, _) = f.lines.iter().enumerate().find(|(_, l)| re.is_match(l))?;
            (i, Confidence::Medium)
        }
    };
    let window = f.lines[i.saturating_sub(5)..f.lines.len().min(i + 15)].join("\n");
    let pack = re!(r"shaderpacks[/\\]([^/\\\s:]+)")
        .and_then(|r| r.captures(&window))
        .map(|c| c[1].to_string());
    let mut related = Vec::new();
    let lower = f.text.to_lowercase();
    for id in [
        "iris",
        "oculus",
        "optifine",
        "sodium",
        "embeddium",
        "rubidium",
    ] {
        if lower.contains(id) {
            related.push(id.to_string());
        }
    }
    finding(
        i,
        confidence,
        match &pack {
            Some(p) => format!("The shader pack {p} failed to compile or load"),
            None => s("A shader pack failed to compile or load"),
        },
        vec![
            s("Select \"Off\" in the shader settings, or remove the shader pack"),
            s("Check that the shader pack supports this Minecraft version and your graphics card"),
            s("Update the shader mod (Iris, Oculus) together with Sodium or Embeddium"),
        ],
        related,
    )
}

// ---------------------------------------------------------------- world, disk, account

fn rule_corrupt_world(f: &Facts) -> Option<Finding> {
    let level = f.lines.iter().enumerate().find(|(_, l)| {
        let low = l.to_lowercase();
        low.contains("level.dat")
            && (low.contains("corrupt")
                || low.contains("failed to read")
                || low.contains("not in gzip format")
                || low.contains("eofexception")
                || low.contains("unexpected end"))
    });
    let chunk = f.find_any(&[
        "Unexpected end of ZLIB input stream",
        "Failed to read chunk",
        "Error reading chunk",
        "Corrupted chunk",
        "Exception reading r.",
        "Chunk file at",
        "RegionFile error",
    ]);
    let (i, confidence, what) = match (level, chunk) {
        (Some((i, _)), _) => (i, Confidence::High, "the world's level.dat"),
        (None, Some((i, _))) => (i, Confidence::Medium, "a chunk of the world"),
        _ => return None,
    };
    let window = f.lines[i.saturating_sub(5)..f.lines.len().min(i + 10)].join("\n");
    let world = re!(r"saves[/\\]([^/\\\s:]+)")
        .and_then(|r| r.captures(&window))
        .map(|c| format!(" ({})", &c[1]))
        .unwrap_or_default();
    finding(
        i,
        confidence,
        format!("Reading {what}{world} failed: the data is damaged"),
        vec![
            s("Restore the world from a backup (Backups tab of the instance)"),
            s("Open the world folder and move the world away to test whether it is the cause"),
        ],
        Vec::new(),
    )
}

fn rule_lwjgl_natives(f: &Facts) -> Option<Finding> {
    let re = re!(r"UnsatisfiedLinkError: Failed to locate library: (?:lib)?lwjgl")?;
    let i = f.lines.iter().position(|l| re.is_match(l))?;
    finding(
        i,
        Confidence::High,
        "The LWJGL native libraries are missing, so the game cannot start its graphics and input layer",
        vec![
            s("Repair or reinstall the game files of this instance's Minecraft version, then start again"),
            s("Delete the instance's natives folder in the launcher data directory so it is extracted again"),
        ],
        Vec::new(),
    )
}

fn rule_disk_full(f: &Facts) -> Option<Finding> {
    let (i, _) = f.find_any(&[
        "No space left on device",
        "There is not enough space on the disk",
        "Disk quota exceeded",
        "IOException: Not enough space",
    ])?;
    finding(
        i,
        Confidence::High,
        "The disk the game writes to is full",
        vec![
            s("Free some disk space"),
            s("Clear the launcher's caches in the storage settings"),
            s("Delete old backups, logs and unused instances"),
        ],
        Vec::new(),
    )
}

fn rule_account_session(f: &Facts) -> Option<Finding> {
    let (i, _) = f.find_any(&[
        "Invalid session",
        "Failed to verify username",
        "InvalidCredentialsException",
        "Authentication servers are down",
        "Not authenticated with Minecraft.net",
        "Multiplayer is disabled",
        "The authentication servers are currently not reachable",
        "Bad login",
    ])?;
    let mut suggestions = Vec::new();
    let detail = if f.offline_account {
        suggestions.push(s("Offline accounts cannot join servers that check accounts (online mode); use a Microsoft account"));
        "The server or Minecraft rejected the account session. The selected account is an offline account"
    } else {
        suggestions.push(s("Sign in with the Microsoft account again"));
        suggestions.push(s("Check the date and time of the computer"));
        "The server or Minecraft rejected the account session"
    };
    suggestions.push(s(
        "Restart the game; if Mojang's servers are down, try again later",
    ));
    finding(i, Confidence::High, detail, suggestions, Vec::new())
}

// ---------------------------------------------------------------- report parts

/// Java and Minecraft versions the log states about itself.
pub(crate) fn log_versions(text: &str) -> (Option<String>, Option<String>) {
    let java = re!(r#"(?im)^\s*Java Version:\s*([\w.+_\-]+)"#)
        .and_then(|r| r.captures(text))
        .or_else(|| re!(r#"(?i)\bjava version "?([\d._]+)"#).and_then(|r| r.captures(text)))
        .or_else(|| re!(r"(?i)\bJava is [^\n]*?, version ([\d._]+)").and_then(|r| r.captures(text)))
        .map(|c| c[1].to_string());
    let mc = re!(r"(?im)^\s*Minecraft Version:\s*(\S+)")
        .and_then(|r| r.captures(text))
        .or_else(|| re!(r"Loading Minecraft (\S+) with").and_then(|r| r.captures(text)))
        .or_else(|| re!(r"(?im)Minecraft Version ID:\s*(\S+)").and_then(|r| r.captures(text)))
        .map(|c| c[1].to_string());
    (java, mc)
}

/// The "Description:" line of a crash report.
pub(crate) fn crash_description(text: &str) -> Option<String> {
    re!(r"(?m)^Description:\s*(.+)$")
        .and_then(|r| r.captures(text))
        .map(|c| c[1].trim().to_string())
        .filter(|d| !d.is_empty())
}

/// At most [`EXCERPT_LINES`] lines: starting a few lines before `anchor`, else
/// the first error-looking line, else the end of the log. Long lines are cut.
pub(crate) fn excerpt(lines: &[&str], anchor: Option<usize>, is_crash_report: bool) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let start = match anchor {
        Some(a) => a.saturating_sub(4),
        None if is_crash_report => 0,
        None => lines
            .iter()
            .position(|l| l.contains("/ERROR]") || l.contains("/FATAL]") || l.contains("Exception"))
            .map_or(lines.len().saturating_sub(EXCERPT_LINES), |p| {
                p.saturating_sub(2)
            }),
    };
    let end = lines.len().min(start + EXCERPT_LINES);
    lines[start..end]
        .iter()
        .map(|l| {
            if l.chars().count() > MAX_LINE_CHARS {
                format!("{}...", l.chars().take(MAX_LINE_CHARS).collect::<String>())
            } else {
                (*l).to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn summary(causes: &[CrashCause], description: Option<&str>, empty: bool) -> String {
    if empty {
        return s("There is no log to analyze yet");
    }
    match causes.first() {
        Some(top) => {
            let more = causes.len() - 1;
            if more == 0 {
                top.title.clone()
            } else {
                format!(
                    "{} (and {more} other possible cause{})",
                    top.title,
                    if more == 1 { "" } else { "s" }
                )
            }
        }
        None => match description {
            Some(d) => format!("The game crashed: {d}. No known cause was found"),
            None => s("No known cause was found in the log"),
        },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn base<'a>(mods: &'a [ModRecord]) -> Base<'a> {
        Base {
            ram_mb: 2048,
            total_ram_mb: 16384,
            mc_version: "1.20.1",
            loader: Loader::Fabric,
            java_args: "-XX:+UseG1GC",
            os: "linux",
            offline_account: false,
            mods,
        }
    }

    fn run(code: &str, log: &str) -> Option<CrashCause> {
        run_with(code, log, &[], |_| {})
    }

    fn run_with(
        code: &str,
        log: &str,
        mods: &[ModRecord],
        tweak: impl Fn(&mut Base),
    ) -> Option<CrashCause> {
        let mut b = base(mods);
        tweak(&mut b);
        let facts = Facts::new(log, &b);
        analyze(&facts).causes.into_iter().find(|c| c.code == code)
    }

    fn record(title: &str, slug: &str, file: &str) -> ModRecord {
        ModRecord {
            title: Some(title.into()),
            slug: Some(slug.into()),
            filename: Some(file.into()),
            ..ModRecord::default()
        }
    }

    #[test]
    fn java_too_old_from_the_class_version_error() {
        let log = "[main/ERROR]: Unable to launch\n\
            java.lang.UnsupportedClassVersionError: net/minecraft/client/main/Main has been compiled by a more recent version of the Java Runtime (class file version 65.0), this version of the Java Runtime only recognizes class file versions up to 61.0\n\
            \tat java.base/java.lang.ClassLoader.defineClass1(Native Method)";
        let c = run("java_version", log).unwrap();
        assert_eq!(c.confidence, Confidence::High);
        assert!(
            c.detail.contains("Java 21") && c.detail.contains("Java 17"),
            "{}",
            c.detail
        );
        assert!(c.suggestions[0].contains("21"));
    }

    #[test]
    fn java_required_by_the_game_or_by_a_mod() {
        let c = run(
            "java_version",
            "Caused by: java.lang.RuntimeException: Minecraft 1.20.5 requires Java 21 or newer",
        )
        .unwrap();
        assert!(c.detail.contains("21"));
        let fabric = "net.fabricmc.loader.impl.FormattedException: Some of your mods are incompatible with the game or each other!\n\
            \t - Mod 'Lithium' (lithium) 0.12.0 requires version 21 or later of java, but only the wrong version is present: 17!";
        let c = run("java_version", fabric).unwrap();
        assert!(
            c.detail.contains("lithium") && c.detail.contains("21"),
            "{}",
            c.detail
        );
        // ...and it is not also reported as a dependency problem.
        assert!(run("mod_dependency", fabric).is_none());
        let asm = "java.lang.IllegalArgumentException: Unsupported class file major version 65\n\tat org.objectweb.asm.ClassReader.<init>";
        assert_eq!(
            run("java_version", asm).unwrap().confidence,
            Confidence::Medium
        );
        assert!(run("java_version", "Java version 17 is fine").is_none());
    }

    #[test]
    fn out_of_memory_suggests_more_memory_from_the_current_value() {
        let log = "---- Minecraft Crash Report ----\nDescription: Exception in server tick loop\n\njava.lang.OutOfMemoryError: Java heap space\n\tat net.minecraft.class_2818.method_12202";
        let c = run("out_of_memory", log).unwrap();
        assert_eq!(c.confidence, Confidence::High);
        assert!(
            c.suggestions[0].contains("2048 MB") && c.suggestions[0].contains("3072 MB"),
            "{:?}",
            c.suggestions
        );
        // At the ceiling the advice changes.
        let c = run_with("out_of_memory", log, &[], |b| {
            b.ram_mb = 12288;
            b.total_ram_mb = 16384;
        })
        .unwrap();
        assert!(
            c.suggestions[0].contains("already has 12288 MB"),
            "{:?}",
            c.suggestions
        );
        let meta = run("out_of_memory", "java.lang.OutOfMemoryError: Metaspace").unwrap();
        assert!(meta.suggestions[0].contains("Metaspace"));
        let thread = run(
            "out_of_memory",
            "java.lang.OutOfMemoryError: unable to create native thread: possibly out of memory",
        )
        .unwrap();
        assert_eq!(thread.confidence, Confidence::Medium);
    }

    #[test]
    fn suggested_ram_is_rounded_capped_and_absent_when_not_more() {
        assert_eq!(suggested_ram(2048, 16384), Some(3072));
        assert_eq!(suggested_ram(3000, 16384), Some(4608));
        assert_eq!(suggested_ram(6144, 10240), Some(7680));
        assert_eq!(suggested_ram(8192, 10240), None);
        assert_eq!(suggested_ram(8192, 8192), None);
    }

    #[test]
    fn missing_fabric_dependency_names_both_mods_and_takes_the_fix() {
        let log = "net.fabricmc.loader.impl.FormattedException: Some of your mods are incompatible with the game or each other!\n\
            A potential solution has been determined, this may resolve your problem:\n\
            \t - Install sodium, version 0.5.0 or later.\n\
            Unmet dependency listing:\n\
            \t - Mod 'Sodium Extra' (sodium-extra) 0.5.4+mc1.20.4 requires version 0.5.0 or later of mod 'Sodium' (sodium), which is missing!\n\
            \t - Mod 'Mod Menu' (modmenu) 7.2.2 requires any version of mod 'Fabric API' (fabric-api), which is missing!\n\
            \tat net.fabricmc.loader.impl.FabricLoaderImpl.load";
        let mods = [record(
            "Sodium Extra",
            "sodium-extra",
            "sodium-extra-0.5.4.jar",
        )];
        let c = run_with("mod_dependency", log, &mods, |_| {}).unwrap();
        assert_eq!(c.confidence, Confidence::High);
        assert!(
            c.detail
                .contains("sodium-extra needs sodium 0.5.0 or later, which is missing"),
            "{}",
            c.detail
        );
        assert!(c.detail.contains("modmenu needs fabric-api"));
        assert_eq!(c.suggestions[0], "Install sodium, version 0.5.0 or later.");
        assert!(
            c.related_mods.contains(&"Sodium Extra".to_string()),
            "{:?}",
            c.related_mods
        );
        assert!(c.related_mods.contains(&"fabric-api".to_string()));
    }

    #[test]
    fn too_old_dependency_and_forge_style_messages() {
        let fabric = "\t - Mod 'Iris' (iris) 1.7.0 requires version 0.5.8 or later of mod 'Sodium' (sodium), but only the wrong version is present: 0.5.3+mc1.20.4!";
        let c = run("mod_dependency", fabric).unwrap();
        assert!(
            c.detail.contains("version 0.5.3+mc1.20.4 is installed"),
            "{}",
            c.detail
        );
        let forge = "Missing or unsupported mandatory dependencies:\n\
            \tMod ID: 'architectury', Requested by: 'create_things', Expected range: '[9.0.0,)', Actual version: '[MISSING]'\n\
            \tMod ID: 'geckolib', Requested by: 'mobs', Expected range: '[4.4,)', Actual version: '4.2.1'";
        let c = run_with("mod_dependency", forge, &[], |b| b.loader = Loader::Forge).unwrap();
        assert!(
            c.detail
                .contains("create_things needs architectury [9.0.0,), which is missing"),
            "{}",
            c.detail
        );
        assert!(c
            .detail
            .contains("mobs needs geckolib [4.4,), but version 4.2.1 is installed"));
        // A loader as the target asks for a loader update.
        let loader = "\t - Mod 'Fabric API' (fabric-api) 0.91 requires version 0.15.0 or later of fabricloader, but only the wrong version is present: 0.14.21!";
        let c = run("mod_dependency", loader).unwrap();
        assert!(
            c.suggestions[0].contains("Update the mod loader"),
            "{:?}",
            c.suggestions
        );
        assert!(!c.related_mods.contains(&"fabricloader".to_string()));
    }

    #[test]
    fn mod_for_another_minecraft_version() {
        let fabric = "\t - Mod 'Sodium' (sodium) 0.5.3 requires version 1.20.4 of minecraft, but only the wrong version is present: 1.20.1!";
        let c = run("wrong_mc_version", fabric).unwrap();
        assert_eq!(c.confidence, Confidence::High);
        assert!(
            c.detail.contains("sodium needs Minecraft 1.20.4")
                && c.detail.contains("runs Minecraft 1.20.1"),
            "{}",
            c.detail
        );
        assert!(run("mod_dependency", fabric).is_none());
        let forge = "\tMod ID: 'minecraft', Requested by: 'create', Expected range: '[1.20.2,1.21)', Actual version: '1.19.2'";
        let c = run_with("wrong_mc_version", forge, &[], |b| b.mc_version = "1.19.2").unwrap();
        assert!(c.detail.contains("create needs Minecraft [1.20.2,1.21)"));
        let prose = run(
            "wrong_mc_version",
            "[main/WARN]: Mod xyz was built for Minecraft 1.19.4 and may not work",
        )
        .unwrap();
        assert_eq!(prose.confidence, Confidence::Medium);
        assert!(
            run("wrong_mc_version", "was built for Minecraft 1.20.1").is_none(),
            "same version is fine"
        );
    }

    #[test]
    fn loader_mismatch_explicit_and_from_the_mod_list() {
        let fabric = "[main/ERROR]: Mod file /home/x/mods/create-1.20.1-0.5.1.jar is missing the fabric.mod.json descriptor";
        let c = run("loader_mismatch", fabric).unwrap();
        assert!(
            c.detail.contains("Forge or NeoForge") && c.detail.contains("uses Fabric"),
            "{}",
            c.detail
        );
        let forge = "net.minecraftforge.fml.loading.moddiscovery.ModFileParser: Mod file /games/mods/sodium-fabric-0.5.8.jar is missing mods.toml file";
        let c = run_with("loader_mismatch", forge, &[], |b| b.loader = Loader::Forge).unwrap();
        assert!(
            c.detail.contains("sodium-fabric-0.5.8.jar") && c.detail.contains("Fabric or Quilt"),
            "{}",
            c.detail
        );
        // No explicit line: a failed load plus a mod whose loaders exclude ours.
        let mut forge_mod = record("Create", "create", "create-1.20.1.jar");
        forge_mod.loaders = Some(vec!["forge".into()]);
        let mut both = record("Lithium", "lithium", "lithium.jar");
        both.loaders = Some(vec!["fabric".into(), "quilt".into()]);
        let mods = [forge_mod, both];
        let c = run_with(
            "loader_mismatch",
            "net.fabricmc.loader.impl.discovery.ModResolutionException: Mod resolution failed",
            &mods,
            |_| {},
        )
        .unwrap();
        assert_eq!(c.confidence, Confidence::Medium);
        assert_eq!(c.related_mods, ["Create"]);
        assert!(run_with("loader_mismatch", "all fine", &mods, |_| {}).is_none());
    }

    #[test]
    fn duplicate_mods_and_conflicts() {
        let log = "net.fabricmc.loader.impl.FormattedException: Duplicate mod ID: 'sodium'\n\
            \t - sodium-fabric-0.5.8+mc1.20.1.jar\n\
            \t - sodium-fabric-0.5.3+mc1.20.1.jar";
        let mods = [record(
            "Sodium",
            "sodium",
            "sodium-fabric-0.5.8+mc1.20.1.jar",
        )];
        let c = run_with("duplicate_mods", log, &mods, |_| {}).unwrap();
        assert!(
            c.detail.contains("sodium is installed more than once"),
            "{}",
            c.detail
        );
        assert!(c.detail.contains("sodium-fabric-0.5.3+mc1.20.1.jar"));
        assert_eq!(c.related_mods[0], "Sodium");
        let c = run("duplicate_mods", "Found duplicate mods: jei").unwrap();
        assert!(c.detail.contains("jei"));
        let conflict = "\t - Mod 'Sodium' (sodium) 0.5.8 conflicts with mod 'Optifabric' (optifabric), which is present!";
        let c = run("duplicate_mods", conflict).unwrap();
        assert!(
            c.detail.contains("cannot be used together with"),
            "{}",
            c.detail
        );
        assert!(
            c.related_mods.contains(&"sodium".to_string())
                && c.related_mods.contains(&"optifabric".to_string())
        );
        assert!(run("duplicate_mods", "[main/INFO]: Loaded 3 mods").is_none());
    }

    #[test]
    fn mixin_failure_names_the_mod_from_the_log_or_the_config() {
        let log = "[main/ERROR]: Mixin apply for mod lithium failed lithium.mixins.json:gen.MixinX from mod lithium -> net.minecraft.class_1937: org.spongepowered.asm.mixin.throwables.MixinApplyError: Mixin [lithium.mixins.json:gen.MixinX from mod lithium] from phase [DEFAULT] in config [lithium.mixins.json] FAILED during APPLY\n\
            Caused by: org.spongepowered.asm.mixin.injection.throwables.InjectionError: Critical injection failure: @Redirect annotation on onTick could not find any targets";
        let mods = [record("Lithium", "lithium", "lithium-fabric-0.11.jar")];
        let c = run_with("mixin_failure", log, &mods, |_| {}).unwrap();
        assert_eq!(c.confidence, Confidence::High);
        assert_eq!(c.related_mods, ["Lithium"]);
        assert!(c.suggestions[0].contains("1.20.1"));
        // Only the config name is known.
        let log = "org.spongepowered.asm.mixin.throwables.MixinApplyError: Mixin transformation of net.minecraft.class_310 failed\nin config [mixins.create.json]: create.mixins.json:client.MixinFoo -> x";
        let c = run("mixin_failure", log).unwrap();
        assert!(
            c.related_mods.contains(&"create".to_string()),
            "{:?}",
            c.related_mods
        );
        // Mod unknown.
        let c = run("mixin_failure", "Caused by: org.spongepowered.asm.mixin.injection.throwables.InjectionError: Critical injection failure").unwrap();
        assert_eq!(c.confidence, Confidence::Medium);
        assert!(run("mixin_failure", "[main/INFO]: Mixin subsystem initialised").is_none());
        assert_eq!(
            mod_of_mixin_config("sodium.mixins.json").as_deref(),
            Some("sodium")
        );
        assert_eq!(
            mod_of_mixin_config("mixins.create.json").as_deref(),
            Some("create")
        );
        assert_eq!(
            mod_of_mixin_config("examplemod.mixin.json").as_deref(),
            Some("examplemod")
        );
    }

    #[test]
    fn graphics_driver_errors_get_platform_advice() {
        for line in [
            "[Render thread/ERROR]: GLFW error 65542: WGL: The driver does not appear to support OpenGL",
            "GLFW error 65543: GLX: Failed to create context: GLXBadFBConfig",
            "org.lwjgl.LWJGLException: Pixel format not accelerated",
            "[Render thread/ERROR]: GLFW error 65545: WGL: Failed to find a suitable pixel format",
        ] {
            let c = run("gl_driver", line).unwrap_or_else(|| panic!("{line}"));
            assert_eq!(c.confidence, Confidence::High);
        }
        let win = run_with("gl_driver", "GLFW error 65542: WGL: x", &[], |b| {
            b.os = "windows"
        })
        .unwrap();
        assert!(win.suggestions.iter().any(|s| s.contains("NVIDIA")));
        let lin = run("gl_driver", "GLFW error 65542: GLX: x").unwrap();
        assert!(lin.suggestions.iter().any(|s| s.contains("Mesa")));
        assert!(run("gl_driver", "[Render thread/INFO]: OpenGL 4.6 ready").is_none());
    }

    #[test]
    fn corrupt_world_and_chunks() {
        let level = "[Server thread/ERROR]: Failed to read level.dat of saves/My World: java.util.zip.ZipException: Not in GZIP format";
        let c = run("corrupt_world", level).unwrap();
        assert_eq!(c.confidence, Confidence::High);
        assert!(
            c.detail.contains("level.dat") && c.detail.contains("My"),
            "{}",
            c.detail
        );
        let chunk = "[Worker-Main-3/ERROR]: Failed to read chunk [12, -4]\njava.io.EOFException: Unexpected end of ZLIB input stream\n\tat net.minecraft.class_2852";
        let c = run("corrupt_world", chunk).unwrap();
        assert_eq!(c.confidence, Confidence::Medium);
        assert!(c.suggestions[0].contains("backup"));
        assert!(run("corrupt_world", "Saving chunks for level").is_none());
    }

    #[test]
    fn shader_failures_name_the_pack_and_the_mods() {
        let log = "[Render thread/ERROR] (Iris) Failed to compile shader program gbuffers_terrain in shaderpacks/ComplementaryReimagined_r5.zip\n\
            net.irisshaders.iris.gl.shader.ShaderCompileException: Failed to compile fragment shader";
        let c = run("shader_failure", log).unwrap();
        assert_eq!(c.confidence, Confidence::High);
        assert!(
            c.detail.contains("ComplementaryReimagined_r5.zip"),
            "{}",
            c.detail
        );
        assert!(c.related_mods.contains(&"iris".to_string()));
        let soft = run(
            "shader_failure",
            "[Render thread/WARN] (Iris) Failed to reload the shaderpack",
        )
        .unwrap();
        assert_eq!(soft.confidence, Confidence::Medium);
        assert!(run(
            "shader_failure",
            "[Render thread/INFO] (Iris) Using shaderpack Off"
        )
        .is_none());
    }

    #[test]
    fn heap_reservation_failures() {
        let log = "Error occurred during initialization of VM\nCould not reserve enough space for 8388608KB object heap";
        let c = run("jvm_heap_reserve", log).unwrap();
        assert!(
            c.detail.contains("2048 MB") && c.detail.contains("16384 MB"),
            "{}",
            c.detail
        );
        assert!(run(
            "jvm_heap_reserve",
            "Invalid maximum heap size: -Xmx99999999g"
        )
        .is_some());
        // The memory rule is for a running game, not for this.
        assert!(run("out_of_memory", log).is_none());
    }

    #[test]
    fn lwjgl_natives_missing() {
        let log = "java.lang.UnsatisfiedLinkError: Failed to locate library: liblwjgl.so\n\
            \tat org.lwjgl.system.Library.loadSystem(Library.java:200)";
        let c = run("lwjgl_natives", log).unwrap();
        assert_eq!(c.confidence, Confidence::High);
        assert!(run(
            "lwjgl_natives",
            "Caused by: java.lang.UnsatisfiedLinkError: Unable to load library 'flite'"
        )
        .is_none());
        let a = analyze(&Facts::new(log, &base(&[])));
        assert_eq!(a.causes[0].code, "lwjgl_natives");
    }

    #[test]
    fn disk_full() {
        for line in [
            "java.io.IOException: No space left on device",
            "java.io.IOException: There is not enough space on the disk",
        ] {
            assert!(run("disk_full", line).is_some(), "{line}");
        }
        assert!(run("disk_full", "free space: plenty").is_none());
    }

    #[test]
    fn java_path_problems() {
        let log = "java.io.IOException: Cannot run program \"/opt/jdk-17/bin/java\" (in directory \"/home/x/.mangoclient/instances/a\"): error=2, No such file or directory";
        let c = run("java_path", log).unwrap();
        assert!(c.detail.contains("/opt/jdk-17/bin/java"), "{}", c.detail);
        assert!(run(
            "java_path",
            "Error: Invalid or corrupt jarfile /x/client.jar"
        )
        .is_some());
        assert!(run(
            "java_path",
            "'java' is not recognized as an internal or external command"
        )
        .is_some());
        assert!(run("java_path", "Running java 17").is_none());
    }

    #[test]
    fn unrecognized_jvm_arguments_are_traced_to_the_instance() {
        let log = "Unrecognized VM option 'UseConcMarkSweepGC'\nError: Could not create the Java Virtual Machine.";
        let c = run_with("jvm_args", log, &[], |b| {
            b.java_args = "-XX:+UseConcMarkSweepGC -Xss1M"
        })
        .unwrap();
        assert!(
            c.detail.contains("UseConcMarkSweepGC")
                && c.detail.contains("in this instance's Java arguments"),
            "{}",
            c.detail
        );
        assert!(run(
            "jvm_args",
            "Error: Could not create the Java Virtual Machine."
        )
        .is_none());
    }

    #[test]
    fn account_session_errors_and_the_offline_hint() {
        let log = "[Render thread/INFO]: Disconnected: Failed to verify username!\n";
        let c = run("account_session", log).unwrap();
        assert!(c.suggestions[0].contains("Sign in"));
        let c = run_with(
            "account_session",
            "Invalid session (Try restarting your game)",
            &[],
            |b| b.offline_account = true,
        )
        .unwrap();
        assert!(c.detail.contains("offline account"));
        assert!(c.suggestions[0].contains("Offline accounts cannot join"));
        assert!(run("account_session", "Session started").is_none());
    }

    #[test]
    fn causes_are_ranked_by_confidence_and_the_excerpt_follows_the_best() {
        // Medium (shader soft) listed first in the log, high (disk) later.
        let mut log = String::new();
        for i in 0..30 {
            log.push_str(&format!("[main/INFO]: filler {i}\n"));
        }
        log.push_str("[Render thread/WARN] (Iris) Failed to reload the shaderpack\n");
        for i in 0..30 {
            log.push_str(&format!("[main/INFO]: more {i}\n"));
        }
        log.push_str("java.io.IOException: No space left on device\n");
        for i in 0..50 {
            log.push_str(&format!("\tat frame {i}\n"));
        }
        let b = base(&[]);
        let facts = Facts::new(&log, &b);
        let a = analyze(&facts);
        assert_eq!(a.causes[0].code, "disk_full");
        assert_eq!(a.causes[1].code, "shader_failure");
        let ex = excerpt(&facts.lines, a.anchor, false);
        assert_eq!(ex.lines().count(), EXCERPT_LINES);
        assert!(ex.contains("No space left on device"));
        assert!(!ex.contains("filler 5"));
    }

    #[test]
    fn excerpt_fallbacks_and_line_cut() {
        let lines: Vec<String> = (0..100).map(|i| format!("line {i}")).collect();
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        assert!(excerpt(&refs, None, true).starts_with("line 0"));
        assert!(excerpt(&refs, None, false).ends_with("line 99"));
        assert_eq!(excerpt(&refs, None, false).lines().count(), EXCERPT_LINES);
        let mut with_error: Vec<&str> = refs.clone();
        with_error[10] = "[main/ERROR]: boom";
        assert!(excerpt(&with_error, None, false).starts_with("line 8"));
        let long = "x".repeat(1000);
        let cut = excerpt(&[long.as_str()], None, false);
        assert_eq!(cut.chars().count(), MAX_LINE_CHARS + 3);
        assert_eq!(excerpt(&[], None, false), "");
        // The anchor near the start does not underflow.
        assert!(excerpt(&refs, Some(1), false).starts_with("line 0"));
    }

    #[test]
    fn versions_summary_and_related_mapping() {
        let crash = "---- Minecraft Crash Report ----\nDescription: Rendering overlay\n\nMinecraft Version: 1.20.4\nJava Version: 21.0.2, Eclipse Adoptium\n";
        let (j, m) = log_versions(crash);
        assert_eq!(
            (j.as_deref(), m.as_deref()),
            (Some("21.0.2"), Some("1.20.4"))
        );
        assert_eq!(
            crash_description(crash).as_deref(),
            Some("Rendering overlay")
        );
        let (_, m) =
            log_versions("[main/INFO]: Loading Minecraft 1.21.1 with Fabric Loader 0.16.9");
        assert_eq!(m.as_deref(), Some("1.21.1"));
        assert_eq!(
            summary(&[], Some("Rendering overlay"), false),
            "The game crashed: Rendering overlay. No known cause was found"
        );
        assert_eq!(summary(&[], None, true), "There is no log to analyze yet");

        let mods = [
            record(
                "Sodium",
                "sodium",
                "sodium-fabric-0.5.8+mc1.20.1.jar.disabled",
            ),
            record("Fabric API", "fabric-api", "fabric-api-0.92.0.jar"),
        ];
        let out = map_related(
            &mods,
            &[
                s("sodium"),
                s("fabric-api"),
                s("fabric-api-0.92.0.jar"),
                s("unknown_mod"),
                s("x"),
            ],
        );
        assert_eq!(out, ["Sodium", "Fabric API", "unknown_mod"]);
    }

    // ------- fixtures from real Minecraft 1.21.4 / Fabric Loader 0.20.1 runs
    // (relevant lines only, home paths replaced with ~).

    #[test]
    fn real_fabric_missing_dependency_and_wrong_minecraft() {
        let missing = "[main/WARN]: Mod resolution failed\n\
            [main/INFO]: Immediate reason: [HARD_DEP_NO_CANDIDATE needsdep 1.0.0 {depends nonexistentlib @ [*]}, ROOT_FORCELOAD_SINGLE needsdep 1.0.0]\n\
            [main/ERROR]: Incompatible mods found!\n\
            net.fabricmc.loader.impl.FormattedException: Some of your mods are incompatible with the game or each other!\n\
            A potential solution has been determined, this may resolve your problem:\n\
            \t - Install nonexistentlib, any version.\n\
            More details:\n\
            \t - Mod 'Needsdep' (needsdep) 1.0.0 requires any version of nonexistentlib, which is missing!\n\
            \tat net.fabricmc.loader.impl.FabricLoaderImpl.load(FabricLoaderImpl.java:207)";
        let c = run("mod_dependency", missing).unwrap();
        assert!(c.related_mods.contains(&"needsdep".to_string()), "{c:?}");
        assert!(c.related_mods.contains(&"nonexistentlib".to_string()));
        assert!(run("wrong_mc_version", missing).is_none());

        let wrong = "[main/INFO]: Immediate reason: [HARD_DEP_INCOMPATIBLE_PRESELECTED oldmc 1.0.0 {depends minecraft @ [>=1.19 <1.20]}, ROOT_FORCELOAD_SINGLE oldmc 1.0.0]\n\
            [main/ERROR]: Incompatible mods found!\n\
            net.fabricmc.loader.impl.FormattedException: Some of your mods are incompatible with the game or each other!\n\
            A potential solution has been determined, this may resolve your problem:\n\
            \t - Replace 'Minecraft' (minecraft) 1.21.4 with any version between 1.19 (inclusive) and 1.20 (exclusive).\n\
            More details:\n\
            \t - Mod 'Oldmc' (oldmc) 1.0.0 requires any version between 1.19 (inclusive) and 1.20 (exclusive) of 'Minecraft' (minecraft), but only the wrong version is present: 1.21.4!";
        let c = run("wrong_mc_version", wrong).unwrap();
        assert_eq!(c.related_mods, vec!["oldmc".to_string()], "{c:?}");
        assert!(run("mod_dependency", wrong).is_none());
    }

    #[test]
    fn real_fabric_two_mods_with_one_id() {
        let log = "[main/WARN]: Mod resolution failed\n\
            [main/INFO]: Immediate reason: [ROOT_FORCELOAD_SINGLE alpha 1.0.0, UNIQUE_ID alpha, ROOT_FORCELOAD_SINGLE beta 1.0.0]\n\
            [main/INFO]: Reason: []\n\
            [main/INFO]: Fix: add [], remove [alpha 1.0.0], replace []\n\
            [main/ERROR]: Incompatible mods found!\n\
            net.fabricmc.loader.impl.FormattedException: Some of your mods are incompatible with the game or each other!\n\
            A potential solution has been determined, this may resolve your problem:\n\
            \t - Remove mod 'Alpha' (alpha) 1.0.0 (~/.mangoclient/instances/x/mods/alpha-1.0.jar).\n\
            More details:";
        let c = run("duplicate_mods", log).unwrap();
        assert!(c.detail.contains("alpha and beta"), "{}", c.detail);
        assert!(c.detail.contains("alpha-1.0.jar"));
        assert!(c.related_mods.contains(&"beta".to_string()));
        // The same solver output for a missing dependency is not a duplicate.
        assert!(run(
            "duplicate_mods",
            "Immediate reason: [HARD_DEP_NO_CANDIDATE a 1.0 {depends b @ [*]}]"
        )
        .is_none());
    }

    #[test]
    fn real_fabric_entrypoint_class_missing() {
        let log = "Time: 2026-11-21 00:21:39\n\
            Description: Initializing game\n\
            \n\
            java.lang.RuntimeException: Could not execute entrypoint stage 'main' due to errors, provided by 'badentry' at 'does.not.Exist'!\n\
            \tat net.fabricmc.loader.impl.FabricLoaderImpl.lambda$invokeEntrypoints$0(FabricLoaderImpl.java:418)\n\
            Caused by: net.fabricmc.loader.api.EntrypointException: Exception while loading entries for entrypoint 'main' provided by 'badentry'\n\
            Caused by: net.fabricmc.loader.api.LanguageAdapterException: java.lang.ClassNotFoundException: does.not.Exist\n\
            Caused by: java.lang.ClassNotFoundException: does.not.Exist";
        let c = run("mod_entrypoint", log).unwrap();
        assert_eq!(c.related_mods, vec!["badentry".to_string()]);
        assert!(c.detail.contains("does.not.Exist"), "{}", c.detail);
        assert!(run("mod_entrypoint", "[main/INFO]: Loading 3 mods").is_none());
    }

    #[test]
    fn real_mixin_with_a_missing_class() {
        let log = "[main/ERROR]: Mixin prepare for mod badmixin failed preparing MissingClassMixin in badmixin.mixins.json: org.spongepowered.asm.mixin.transformer.throwables.InvalidMixinException The specified mixin 'com.example.badmixin.mixin.MissingClassMixin' was not found\n\
            [main/ERROR]: Uncaught exception in thread \"main\"\n\
            java.lang.RuntimeException: Mixin transformation of net.minecraft.client.main.Main failed\n\
            Caused by: org.spongepowered.asm.mixin.throwables.MixinApplyError: Mixin [badmixin.mixins.json:MissingClassMixin from mod badmixin] from phase [DEFAULT] in config [badmixin.mixins.json] FAILED during PREPARE";
        let c = run("mixin_failure", log).unwrap();
        assert_eq!(c.related_mods, vec!["badmixin".to_string()], "{c:?}");
        // A mixin that was only mentioned while loading is not a failure.
        assert!(run(
            "mixin_failure",
            "[main/INFO]: SpongePowered MIXIN Subsystem Version=0.8.7 Source=file:~/libraries/sponge-mixin.jar"
        )
        .is_none());
    }

    #[test]
    fn real_java_17_on_a_java_21_game_in_two_languages() {
        let de = "Fehler: Beim Laden der Klasse net.minecraft.client.main.Main ist ein LinkageError aufgetreten\n\
            \tjava.lang.UnsupportedClassVersionError: net/minecraft/client/main/Main has been compiled by a more recent version of the Java Runtime (class file version 65.0), this version of the Java Runtime only recognizes class file versions up to 61.0";
        let en = "Error: LinkageError occurred while loading main class net.minecraft.client.main.Main\n\
            \tjava.lang.UnsupportedClassVersionError: net/minecraft/client/main/Main has been compiled by a more recent version of the Java Runtime (class file version 65.0), this version of the Java Runtime only recognizes class file versions up to 61.0";
        for log in [de, en] {
            let c = run("java_version", log).unwrap();
            assert_eq!(
                c.detail, "This needs Java 21 or newer, but Java 17 is running",
                "{log}"
            );
        }
    }

    #[test]
    fn real_jvm_start_failures_from_the_process_output() {
        // 128 MB with the launcher's own G1 tuning.
        let ratio = "SurvivorRatio (32) must be less than or equal to ergonomic SurvivorRatio maximum (16)\n\
            Error: Could not create the Java Virtual Machine.\n\
            Error: A fatal exception has occurred. Program will exit.";
        let c = run("jvm_args", ratio).unwrap();
        assert!(c.detail.contains("SurvivorRatio (32)"), "{}", c.detail);

        let reserve = "OpenJDK 64-Bit Server VM warning: INFO: os::commit_memory(0x00007f036c000000, 858993459200, 0) failed; error='Not enough space' (errno=12)\n\
            #\n\
            # There is insufficient memory for the Java Runtime Environment to continue.\n\
            # Native memory allocation (mmap) failed to map 858993459200 bytes. Error detail: committing reserved memory.\n\
            # An error report file with more information is saved as:\n\
            # ~/.mangoclient/instances/x/hs_err_pid1036348.log";
        let c = run("jvm_heap_reserve", reserve).unwrap();
        assert!(c.detail.contains("819200 MB"), "{}", c.detail);
        // An out-of-memory error inside the game is another cause.
        assert!(run(
            "jvm_heap_reserve",
            "java.lang.OutOfMemoryError: Java heap space"
        )
        .is_none());
        assert!(run("jvm_args", reserve).is_none());
    }

    #[test]
    fn real_heap_exhaustion_blames_a_small_xmx_in_the_java_arguments() {
        let log = "[main/INFO]: Loading Minecraft 1.21.4 with Fabric Loader 0.20.1\n\
            [main/ERROR]: Uncaught exception in thread \"main\"\n\
            java.lang.OutOfMemoryError: Java heap space";
        let c = run_with("out_of_memory", log, &[], |b| {
            b.java_args = "-XX:+UseG1GC -Xmx48m -Xms32m";
        })
        .unwrap();
        assert!(c.detail.contains("-Xmx48m"), "{}", c.detail);
        assert!(
            c.suggestions[0].contains("overrides"),
            "{:?}",
            c.suggestions
        );
        let c = run("out_of_memory", log).unwrap();
        assert!(c.suggestions[0].contains("2048 MB"), "{:?}", c.suggestions);
    }

    #[test]
    fn a_clean_real_start_is_no_cause_even_with_the_linux_narrator_error() {
        let log = "[main/INFO]: Loading Minecraft 1.21.4 with Fabric Loader 0.20.1\n\
            [main/INFO]: Loading 4 mods:\n\
            \t- fabricloader 0.20.1\n\
            \t   \\-- mixinextras 0.5.5\n\
            \t- java 21\n\
            \t- minecraft 1.21.4\n\
            [main/INFO]: SpongePowered MIXIN Subsystem Version=0.8.7 Source=file:~/libraries/net/fabricmc/sponge-mixin/0.17.5+mixin.0.8.7/sponge-mixin-0.17.5+mixin.0.8.7.jar Service=Knot/Fabric Env=CLIENT\n\
            [Datafixer Bootstrap/INFO]: 243 Datafixer optimizations took 258 milliseconds\n\
            [Render thread/INFO]: Setting user: Player\n\
            [Render thread/ERROR]: Error while loading the narrator\n\
            com.mojang.text2speech.Narrator$InitializeException: Failed to load library flite\n\
            Caused by: java.lang.UnsatisfiedLinkError: Unable to load library 'flite':\n\
            \tSuppressed: java.io.IOException: Native library (linux-x86-64/libflite.so) not found in resource path (~/libraries/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar)\n\
            [Render thread/INFO]: Sound engine started";
        let b = base(&[]);
        let facts = Facts::new(log, &b);
        let causes = analyze(&facts).causes;
        assert!(causes.is_empty(), "{causes:?}");
    }
}
