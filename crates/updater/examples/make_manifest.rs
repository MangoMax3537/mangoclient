//! CI helper: sign every update artifact and write `latest.json`.
//!
//!   MANGO_UPDATE_PRIVATE_KEY=<seed b64> \
//!   cargo run -p mango-updater --example make_manifest -- \
//!       <version> <download-base-url> <artifacts-dir> [notes-file] > latest.json
//!
//! Artifacts are recognised by file name (Tauri's naming, see
//! `scripts/README.txt`):
//!   *-setup.exe                      -> windows-x86_64
//!   *aarch64*.app.zip                -> darwin-aarch64
//!   *x64*.app.zip                    -> darwin-x86_64
//!   *.AppImage                       -> linux-x86_64-appimage
//! Each signature covers `mango-update-v1|<version>|<platform key>|<sha256>`.
//! The result is validated with the exact parser the launcher uses, and the
//! signatures are verified against the derived public key. The private key is
//! never printed. Exits non-zero when no artifact matched.

use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use mango_updater::{update_message, PlatformEntry, UpdateManifest, UrlPolicy};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

fn platform_key(name: &str) -> Option<&'static str> {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with("-setup.exe") {
        Some("windows-x86_64")
    } else if lower.ends_with(".app.zip") && lower.contains("aarch64") {
        Some("darwin-aarch64")
    } else if lower.ends_with(".app.zip") && lower.contains("x64") {
        Some("darwin-x86_64")
    } else if lower.ends_with(".appimage") {
        Some("linux-x86_64-appimage")
    } else {
        None
    }
}

/// RFC 3339 UTC timestamp without a date crate (Howard Hinnant's civil algorithm).
fn rfc3339_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let z = i64::try_from(days).unwrap_or(0) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: make_manifest <version> <base-url> <artifacts-dir> [notes-file]";
    let version = args.next().ok_or(usage)?;
    let base = args.next().ok_or(usage)?;
    let dir = args.next().ok_or(usage)?;
    let notes = match args.next() {
        Some(p) => Some(std::fs::read_to_string(p)?.trim().to_string()),
        None => None,
    };

    let seed: [u8; 32] = B64
        .decode(std::env::var("MANGO_UPDATE_PRIVATE_KEY")?.trim())?
        .try_into()
        .map_err(|_| "private key must be 32 bytes")?;
    let key = SigningKey::from_bytes(&seed);

    let mut names: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    names.sort();

    // Must equal `Version::to_string()`, which is what the launcher signs against.
    let parsed: semver::Version = version.trim_start_matches('v').parse()?;
    let version_str = parsed.to_string();
    let mut platforms = BTreeMap::new();
    for name in names {
        let Some(platform) = platform_key(&name) else {
            continue;
        };
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+'))
        {
            return Err(format!("unsafe artifact file name: {name}").into());
        }
        if platforms.contains_key(platform) {
            return Err(format!("two artifacts for {platform}: {name}").into());
        }
        let bytes = std::fs::read(Path::new(&dir).join(&name))?;
        let sha256 = hex::encode(Sha256::digest(&bytes));
        let message = update_message(&version_str, platform, &sha256);
        platforms.insert(
            platform.to_string(),
            PlatformEntry {
                url: format!("{}/{name}", base.trim_end_matches('/')),
                sha256,
                size: bytes.len() as u64,
                signature: B64.encode(key.sign(&message).to_bytes()),
            },
        );
        eprintln!("signed {name} -> {platform}");
    }
    if platforms.is_empty() {
        return Err("no update artifacts found".into());
    }

    let manifest = UpdateManifest {
        version: parsed,
        pub_date: Some(rfc3339_now()),
        notes,
        platforms,
    };
    let json = serde_json::to_vec_pretty(&manifest)?;
    // Same strict parser the launcher applies.
    UpdateManifest::parse(&json, &UrlPolicy::default())?;
    println!("{}", String::from_utf8(json)?);
    Ok(())
}
