//! Part 9 of `docs/ipc.md`: `crash_analyze` and `diagnostics_export`.
//!
//! The rules live in `crash.rs`, the scrubber in `scrub.rs`; this file reads
//! the logs (through the same tail reader as the Logs tab: at most 4 MB, gzip
//! streamed) and puts the pieces together.

use crate::crash::{self, Base, CrashCause, CrashReport, Facts};
use crate::error::{AppError, Res};
use crate::media::{list_dir, mtime_ms, read_tail, MAX_LOG_BYTES};
use crate::models::LogKind;
use crate::outfile::{add_bytes, write_zip_atomic, FileExport};
use crate::scrub::{scrub, ScrubContext};
use crate::service::AppService;
use mango_core::{AccountType, ModRecord, Profile};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Parts of a diagnostics bundle.
const DIAG_LOG_BYTES: usize = 2 * 1024 * 1024;
const DIAG_CRASH_BYTES: usize = 1024 * 1024;
const DIAG_LAUNCHER_BYTES: usize = 1024 * 1024;
const MAX_LISTED_MODS: usize = 2000;

/// What the scrubber needs from the account list.
pub(crate) fn scrub_context(svc: &AppService) -> ScrubContext {
    let (names, uuids) = svc.inner.with_store(|s| {
        (
            s.accounts
                .iter()
                .map(|a| a.name.clone())
                .collect::<Vec<_>>(),
            s.accounts
                .iter()
                .map(|a| a.uuid.clone())
                .collect::<Vec<_>>(),
        )
    });
    ScrubContext::for_machine(names, uuids)
}

fn newest(entries: Vec<crate::models::LogEntry>) -> Option<crate::models::LogEntry> {
    entries
        .into_iter()
        .max_by(|a, b| a.modified.cmp(&b.modified).then_with(|| b.id.cmp(&a.id)))
}

/// The launcher log counts as newer than the game's own logs when it was
/// written more than this much later: the JVM then died before the game
/// could write `latest.log` (bad Java arguments, no memory, old Java).
const LAUNCHER_NEWER_MS: u64 = 5_000;

/// The newest crash report of the instance, else `latest.log`; the launcher
/// log (the process output) when that is clearly newer or nothing else exists.
fn default_source(crash_dir: &Path, game_dir: &Path, launcher: &Path) -> Option<(String, PathBuf)> {
    let game = if let Some(e) = newest(list_dir(crash_dir, LogKind::Crash)) {
        Some((e.id, crash_dir.join(e.name), e.modified))
    } else {
        let latest = game_dir.join("latest.log");
        std::fs::symlink_metadata(&latest)
            .ok()
            .filter(|m| m.is_file())
            .map(|m| ("game:latest.log".to_string(), latest, mtime_ms(&m)))
    };
    let launcher_log = std::fs::symlink_metadata(launcher)
        .ok()
        .filter(|m| m.is_file())
        .and_then(|m| {
            let name = launcher.file_name()?.to_string_lossy().into_owned();
            Some((
                format!("launcher:{name}"),
                launcher.to_path_buf(),
                mtime_ms(&m),
            ))
        });
    match (game, launcher_log) {
        (Some(g), Some(l)) if l.2 > g.2.saturating_add(LAUNCHER_NEWER_MS) => Some((l.0, l.1)),
        (Some(g), _) => Some((g.0, g.1)),
        (None, Some(l)) => Some((l.0, l.1)),
        (None, None) => None,
    }
}

