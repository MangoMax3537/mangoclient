//! The Minecraft version range a server accepts, derived from the `version`
//! object of a Server List Ping response. Pure: no I/O.
//!
//! Server software writes the range into the version *name* in many shapes
//! (`VincentVanilla 1.21.5 - 26.x`, `Gate 1.7.2-1.7.5-26.3`, `Paper 1.21.4`,
//! `Velocity 1.7-1.21`). Every plausible version token in the name is
//! collected; the lowest and the highest bound the range. Without any token
//! the numeric protocol is mapped through a table of release protocols.

use serde::Serialize;
use std::cmp::Ordering;

/// Inclusive range of game versions. `max` may end in `.x` (any patch or
/// minor of that line, for example `26.x`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VersionRange {
    pub min: String,
    pub max: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    /// Numeric segments; a wildcard segment ends the list and sets `wild`.
    parts: Vec<u32>,
    wild: bool,
}

impl Token {
    /// `1.21.0` is `1.21`; a wildcard keeps `.x` only as an upper bound.
    fn text(&self, as_max: bool) -> String {
        let mut parts = self.parts.clone();
        if !self.wild && parts.len() == 3 && parts[2] == 0 {
            parts.pop();
        }
        let mut s = parts
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(".");
        if self.wild && as_max {
            s.push_str(".x");
        }
        s
    }

    /// Ordering key: a wildcard sorts after every concrete version below it.
    fn cmp_key(&self, other: &Token) -> Ordering {
        for i in 0..3 {
            let a = self.parts.get(i).copied();
            let b = other.parts.get(i).copied();
            let ord = match (a, b) {
                (Some(a), Some(b)) => a.cmp(&b),
                (None, None) => Ordering::Equal,
                (None, Some(_)) => {
                    if self.wild {
                        Ordering::Greater
                    } else {
                        Ordering::Less
                    }
                }
                (Some(_), None) => {
                    if other.wild {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    }
                }
            };
            if ord != Ordering::Equal {
                return ord;
            }
        }
        Ordering::Equal
    }
}

fn plausible(parts: &[u32]) -> bool {
    match parts {
        [1, minor, ..] => *minor <= 40,
        // The year-based scheme (26.1, 26.3.1).
        [major, ..] => (25..=99).contains(major),
        [] => false,
    }
}

fn read_num(b: &[u8], i: &mut usize) -> Option<u32> {
    let start = *i;
    while *i < b.len() && b[*i].is_ascii_digit() && *i - start < 3 {
        *i += 1;
    }
    if *i == start {
        return None;
    }
    std::str::from_utf8(&b[start..*i]).ok()?.parse().ok()
}

fn tokens(name: &str) -> Vec<Token> {
    let b = name.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let boundary = i == 0 || !b[i - 1].is_ascii_alphanumeric();
        if !(b[i].is_ascii_digit() && boundary) {
            i += 1;
            continue;
        }
        let mut parts = Vec::new();
        let mut wild = false;
        let mut j = i;
        let Some(first) = read_num(b, &mut j) else {
            i += 1;
            continue;
        };
        parts.push(first);
        while parts.len() < 3 && j + 1 < b.len() && b[j] == b'.' {
            if b[j + 1].eq_ignore_ascii_case(&b'x') {
                wild = true;
                j += 2;
                break;
            }
            let mut k = j + 1;
            match read_num(b, &mut k) {
                Some(n) => {
                    parts.push(n);
                    j = k;
                }
                None => break,
            }
        }
        // Not part of a longer dotted number (1.2.3.4, 1.21.1234).
        let glued = j < b.len()
            && (b[j].is_ascii_digit()
                || (b[j] == b'.' && b.get(j + 1).is_some_and(u8::is_ascii_digit)));
        if (parts.len() >= 2 || wild && parts.len() == 1) && !glued && plausible(&parts) {
            out.push(Token { parts, wild });
        }
        i = j.max(i + 1);
    }
    out
}

