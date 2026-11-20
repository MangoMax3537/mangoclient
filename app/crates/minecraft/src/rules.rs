//! Mojang rule evaluation: last match wins, default deny once any rule exists.

use crate::host::HostEnv;
use serde_json::Value;
use std::collections::HashMap;

pub type Features = HashMap<String, bool>;

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

fn match_os(os_rule: Option<&Value>, env: &HostEnv) -> bool {
    let Some(rule) = os_rule.filter(|v| !v.is_null()) else {
        return true;
    };
    if let Some(name) = rule
        .get("name")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        if name != env.os.rule_name() {
            return false;
        }
    }
    if let Some(arch) = rule
        .get("arch")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        if arch != env.arch.rule_name() {
            return false;
        }
    }
    if let Some(version) = rule
        .get("version")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        // An unparsable pattern never matches rather than aborting the launch.
        match regex::Regex::new(version) {
            Ok(re) if re.is_match(&env.os_version) => {}
            _ => return false,
        }
    }
    true
}

/// Evaluate a `rules` array. Missing or empty means allowed.
pub fn rules_allow(rules: Option<&Value>, features: &Features, env: &HostEnv) -> bool {
    let Some(list) = rules.and_then(Value::as_array).filter(|l| !l.is_empty()) else {
        return true;
    };
    let mut allowed = false;
    for rule in list {
        let mut applies = match_os(rule.get("os"), env);
        if applies {
            if let Some(wanted) = rule.get("features").and_then(Value::as_object) {
                for (key, want) in wanted {
                    let have = features.get(key).copied().unwrap_or(false);
                    if have != truthy(want) {
                        applies = false;
                        break;
                    }
                }
            }
        }
        if applies {
            allowed = rule.get("action").and_then(Value::as_str) == Some("allow");
        }
    }
    allowed
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::host::{HostArch, HostOs};
    use serde_json::json;

    fn linux() -> HostEnv {
        HostEnv {
            os: HostOs::Linux,
            arch: HostArch::X64,
            os_version: "6.1.0-arch".into(),
        }
    }

    fn allow(rules: Value, features: &[(&str, bool)], env: &HostEnv) -> bool {
        let f: Features = features.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        rules_allow(Some(&rules), &f, env)
    }

    #[test]
    fn no_rules_allow_and_default_is_deny() {
        let env = linux();
        assert!(rules_allow(None, &Features::new(), &env));
        assert!(rules_allow(Some(&json!([])), &Features::new(), &env));
        assert!(!allow(
            json!([{"action":"allow","os":{"name":"windows"}}]),
            &[],
            &env
        ));
    }

    #[test]
    fn last_matching_rule_wins() {
        let env = linux();
        let r = json!([
            {"action":"allow"},
            {"action":"disallow","os":{"name":"linux"}}
        ]);
        assert!(!allow(r, &[], &env));
        let r = json!([
            {"action":"disallow","os":{"name":"linux"}},
            {"action":"allow"}
        ]);
        assert!(allow(r, &[], &env));
    }

    #[test]
    fn arch_and_version_regex() {
        let env = linux();
        assert!(allow(
            json!([{"action":"allow","os":{"arch":"x64"}}]),
            &[],
            &env
        ));
        // Mojang's "x86" means 32-bit and must not match an x64 host.
        assert!(!allow(
            json!([{"action":"allow","os":{"arch":"x86"}}]),
            &[],
            &env
        ));
        assert!(allow(
            json!([{"action":"allow","os":{"name":"linux","version":"^6\\."}}]),
            &[],
            &env
        ));
        assert!(!allow(
            json!([{"action":"allow","os":{"version":"^10\\."}}]),
            &[],
            &env
        ));
    }

    #[test]
    fn features_must_match_both_ways() {
        let env = linux();
        let r = json!([{"action":"allow","features":{"has_custom_resolution":true}}]);
        assert!(allow(r.clone(), &[("has_custom_resolution", true)], &env));
        assert!(!allow(r.clone(), &[], &env));
        let r = json!([{"action":"allow","features":{"is_demo_user":false}}]);
        assert!(allow(r, &[], &env));
    }
}
