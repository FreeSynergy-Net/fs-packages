//! Pattern: none (tests). P1 (0.0.19): what a card shows for an icon: the cache first, then the installed one, else nothing (the letter).

use std::fs;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use fs_packages_engine::Wanted;
use fs_packages_engine::service::Fetch;
use sha2::{Digest, Sha256};

use crate::backend::StoreBackend;
use crate::engine::EngineBackend;

fn png(tail: &[u8]) -> Vec<u8> {
    let mut b = b"\x89PNG\r\n\x1a\n".to_vec();
    b.extend_from_slice(tail);
    b
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

struct Fixed(Vec<u8>, Mutex<usize>);

impl Fetch for Fixed {
    fn get(&self, _url: &str) -> Result<Vec<u8>, String> {
        *self.1.lock().unwrap() += 1;
        Ok(self.0.clone())
    }
}

fn wish(backend: &EngineBackend, name: &str, sha: &str, all_read: bool) {
    backend.set_icon_wishes_for_test(
        vec![Wanted {
            name: name.to_owned(),
            url: format!("https://example.org/{name}.png"),
            sha256: sha.to_owned(),
        }],
        all_read,
    );
}

#[test]
fn the_cache_comes_before_the_installed_icon_before_the_letter() {
    let tmp = tempfile::tempdir().unwrap();
    let backend = EngineBackend::new(tmp.path().to_path_buf());
    let bytes = png(b"offer");
    let sha = hash(&bytes);
    wish(&backend, "fs-a", &sha, true);
    // nothing anywhere: the letter
    assert_eq!(backend.icon_path("fs-a"), None);
    // installed icon only
    fs::create_dir_all(tmp.path().join("icons")).unwrap();
    fs::write(tmp.path().join("icons/fs-a.png"), png(b"installed")).unwrap();
    // a new listing reads the icon folder again (the names are kept once per listing)
    *backend.icons.lock().unwrap() = None;
    assert_eq!(
        backend.icon_path("fs-a"),
        Some(tmp.path().join("icons/fs-a.png"))
    );
    // cache and installed: the cache
    let fetch = Arc::new(Fixed(bytes, Mutex::new(0)));
    let backend = backend.with_icon_fetch(fetch);
    assert_eq!(
        backend.load_icons_at(SystemTime::now()),
        Vec::<String>::new()
    );
    let cached = tmp.path().join("cache/icons").join(format!("{sha}.png"));
    assert_eq!(backend.icon_path("fs-a"), Some(cached));
    // a name the catalog does not name keeps only its installed icon
    assert_eq!(backend.icon_path("fs-other"), None);
    assert_eq!(backend.icon_path("../x"), None);
}

#[test]
fn a_failed_icon_is_told_and_the_letter_stays() {
    let tmp = tempfile::tempdir().unwrap();
    let wrong = hash(&png(b"promised"));
    let backend = EngineBackend::new(tmp.path().to_path_buf())
        .with_icon_fetch(Arc::new(Fixed(png(b"served"), Mutex::new(0))));
    wish(&backend, "fs-a", &wrong, true);
    let problems = backend.load_icons_at(SystemTime::now());
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].starts_with("fs-a: icon not cached"),
        "{problems:?}"
    );
    assert_eq!(backend.icon_path("fs-a"), None);
}

#[test]
fn tidying_waits_for_a_complete_read() {
    let tmp = tempfile::tempdir().unwrap();
    let icons = tmp.path().join("cache/icons");
    fs::create_dir_all(&icons).unwrap();
    let stale = icons.join(format!("{}.png", "e".repeat(64)));
    fs::write(&stale, b"x").unwrap();
    let old = SystemTime::now() - Duration::from_secs(40 * 86_400);
    fs::File::options()
        .write(true)
        .open(&stale)
        .unwrap()
        .set_modified(old)
        .unwrap();
    let bytes = png(b"offer");
    let backend = EngineBackend::new(tmp.path().to_path_buf())
        .with_icon_fetch(Arc::new(Fixed(bytes.clone(), Mutex::new(0))));
    // one source could not be read: its icons might look unused, so nothing is removed
    wish(&backend, "fs-a", &hash(&bytes), false);
    assert_eq!(
        backend.load_icons_at(SystemTime::now()),
        Vec::<String>::new()
    );
    assert!(stale.exists());
    wish(&backend, "fs-a", &hash(&bytes), true);
    assert_eq!(
        backend.load_icons_at(SystemTime::now()),
        Vec::<String>::new()
    );
    assert!(!stale.exists());
    assert!(icons.join(format!("{}.png", hash(&bytes))).exists());
}
