#![cfg(unix)] // symbolic links of Unix: Windows port open
//! The lists of files per version: both exist after an update, `prune` takes list and folder, `adopt_missing` stays retroactive.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{Idle, archive, verified};
use fs_packages_core::*;
use fs_packages_engine::{TarGz, adopt_missing, prune_lists, record_install};
use fs_packages_sources::{Basis, ManifestStore};

fn install(tmp: &Path, name: &str, v: Version, content: &str) -> PathBuf {
    let root = tmp.join("root");
    let a = archive(tmp, &format!("{name}-{v}"), content);
    run_update(
        &Update {
            package: &verified(name, v),
            previous: None,
        },
        &mut TarGz::new(&root, &a),
        &Idle,
    )
    .unwrap();
    root
}

fn none(_: &str) -> String {
    String::new()
}

/// Test 62 (S18): after an update 0.0.7 -> 0.0.8 both lists exist; pruning removes the list and the folder of the older
/// one together and never the active one; a list made afterwards stays `Retroactive`.
#[test]
fn after_an_update_both_lists_exist_and_prune_removes_list_and_folder_never_current() {
    let tmp = tempfile::tempdir().unwrap();
    let root = install(tmp.path(), "prog", Version::new(0, 0, 7), "seven");
    assert!(record_install(&root, "prog", "0.0.7", "s", "K").unwrap());
    let a = archive(tmp.path(), "prog-8", "eight!");
    run_update(
        &Update {
            package: &verified("prog", Version::new(0, 0, 8)),
            previous: Some(Version::new(0, 0, 7)),
        },
        &mut TarGz::new(&root, &a),
        &Idle,
    )
    .unwrap();
    assert!(record_install(&root, "prog", "0.0.8", "s", "K").unwrap());
    let store = ManifestStore::new(&root);
    assert_eq!(store.versions("prog").unwrap(), ["0.0.7", "0.0.8"]);
    assert!(root.join("manifests/prog/0.0.7.toml").is_file());
    assert!(root.join("manifests/prog/0.0.8.toml").is_file());
    let old = store.load("prog", "0.0.7").unwrap().unwrap();
    assert_eq!(
        (old.version.as_str(), old.basis),
        ("0.0.7", Basis::Installed)
    );
    assert_eq!(
        old.files[0].size, 5,
        "the list of the old version is its own"
    );
    // the old version's folder is deleted: its list and, at the end, the list folder go with it; the active one stays
    fs::remove_dir_all(root.join("packages/prog/0.0.7")).unwrap();
    assert_eq!(prune_lists(&root, "prog", "0.0.8").unwrap(), ["0.0.7"]);
    assert!(!root.join("manifests/prog/0.0.7.toml").exists());
    assert!(root.join("manifests/prog/0.0.8.toml").is_file());
    // never current: even when its folder were missing, the active version keeps its list
    fs::remove_dir_all(root.join("packages/prog/0.0.8")).unwrap();
    assert!(prune_lists(&root, "prog", "9.9.9").unwrap().is_empty());
    assert!(root.join("manifests/prog/0.0.8.toml").is_file());
    // nothing active and nothing on disk: the list folder goes as well
    fs::remove_file(root.join("current/prog")).unwrap();
    assert_eq!(prune_lists(&root, "prog", "9.9.9").unwrap(), ["0.0.8"]);
    assert!(!root.join("manifests/prog").exists());
}

/// `adopt_missing` makes a list per active version, marked `Retroactive`, and never touches a list of an installed one.
#[test]
fn adopt_missing_stays_retroactive_and_leaves_the_list_of_the_other_version() {
    let tmp = tempfile::tempdir().unwrap();
    let root = install(tmp.path(), "prog", Version::new(0, 0, 7), "seven");
    record_install(&root, "prog", "0.0.7", "s", "K").unwrap();
    let a = archive(tmp.path(), "prog-8", "eight!");
    run_update(
        &Update {
            package: &verified("prog", Version::new(0, 0, 8)),
            previous: Some(Version::new(0, 0, 7)),
        },
        &mut TarGz::new(&root, &a),
        &Idle,
    )
    .unwrap();
    assert_eq!(adopt_missing(&root, &none).unwrap(), ["prog"]);
    let store = ManifestStore::new(&root);
    let made = store.load("prog", "0.0.8").unwrap().unwrap();
    assert_eq!((made.basis, made.signer.as_str()), (Basis::Retroactive, ""));
    let before = store.load("prog", "0.0.7").unwrap().unwrap();
    assert_eq!(
        (before.basis, before.signer.as_str()),
        (Basis::Installed, "K")
    );
    assert!(adopt_missing(&root, &none).unwrap().is_empty());
}
