//! Pure URL and path policy, ported from `src/main/security.js`
//! (Electron-only pieces such as IPC sender checks are not ported).

use std::path::{Component, Path, PathBuf};

use url::Url;

use crate::error::{Error, Result};

pub const IMAGE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "gif"];
pub const LEGACY_API: &str = "http://94.249.184.45:8880";

/// `file://` URL of a path (made absolute first).
pub fn renderer_url(file: &Path) -> Option<Url> {
    let abs = std::path::absolute(file).ok()?;
    Url::from_file_path(abs).ok()
}

/// True only when `url` is the file URL of exactly `file`: no credentials,
/// host, query or fragment.
pub fn is_exact_renderer_url(url: &str, file: &Path) -> bool {
    let (Ok(actual), Some(expected)) = (Url::parse(url), renderer_url(file)) else {
        return false;
    };
    actual.scheme() == "file"
        && actual.username().is_empty()
        && actual.password().is_none()
        && actual.host_str().is_none_or(str::is_empty)
        && actual.query().is_none()
        && actual.fragment().is_none()
        && actual.path() == expected.path()
}

/// External links open only over HTTPS and without embedded credentials.
pub fn is_allowed_external_url(raw: &str) -> bool {
    match Url::parse(raw) {
        Ok(u) => u.scheme() == "https" && u.username().is_empty() && u.password().is_none(),
        Err(_) => false,
    }
}

/// Validate the presence API base. `None`/empty means the legacy endpoint.
/// Only a bare HTTPS origin, or exactly the legacy URL, is accepted. Trailing
/// slashes are stripped.
pub fn validated_api_base(raw: Option<&str>) -> Result<String> {
    let raw = raw.filter(|s| !s.is_empty()).unwrap_or(LEGACY_API);
    let value = raw.trim_end_matches('/');
    let url = Url::parse(value).map_err(|_| Error::ApiUrlInvalid)?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || url.host_str().is_none()
    {
        return Err(Error::ApiUrlNotOrigin);
    }
    if url.scheme() != "https" && value != LEGACY_API {
        return Err(Error::ApiUrlNotHttps);
    }
    Ok(value.to_string())
}

