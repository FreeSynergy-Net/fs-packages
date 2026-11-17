//! Pattern: Guard + Unit of Work (one catalog read at a time, and a clone that is never half there).
//!
//! Every read of a source clones it. Before this module each read cloned into the same folder after deleting it, so two reads at once
//! (two windows, the window and a command, two commands) deleted each other's folder or failed with "shallow file could not be locked".
//!
//! How it is done: a read takes the lock of the store (a mutex for the threads of this process, `flock` on `<root>/sources/.lock` for
//! other processes; both go away with the process, also after a crash), clones into a folder of its own below `<root>/sources/` and,
//! only when the clone is whole, renames it to the place of that source. The old state is removed after that, so a failed or aborted
//! clone leaves the old state as it was. Whoever reads the folder does so inside the lock ([`with_clone`]).
//! [`CatalogCache`] keeps a verified catalog in memory so a batch or a window session fetches each source once.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use sha2::{Digest, Sha256};

use crate::net::clone_catalog;

/// How long a fetched catalog is reused in memory before it is fetched again (unless "refresh" was asked).
pub const CATALOG_FRESH_FOR: Duration = Duration::from_secs(10 * 60);
/// A clone folder left behind by a process that was killed is removed when it is older than this.
pub const ORPHAN_AFTER: Duration = Duration::from_secs(15 * 60);

const TMP_PREFIX: &str = ".clone-tmp-";
const OLD_PREFIX: &str = ".clone-old-";

static THREADS: Mutex<()> = Mutex::new(());
static COUNTER: AtomicU64 = AtomicU64::new(0);

/// The lock of the store's source reads; let go when dropped.
struct ReadLock {
    _file: File,
    _threads: MutexGuard<'static, ()>,
}

impl ReadLock {
    fn take(sources: &Path) -> io::Result<Self> {
        let threads = THREADS.lock().unwrap_or_else(PoisonError::into_inner);
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(sources.join(".lock"))?;
        file.lock()?;
        Ok(Self {
            _file: file,
            _threads: threads,
        })
    }
}

/// Clones `url` (newest state, depth 1) into a folder of its own, puts it in place atomically and runs `read` on it, all inside the
/// lock of the store. The folder stays below `<root>/sources/` as the last good state of this source.
///
/// # Errors
/// Text: the clone failed (the old state is untouched), the folder could not be put in place, or what `read` said.
pub fn with_clone<T>(
    root: &Path,
    url: &str,
    read: impl FnOnce(&Path) -> Result<T, String>,
) -> Result<T, String> {
    with_clone_using(root, url, &clone_catalog, true, ORPHAN_AFTER, read)
}

/// [`with_clone`] with the clone, the lock and the orphan age given (tests only; production always locks).
pub(crate) fn with_clone_using<T>(
    root: &Path,
    url: &str,
    clone: &dyn Fn(&str, &Path) -> Result<(), String>,
    lock: bool,
    orphan_after: Duration,
    read: impl FnOnce(&Path) -> Result<T, String>,
) -> Result<T, String> {
    let sources = root.join("sources");
    fs::create_dir_all(&sources).map_err(|e| format!("{}: {e}", sources.display()))?;
    let _held = if lock {
        Some(
            crate::timing::timed("catalog lock wait", || ReadLock::take(&sources))
                .map_err(|e| format!("source lock: {e}"))?,
        )
    } else {
        None
    };
    sweep_orphans(&sources, orphan_after);
    let tmp = unique(&sources, TMP_PREFIX);
    if let Err(e) = crate::timing::timed("catalog clone", || clone(url, &tmp)) {
        let _ = fs::remove_dir_all(&tmp);
        return Err(e);
    }
    let target = place_of(&sources, url);
    swap_in(&sources, &tmp, &target)?;
    read(&target)
}

/// The folder that holds the last clone of `url`.
fn place_of(sources: &Path, url: &str) -> PathBuf {
    let hash = Sha256::digest(url.as_bytes());
    let short = hash.iter().take(8).fold(String::new(), |mut acc, b| {
        use std::fmt::Write;
        let _ = write!(acc, "{b:02x}");
        acc
    });
    sources.join(format!("work-{short}"))
}

fn unique(sources: &Path, prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    sources.join(format!("{prefix}{}-{nanos}-{n}", std::process::id()))
}

/// Puts the whole clone `tmp` in the place `target`: the old state is renamed away first and removed only after the new one is there;
/// when the new one cannot be put in place the old one comes back.
fn swap_in(sources: &Path, tmp: &Path, target: &Path) -> Result<(), String> {
    let old = target.exists().then(|| unique(sources, OLD_PREFIX));
    if let Some(old) = &old {
        fs::rename(target, old).map_err(|e| format!("{}: {e}", target.display()))?;
    }
    if let Err(e) = fs::rename(tmp, target) {
        if let Some(old) = &old {
            let _ = fs::rename(old, target);
        }
        let _ = fs::remove_dir_all(tmp);
        return Err(format!("{}: {e}", target.display()));
    }
    if let Some(old) = old {
        let _ = fs::remove_dir_all(old);
    }
    Ok(())
}

/// Removes leftovers of a killed read: only folders with our own name patterns, and only older than `older_than`.
fn sweep_orphans(sources: &Path, older_than: Duration) {
    let Ok(entries) = fs::read_dir(sources) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !(name.starts_with(TMP_PREFIX) || name.starts_with(OLD_PREFIX)) {
            continue;
        }
        let age = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok());
        if age.is_some_and(|a| a >= older_than) {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

/// Pattern: Cache (in memory, per source address). A catalog that was fetched and verified is kept for `fresh_for`; every caller in the
/// time gets the same one, so a batch of installs or a window session fetches each source once.
#[derive(Debug)]
pub struct CatalogCache<T> {
    fresh_for: Duration,
    entries: Mutex<HashMap<String, (Instant, Arc<T>)>>,
}

impl<T> CatalogCache<T> {
    /// An empty cache that reuses an entry for `fresh_for`.
    #[must_use]
    pub fn new(fresh_for: Duration) -> Self {
        Self {
            fresh_for,
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// The cached entry of `url`, or the result of `fetch` (which is then kept). Callers at the same time wait for one fetch.
    ///
    /// # Errors
    /// What `fetch` said; nothing is kept then.
    pub fn get_or_fetch(
        &self,
        url: &str,
        fetch: impl FnOnce() -> Result<T, String>,
    ) -> Result<Arc<T>, String> {
        self.get_or_fetch_at(url, Instant::now(), fetch)
    }

    /// [`CatalogCache::get_or_fetch`] with the time given (tests).
    ///
    /// # Errors
    /// What `fetch` said.
    pub fn get_or_fetch_at(
        &self,
        url: &str,
        now: Instant,
        fetch: impl FnOnce() -> Result<T, String>,
    ) -> Result<Arc<T>, String> {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((at, value)) = entries.get(url)
            && now.saturating_duration_since(*at) < self.fresh_for
        {
            return Ok(Arc::clone(value));
        }
        let value = Arc::new(fetch()?);
        entries.insert(url.to_owned(), (now, Arc::clone(&value)));
        Ok(value)
    }

    /// "Refresh": forgets everything, the next read fetches again.
    pub fn forget_all(&self) {
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }
}

impl<T> Default for CatalogCache<T> {
    fn default() -> Self {
        Self::new(CATALOG_FRESH_FOR)
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_process;
