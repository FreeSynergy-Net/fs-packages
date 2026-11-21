use fs_packages_sources::{Basis, FileRecord, Manifest, ManifestStore};

fn sample(name: &str) -> Manifest {
    Manifest {
        schema: 1,
        name: name.into(),
        version: "1.0.0".into(),
        source: "https://example.org/cat".into(),
        signer: "ABCD".into(),
        basis: Basis::Installed,
        recorded_at: 7,
        files: vec![FileRecord {
            path: "bin/x".into(),
            hash: "sha256:00".into(),
            size: 1,
            mode: 0o755,
        }],
    }
}

#[test]
fn a_list_is_saved_read_listed_and_forgotten() {
    let tmp = tempfile::tempdir().unwrap();
    let store = ManifestStore::new(tmp.path());
    assert!(store.load("x", "1.0.0").unwrap().is_none());
    store.save(&sample("x")).unwrap();
    store.save(&sample("a")).unwrap();
    assert_eq!(store.load("x", "1.0.0").unwrap().unwrap(), sample("x"));
    assert_eq!(store.names().unwrap(), ["a", "x"]);
    store.forget("x").unwrap();
    store.forget("x").unwrap();
    assert_eq!(store.names().unwrap(), ["a"]);
}

#[test]
fn a_name_that_is_not_a_plain_file_name_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let store = ManifestStore::new(tmp.path());
    assert!(store.save(&sample("../evil")).is_err());
    assert!(store.load(".hidden", "1.0.0").is_err());
    assert!(store.forget("a/b").is_err());
    let mut odd = sample("x");
    odd.version = "../1".into();
    assert!(store.save(&odd).is_err());
}

#[test]
fn the_text_form_is_stable_and_marks_the_basis() {
    let tmp = tempfile::tempdir().unwrap();
    let store = ManifestStore::new(tmp.path());
    let mut m = sample("x");
    m.basis = Basis::Retroactive;
    store.save(&m).unwrap();
    let text = std::fs::read_to_string(tmp.path().join("manifests/x/1.0.0.toml")).unwrap();
    assert!(text.contains("basis = \"retroactive\""), "{text}");
    assert!(text.contains("hash = \"sha256:00\""));
}

#[test]
fn saving_from_several_threads_never_tears_the_list_and_leaves_no_temporary_file() {
    let tmp = tempfile::tempdir().unwrap();
    let store = std::sync::Arc::new(ManifestStore::new(tmp.path()));
    let handles: Vec<_> = (0..8)
        .map(|i| {
            let store = std::sync::Arc::clone(&store);
            std::thread::spawn(move || {
                for n in 0..20 {
                    let mut m = sample("x");
                    m.recorded_at = u64::try_from(i * 100 + n).unwrap();
                    store.save(&m).unwrap();
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(
        store.load("x", "1.0.0").unwrap().unwrap().name,
        "x",
        "the list reads back whole"
    );
    let left: Vec<String> = std::fs::read_dir(tmp.path().join("manifests/x"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(left, ["1.0.0.toml"], "no temporary file is left");
}

fn at(name: &str, version: &str) -> Manifest {
    let mut m = sample(name);
    m.version = version.into();
    m
}

/// Test 62 (S18), the store part. Mutant: `file()` without the version (one list per name again) makes the first assertion
/// red (only one version is left); `prune` that keeps the folder makes the last one red.
#[test]
fn two_versions_keep_two_lists_and_prune_removes_list_and_folder_together() {
    let tmp = tempfile::tempdir().unwrap();
    let store = ManifestStore::new(tmp.path());
    store.save(&at("p", "0.0.7")).unwrap();
    store.save(&at("p", "0.0.8")).unwrap();
    assert_eq!(store.versions("p").unwrap(), ["0.0.7", "0.0.8"]);
    assert_eq!(store.load("p", "0.0.7").unwrap().unwrap().version, "0.0.7");
    assert_eq!(store.names().unwrap(), ["p"]);
    assert_eq!(store.prune("p", &["0.0.8"]).unwrap(), ["0.0.7"]);
    assert!(!tmp.path().join("manifests/p/0.0.7.toml").exists());
    assert!(tmp.path().join("manifests/p/0.0.8.toml").is_file());
    assert!(store.prune("p", &["0.0.8"]).unwrap().is_empty());
    assert_eq!(store.prune("p", &[]).unwrap(), ["0.0.8"]);
    assert!(
        !tmp.path().join("manifests/p").exists(),
        "the folder goes with the last list"
    );
    // forget takes every version and the folder
    store.save(&at("q", "1")).unwrap();
    store.save(&at("q", "2")).unwrap();
    store.forget("q").unwrap();
    assert!(!tmp.path().join("manifests/q").exists());
    assert!(store.names().unwrap().is_empty());
}
