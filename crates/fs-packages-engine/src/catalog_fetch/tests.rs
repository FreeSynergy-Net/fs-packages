//! Pattern: none (tests of the locked catalog read: parallel reads, aborted clone, orphan sweep, cache).

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::*;

/// A clone that takes its time, writes the folder in two steps and fails the way the old shared folder did
/// ("shallow file could not be locked") when a second clone runs at the same moment.
struct Slow {
    active: AtomicUsize,
    conflicts: AtomicUsize,
    body: &'static str,
}

impl Slow {
    fn new(body: &'static str) -> Arc<Self> {
        Arc::new(Self {
            active: AtomicUsize::new(0),
            conflicts: AtomicUsize::new(0),
            body,
        })
    }

    fn fake_clone(&self, _url: &str, dest: &Path) -> Result<(), String> {
        if self.active.fetch_add(1, Ordering::SeqCst) > 0 {
            self.conflicts.fetch_add(1, Ordering::SeqCst);
            self.active.fetch_sub(1, Ordering::SeqCst);
            return Err("shallow file could not be locked".to_owned());
        }
        let result = (|| {
            fs::create_dir_all(dest).map_err(|e| e.to_string())?;
            fs::write(dest.join("catalog.xml"), self.body).map_err(|e| e.to_string())?;
            std::thread::sleep(Duration::from_millis(60));
            fs::write(dest.join("catalog.xml.asc"), "sig").map_err(|e| e.to_string())
        })();
        self.active.fetch_sub(1, Ordering::SeqCst);
        result
    }
}

pub(super) fn read_both(dir: &Path) -> Result<String, String> {
    let a = fs::read_to_string(dir.join("catalog.xml")).map_err(|e| e.to_string())?;
    fs::read_to_string(dir.join("catalog.xml.asc")).map_err(|e| format!("half folder: {e}"))?;
    Ok(a)
}

fn run_parallel(root: &Path, lock: bool, threads: usize) -> (Vec<Result<String, String>>, usize) {
    let slow = Slow::new("v1");
    let results = Arc::new(Mutex::new(Vec::new()));
    std::thread::scope(|s| {
        for _ in 0..threads {
            let (slow, results) = (Arc::clone(&slow), Arc::clone(&results));
            s.spawn(move || {
                let r = with_clone_using(
                    root,
                    "https://example.org/cat.git",
                    &|u, d| slow.fake_clone(u, d),
                    lock,
                    ORPHAN_AFTER,
                    read_both,
                );
                results.lock().unwrap().push(r);
            });
        }
    });
    let out = results.lock().unwrap().clone();
    (out, slow.conflicts.load(Ordering::SeqCst))
}

#[test]
fn two_parallel_reads_both_get_a_result_and_never_a_half_folder() {
    let dir = tempfile::tempdir().unwrap();
    let (results, conflicts) = run_parallel(dir.path(), true, 4);
    assert_eq!(conflicts, 0, "no clone ran next to another");
    assert_eq!(results.len(), 4);
    for r in results {
        assert_eq!(r.unwrap(), "v1");
    }
}

#[test]
fn without_the_lock_the_test_turns_red() {
    // the mutant: the lock switched off brings back the error of the old shared folder
    let dir = tempfile::tempdir().unwrap();
    let (results, conflicts) = run_parallel(dir.path(), false, 4);
    assert!(
        conflicts > 0 || results.iter().any(Result::is_err),
        "without the lock the reads must collide"
    );
}

#[test]
fn an_aborted_clone_leaves_the_old_state() {
    let dir = tempfile::tempdir().unwrap();
    let url = "https://example.org/cat.git";
    let ok = Slow::new("old");
    with_clone_using(
        dir.path(),
        url,
        &|u, d| ok.fake_clone(u, d),
        true,
        ORPHAN_AFTER,
        read_both,
    )
    .unwrap();
    let broken = |_: &str, dest: &Path| -> Result<(), String> {
        fs::create_dir_all(dest).unwrap();
        fs::write(dest.join("catalog.xml"), "new, but cut off").unwrap();
        Err("connection lost".to_owned())
    };
    let err = with_clone_using(dir.path(), url, &broken, true, ORPHAN_AFTER, |d| {
        read_both(d)
    })
    .unwrap_err();
    assert_eq!(err, "connection lost");
    let kept = place_of(&dir.path().join("sources"), url);
    assert_eq!(read_both(&kept).unwrap(), "old");
    let leftovers: Vec<_> = fs::read_dir(dir.path().join("sources"))
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(".clone-"))
        .collect();
    assert!(leftovers.is_empty(), "the partial folder is removed");
}

