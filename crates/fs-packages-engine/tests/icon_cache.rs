//! P1 (0.0.19): the icon cache of the signed catalog: where it is, what is taken, what is tidied.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use fs_packages_engine::service::Fetch;
use fs_packages_engine::{
    KEEP_UNUSED, Wanted, cache_dir, cached_bytes, cached_icon, fetch_into_cache, fetch_missing,
    prune,
};
use sha2::{Digest, Sha256};

fn png(tail: &[u8]) -> Vec<u8> {
    let mut b = b"\x89PNG\r\n\x1a\n".to_vec();
    b.extend_from_slice(tail);
    b
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Answers every address with `bytes` and counts the calls.
struct Fixed {
    bytes: Vec<u8>,
    calls: Mutex<Vec<String>>,
}

impl Fixed {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            calls: Mutex::new(Vec::new()),
        }
    }
}

impl Fetch for Fixed {
    fn get(&self, url: &str) -> Result<Vec<u8>, String> {
        self.calls.lock().unwrap().push(url.to_owned());
        Ok(self.bytes.clone())
    }
}

#[test]
fn the_cache_is_where_the_rule_says() {
    let p = Path::new;
    assert_eq!(
        cache_dir(Some(p("/r")), Some(p("/x")), Some(p("/h"))),
        p("/r/cache")
    );
    assert_eq!(
        cache_dir(None, Some(p("/x")), Some(p("/h"))),
        p("/x/fs-packages")
    );
    assert_eq!(
        cache_dir(None, None, Some(p("/h"))),
        p("/h/.cache/fs-packages")
    );
    // a relative or empty XDG_CACHE_HOME is ignored
    assert_eq!(
        cache_dir(None, Some(p("rel")), Some(p("/h"))),
        p("/h/.cache/fs-packages")
    );
    assert_eq!(
        cache_dir(None, Some(p("")), Some(p("/h"))),
        p("/h/.cache/fs-packages")
    );
}

#[test]
fn a_good_icon_is_cached_under_its_hash() {
    let tmp = tempfile::tempdir().unwrap();
    let bytes = png(b"good");
    let sha = hash(&bytes);
    let fetch = Fixed::new(bytes.clone());
    fetch_into_cache(tmp.path(), &fetch, "https://example.org/a.png", &sha).unwrap();
    let file = tmp.path().join("icons").join(format!("{sha}.png"));
    assert_eq!(fs::read(&file).unwrap(), bytes);
    assert_eq!(cached_icon(tmp.path(), &sha), Some(file));
    // the catalog may write the hash in capitals
    assert!(cached_icon(tmp.path(), &sha.to_uppercase()).is_some());
    assert_eq!(cached_bytes(tmp.path(), &sha), Some(bytes));
}

#[test]
fn a_wrong_hash_is_not_taken() {
    let tmp = tempfile::tempdir().unwrap();
    let bytes = png(b"good");
    let other = hash(&png(b"other"));
    let fetch = Fixed::new(bytes);
    let err =
        fetch_into_cache(tmp.path(), &fetch, "https://example.org/a.png", &other).unwrap_err();
    assert!(err.contains("hash"), "{err}");
    assert!(!tmp.path().join("icons").exists());
    assert!(cached_icon(tmp.path(), &other).is_none());
}

#[test]
fn plain_http_is_never_asked() {
    let tmp = tempfile::tempdir().unwrap();
    let bytes = png(b"good");
    let fetch = Fixed::new(bytes.clone());
    for url in [
        "http://example.org/a.png",
        "file:///etc/passwd",
        "ftp://x/a.png",
        "",
    ] {
        assert!(fetch_into_cache(tmp.path(), &fetch, url, &hash(&bytes)).is_err());
    }
    assert!(
        fetch.calls.lock().unwrap().is_empty(),
        "no request went out"
    );
    assert!(!tmp.path().join("icons").exists());
}

#[test]
fn too_large_and_not_png_are_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let mut big = png(b"");
    big.resize(1024 * 1024 + 1, 0);
    let err = fetch_into_cache(
        tmp.path(),
        &Fixed::new(big.clone()),
        "https://e.org/a.png",
        &hash(&big),
    )
    .unwrap_err();
    assert!(err.contains("larger"), "{err}");
    // exactly 1 MiB is still fine
    let mut edge = png(b"");
    edge.resize(1024 * 1024, 7);
    fetch_into_cache(
        tmp.path(),
        &Fixed::new(edge.clone()),
        "https://e.org/b.png",
        &hash(&edge),
    )
    .unwrap();

    let svg = b"<svg xmlns='http://www.w3.org/2000/svg'/>".to_vec();
    let err = fetch_into_cache(
        tmp.path(),
        &Fixed::new(svg.clone()),
        "https://e.org/c.svg",
        &hash(&svg),
    )
    .unwrap_err();
    assert!(err.contains("PNG"), "{err}");
    assert!(cached_icon(tmp.path(), &hash(&svg)).is_none());
}

