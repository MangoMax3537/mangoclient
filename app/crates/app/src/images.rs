//! `AppService::image`: the async half of the image protocol (remote icons).

use crate::error::{AppError, Res};
use crate::image_protocol::{resolve, sniff_extension, Image, ImageError, RemoteImage};
use crate::service::AppService;

/// Remote images (icons, gallery shots) larger than this are refused.
const MAX_REMOTE_IMAGE: usize = 8 * 1024 * 1024;

impl AppService {
    /// Resolve a `mangoimg://` URL, fetching a remote Modrinth image into the
    /// cache first when needed. The protocol handler calls this.
    pub async fn image(&self, url: &str) -> Result<Image, ImageError> {
        let paths = &self.inner.paths;
        match resolve(paths, url) {
            Err(ImageError::NeedsFetch(remote)) => {
                if let Err(e) = self.fetch_remote_image(&remote).await {
                    tracing::debug!("remote image {} not cached: {e}", remote.url);
                    return Err(ImageError::NotFound);
                }
                resolve(paths, url)
            }
            other => other,
        }
    }

    async fn fetch_remote_image(&self, remote: &RemoteImage) -> Res<()> {
        let opts = &self.inner.opts;
        let url = url::Url::parse(&remote.url).map_err(|_| AppError::validation("bad url"))?;
        let scheme_ok = url.scheme() == "https" || (opts.icon_allow_http && url.scheme() == "http");
        let host_ok = url
            .host_str()
            .is_some_and(|h| opts.icon_hosts.iter().any(|a| a.eq_ignore_ascii_case(h)));
        if !scheme_ok || !host_ok {
            return Err(AppError::new("forbidden_host", "Image host is not allowed"));
        }
        let bytes = mango_net::get_bytes(&remote.url, &opts.fetch).await?;
        if bytes.len() > MAX_REMOTE_IMAGE {
            return Err(AppError::new("image_too_large", "Image is too large"));
        }
        let ext =
            sniff_extension(&bytes).ok_or_else(|| AppError::new("not_an_image", "Not an image"))?;
        let dest = remote.stem.with_extension(ext);
        tokio::task::spawn_blocking(move || -> std::io::Result<()> {
            if let Some(dir) = dest.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let mut tmp = dest.clone().into_os_string();
            tmp.push(".part");
            std::fs::write(&tmp, &bytes)?;
            std::fs::rename(&tmp, &dest)
        })
        .await??;
        Ok(())
    }
}
