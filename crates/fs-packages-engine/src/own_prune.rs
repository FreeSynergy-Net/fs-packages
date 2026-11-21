//! Pattern: none (one function). Keeps the lists of files per version (`ManifestStore`) in step with the version folders.

use std::path::Path;

use fs_packages_sources::ManifestStore;

use crate::own_files::{current_versions, package_dir};

/// Removes the lists of the versions of `name` whose folder `packages/<name>/<version>` is gone, together with the list
/// folder when nothing is left. The list of `keep` (the version just installed) and the version `current/<name>` points to
/// are never removed, whatever is on disk.
///
/// # Errors
/// A sentence when a list cannot be removed.
pub fn prune_lists(root: &Path, name: &str, keep: &str) -> Result<Vec<String>, String> {
    let current = current_versions(root)
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v);
    let store = ManifestStore::new(root);
    let versions = store.versions(name).map_err(|e| format!("{name}: {e:?}"))?;
    let kept: Vec<&str> = versions
        .iter()
        .map(String::as_str)
        .filter(|v| {
            *v == keep || current.as_deref() == Some(*v) || package_dir(root, name, v).is_dir()
        })
        .collect();
    store
        .prune(name, &kept)
        .map_err(|e| format!("{name}: {e:?}"))
}
