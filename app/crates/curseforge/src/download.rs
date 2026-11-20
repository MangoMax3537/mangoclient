//! Downloading one CurseForge file.

use std::path::Path;

use mango_net::{download_file, DownloadOptions, Expected, ProgressFn};

use crate::client::Client;
use crate::error::{Error, Result};
use crate::modpack::safe_filename;
use crate::types::{File, Project};

/// Download `file` to `dest`, verifying SHA-1 (when the API gave one) and the
/// exact size. Refuses with [`Error::DistributionBlocked`] when the author
/// does not allow third-party downloads. The API key is not sent to the CDN.
pub async fn download_cf_file(
    client: &Client,
    project: Option<&Project>,
    file: &File,
    dest: &Path,
    progress: Option<ProgressFn>,
) -> Result<()> {
    safe_filename(&file.file_name)?;
    let Some(url) = client.resolve_download_url(project, file).await? else {
        return Err(Error::DistributionBlocked(file.file_name.clone()));
    };
    download_file(
        &url,
        dest,
        &DownloadOptions {
            expected: Expected {
                sha1: file.sha1(),
                size: (file.file_length > 0).then_some(file.file_length),
                ..Expected::default()
            },
            progress,
            ..DownloadOptions::default()
        },
    )
    .await?;
    Ok(())
}