/// Release protocol numbers: (protocol, first version, last version).
const PROTOCOLS: &[(i64, &str, &str)] = &[
    (4, "1.7.2", "1.7.5"),
    (5, "1.7.6", "1.7.10"),
    (47, "1.8", "1.8.9"),
    (107, "1.9", "1.9"),
    (108, "1.9.1", "1.9.1"),
    (109, "1.9.2", "1.9.2"),
    (110, "1.9.3", "1.9.4"),
    (210, "1.10", "1.10.2"),
    (315, "1.11", "1.11"),
    (316, "1.11.1", "1.11.2"),
    (335, "1.12", "1.12"),
    (338, "1.12.1", "1.12.1"),
    (340, "1.12.2", "1.12.2"),
    (393, "1.13", "1.13"),
    (401, "1.13.1", "1.13.1"),
    (404, "1.13.2", "1.13.2"),
    (477, "1.14", "1.14"),
    (480, "1.14.1", "1.14.1"),
    (485, "1.14.2", "1.14.2"),
    (490, "1.14.3", "1.14.3"),
    (498, "1.14.4", "1.14.4"),
    (573, "1.15", "1.15"),
    (575, "1.15.1", "1.15.1"),
    (578, "1.15.2", "1.15.2"),
    (735, "1.16", "1.16"),
    (736, "1.16.1", "1.16.1"),
    (751, "1.16.2", "1.16.2"),
    (753, "1.16.3", "1.16.3"),
    (754, "1.16.4", "1.16.5"),
    (755, "1.17", "1.17"),
    (756, "1.17.1", "1.17.1"),
    (757, "1.18", "1.18.1"),
    (758, "1.18.2", "1.18.2"),
    (759, "1.19", "1.19"),
    (760, "1.19.1", "1.19.2"),
    (761, "1.19.3", "1.19.3"),
    (762, "1.19.4", "1.19.4"),
    (763, "1.20", "1.20.1"),
    (764, "1.20.2", "1.20.2"),
    (765, "1.20.3", "1.20.4"),
    (766, "1.20.5", "1.20.6"),
    (767, "1.21", "1.21.1"),
    (768, "1.21.2", "1.21.3"),
    (769, "1.21.4", "1.21.4"),
    (770, "1.21.5", "1.21.5"),
    (771, "1.21.6", "1.21.6"),
    (772, "1.21.7", "1.21.8"),
    (773, "1.21.9", "1.21.10"),
    (774, "1.21.11", "1.21.11"),
];

/// The versions that speak a given release protocol.
pub fn protocol_range(protocol: i64) -> Option<VersionRange> {
    PROTOCOLS
        .iter()
        .find(|(p, _, _)| *p == protocol)
        .map(|(_, a, b)| VersionRange {
            min: (*a).into(),
            max: (*b).into(),
        })
}

/// The supported range from the ping's version name, falling back to the
/// protocol number. `None` when neither says anything usable.
pub fn parse_version_range(name: &str, protocol: Option<i64>) -> Option<VersionRange> {
    let toks = tokens(name);
    if toks.is_empty() {
        return protocol.and_then(protocol_range);
    }
    let lo = toks.iter().min_by(|a, b| a.cmp_key(b))?;
    let hi = toks.iter().max_by(|a, b| a.cmp_key(b))?;
    Some(VersionRange {
        min: lo.text(false),
        max: hi.text(true),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(name: &str) -> Option<(String, String)> {
        parse_version_range(name, None).map(|v| (v.min, v.max))
    }
    fn range(min: &str, max: &str) -> Option<(String, String)> {
        Some((min.into(), max.into()))
    }

    #[test]
    fn real_looking_names() {
        assert_eq!(r("VincentVanilla 1.21.5 - 26.x"), range("1.21.5", "26.x"));
        assert_eq!(r("Gate 1.7.2-1.7.5-26.3"), range("1.7.2", "26.3"));
        assert_eq!(r("Paper 1.21.4"), range("1.21.4", "1.21.4"));
        assert_eq!(r("Velocity 1.7-1.21"), range("1.7", "1.21"));
        assert_eq!(r("BungeeCord 1.8.x-1.21.x"), range("1.8", "1.21.x"));
        assert_eq!(r("Spigot 1.20.1"), range("1.20.1", "1.20.1"));
        assert_eq!(r("§aHypixel 1.8-1.21"), range("1.8", "1.21"));
        assert_eq!(r("ViaProxy 1.21.11 - 26.3"), range("1.21.11", "26.3"));
        assert_eq!(r("1.21.x"), range("1.21", "1.21.x"));
        assert_eq!(r("Paper 1.21.0"), range("1.21", "1.21"));
        assert_eq!(r("Purpur 26.1.2"), range("26.1.2", "26.1.2"));
        assert_eq!(r("Waterfall 1.21.4 / 1.20.6"), range("1.20.6", "1.21.4"));
    }

    #[test]
    fn unordered_tokens_use_lowest_and_highest() {
        assert_eq!(r("Proxy 26.3 1.8 1.21.5"), range("1.8", "26.3"));
    }

    #[test]
    fn ignores_things_that_are_not_game_versions() {
        assert_eq!(r("MyServer v2.0"), None);
        assert_eq!(r("Velocity 3.4.0-SNAPSHOT"), None);
        assert_eq!(r("Folia"), None);
        assert_eq!(r(""), None);
        assert_eq!(r("Build 1.2.3.4"), None);
        assert_eq!(r("Paper 1.99"), None);
        assert_eq!(r("Lobby 24.5"), None);
    }

    #[test]
    fn protocol_fallback_only_when_the_name_has_no_version() {
        let p = |n: &str, proto| parse_version_range(n, proto).map(|v| (v.min, v.max));
        assert_eq!(p("Folia", Some(774)), range("1.21.11", "1.21.11"));
        assert_eq!(p("Spigot", Some(772)), range("1.21.7", "1.21.8"));
        assert_eq!(p("Paper 1.21.4", Some(774)), range("1.21.4", "1.21.4"));
        assert_eq!(p("Folia", Some(-1)), None);
        assert_eq!(p("Folia", Some(999_999)), None);
        assert_eq!(p("Folia", None), None);
    }
}
