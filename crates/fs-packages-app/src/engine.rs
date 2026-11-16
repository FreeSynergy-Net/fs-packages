//! Pattern: Adapter. The in-process engine behind [`StoreBackend`]; it does what the CLI `fs-packages` does (the
//! CLI keeps that code inside its `main`, so it is repeated here on purpose, not shared). Replaced by a
//! Cap'n Proto client of `store.capnp` later.

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use fs_packages_core::{SourceId, SourceRepository, Version};
use fs_packages_engine::CatalogCache;
use fs_packages_engine::net::HttpsFetch;
use fs_packages_engine::service::{Fetch, FileFetch};
use fs_packages_sources::FileSources;

use crate::backend::{
    BackendError, InstalledDetails, InstalledRow, JobError, JobKind, Listing, PackageRow, Phase,
    Prepared, SourceRow, StoreBackend,
};
use crate::engine_icons::{IconSource, IconWishes, icon_names};
pub(crate) use crate::engine_read::SourceRead;

pub(crate) struct AnyFetch;

impl Fetch for AnyFetch {
    fn get(&self, url: &str) -> Result<Vec<u8>, String> {
        if url.starts_with("file://") {
            FileFetch.get(url)
        } else {
            HttpsFetch.get(url)
        }
    }
}

/// The engine, working below `root`.
#[derive(Debug, Clone)]
pub struct EngineBackend {
    pub(crate) root: PathBuf,
    /// Names the sources offered last time: they are looked up in rpm too.
    pub(crate) offered: Arc<Mutex<Vec<String>>>,
    /// Catalogs read and verified in this session: each source is fetched once, then reused until it is old or "refresh" is pressed.
    pub(crate) catalogs: Arc<CatalogCache<SourceRead>>,
    /// Unix seconds of the oldest catalog read the last listing used (0 = none): what "Checked ..." says.
    pub(crate) listed_at: Arc<Mutex<u64>>,
    /// The names that have an icon file in `<root>/icons`, read once per listing (not one `stat` per card per draw).
    pub(crate) icons: Arc<Mutex<Option<std::collections::HashSet<String>>>>,
    /// The format this system takes, asked once (it starts `rpm -qf` or `dpkg -S`, ~8 ms: not once per card per draw).
    pub(crate) format: Arc<std::sync::OnceLock<fs_packages_core::Format>>,
    /// The folder of the icon cache (`<cache>/icons/<sha256>.png`).
    pub(crate) cache: PathBuf,
    /// The icons the signed catalogs name (what the cache should hold).
    pub(crate) icon_wishes: Arc<Mutex<IconWishes>>,
    pub(crate) icon_source: IconSource,
}

impl EngineBackend {
    /// Works below `root` (`sources.toml`, `packages/`, `current/`); the icon cache is `<root>/cache`.
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        let cache = root.join("cache");
        Self::with_cache(root, cache)
    }

    /// `FS_PACKAGES_ROOT`, or `~/.local/share/fs-packages`, like the CLI. The icon cache is `<root>/cache` when
    /// `FS_PACKAGES_ROOT` is set, else `$XDG_CACHE_HOME/fs-packages` (`~/.cache/fs-packages`).
    #[must_use]
    pub fn from_environment() -> Self {
        let root = std::env::var_os("FS_PACKAGES_ROOT").map_or_else(
            || {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                    .join(".local/share/fs-packages")
            },
            PathBuf::from,
        );
        Self::with_cache(root, fs_packages_engine::cache_dir_from_environment())
    }

    pub(crate) fn root(&self) -> &std::path::Path {
        &self.root
    }

    pub(crate) fn sources_file(&self) -> Result<FileSources, BackendError> {
        self.file()
    }

    fn file(&self) -> Result<FileSources, BackendError> {
        FileSources::load(&self.root.join("sources.toml")).map_err(other)
    }

    pub(crate) fn installed_version(&self, name: &str) -> Option<Version> {
        let known = crate::installed::detect(
            &self.root,
            &fs_packages_engine::native_installed_version,
            &[name.to_owned()],
        );
        known
            .into_iter()
            .find(|r| r.name == name)
            .map(|r| r.version)
    }
}

fn other(error: impl std::fmt::Debug) -> BackendError {
    BackendError::Other(format!("{error:?}"))
}

use crate::engine_map::{id_from_url, row};

impl StoreBackend for EngineBackend {
    fn sources(&self) -> Result<Vec<SourceRow>, BackendError> {
        Ok(self.file()?.list().into_iter().map(row).collect())
    }

    fn add_source(&self, url: &str) -> Result<SourceRow, BackendError> {
        let mut list = self.file()?;
        fs::create_dir_all(&self.root).map_err(other)?;
        let id = id_from_url(url);
        list.add(url, SourceId(id.clone()))
            .map_err(BackendError::Source)?;
        list.save().map_err(other)?;
        list.list()
            .into_iter()
            .find(|s| s.id.0 == id)
            .map(row)
            .ok_or_else(|| BackendError::Other("the source was not saved".to_owned()))
    }

    fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), BackendError> {
        let mut list = self.file()?;
        fs::create_dir_all(&self.root).map_err(other)?;
        list.set_enabled(&SourceId(id.to_owned()), enabled)
            .map_err(BackendError::Source)?;
        list.save().map_err(other)
    }

    fn remove_source(&self, id: &str) -> Result<(), BackendError> {
        let mut list = self.file()?;
        list.remove(&SourceId(id.to_owned()))
            .map_err(BackendError::Source)?;
        list.save().map_err(other)
    }

    fn available(&self) -> Result<Listing, BackendError> {
        // A new listing: the icon folder is read again at the next draw.
        *self.icons.lock().unwrap_or_else(PoisonError::into_inner) = None;
        let mut listing = Listing::default();
        let mut oldest = u64::MAX;
        let mut wanted = Vec::new();
        for source in self.file()?.list().into_iter().filter(|s| s.enabled) {
            match self.read_cached(&source.url) {
                Ok(read) => {
                    oldest = oldest.min(read.at);
                    let catalog = read.catalog.clone();
                    wanted.extend(crate::engine_icons::wanted_icons(&catalog));
                    listing
                        .packages
                        .extend(catalog.entries.into_iter().map(|e| {
                            let native = crate::engine_after::native_formats(&e);
                            PackageRow {
                                name: e.name.as_str().to_owned(),
                                title: e.title,
                                kind: e.kind,
                                version: e.version,
                                source: source.id.0.clone(),
                                description: e.description,
                                homepage: e.homepage,
                                native,
                                locked: e.locked,
                                details: {
                                    let mut d = e.details;
                                    d.origin.source.clone_from(&source.url);
                                    d
                                },
                            }
                        }));
                    listing
                        .skipped
                        .extend(catalog.skipped.into_iter().map(|s| (s.id, s.reason)));
                }
                Err(detail) => listing.unreadable.push((source.id.0, detail)),
            }
        }
        *self.offered.lock().unwrap_or_else(PoisonError::into_inner) =
            listing.packages.iter().map(|p| p.name.clone()).collect();
        *self
            .listed_at
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = if oldest == u64::MAX { 0 } else { oldest };
        *self
            .icon_wishes
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = IconWishes {
            wanted,
            all_read: listing.unreadable.is_empty(),
        };
        Ok(listing)
    }

    fn installed(&self) -> Result<Vec<InstalledRow>, BackendError> {
        let candidates = self
            .offered
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        Ok(crate::installed::detect(
            &self.root,
            &fs_packages_engine::native_installed_version,
            &candidates,
        ))
    }

    fn forget_catalogs(&self) {
        self.catalogs.forget_all();
    }

    fn catalog_read_at(&self) -> u64 {
        *self
            .listed_at
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn system_format(&self) -> fs_packages_core::Format {
        *self
            .format
            .get_or_init(fs_packages_engine::preferred_format)
    }

    fn icon_path(&self, name: &str) -> Option<PathBuf> {
        if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
            return None;
        }
        // First the icon of the signed offer in the cache, then the one kept at install (its name known from one read of the
        // folder per listing), else none (the letter).
        if let Some(cached) = self.cached_icon_of(name) {
            return Some(cached);
        }
        let mut icons = self.icons.lock().unwrap_or_else(PoisonError::into_inner);
        let known = icons.get_or_insert_with(|| icon_names(&self.root.join("icons")));
        known
            .contains(name)
            .then(|| self.root.join("icons").join(format!("{name}.png")))
    }

    fn load_icons(&self) -> Vec<String> {
        self.load_icons_at(SystemTime::now())
    }

    fn details(&self, name: &str) -> InstalledDetails {
        crate::installed::details(
            &self.root,
            &fs_packages_engine::default_bin_dir(),
            &fs_packages_engine::native_installed_version,
            crate::installed::native_kind(),
            name,
        )
    }

    fn uninstall(&self, name: &str) -> Result<(), String> {
        crate::engine_remove::uninstall(&self.root, name)
    }

    fn run(
        &self,
        kind: JobKind,
        name: &str,
        observe: &mut dyn FnMut(Phase),
    ) -> Result<Version, JobError> {
        let prepared = self.prepare(kind, name)?;
        observe(Phase::Verified);
        observe(Phase::Installing);
        self.commit(vec![(name.to_owned(), prepared)], &mut |_, _| {})
            .pop()
            .map_or(Err(JobError::NotOffered), |(_, r)| r)
    }

    fn prepare(&self, kind: JobKind, name: &str) -> Result<Prepared, JobError> {
        self.prepare_job(kind, name, &crate::backend::JobControl::none())
    }

    fn prepare_controlled(
        &self,
        kind: JobKind,
        name: &str,
        control: &crate::backend::JobControl,
    ) -> Result<Prepared, JobError> {
        self.prepare_job(kind, name, control)
    }

    fn commit(
        &self,
        batch: Vec<(String, Prepared)>,
        observe: &mut dyn FnMut(&str, Phase),
    ) -> Vec<(String, Result<Version, JobError>)> {
        self.commit_jobs(batch, observe)
    }
}