impl AppService {
    /// Explain why the game crashed, from the newest crash report (else
    /// `latest.log`), or from `log_id` (an id of `logs_list`).
    pub async fn crash_analyze(
        &self,
        profile_id: String,
        log_id: Option<String>,
    ) -> Res<CrashReport> {
        let profile = self.profile(&profile_id)?;
        let (game_dir, crash_dir, launcher) = self.log_dirs(&profile)?;
        let source = match log_id {
            Some(id) => {
                let (file, _) = self.log_file(&profile_id, &id)?;
                Some((id, file))
            }
            None => {
                let (c, g) = (crash_dir, game_dir);
                tokio::task::spawn_blocking(move || default_source(&c, &g, &launcher)).await?
            }
        };
        let ctx = scrub_context(self);
        let (ram_default, offline) = self.inner.with_store(|s| {
            (
                s.config.ram,
                s.selected_account()
                    .is_some_and(|a| a.kind == AccountType::Offline),
            )
        });
        let svc_profile = profile;
        let Some((id, file)) = source else {
            return Ok(CrashReport {
                summary: crash::summary(&[], None, true),
                causes: Vec::new(),
                java_version: None,
                mc_version: Some(svc_profile.mc_version),
                excerpt: String::new(),
                source_log: None,
            });
        };
        let text = tokio::task::spawn_blocking(move || read_tail(&file, MAX_LOG_BYTES))
            .await?
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => AppError::new("not_found", "Log not found"),
                _ => AppError::from(e),
            })?
            .text;
        let is_crash_report = id.starts_with("crash:");
        Ok(tokio::task::spawn_blocking(move || {
            build_report(
                &svc_profile,
                ram_default,
                offline,
                &text,
                id,
                is_crash_report,
                &ctx,
            )
        })
        .await?)
    }

    /// A zip for bug reports: instance summary, system and Java facts, the
    /// mod list, the latest log, the newest crash report and the launcher
    /// log, all scrubbed (see `scrub.rs`). No account or token data.
    pub async fn diagnostics_export(
        &self,
        profile_id: String,
        out_path: String,
    ) -> Res<FileExport> {
        let profile = self.profile(&profile_id)?;
        let instance = self.inner.paths.instance_dir(&profile.id)?;
        let out = self.checked_out_path(&out_path, &[&instance])?;
        let (game_dir, crash_dir, launcher) = self.log_dirs(&profile)?;
        let ctx = scrub_context(self);
        let config = self.inner.with_store(|s| s.config.clone());

        // Java facts: the configured Java and what the launcher manages.
        let configured = (!config.java_path.is_empty()).then(|| PathBuf::from(&config.java_path));
        let configured_major = match &configured {
            Some(p) => mango_java::probe(p).await.map(|j| j.major),
            None => None,
        };
        let managed: Vec<Value> = mango_java::list_managed_runtimes(
            &self.inner.paths.runtimes(),
            mango_java::Platform::current(),
        )
        .await
        .into_iter()
        .map(|c| json!({"id": c.id, "major": c.major}))
        .collect();

        let version = self.inner.opts.app_version.clone();
        let total_ram = mango_core::model::total_ram_mb();
        let size = tokio::task::spawn_blocking(move || -> Res<FileExport> {
            let mut warnings = Vec::new();
            let mut parts: Vec<(String, String)> = Vec::new();
            parts.push((
                "profile.json".to_string(),
                profile_json(&profile, &config.java_args)?,
            ));
            let system = json!({
                "launcher": {"version": version},
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
                "totalRamMb": total_ram,
                "java": {
                    "configuredPath": config.java_path,
                    "configuredMajor": configured_major,
                    "requiredMajor": mango_java::required_major(None, &profile.mc_version),
                    "managedRuntimes": managed,
                },
                "settings": {
                    "ram": config.ram,
                    "javaArgs": config.java_args,
                    "concurrentDownloads": config.concurrent_downloads,
                    "performanceMods": config.performance_mods,
                    "mangoConfig": config.mango_config,
                    "language": config.language,
                },
            });
            parts.push((
                "system.json".to_string(),
                serde_json::to_string_pretty(&system)?,
            ));
            parts.push(("mods.json".to_string(), mods_json(&profile.mods)?));

            let mut wanted: Vec<(String, PathBuf, usize)> = vec![(
                "logs/latest.log".to_string(),
                game_dir.join("latest.log"),
                DIAG_LOG_BYTES,
            )];
            if let Some(c) = newest(list_dir(&crash_dir, LogKind::Crash)) {
                wanted.push((
                    format!("crash-reports/{}", c.name),
                    crash_dir.join(&c.name),
                    DIAG_CRASH_BYTES,
                ));
            }
            if launcher.is_file() {
                wanted.push((
                    "logs/launcher.log".to_string(),
                    launcher,
                    DIAG_LAUNCHER_BYTES,
                ));
            }
            for (name, file, cap) in wanted {
                match read_tail(&file, cap) {
                    Ok(t) => {
                        if t.truncated {
                            warnings.push(format!("{name} was cut to its last {} KB", cap / 1024));
                        }
                        parts.push((name, t.text));
                    }
                    Err(_) => warnings.push(format!("{name} does not exist")),
                }
            }

            let out2 = out.clone();
            let bytes = write_zip_atomic(&out2, |zw| {
                for (name, text) in &parts {
                    add_bytes(zw, name, scrub(text, &ctx).as_bytes())?;
                }
                Ok(())
            })?;
            Ok(FileExport {
                path: out.display().to_string(),
                size_bytes: bytes,
                warnings,
            })
        })
        .await??;
        Ok(size)
    }
}

