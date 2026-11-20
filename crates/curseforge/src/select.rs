//! Choosing the right file of a project for an instance.

use crate::types::{File, FileDependency, ModLoaderType, Relation};

/// Does `file` run on `mc_version` and (when given) `loader`? Quilt accepts
/// Fabric files and NeoForge accepts Forge files. Server packs and
/// unavailable files never fit.
pub fn is_compatible(file: &File, mc_version: &str, loader: Option<ModLoaderType>) -> bool {
    if !file.is_available() || file.is_server_pack == Some(true) {
        return false;
    }
    if !file.minecraft_versions().any(|v| v == mc_version) {
        return false;
    }
    match loader {
        None => true,
        Some(l) => {
            let have = file.loaders();
            l.accepts().iter().any(|a| have.contains(a))
        }
    }
}

/// The best file for `(mc_version, loader)`: files that name the loader
/// itself win over accepted fallbacks (Fabric files on Quilt), then release
/// over beta over alpha, then the newest by `fileDate`, then the highest id.
/// `loader == None` means the content does not care (resource packs,
/// shaders).
pub fn pick_best_file<'a>(
    files: &'a [File],
    mc_version: &str,
    loader: Option<ModLoaderType>,
) -> Option<&'a File> {
    let fits: Vec<&File> = files
        .iter()
        .filter(|f| is_compatible(f, mc_version, loader))
        .collect();
    let pool: Vec<&File> = match loader {
        Some(l) => {
            let exact: Vec<&File> = fits
                .iter()
                .copied()
                .filter(|f| f.loaders().contains(&l))
                .collect();
            if exact.is_empty() {
                fits
            } else {
                exact
            }
        }
        None => fits,
    };
    pool.into_iter().max_by(|a, b| {
        b.release()
            .cmp(&a.release())
            .then_with(|| a.file_date.cmp(&b.file_date))
            .then_with(|| a.id.cmp(&b.id))
    })
}

/// Dependencies of one relation kind, deduplicated by project id.
pub fn dependencies_of(file: &File, relation: Relation) -> Vec<&FileDependency> {
    let mut seen = Vec::new();
    file.dependencies
        .iter()
        .filter(|d| d.relation() == relation)
        .filter(|d| {
            let fresh = !seen.contains(&d.mod_id);
            seen.push(d.mod_id);
            fresh
        })
        .collect()
}
