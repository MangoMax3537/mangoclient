//! Memory (RAM) policy: the default heap per machine, the limits a user may
//! pick, modpack hints and the rules that turn a hint into a number.
//! Everything here is pure; only [`total_ram_mb`] touches the system.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// Smallest heap a profile may have.
pub const MIN_RAM_MB: u32 = 1024;
/// Largest heap a profile may have (matches profile validation).
pub const MAX_RAM_MB: u32 = 131_072;
/// Memory left to the OS and the launcher on machines with >= 6 GB.
const RESERVED_MB: u64 = 2048;

/// Machine size in whole GB, rounded to nearest: firmware and the GPU keep a
/// slice, so a "16 GB" machine reports about 15.6 GB.
fn gb_rounded(total_mb: u64) -> u64 {
    (total_mb + 512) / 1024
}

/// What the user may pick on this machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RamLimits {
    pub min_mb: u32,
    pub max_mb: u32,
    pub total_mb: u64,
}

/// `min` is always 1024. `max` is `total - 2048` on machines of >= 6 GB and
/// `total / 2` below that, never below `min` and never above 131072.
pub fn ram_limits(total_mb: u64) -> RamLimits {
    let raw = if gb_rounded(total_mb) >= 6 {
        total_mb.saturating_sub(RESERVED_MB)
    } else {
        total_mb / 2
    };
    let max = raw.clamp(u64::from(MIN_RAM_MB), u64::from(MAX_RAM_MB)) as u32;
    RamLimits {
        min_mb: MIN_RAM_MB,
        max_mb: max,
        total_mb,
    }
}

/// Default heap for an instance without override or hint, in steps on the
/// total T (MB): `T < 6144` -> `min(2048, T/2)`; `< 12288` -> 3072; `< 24576`
/// -> 4096 (16 GB machines report slightly less); `< 49152` -> 5120; else
/// 6144. Always inside [`ram_limits`] (so at most `T - 2048` from 6 GB).
pub fn default_ram_mb(total_mb: u64) -> u32 {
    let base: u32 = match total_mb {
        0..=6143 => 2048.min((total_mb / 2) as u32),
        6144..=12287 => 3072,
        12288..=24575 => 4096,
        24576..=49151 => 5120,
        _ => 6144,
    };
    let limits = ram_limits(total_mb);
    base.clamp(limits.min_mb, limits.max_mb)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum RamHintSource {
    Curseforge,
    Prism,
    Modrinth,
    User,
}

/// Who chose a profile's heap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum RamSource {
    Default,
    Modpack,
    User,
}

/// A pack's memory wish. Values are MB inside `1024..=131072`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct RamHint {
    pub recommended_mb: Option<u32>,
    pub min_mb: Option<u32>,
    pub max_mb: Option<u32>,
    pub source: RamHintSource,
}

fn sane(v: Option<u32>) -> Option<u32> {
    v.filter(|v| (MIN_RAM_MB..=MAX_RAM_MB).contains(v))
}

impl RamHint {
    /// Drops out-of-range values and a `min` above `max`; `None` if nothing
    /// usable is left.
    pub fn new(
        recommended_mb: Option<u32>,
        min_mb: Option<u32>,
        max_mb: Option<u32>,
        source: RamHintSource,
    ) -> Option<RamHint> {
        let (recommended_mb, mut min_mb, max_mb) =
            (sane(recommended_mb), sane(min_mb), sane(max_mb));
        if let (Some(lo), Some(hi)) = (min_mb, max_mb) {
            if lo > hi {
                min_mb = None;
            }
        }
        if recommended_mb.is_none() && min_mb.is_none() && max_mb.is_none() {
            return None;
        }
        Some(RamHint {
            recommended_mb,
            min_mb,
            max_mb,
            source,
        })
    }

    /// The heap a new instance gets: recommended, else max, else min; raised
    /// to the hint's min, lowered to the hint's max, then clamped into the
    /// machine's `limits` (the machine wins over the pack).
    pub fn applied_mb(&self, limits: &RamLimits) -> u32 {
        let want = self
            .recommended_mb
            .or(self.max_mb)
            .or(self.min_mb)
            .unwrap_or(MIN_RAM_MB);
        let mut v = want.max(self.min_mb.unwrap_or(MIN_RAM_MB));
        if let Some(cap) = self.max_mb {
            v = v.min(cap);
        }
        v.clamp(limits.min_mb, limits.max_mb.max(limits.min_mb))
    }
}

