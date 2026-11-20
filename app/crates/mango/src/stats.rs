//! Play-session journal (port of `src/main/stats.js`): `stats.json`
//! `{version:1, sessions:[{p, s, ms}]}`.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use chrono::{DateTime, Duration, Local, NaiveDate, TimeZone};
use mango_core::{persistence, Paths, Profile};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::Result;

/// Roughly four years of daily play; old entries fall off the front.
pub const MAX_SESSIONS: usize = 4000;
/// Anything shorter is a crash, not a play session.
pub const MIN_SESSION_MS: u64 = 30_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionEntry {
    /// Profile id.
    pub p: String,
    /// Start, epoch ms.
    pub s: i64,
    pub ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayTotal {
    pub date: String,
    pub ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Longest {
    pub ms: u64,
    pub start: i64,
    pub profile_id: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileStats {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub mc_version: String,
    pub loader: mango_core::Loader,
    pub total_ms: u64,
    pub window_ms: u64,
    pub last_played: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub days: Vec<DayTotal>,
    pub window_ms: u64,
    pub total_ms: u64,
    pub sessions: usize,
    pub longest: Option<Longest>,
    pub per_profile: Vec<ProfileStats>,
    pub most_played: Option<ProfileStats>,
    /// False when nothing was journalled yet.
    pub journalled: bool,
}

pub struct Stats {
    file: PathBuf,
}

impl Stats {
    pub fn new(paths: &Paths) -> Self {
        Stats {
            file: paths.stats_file(),
        }
    }

    fn read(&self) -> Vec<SessionEntry> {
        let Some(Value::Object(data)) = persistence::read_json_value(&self.file) else {
            return Vec::new();
        };
        data.get("sessions")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Record a finished stretch of play starting at `start_ms`. Returns
    /// whether it was journalled (short or id-less stretches are ignored).
    pub fn record(&self, profile_id: &str, ms: u64, start_ms: i64) -> Result<bool> {
        if profile_id.is_empty() || ms < MIN_SESSION_MS {
            return Ok(false);
        }
        let mut sessions = self.read();
        sessions.push(SessionEntry {
            p: profile_id.to_string(),
            s: start_ms,
            ms,
        });
        if sessions.len() > MAX_SESSIONS {
            let excess = sessions.len() - MAX_SESSIONS;
            sessions.drain(..excess);
        }
        persistence::write_json_atomic(&self.file, &json!({"version": 1, "sessions": sessions}))?;
        Ok(true)
    }

    /// Like [`record`](Self::record) with `start = now - ms`.
    pub fn record_ending_now(&self, profile_id: &str, ms: u64) -> Result<bool> {
        let start = Local::now().timestamp_millis() - i64::try_from(ms).unwrap_or(i64::MAX);
        self.record(profile_id, ms, start)
    }

    /// Summary of the last `days` local days (today included).
    pub fn summary(&self, profiles: &[Profile], days: u32) -> Summary {
        self.summary_at(profiles, days, Local::now())
    }

    /// Same, for an explicit "now" and time zone.
    pub fn summary_at<Tz: TimeZone>(
        &self,
        profiles: &[Profile],
        days: u32,
        now: DateTime<Tz>,
    ) -> Summary {
        let tz = now.timezone();
        let sessions = self.read();
        let names: HashMap<&str, &str> = profiles
            .iter()
            .map(|p| (p.id.as_str(), p.name.as_str()))
            .collect();

        let today = now.date_naive();
        let first = today - Duration::days(i64::from(days.saturating_sub(1)));
        let from_ms = tz
            .from_local_datetime(&first.and_hms_opt(0, 0, 0).unwrap_or_default())
            .earliest()
            .map_or(i64::MIN, |d| d.timestamp_millis());

        let mut buckets: BTreeMap<NaiveDate, u64> = BTreeMap::new();
        for i in 0..days {
            buckets.insert(first + Duration::days(i64::from(i)), 0);
        }

        let mut window_ms = 0u64;
        let mut longest: Option<Longest> = None;
        let mut per_window: HashMap<&str, u64> = HashMap::new();

        for s in &sessions {
            if longest.as_ref().is_none_or(|l| s.ms > l.ms) {
                longest = Some(Longest {
                    ms: s.ms,
                    start: s.s,
                    profile_id: s.p.clone(),
                    name: names.get(s.p.as_str()).map(|n| n.to_string()),
                });
            }
            if s.s < from_ms {
                continue;
            }
            if let Some(dt) = DateTime::from_timestamp_millis(s.s) {
                let day = dt.with_timezone(&tz).date_naive();
                if let Some(b) = buckets.get_mut(&day) {
                    *b += s.ms;
                }
            }
            window_ms += s.ms;
            *per_window.entry(s.p.as_str()).or_default() += s.ms;
        }

        let mut per_profile: Vec<ProfileStats> = profiles
            .iter()
            .map(|p| ProfileStats {
                id: p.id.clone(),
                name: p.name.clone(),
                color: p.color.clone(),
                mc_version: p.mc_version.clone(),
                loader: p.loader,
                total_ms: p.play_time_ms,
                window_ms: per_window.get(p.id.as_str()).copied().unwrap_or(0),
                last_played: p.last_played,
            })
            .collect();
        per_profile.sort_by_key(|p| std::cmp::Reverse(p.total_ms)); // stable
        let total_ms = per_profile.iter().map(|p| p.total_ms).sum();

        Summary {
            days: buckets
                .into_iter()
                .map(|(d, ms)| DayTotal {
                    date: d.format("%Y-%m-%d").to_string(),
                    ms,
                })
                .collect(),
            window_ms,
            total_ms,
            sessions: sessions.len(),
            longest,
            most_played: per_profile.first().cloned(),
            per_profile,
            journalled: !sessions.is_empty(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    fn setup() -> (tempfile::TempDir, Stats) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        (tmp, Stats::new(&paths))
    }

    fn profile(id: &str, name: &str, total: u64) -> Profile {
        let mut p = Profile::new(id.into(), 0);
        p.name = name.into();
        p.play_time_ms = total;
        p
    }

    #[test]
    fn ignores_short_and_anonymous() {
        let (_t, s) = setup();
        assert!(!s.record("a", 29_999, 0).unwrap());
        assert!(!s.record("", 60_000, 0).unwrap());
        assert!(s.record("a", 30_000, 0).unwrap());
        assert_eq!(s.read().len(), 1);
    }

    #[test]
    fn caps_at_4000_dropping_oldest() {
        let (_t, s) = setup();
        let entries: Vec<_> = (0..MAX_SESSIONS as i64)
            .map(|i| json!({"p": "a", "s": i, "ms": 40_000}))
            .collect();
        persistence::write_json_atomic(&s.file, &json!({"version":1,"sessions":entries})).unwrap();
        s.record("b", 40_000, 99_999).unwrap();
        let all = s.read();
        assert_eq!(all.len(), MAX_SESSIONS);
        assert_eq!(all[0].s, 1);
        assert_eq!(all.last().unwrap().p, "b");
    }

    #[test]
    fn corrupt_file_and_malformed_entries_tolerated() {
        let (_t, s) = setup();
        std::fs::write(&s.file, "not json").unwrap();
        assert!(s.read().is_empty());
        std::fs::write(
            &s.file,
            r#"{"version":1,"sessions":[{"p":"a","s":1,"ms":40000},{"bad":1}]}"#,
        )
        .unwrap();
        assert_eq!(s.read().len(), 1);
    }

    #[test]
    fn summary_buckets_local_days() {
        let (_t, s) = setup();
        let tz = FixedOffset::east_opt(2 * 3600).unwrap();
        let now = tz.with_ymd_and_hms(2026, 3, 10, 15, 0, 0).unwrap();
        let at = |d: u32, h: u32| {
            tz.with_ymd_and_hms(2026, 3, d, h, 0, 0)
                .unwrap()
                .timestamp_millis()
        };
        s.record("a", 60_000, at(10, 1)).unwrap(); // today, local early morning
        s.record("a", 120_000, at(9, 23)).unwrap();
        s.record("b", 90_000, at(8, 12)).unwrap();
        s.record("gone", 500_000, at(7, 12)).unwrap(); // outside 3-day window
        let profiles = vec![profile("a", "Alpha", 10), profile("b", "Beta", 99)];
        let sum = s.summary_at(&profiles, 3, now);
        let days: Vec<_> = sum.days.iter().map(|d| (d.date.as_str(), d.ms)).collect();
        assert_eq!(
            days,
            [
                ("2026-03-08", 90_000),
                ("2026-03-09", 120_000),
                ("2026-03-10", 60_000)
            ]
        );
        assert_eq!(sum.window_ms, 270_000);
        assert_eq!(sum.sessions, 4);
        assert!(sum.journalled);
        assert_eq!(sum.total_ms, 109);
        assert_eq!(sum.most_played.as_ref().unwrap().id, "b");
        assert_eq!(sum.per_profile[1].window_ms, 180_000);
        let l = sum.longest.unwrap();
        assert_eq!(
            (l.ms, l.profile_id.as_str(), l.name),
            (500_000, "gone", None)
        );
    }

    #[test]
    fn empty_summary() {
        let (_t, s) = setup();
        let sum = s.summary(&[], 14);
        assert_eq!(sum.days.len(), 14);
        assert!(!sum.journalled && sum.longest.is_none() && sum.most_played.is_none());
        let v = serde_json::to_value(&sum).unwrap();
        assert!(v.get("windowMs").is_some());
    }
}
