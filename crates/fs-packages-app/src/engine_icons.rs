//! Pattern: Content-addressed cache (the app side). What the backend remembers of the icons the signed catalogs name, how they are
//! fetched into the cache and tidied; the files and the rules are in `fs_packages_engine::icon_cache`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, PoisonError};
use std::time::SystemTime;

use fs_packages_engine::service::Fetch;

use crate::engine::EngineBackend;

/// The icons the last full read of the sources named, and whether every source was read.
#[derive(Debug, Default)]
pub(crate) struct IconWishes {
    pub(crate) wanted: Vec<fs_packages_engine::Wanted>,
    pub(crate) all_read: bool,
}

/// Where icons are downloaded from (the network; tests hand in their own).
#[derive(Clone)]
pub(crate) struct IconSource(pub(crate) Arc<dyn Fetch + Send + Sync>);

impl std::fmt::Debug for IconSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("IconSource")
    }
}

impl EngineBackend {
    pub(crate) fn with_cache(root: PathBuf, cache: PathBuf) -> Self {
        Self {
            root,
            offered: Arc::default(),
            catalogs: Arc::default(),
            listed_at: Arc::default(),
            icons: Arc::default(),
            format: Arc::default(),
            cache,
            icon_wishes: Arc::default(),
            icon_source: IconSource(Arc::new(fs_packages_engine::net::IconFetch)),
        }
    }

    /// The cached file of the icon the signed offer of `name` names, if it is there.
    pub(crate) fn cached_icon_of(&self, name: &str) -> Option<std::path::PathBuf> {
        let wishes = self
            .icon_wishes
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        wishes
            .wanted
            .iter()
            .filter(|w| w.name == name)
            .find_map(|w| fs_packages_engine::cached_icon(&self.cache, &w.sha256))
    }

    /// The same backend, with its own way to download icons (tests).
    #[must_use]
    pub fn with_icon_fetch(mut self, fetch: Arc<dyn Fetch + Send + Sync>) -> Self {
        self.icon_source = IconSource(fetch);
        self
    }

    #[cfg(test)]
    pub(crate) fn set_icon_wishes_for_test(
        &self,
        wanted: Vec<fs_packages_engine::Wanted>,
        all_read: bool,
    ) {
        *self
            .icon_wishes
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = IconWishes { wanted, all_read };
    }

    /// The folder of the icon cache.
    #[must_use]
    pub fn cache_dir(&self) -> &std::path::Path {
        &self.cache
    }

    /// Fetches the icons the last read named (those not cached yet), then removes cached files no entry names
    /// that are older than 30 days at `now`. Returns what did not work, one line per package.
    #[must_use]
    pub fn load_icons_at(&self, now: SystemTime) -> Vec<String> {
        let (wanted, all_read) = {
            let wishes = self
                .icon_wishes
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            (wishes.wanted.clone(), wishes.all_read)
        };
        let mut problems =
            fs_packages_engine::fetch_missing(&self.cache, self.icon_source.0.as_ref(), &wanted);
        // Only after every source was read: an unreadable source must not make its icons look unused.
        if all_read {
            let keep: HashSet<String> = wanted
                .iter()
                .map(|w| w.sha256.to_ascii_lowercase())
                .collect();
            if let Err(e) =
                fs_packages_engine::prune(&self.cache, &keep, now, fs_packages_engine::KEEP_UNUSED)
            {
                problems.push(format!("icon cache: not cleaned: {e}"));
            }
        }
        problems
    }
}

/// The icons the entries of a catalog name (an entry without an icon with a hash names none).
pub(crate) fn wanted_icons(
    catalog: &fs_packages_catalog::Catalog,
) -> impl Iterator<Item = fs_packages_engine::Wanted> + '_ {
    catalog.entries.iter().filter_map(|e| {
        e.icon.as_ref().map(|i| fs_packages_engine::Wanted {
            name: e.name.as_str().to_owned(),
            url: i.url.clone(),
            sha256: i.sha256.clone(),
        })
    })
}

/// The names of the `<name>.png` files (regular files only) in `dir`; a missing folder is an empty set.
pub(crate) fn icon_names(dir: &std::path::Path) -> std::collections::HashSet<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return std::collections::HashSet::new();
    };
    entries
        .flatten()
        .filter(|e| e.path().is_file())
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            name.strip_suffix(".png").map(str::to_owned)
        })
        .collect()
}