/// CurseForge `manifest.json`: `minecraft.recommendedRam` (integer MB).
/// Missing, non-integer or out-of-range values give `None`.
pub fn parse_curseforge_ram(manifest: &serde_json::Value) -> Option<RamHint> {
    let v = manifest.get("minecraft")?.get("recommendedRam")?;
    let mb = v.as_u64().or_else(|| {
        v.as_f64()
            .filter(|f| f.fract() == 0.0 && *f >= 0.0)
            .map(|f| f as u64)
    })?;
    RamHint::new(
        u32::try_from(mb).ok(),
        None,
        None,
        RamHintSource::Curseforge,
    )
}

/// Prism/MultiMC `instance.cfg` (`key=value` lines). `MinMemAlloc` is the
/// hint's minimum, `MaxMemAlloc` its recommendation (Prism's max is the
/// author's chosen heap, not a cap, so `maxMb` stays unset).
pub fn parse_prism_ram(cfg: &str) -> Option<RamHint> {
    let num = |key: &str| -> Option<u32> {
        cfg.trim_start_matches('\u{feff}')
            .lines()
            .filter_map(|l| l.split_once('='))
            .find(|(k, _)| k.trim() == key)
            .and_then(|(_, v)| v.trim().parse::<u32>().ok())
    };
    RamHint::new(
        num("MaxMemAlloc"),
        num("MinMemAlloc"),
        None,
        RamHintSource::Prism,
    )
}

/// `MemTotal:   16384 kB` lines of `/proc/meminfo`, as MB.
pub fn parse_meminfo(text: &str) -> Option<u64> {
    let rest = text.lines().find_map(|l| l.strip_prefix("MemTotal:"))?;
    let kb: u64 = rest.trim().trim_end_matches("kB").trim().parse().ok()?;
    (kb >= 1024).then_some(kb / 1024)
}

/// Output of `sysctl -n hw.memsize`, `wmic ... get TotalPhysicalMemory` or
/// PowerShell's `TotalPhysicalMemory`: the first line that is a plain byte
/// count, as MB.
pub fn parse_bytes_output(text: &str) -> Option<u64> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && l.bytes().all(|b| b.is_ascii_digit()))
        .find_map(|l| l.parse::<u64>().ok())
        .map(|b| b / (1024 * 1024))
        .filter(|mb| *mb > 0)
}

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let mut cmd = std::process::Command::new(program);
    // No console window flashing up when a GUI app asks Windows for the RAM size.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd
        .args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn detect_total_ram_mb() -> Option<u64> {
    if cfg!(target_os = "linux") {
        parse_meminfo(&std::fs::read_to_string("/proc/meminfo").ok()?)
    } else if cfg!(target_os = "macos") {
        parse_bytes_output(&command_output("sysctl", &["-n", "hw.memsize"])?)
    } else if cfg!(windows) {
        let ps = command_output(
            "powershell",
            &[
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory",
            ],
        )
        .and_then(|t| parse_bytes_output(&t));
        ps.or_else(|| {
            parse_bytes_output(&command_output(
                "wmic",
                &["ComputerSystem", "get", "TotalPhysicalMemory"],
            )?)
        })
    } else {
        None
    }
}

