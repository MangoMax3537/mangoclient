//! Removing private data from text that leaves the machine (diagnostics
//! exports, crash excerpts shown for sharing).
//!
//! What goes, in this order:
//!
//! 1. values of launch arguments that carry credentials or identity
//!    (`--accessToken`, `--session`, `--xuid`, `--uuid`, `--username`,
//!    `--userProperties`, `--clientId`), in any spelling the launcher or a
//!    JSON/list rendering of the command line produces,
//! 2. JWTs, `Bearer` tokens, the value of any `key: value` / `key=value`
//!    whose key names a token, secret, password, API key, authorization or
//!    session id,
//! 3. long opaque strings that look like a token (40 or more base64url
//!    characters with a digit, an upper and a lower case letter),
//! 4. e-mail addresses,
//! 5. the home directory (`~`), in every slash style, plus any other
//!    `/home/<name>`, `/Users/<name>` or `C:\Users\<name>`,
//! 6. account names, the OS user name and account UUIDs.
//!
//! The function is pure and idempotent.

use regex::Regex;
use std::sync::OnceLock;

pub(crate) const REDACTED: &str = "<redacted>";

/// Who and where the text came from.
#[derive(Debug, Clone, Default)]
pub(crate) struct ScrubContext {
    /// Home directories, as written on this machine.
    pub homes: Vec<String>,
    /// Minecraft account names and the OS user name.
    pub names: Vec<String>,
    /// Account UUIDs (dashed or not).
    pub uuids: Vec<String>,
}

/// OS user names that are also ordinary words in logs.
const COMMON_WORDS: [&str; 10] = [
    "root",
    "user",
    "admin",
    "minecraft",
    "java",
    "home",
    "guest",
    "default",
    "public",
    "mango",
];

impl ScrubContext {
    /// The current machine's home and user name, plus the given accounts.
    pub(crate) fn for_machine(account_names: Vec<String>, uuids: Vec<String>) -> Self {
        let mut homes = Vec::new();
        for var in ["HOME", "USERPROFILE"] {
            if let Some(h) = std::env::var_os(var) {
                let h = h
                    .to_string_lossy()
                    .trim_end_matches(['/', '\\'])
                    .to_string();
                if h.len() > 1 && !homes.contains(&h) {
                    homes.push(h);
                }
            }
        }
        let mut names = account_names;
        for var in ["USER", "USERNAME", "LOGNAME"] {
            if let Ok(u) = std::env::var(var) {
                if u.chars().count() >= 4 && !COMMON_WORDS.contains(&u.to_lowercase().as_str()) {
                    names.push(u);
                }
            }
        }
        ScrubContext {
            homes,
            names,
            uuids,
        }
    }
}

struct Rules {
    args: Regex,
    jwt: Regex,
    bearer: Regex,
    secret: Regex,
    opaque: Regex,
    email: Regex,
    win_home: Regex,
    posix_home: Regex,
}

fn rules() -> Option<&'static Rules> {
    static R: OnceLock<Option<Rules>> = OnceLock::new();
    R.get_or_init(|| {
        Some(Rules {
            args: Regex::new(concat!(
                r#"(?i)(--(?:accesstoken|session|xuid|userproperties|uuid|clientid|username)"#,
                r#"["']?(?:\s*[=,]\s*|\s+))("[^"\r\n]*"|'[^'\r\n]*'|\{\S*|[^\s,"'\]\)]+)"#
            ))
            .ok()?,
            jwt: Regex::new(r"eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}(?:\.[A-Za-z0-9_-]*)?")
                .ok()?,
            bearer: Regex::new(r"(?i)\bBearer\s+[A-Za-z0-9._~+/=-]{6,}").ok()?,
            secret: Regex::new(concat!(
                r#"(?i)(\b[\w.-]*(?:token|secret|passw(?:or)?d|api[_-]?key|authorization|"#,
                r#"session[_-]?id|credential)[\w.-]*["']?\s*[:=]\s*)"#,
                r#"("[^"\r\n]*"|'[^'\r\n]*'|[^\s"',;&})\]]+)"#
            ))
            .ok()?,
            opaque: Regex::new(r"[A-Za-z0-9_+=-]{40,}").ok()?,
            email: Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}")
                .ok()?,
            win_home: Regex::new(r#"(?i)\b[A-Z]:[\\/]+Users[\\/]+[^\\/\s"':*?<>|]+"#).ok()?,
            posix_home: Regex::new(r#"(^|[\s"'=:(\[<,;])(?:/home/[^/\s"':]+|/Users/[^/\s"':]+)"#)
                .ok()?,
        })
    })
    .as_ref()
}

