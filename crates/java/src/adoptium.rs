use crate::discover::{java_executable, launch_executable};
use crate::error::JavaError;
use crate::resolve::ResolveOptions;
use mango_archive::Limits;
use mango_net::{download_file, get_json, DownloadOptions, Expected, FetchOptions};
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const DEFAULT_ADOPTIUM_BASE: &str = "https://api.adoptium.net/v3";

#[derive(Debug, Deserialize)]
struct Asset {
    binary: Binary,
}
#[derive(Debug, Deserialize)]
struct Binary {
    package: Package,
}
#[derive(Debug, Deserialize)]
struct Package {
    name: String,
    link: String,
    #[serde(default)]
    checksum: Option<String>,
    #[serde(default)]
    size: Option<u64>,
}

/// The name must be a bare filename: no separators, no `.`/`..`, no NUL.
/// https only: the Adoptium API, GitHub releases and the hosts GitHub
/// redirects release assets to.
pub fn default_link_policy() -> mango_net::HostPolicy {
    mango_net::HostPolicy::https_hosts(&[
        "api.adoptium.net",
        "github.com",
        "objects.githubusercontent.com",
        "release-assets.githubusercontent.com",
    ])
}

fn valid_package_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', '\0'])
        && Path::new(name).file_name().is_some_and(|f| f == name)
}

/// Download and unpack a Temurin JRE into `runtimes/jre-<major>`. Returns the
/// existing runtime without any network access when one is already there.
pub async fn download_runtime(major: u32, opts: &ResolveOptions) -> Result<PathBuf, JavaError> {
    let platform = opts.platform;
    let dir = opts.runtimes_dir.join(format!("jre-{major}"));
    if let Some(p) = java_executable(&dir, platform, false) {
        return Ok(launch_executable(&p, platform, opts.prefer_javaw));
    }

    let url = format!(
        "{}/assets/latest/{major}/hotspot?architecture={}&image_type=jre&os={}&vendor=eclipse",
        opts.adoptium_base.trim_end_matches('/'),
        platform.adoptium_arch(),
        platform.adoptium_os()
    );
    let fetch = FetchOptions {
        host_policy: Some(opts.link_policy.clone()),
        ..FetchOptions::default()
    };
    let assets: Vec<Asset> = get_json(&url, &fetch).await?;
    let Some(asset) = assets.into_iter().next() else {
        return Err(JavaError::NoRuntime {
            major,
            os: platform.adoptium_os(),
            arch: platform.adoptium_arch(),
        });
    };
    let pkg = asset.binary.package;
    if !valid_package_name(&pkg.name) {
        return Err(JavaError::InvalidPackageName(pkg.name));
    }

    tokio::fs::create_dir_all(&opts.runtimes_dir).await?;
    tokio::fs::create_dir_all(&opts.cache_dir).await?;
    let archive = opts.cache_dir.join(&pkg.name);
    download_file(
        &pkg.link,
        &archive,
        &DownloadOptions {
            expected: Expected {
                sha256: pkg.checksum,
                size: pkg.size,
                ..Default::default()
            },
            progress: opts.progress.clone(),
            fetch,
            ..Default::default()
        },
    )
    .await?;

    // Unpack next to the final folder and move it into place only when complete: an interrupted unpack (Stop, crash,
    // power loss) must never leave a half runtime that the "already installed" check above would accept next time.
    let staging = opts.runtimes_dir.join(format!(".jre-{major}.partial"));
    let (a, d, target) = (archive.clone(), staging.clone(), dir.clone());
    let extracted = tokio::task::spawn_blocking(move || -> Result<(), JavaError> {
        let _ = std::fs::remove_dir_all(&d);
        let file = std::fs::File::open(&a)?;
        if a.extension().is_some_and(|e| e == "zip") {
            mango_archive::extract_zip(file, &d, &Limits::default())?;
        } else {
            mango_archive::extract_tar_gz(std::io::BufReader::new(file), &d, &Limits::default())?;
        }
        if java_executable(&d, platform, false).is_none() {
            let _ = std::fs::remove_dir_all(&d);
            return Err(JavaError::NoExecutable);
        }
        let _ = std::fs::remove_dir_all(&target);
        std::fs::rename(&d, &target)?;
        Ok(())
    })
    .await
    .map_err(|e| JavaError::Join(e.to_string()))?;
    let _ = tokio::fs::remove_file(&archive).await;
    if extracted.is_err() {
        let _ = tokio::fs::remove_dir_all(&staging).await;
    }
    extracted?;

    let exe = java_executable(&dir, platform, false).ok_or(JavaError::NoExecutable)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755));
    }
    Ok(launch_executable(&exe, platform, opts.prefer_javaw))
}

#[cfg(test)]
mod tests {
    use super::valid_package_name;

    #[test]
    fn package_names() {
        assert!(valid_package_name(
            "OpenJDK21U-jre_x64_linux_hotspot.tar.gz"
        ));
        for bad in ["", "..", ".", "a/b.tar.gz", "../x", "a\\b", "x\0y"] {
            assert!(!valid_package_name(bad), "{bad:?}");
        }
    }
}