/// Total system RAM in MB, detected once (Linux `/proc/meminfo`, macOS
/// `sysctl`, Windows PowerShell/`wmic`); 8192 if detection fails.
pub fn total_ram_mb() -> u64 {
    static TOTAL: OnceLock<u64> = OnceLock::new();
    *TOTAL.get_or_init(|| detect_total_ram_mb().unwrap_or(8192))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const GB: u64 = 1024;

    #[test]
    fn policy_table() {
        for (total, want) in [
            (2 * GB, 1024),
            (4 * GB, 2048),
            (6 * GB, 3072),
            (8 * GB, 3072),
            (12 * GB, 4096),
            (16 * GB, 4096),
            (32 * GB, 5120),
            (64 * GB, 6144),
            (128 * GB, 6144),
            (15_974, 4096), // 15.6 GB reported
            (32_051, 5120), // 31.3 GB
            (64_205, 6144), // 62.7 GB
            (7_900, 3072),
            (5_900, 2048),
        ] {
            assert_eq!(default_ram_mb(total), want, "{total} MB");
        }
    }

    #[test]
    fn limits_table() {
        let l = ram_limits(4 * GB);
        assert_eq!((l.min_mb, l.max_mb, l.total_mb), (1024, 2048, 4096));
        assert_eq!(ram_limits(6 * GB).max_mb, 4096);
        assert_eq!(ram_limits(16 * GB).max_mb, 14336);
        assert_eq!(ram_limits(GB).max_mb, 1024);
        assert_eq!(ram_limits(0).max_mb, 1024);
        assert_eq!(ram_limits(4096 * GB).max_mb, MAX_RAM_MB);
        for total in [0, GB, 4 * GB, 6 * GB, 8 * GB, 16 * GB, 64 * GB] {
            let l = ram_limits(total);
            let d = default_ram_mb(total);
            assert!(d >= l.min_mb && d <= l.max_mb, "{total}");
        }
    }

    #[test]
    fn parses_curseforge_hint_and_tolerates_garbage() {
        let h = parse_curseforge_ram(&json!({"minecraft": {"recommendedRam": 6144}})).unwrap();
        assert_eq!(h.recommended_mb, Some(6144));
        assert_eq!(h.source, RamHintSource::Curseforge);
        assert!(parse_curseforge_ram(&json!({"minecraft": {"recommendedRam": 6144.0}})).is_some());
        for bad in [
            json!({}),
            json!({"minecraft": {}}),
            json!({"minecraft": {"recommendedRam": "lots"}}),
            json!({"minecraft": {"recommendedRam": -5}}),
            json!({"minecraft": {"recommendedRam": 12.5}}),
            json!({"minecraft": {"recommendedRam": 0}}),
            json!({"minecraft": {"recommendedRam": 99999999999u64}}),
            json!({"minecraft": {"recommendedRam": null}}),
            json!("junk"),
        ] {
            assert_eq!(parse_curseforge_ram(&bad), None, "{bad}");
        }
    }

    #[test]
    fn parses_prism_hint_and_tolerates_garbage() {
        let h = parse_prism_ram("[General]\nMinMemAlloc=2048\r\nMaxMemAlloc = 6144\n").unwrap();
        assert_eq!(
            (h.min_mb, h.recommended_mb, h.max_mb),
            (Some(2048), Some(6144), None)
        );
        let only_min = parse_prism_ram("MinMemAlloc=2048\nMaxMemAlloc=abc").unwrap();
        assert_eq!(only_min.min_mb, Some(2048));
        assert_eq!(only_min.recommended_mb, None);
        for bad in [
            "",
            "garbage",
            "MaxMemAlloc=-1",
            "MaxMemAlloc=12",
            "MaxMemAlloc=\n",
        ] {
            assert_eq!(parse_prism_ram(bad), None, "{bad:?}");
        }
    }

    fn hint(rec: Option<u32>, min: Option<u32>, max: Option<u32>) -> RamHint {
        RamHint::new(rec, min, max, RamHintSource::Curseforge).unwrap()
    }

    #[test]
    fn clamping() {
        let big = ram_limits(32 * GB);
        assert_eq!(hint(Some(6144), None, None).applied_mb(&big), 6144);
        // recommended, else max, else min
        assert_eq!(hint(None, Some(2048), Some(5000)).applied_mb(&big), 5000);
        assert_eq!(hint(None, Some(3000), None).applied_mb(&big), 3000);
        // below the hint's min is raised, above its max is lowered
        assert_eq!(hint(Some(2048), Some(4096), None).applied_mb(&big), 4096);
        assert_eq!(hint(Some(9000), None, Some(6000)).applied_mb(&big), 6000);
        // below the global min
        assert_eq!(
            hint(Some(1024), None, None).applied_mb(&ram_limits(2 * GB)),
            1024
        );
        // above the system limit: the machine wins
        let small = ram_limits(8 * GB);
        assert_eq!(hint(Some(16384), None, None).applied_mb(&small), 6144);
        assert_eq!(hint(Some(2048), Some(8192), None).applied_mb(&small), 6144);
    }

    #[test]
    fn hint_sanitising() {
        assert!(RamHint::new(Some(10), None, Some(5), RamHintSource::Prism).is_none());
        let h = RamHint::new(None, Some(8192), Some(4096), RamHintSource::Prism).unwrap();
        assert_eq!((h.min_mb, h.max_mb), (None, Some(4096)));
    }

    #[test]
    fn system_parsers() {
        assert_eq!(
            parse_meminfo("MemTotal:       16384000 kB\nMemFree: 1 kB\n"),
            Some(16000)
        );
        assert_eq!(parse_meminfo("MemFree: 1 kB"), None);
        assert_eq!(parse_meminfo("MemTotal: junk kB"), None);
        assert_eq!(parse_bytes_output("17179869184\n"), Some(16384));
        assert_eq!(
            parse_bytes_output("TotalPhysicalMemory  \r\n34359738368  \r\n\r\n"),
            Some(32768)
        );
        assert_eq!(parse_bytes_output("error"), None);
        assert_eq!(parse_bytes_output(""), None);
    }
}