/// Replace the matches of `re` for which `f(match, previous char, next char)`
/// returns a replacement.
fn replace_bounded(
    text: &str,
    re: &Regex,
    f: impl Fn(&str, Option<char>, Option<char>) -> Option<String>,
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for m in re.find_iter(text) {
        let prev = text[..m.start()].chars().next_back();
        let next = text[m.end()..].chars().next();
        if let Some(rep) = f(m.as_str(), prev, next) {
            out.push_str(&text[last..m.start()]);
            out.push_str(&rep);
            last = m.end();
        }
    }
    out.push_str(&text[last..]);
    out
}

fn word_char(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// Every way `home` can show up in a log: both slash styles and the doubled
/// backslashes of JSON and Java string literals.
fn home_forms(home: &str) -> Vec<String> {
    let fwd = home.replace('\\', "/");
    let back = home.replace('/', "\\");
    let mut forms = vec![
        home.to_string(),
        fwd.clone(),
        back.clone(),
        back.replace('\\', "\\\\"),
    ];
    // `file:///home/x` keeps the path part; percent-encoded spaces too.
    forms.push(fwd.replace(' ', "%20"));
    forms.sort_by_key(|f| std::cmp::Reverse(f.len()));
    forms.dedup();
    forms
}

/// Remove private data from `text` (see the module docs).
pub(crate) fn scrub(text: &str, ctx: &ScrubContext) -> String {
    let Some(r) = rules() else {
        // The patterns are constants and tested; if they ever fail to build,
        // leaking the text would be worse than losing it.
        return "<text withheld: scrubbing is unavailable>".to_string();
    };
    let mut s = r.args.replace_all(text, "${1}<redacted>").into_owned();
    s = r.jwt.replace_all(&s, REDACTED).into_owned();
    s = r.bearer.replace_all(&s, "Bearer <redacted>").into_owned();
    s = r.secret.replace_all(&s, "${1}<redacted>").into_owned();
    s = replace_bounded(&s, &r.opaque, |m, _, _| {
        let digit = m.chars().any(|c| c.is_ascii_digit());
        let upper = m.chars().any(|c| c.is_ascii_uppercase());
        let lower = m.chars().any(|c| c.is_ascii_lowercase());
        (digit && upper && lower).then(|| REDACTED.to_string())
    });
    s = r.email.replace_all(&s, "<email>").into_owned();

    for home in &ctx.homes {
        for form in home_forms(home) {
            let windows = form.contains(':') || form.contains('\\');
            let pat = format!(
                "{}{}",
                if windows { "(?i)" } else { "" },
                regex::escape(&form)
            );
            let Ok(re) = Regex::new(&pat) else { continue };
            // Not `/home/bobby` when the home is `/home/bob`.
            s = replace_bounded(&s, &re, |_, _, next| {
                let glued = next.is_some_and(|c| c.is_alphanumeric() || "_.-".contains(c));
                (!glued).then(|| "~".to_string())
            });
        }
    }
    s = r.win_home.replace_all(&s, "~").into_owned();
    s = r.posix_home.replace_all(&s, "${1}~").into_owned();

    let mut names: Vec<&String> = ctx
        .names
        .iter()
        .filter(|n| n.chars().count() >= 3)
        .collect();
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    for name in names {
        let Ok(re) = Regex::new(&format!("(?i){}", regex::escape(name))) else {
            continue;
        };
        s = replace_bounded(&s, &re, |_, prev, next| {
            (!word_char(prev) && !word_char(next)).then(|| "<user>".to_string())
        });
    }
    for uuid in &ctx.uuids {
        let plain = uuid.replace('-', "");
        for form in [uuid.clone(), plain] {
            if form.len() < 32 {
                continue;
            }
            if let Ok(re) = Regex::new(&format!("(?i){}", regex::escape(&form))) {
                s = re.replace_all(&s, "<uuid>").into_owned();
            }
        }
    }
    s
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const JWT: &str = "eyJhbGciOiJSUzI1NiJ9.eyJ4dWlkIjoiMjUzNTQyIiwiYWdnIjoiQWR1bHQifQ.c2lnbmF0dXJlLXBhcnQtaGVyZQ";

    fn ctx() -> ScrubContext {
        ScrubContext {
            homes: vec!["/home/bob".into(), "C:\\Users\\Bob Builder".into()],
            names: vec!["Steve_99".into(), "bobby".into()],
            uuids: vec!["069a79f4-44e9-4726-a5be-fca90e38aaf5".into()],
        }
    }

    fn clean(s: &str) -> String {
        scrub(s, &ctx())
    }

    #[test]
    fn launch_arguments_lose_their_values_in_every_rendering() {
        for (input, secret) in [
            (
                "java --accessToken abc123SECRET --version 1.21",
                "abc123SECRET",
            ),
            (
                "java --accessToken=abc123SECRET --version 1.21",
                "abc123SECRET",
            ),
            (
                "java --ACCESSTOKEN abc123SECRET --version 1.21",
                "abc123SECRET",
            ),
            (
                "java --accessToken \"abc 123 SECRET\" --version 1.21",
                "123 SECRET",
            ),
            (
                "java --accessToken 'abc123SECRET' --version 1.21",
                "abc123SECRET",
            ),
            (
                "args: [--accessToken, abc123SECRET, --version, 1.21]",
                "abc123SECRET",
            ),
            (
                "{\"args\":[\"--accessToken\",\"abc123SECRET\"]}",
                "abc123SECRET",
            ),
            (
                "java --accessToken\nabc123SECRET\n--version 1.21",
                "abc123SECRET",
            ),
            (
                "java --session token:abc123SECRET:xyz --version 1.21",
                "abc123SECRET",
            ),
            (
                "java --xuid 2535421234567890 --version 1.21",
                "2535421234567890",
            ),
            ("java --username Steve_99 --version 1.21", "Steve_99"),
            (
                "java --userProperties {\"a\":[\"b\"]} --version 1.21",
                "\"a\"",
            ),
        ] {
            let out = clean(input);
            assert!(!out.contains(secret), "{input:?} -> {out:?}");
            assert!(out.contains(REDACTED), "{input:?} -> {out:?}");
        }
        // Other arguments survive.
        let out = clean("--version 1.21 --accessToken zzz --gameDir /x --width 854");
        assert!(
            out.contains("--version 1.21") && out.contains("--width 854"),
            "{out}"
        );
    }

    #[test]
    fn tokens_in_json_headers_urls_and_bare_form_are_removed() {
        for (input, secret) in [
            (format!("{{\"accessToken\":\"{JWT}\"}}"), JWT),
            (format!("Authorization: Bearer {JWT}"), JWT),
            ("Authorization:Bearer abcDEF123456".to_string(), "abcDEF123456"),
            ("GET /x?access_token=s3cr3tvalue&x=1".to_string(), "s3cr3tvalue"),
            ("\"msa_refresh_token\": \"M.R3_BAY.-CRz!secret\"".to_string(), "M.R3_BAY.-CRz!secret"),
            ("password=hunter2 user=x".to_string(), "hunter2"),
            ("X-Api-Key: 0123456789abcdef".to_string(), "0123456789abcdef"),
            ("\"password\": \"my pass phrase\"".to_string(), "pass phrase"),
            (format!("legacy session token:{JWT}:069a79f444e94726a5befca90e38aaf5"), JWT),
            (
                "leaked EwBwA8l6BAAUO9chh8cJscQLmU+LSWpbnr0vmwwAAXRDZ1Qg9rCQLUY0lMEdBWvYXWpq7R3zD6 end"
                    .to_string(),
                "EwBwA8l6BAAUO9chh8cJscQLmU",
            ),
        ] {
            let out = clean(&input);
            assert!(!out.contains(secret), "{input:?} -> {out:?}");
        }
        assert!(clean("client secret: abc").contains(REDACTED));
    }

    #[test]
    fn ordinary_long_identifiers_and_hashes_are_kept() {
        let line = "at com.mojang.blaze3d.systems.RenderSystem.flipFrame(RenderSystem.java:120)";
        assert_eq!(clean(line), line);
        let sha = "sha1 2fd12e3c3ee2c1b1d2a0c9b1f5d3a4b5c6d7e8f9 sha512 \
                   cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e";
        assert_eq!(clean(sha), sha);
        let jar = "Loaded sodium-fabric-0.5.8+mc1.21.jar and fabric-api-0.100.0+1.21.jar";
        assert_eq!(clean(jar), jar);
        let long = "a".repeat(60);
        assert_eq!(clean(&long), long, "no digit/upper mix: not a token");
    }

    #[test]
    fn home_directories_become_a_tilde_in_every_style() {
        let cases = [
            (
                "/home/bob/.mangoclient/instances/x",
                "~/.mangoclient/instances/x",
            ),
            ("path=\"/home/bob\"", "path=\"~\""),
            (
                "C:\\Users\\Bob Builder\\AppData\\Roaming",
                "~\\AppData\\Roaming",
            ),
            ("c:\\users\\bob builder\\x", "~\\x"),
            ("C:/Users/Bob Builder/x", "~/x"),
            ("\"C:\\\\Users\\\\Bob Builder\\\\x\"", "\"~\\\\x\""),
            ("file:///home/bob/a.png", "file://~/a.png"),
            // Somebody else's home and the other systems' layouts.
            ("/home/alice/.minecraft", "~/.minecraft"),
            (
                "/Users/carol/Library/Application Support",
                "~/Library/Application Support",
            ),
            ("D:\\Users\\dave\\x", "~\\x"),
        ];
        for (input, want) in cases {
            assert_eq!(clean(input), want, "{input}");
        }
        // A sibling that merely starts with the home is not the home.
        assert_eq!(clean("/home/bobby2/x"), "~/x");
        assert!(!clean("/home/bobby2/x").contains("bobby"));
        // Unrelated paths stay.
        assert_eq!(clean("/usr/lib/jvm/java-21"), "/usr/lib/jvm/java-21");
        assert_eq!(clean("/opt/home/x"), "/opt/home/x");
    }

    #[test]
    fn names_uuids_and_emails_are_replaced_as_whole_words() {
        let out = clean(
            "[CHAT] <Steve_99> hi; Setting user: Steve_99; uuid 069a79f444e94726a5befca90e38aaf5 \
             and 069A79F4-44E9-4726-A5BE-FCA90E38AAF5; mail steve@example.com; bobby_jr stays",
        );
        assert!(!out.contains("Steve_99"), "{out}");
        assert!(
            !out.contains("069a79f4") && !out.contains("069A79F4"),
            "{out}"
        );
        assert!(!out.contains("example.com"), "{out}");
        assert!(
            out.contains("bobby_jr"),
            "a longer word is not the name: {out}"
        );
        assert_eq!(clean("steve_99,"), "<user>,");
        assert_eq!(clean("Steve_99Steve_99"), "Steve_99Steve_99");
    }

    #[test]
    fn scrubbing_is_idempotent_and_leaves_plain_text_alone() {
        let nasty = format!(
            "java --accessToken {JWT} -Djava.io.tmpdir=/home/bob/tmp C:\\Users\\Bob Builder\\x \
             token=abc Steve_99 bob@mail.org"
        );
        let once = clean(&nasty);
        assert_eq!(clean(&once), once);
        let plain = "[Render thread/INFO]: Reloading ResourceManager: vanilla, fabric";
        assert_eq!(clean(plain), plain);
        assert_eq!(clean(""), "");
    }

    #[test]
    fn a_huge_single_line_is_handled() {
        let mut line = String::new();
        for i in 0..20_000 {
            line.push_str(&format!("--arg{i} v{i} "));
        }
        line.push_str("--accessToken SECRETSECRET1");
        let out = clean(&line);
        assert!(!out.contains("SECRETSECRET1"));
    }
}