#[test]
fn a_hash_that_is_no_hash_names_no_file() {
    let tmp = tempfile::tempdir().unwrap();
    let fetch = Fixed::new(png(b"x"));
    assert!(fetch_into_cache(tmp.path(), &fetch, "https://e.org/a.png", "../../x").is_err());
    assert!(cached_icon(tmp.path(), "../../x").is_none());
    assert!(cached_icon(tmp.path(), "").is_none());
}

#[test]
fn a_damaged_cache_file_is_not_used_for_an_install() {
    let tmp = tempfile::tempdir().unwrap();
    let bytes = png(b"good");
    let sha = hash(&bytes);
    fs::create_dir_all(tmp.path().join("icons")).unwrap();
    fs::write(
        tmp.path().join("icons").join(format!("{sha}.png")),
        png(b"tampered"),
    )
    .unwrap();
    assert!(cached_bytes(tmp.path(), &sha).is_none());
}

#[test]
fn missing_icons_are_fetched_once_each_and_failures_are_told_per_package() {
    let tmp = tempfile::tempdir().unwrap();
    let good = png(b"one");
    let sha = hash(&good);
    let wanted = vec![
        Wanted {
            name: "fs-a".into(),
            url: "https://e.org/a.png".into(),
            sha256: sha.clone(),
        },
        Wanted {
            name: "fs-b".into(),
            url: "https://e.org/a.png".into(),
            sha256: sha.clone(),
        },
        Wanted {
            name: "fs-c".into(),
            url: "http://e.org/c.png".into(),
            sha256: hash(&png(b"two")),
        },
    ];
    let fetch = Fixed::new(good);
    let problems = fetch_missing(tmp.path(), &fetch, &wanted);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].starts_with("fs-c: icon not cached"),
        "{problems:?}"
    );
    assert_eq!(
        fetch.calls.lock().unwrap().len(),
        1,
        "the same hash is fetched once"
    );
    // offline with a filled cache: nothing is asked, the icon is there
    let offline = Fixed::new(Vec::new());
    assert_eq!(
        fetch_missing(tmp.path(), &offline, &wanted[..1]),
        Vec::<String>::new()
    );
    assert!(offline.calls.lock().unwrap().is_empty());
    assert!(cached_icon(tmp.path(), &sha).is_some());
}

fn age(path: &Path, days: u64, now: SystemTime) {
    let file = fs::File::options().write(true).open(path).unwrap();
    file.set_modified(now - Duration::from_secs(days * 86_400))
        .unwrap();
}

#[test]
fn unused_files_go_after_thirty_days_and_nothing_else() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("icons");
    fs::create_dir_all(&dir).unwrap();
    let now = SystemTime::now();
    let name = |c: char| format!("{}.png", c.to_string().repeat(64));
    // used + old, unused + old, unused + young (29 days), unused + old but not a hash name
    let (used, old, young) = (name('a'), name('b'), name('c'));
    for f in [
        &used,
        &old,
        &young,
        &"notes.png".to_owned(),
        &format!("{}.txt", "d".repeat(64)),
    ] {
        fs::write(dir.join(f), b"x").unwrap();
        age(&dir.join(f), 40, now);
    }
    age(&dir.join(&young), 29, now);
    let keep: HashSet<String> = [used.trim_end_matches(".png").to_uppercase()].into();
    assert_eq!(prune(tmp.path(), &keep, now, KEEP_UNUSED).unwrap(), 1);
    assert!(dir.join(&used).exists());
    assert!(!dir.join(&old).exists());
    assert!(dir.join(&young).exists());
    assert!(dir.join("notes.png").exists());
    assert!(dir.join(format!("{}.txt", "d".repeat(64))).exists());
    // 31 days later (the clock is handed in) the young one goes too
    let later = now + Duration::from_secs(2 * 86_400);
    assert_eq!(prune(tmp.path(), &keep, later, KEEP_UNUSED).unwrap(), 1);
    assert!(!dir.join(&young).exists());
    // no folder at all is fine
    assert_eq!(
        prune(&tmp.path().join("none"), &keep, now, KEEP_UNUSED).unwrap(),
        0
    );
}
