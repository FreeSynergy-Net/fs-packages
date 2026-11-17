//! Pattern: Content-addressed cache. The icons of the offers of the signed catalog, kept as `<cache>/icons/<sha256>.png`: the
//! name is the hash the catalog gives, so a file is right by its name, needs no expiry, and a new icon is a new file.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use crate::icons::{is_good_icon, sha256_hex};
use crate::service::Fetch;

/// Files no catalog entry names any more are removed after this long.
pub const KEEP_UNUSED: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// At most this many icons are fetched at the same time.
const PARALLEL: usize = 4;

/// One icon an offer names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
    /// Package name (for the message).
    pub name: String,
    /// Where the PNG is (https).
    pub url: String,
    /// SHA-256 the signed catalog gives.
    pub sha256: String,
}

/// Where the cache is: `<root>/cache` when `FS_PACKAGES_ROOT` is set, else `$XDG_CACHE_HOME/fs-packages`
/// (a relative or empty value is ignored, like the XDG rule says), else `<home>/.cache/fs-packages`.
#[must_use]
pub fn cache_dir(root: Option<&Path>, xdg: Option<&Path>, home: Option<&Path>) -> PathBuf {
    if let Some(root) = root.filter(|p| !p.as_os_str().is_empty()) {
        return root.join("cache");
    }
    if let Some(xdg) = xdg.filter(|p| p.is_absolute()) {
        return xdg.join("fs-packages");
    }
    home.unwrap_or_else(|| Path::new(""))
        .join(".cache")
        .join("fs-packages")
}

/// [`cache_dir`] from `FS_PACKAGES_ROOT`, `XDG_CACHE_HOME` and `HOME`.
#[must_use]
pub fn cache_dir_from_environment() -> PathBuf {
    let var = |name| std::env::var_os(name).map(PathBuf::from);
    cache_dir(
        var("FS_PACKAGES_ROOT").as_deref(),
        var("XDG_CACHE_HOME").as_deref(),
        var("HOME").as_deref(),
    )
}

/// Counts the writes of this process (part of the temporary name).
static WRITES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn is_hash(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The cached file of `sha256` (as the catalog writes it, upper case allowed), when there is one.
#[must_use]
pub fn cached_icon(cache: &Path, sha256: &str) -> Option<PathBuf> {
    let hash = sha256.to_ascii_lowercase();
    if !is_hash(&hash) {
        return None;
    }
    let path = cache.join("icons").join(format!("{hash}.png"));
    path.is_file().then_some(path)
}

/// The bytes of a cached icon, read again and checked against their name, so a damaged file is not used.
#[must_use]
pub fn cached_bytes(cache: &Path, sha256: &str) -> Option<Vec<u8>> {
    let bytes = fs::read(cached_icon(cache, sha256)?).ok()?;
    is_good_icon(&bytes, sha256).then_some(bytes)
}

/// Fetches one icon into the cache: https only, a PNG of at most 1 MiB, with the hash of the catalog; otherwise nothing is
/// stored. The file is written next to its place and renamed.
///
/// # Errors
/// Text saying why the icon was not taken (not https, not fetched, too large, not a PNG, hash differs, disk).
pub fn fetch_into_cache(
    cache: &Path,
    fetch: &dyn Fetch,
    url: &str,
    sha256: &str,
) -> Result<(), String> {
    let hash = sha256.to_ascii_lowercase();
    if !is_hash(&hash) {
        return Err("the hash of the catalog is not a SHA-256".to_owned());
    }
    if !url.starts_with("https://") {
        return Err(format!("{url}: only https addresses are downloaded"));
    }
    let bytes = fetch.get(url)?;
    if bytes.len() > crate::icons::MAX_ICON {
        return Err("larger than an icon may be".to_owned());
    }
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("not a PNG file".to_owned());
    }
    if sha256_hex(&bytes) != hash {
        return Err("does not match the hash of the signed catalog".to_owned());
    }
    let dir = cache.join("icons");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    // One name for each write: two refreshes at the same time must not write the same temporary file.
    let n = WRITES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = dir.join(format!(".{hash}.{}.{n}.new", std::process::id()));
    fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
    fs::rename(&tmp, dir.join(format!("{hash}.png"))).map_err(|e| e.to_string())
}

/// Fetches every icon of `wanted` that is not cached yet (up to four at a time, each hash once). Returns one line per
/// package whose icon could not be taken; the caller shows them, nothing is dropped silently.
#[must_use]
pub fn fetch_missing(cache: &Path, fetch: &dyn Fetch, wanted: &[Wanted]) -> Vec<String> {
    let mut seen = HashSet::new();
    let todo: Vec<&Wanted> = wanted
        .iter()
        .filter(|w| cached_icon(cache, &w.sha256).is_none())
        .filter(|w| seen.insert(w.sha256.to_ascii_lowercase()))
        .collect();
    let queue = Mutex::new(todo.into_iter());
    let problems = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..PARALLEL {
            scope.spawn(|| {
                loop {
                    let next = queue.lock().unwrap_or_else(PoisonError::into_inner).next();
                    let Some(w) = next else { break };
                    if let Err(e) = fetch_into_cache(cache, fetch, &w.url, &w.sha256) {
                        problems
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .push(format!("{}: icon not cached: {e}", w.name));
                    }
                }
            });
        }
    });
    let mut out = problems
        .into_inner()
        .unwrap_or_else(PoisonError::into_inner);
    out.sort();
    out
}

/// Removes the files of the cache that no loaded catalog entry names (`keep` = their hashes) and that are older than
/// `max_age` at `now`. Only files named like a hash are touched; anything else in the folder stays. Returns how many went.
///
/// # Errors
/// `io::Error` when the folder cannot be read (a missing folder is fine).
pub fn prune<S: std::hash::BuildHasher>(
    cache: &Path,
    keep: &HashSet<String, S>,
    now: SystemTime,
    max_age: Duration,
) -> io::Result<usize> {
    let dir = match fs::read_dir(cache.join("icons")) {
        Ok(dir) => dir,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    let mut removed = 0;
    for entry in dir.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(hash) = name.strip_suffix(".png").filter(|h| is_hash(h)) else {
            continue;
        };
        if keep.iter().any(|k| k.eq_ignore_ascii_case(hash)) {
            continue;
        }
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| now.duration_since(t).ok())
            .is_some_and(|age| age > max_age);
        if old && fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}