fn normalize(path: &Path) -> PathBuf {
    let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut out = PathBuf::new();
    for c in abs.components() {
        match c {
            Component::ParentDir => {
                if !out.pop() {
                    out.push(c);
                }
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// True when `target` is strictly inside `root` (lexically; equal paths,
/// `..` escapes and sibling prefixes like `/root-evil` are rejected).
pub fn contains_path(root: &Path, target: &Path) -> bool {
    let root = normalize(root);
    let target = normalize(target);
    match target.strip_prefix(&root) {
        Ok(rest) => rest.components().next().is_some(),
        Err(_) => false,
    }
}

/// Resolve `file` to its real path if it is an image (extension allow-list),
/// a regular file, and its real path lies inside the real path of one of
/// `roots`. Symlinks that escape the roots are rejected.
pub fn approved_image_path(file: &Path, roots: &[PathBuf]) -> Option<PathBuf> {
    let ext = file.extension()?.to_str()?.to_ascii_lowercase();
    if !IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        return None;
    }
    let real = std::fs::canonicalize(std::path::absolute(file).ok()?).ok()?;
    if !std::fs::metadata(&real).ok()?.is_file() {
        return None;
    }
    roots.iter().find_map(|root| {
        // An optional image directory may not exist yet.
        let real_root = std::fs::canonicalize(root).ok()?;
        contains_path(&real_root, &real).then(|| real.clone())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_urls() {
        assert!(is_allowed_external_url("https://modrinth.com/mod/x"));
        for bad in [
            "http://modrinth.com",
            "https://user:pw@example.com",
            "https://user@example.com",
            "javascript:alert(1)",
            "file:///etc/passwd",
            "not a url",
            "",
        ] {
            assert!(!is_allowed_external_url(bad), "{bad}");
        }
    }

    #[test]
    fn api_base_table() {
        assert_eq!(
            validated_api_base(Some("https://api.mangoclient.gg/")).unwrap(),
            "https://api.mangoclient.gg"
        );
        assert_eq!(validated_api_base(None).unwrap(), LEGACY_API);
        assert_eq!(validated_api_base(Some("")).unwrap(), LEGACY_API);
        assert_eq!(validated_api_base(Some(LEGACY_API)).unwrap(), LEGACY_API);
        assert!(matches!(
            validated_api_base(Some("http://example.com")),
            Err(Error::ApiUrlNotHttps)
        ));
        assert!(matches!(
            validated_api_base(Some("http://94.249.184.45:8881")),
            Err(Error::ApiUrlNotHttps)
        ));
        for bad in [
            "https://example.com/path",
            "https://example.com/?a=1",
            "https://example.com/#x",
            "https://u:p@example.com",
        ] {
            assert!(
                matches!(validated_api_base(Some(bad)), Err(Error::ApiUrlNotOrigin)),
                "{bad}"
            );
        }
        assert!(matches!(
            validated_api_base(Some("nonsense")),
            Err(Error::ApiUrlInvalid)
        ));
    }

    #[test]
    fn contains_path_table() {
        let root = Path::new("/data/covers");
        assert!(contains_path(root, Path::new("/data/covers/a.png")));
        assert!(contains_path(root, Path::new("/data/covers/sub/a.png")));
        assert!(!contains_path(root, Path::new("/data/covers")));
        assert!(!contains_path(
            root,
            Path::new("/data/covers/../secret.png")
        ));
        assert!(!contains_path(root, Path::new("/data/covers-evil/a.png")));
        assert!(!contains_path(root, Path::new("/etc/passwd")));
    }

    #[test]
    fn exact_renderer_url() {
        // an absolute path on each platform (`/app/...` is not absolute on Windows)
        #[cfg(windows)]
        let (f, base) = (Path::new(r"C:\app\ui\index.html"), "C:/app/ui");
        #[cfg(not(windows))]
        let (f, base) = (Path::new("/app/ui/index.html"), "app/ui");
        assert!(is_exact_renderer_url(
            &format!("file:///{base}/index.html"),
            f
        ));
        for bad in [
            format!("file:///{base}/index.html?x=1"),
            format!("file:///{base}/index.html#x"),
            format!("file:///{base}/other.html"),
            "https://app/ui/index.html".to_string(),
            // a real host is refused (on Windows `file://host/C:/...` is not one: the URL standard drops a host before a
            // drive letter, so it parses to exactly `file:///C:/...`, the same local file)
            "file://host/app/ui/index.html".to_string(),
            "garbage".to_string(),
        ] {
            assert!(!is_exact_renderer_url(&bad, f), "{bad}");
        }
    }

    #[test]
    fn image_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("covers");
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let good = root.join("a.PNG");
        let bad = outside.join("b.png");
        std::fs::write(&good, b"x").unwrap();
        std::fs::write(&bad, b"x").unwrap();
        std::fs::write(root.join("note.txt"), b"x").unwrap();
        let roots = vec![root.clone(), tmp.path().join("missing")];

        assert_eq!(
            approved_image_path(&good, &roots),
            Some(std::fs::canonicalize(&good).unwrap())
        );
        assert_eq!(approved_image_path(&bad, &roots), None);
        assert!(!contains_path(&root, &bad));
        assert_eq!(approved_image_path(&root.join("note.txt"), &roots), None);
        assert_eq!(approved_image_path(&root.join("gone.png"), &roots), None);
        assert_eq!(approved_image_path(&root, &roots), None);

        #[cfg(unix)]
        {
            let link = root.join("link.png");
            std::os::unix::fs::symlink(&bad, &link).unwrap();
            assert_eq!(approved_image_path(&link, &roots), None);
        }
    }
}