#[test]
fn a_new_clone_replaces_the_old_state_whole() {
    let dir = tempfile::tempdir().unwrap();
    let url = "https://example.org/cat.git";
    for body in ["one", "two"] {
        let s = Slow::new(body);
        let got = with_clone_using(
            dir.path(),
            url,
            &|u, d| s.fake_clone(u, d),
            true,
            ORPHAN_AFTER,
            read_both,
        )
        .unwrap();
        assert_eq!(got, body);
    }
}

#[test]
fn only_old_folders_of_our_own_names_are_swept() {
    let dir = tempfile::tempdir().unwrap();
    let sources = dir.path().join("sources");
    for name in [
        ".clone-tmp-1-2-3",
        ".clone-old-1-2-3",
        "mine",
        ".clone-tmpx",
    ] {
        fs::create_dir_all(sources.join(name)).unwrap();
    }
    sweep_orphans(&sources, Duration::from_secs(3600));
    assert!(sources.join(".clone-tmp-1-2-3").exists(), "too young to go");
    sweep_orphans(&sources, Duration::ZERO);
    assert!(!sources.join(".clone-tmp-1-2-3").exists());
    assert!(!sources.join(".clone-old-1-2-3").exists());
    assert!(
        sources.join("mine").exists(),
        "a foreign folder is never touched"
    );
    assert!(
        sources.join(".clone-tmpx").exists(),
        "only the own pattern with its dash"
    );
}

#[test]
fn the_cache_fetches_once_and_again_after_the_time_or_a_refresh() {
    let cache = CatalogCache::<u32>::new(Duration::from_secs(600));
    let calls = AtomicUsize::new(0);
    let fetch = || Ok(u32::try_from(calls.fetch_add(1, Ordering::SeqCst)).unwrap());
    let t0 = Instant::now();
    assert_eq!(*cache.get_or_fetch_at("a", t0, fetch).unwrap(), 0);
    assert_eq!(
        *cache
            .get_or_fetch_at("a", t0 + Duration::from_secs(599), fetch)
            .unwrap(),
        0
    );
    assert_eq!(
        *cache.get_or_fetch_at("b", t0, fetch).unwrap(),
        1,
        "another source"
    );
    assert_eq!(
        *cache
            .get_or_fetch_at("a", t0 + Duration::from_secs(601), fetch)
            .unwrap(),
        2
    );
    cache.forget_all();
    assert_eq!(
        *cache
            .get_or_fetch_at("a", t0 + Duration::from_secs(602), fetch)
            .unwrap(),
        3
    );
    assert!(
        cache
            .get_or_fetch_at("c", t0, || Err("down".to_owned()))
            .is_err()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 4, "a failure is not kept");
}

fn git(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.org")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.org")
        .output()
        .is_ok_and(|o| o.status.success())
}

#[test]
fn two_real_clones_at_once_both_work() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("cat");
    fs::create_dir_all(&repo).unwrap();
    fs::write(repo.join("catalog.xml"), "real").unwrap();
    fs::write(repo.join("catalog.xml.asc"), "sig").unwrap();
    if !(git(&repo, &["init", "-q"])
        && git(&repo, &["add", "."])
        && git(&repo, &["commit", "-q", "-m", "c"]))
    {
        eprintln!("git is not there: skipped");
        return;
    }
    let url = format!("file://{}", repo.display());
    let root = dir.path().join("root");
    std::thread::scope(|s| {
        let hs: Vec<_> = (0..3)
            .map(|_| s.spawn(|| with_clone(&root, &url, read_both)))
            .collect();
        for h in hs {
            assert_eq!(h.join().unwrap().unwrap(), "real");
        }
    });
}