fn profile_json(p: &Profile, global_args: &str) -> Res<String> {
    Ok(serde_json::to_string_pretty(&json!({
        "id": p.id,
        "name": p.name,
        "mcVersion": p.mc_version,
        "loader": p.loader,
        "loaderVersion": p.loader_version,
        "ramMb": p.ram,
        "javaArgs": p.java_args,
        "globalJavaArgs": global_args,
        "mangoConfig": p.mango_config,
        "group": p.group,
        "tags": p.tags,
        "created": p.created,
        "lastPlayed": p.last_played,
        "playTimeMs": p.play_time_ms,
        "modCount": p.mods.len(),
    }))?)
}

fn mods_json(mods: &[ModRecord]) -> Res<String> {
    let list: Vec<Value> = mods
        .iter()
        .take(MAX_LISTED_MODS)
        .map(|m| {
            json!({
                "title": m.title,
                "file": m.filename,
                "version": m.version_number,
                "type": m.kind,
                "provider": if m.local == Some(true) || crate::content::has_foreign_provider(m) { "local" } else { "modrinth" },
                "projectId": m.project_id,
                "enabled": m.enabled.unwrap_or(true),
                "dependency": m.dependency.unwrap_or(false),
                "gameVersions": m.game_versions,
                "loaders": m.loaders,
            })
        })
        .collect();
    Ok(serde_json::to_string_pretty(&json!({
        "count": mods.len(),
        "mods": list,
    }))?)
}

fn build_report(
    profile: &Profile,
    ram_default: u32,
    offline: bool,
    text: &str,
    source_id: String,
    is_crash_report: bool,
    ctx: &ScrubContext,
) -> CrashReport {
    let base = Base {
        ram_mb: profile.effective_ram_mb(ram_default),
        total_ram_mb: mango_core::model::total_ram_mb(),
        mc_version: &profile.mc_version,
        loader: profile.loader,
        java_args: &profile.java_args,
        os: std::env::consts::OS,
        offline_account: offline,
        mods: &profile.mods,
    };
    let facts = Facts::new(text, &base);
    let analysis = crash::analyze(&facts);
    let (java, mc) = crash::log_versions(text);
    let description = crash::crash_description(text);
    let clean = |s: &str| scrub(s, ctx);
    let causes: Vec<CrashCause> = analysis
        .causes
        .into_iter()
        .map(|c| CrashCause {
            detail: clean(&c.detail),
            suggestions: c.suggestions.iter().map(|s| clean(s)).collect(),
            related_mods: c.related_mods.iter().map(|s| clean(s)).collect(),
            ..c
        })
        .collect();
    CrashReport {
        summary: crash::summary(&causes, description.as_deref(), text.trim().is_empty()),
        causes,
        java_version: java.map(|j| clean(&j)),
        mc_version: Some(mc.unwrap_or_else(|| profile.mc_version.clone())),
        excerpt: clean(&crash::excerpt(
            &facts.lines,
            analysis.anchor,
            is_crash_report,
        )),
        source_log: Some(source_id),
    }
}
