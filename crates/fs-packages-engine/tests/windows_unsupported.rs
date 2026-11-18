//! The wanted Windows outcome, positive: what is not ported ends with the open error `unsupported-on-windows{<feature>}` and
//! leaves nothing behind (no `current` link, no half version, no private file written unprotected, no package-manager call).
//!
//! The tests run on Windows only (`cfg_attr(not(windows), ignore)`): there the engine cannot make a link or a private folder.
//! They are compiled everywhere, so a type error shows on Linux too; run on Linux with `--ignored` they FAIL on purpose
//! (the install succeeds there), which proves they do not pass without the Windows behaviour.

mod common;

use std::fs;
use std::path::Path;

use common::{Idle, archive, verified};
use fs_packages_core::*;
use fs_packages_engine::{Commander, Deb, Rpm, TarGz};

fn entries(dir: &Path) -> usize {
    fs::read_dir(dir).map_or(0, Iterator::count)
}

/// The open error comes up in the cause (code and parameter, no sentence), and the update is reported as rolled back.
fn assert_open(r: Result<InstallState, UpdateError>, feature: &str) {
    let want = format!("unsupported-on-windows{{{feature}}}");
    match r {
        Err(UpdateError::RolledBack(InstallError(cause))) => assert_eq!(cause, want),
        other => panic!("expected RolledBack({want}), got {other:?}"),
    }
}

#[derive(Default)]
struct Fake {
    calls: usize,
}
impl Commander for &mut Fake {
    fn run(&mut self, _: &str, _: &[&str]) -> Result<(), String> {
        self.calls += 1;
        Ok(())
    }
}

#[test]
#[cfg_attr(
    not(windows),
    ignore = "checks the open Windows outcome (run on Windows only)"
)]
fn a_tar_gz_install_ends_with_the_open_symlink_error_and_leaves_no_link() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    let a = archive(tmp.path(), "one", "v1");
    let mut s = TarGz::new(&root, &a);
    let r = run_update(
        &Update {
            package: &verified("tasks", Version::new(1, 0, 0)),
            previous: None,
        },
        &mut s,
        &Idle,
    );
    assert_open(r, "symlink");
    assert!(!root.join("current/tasks").exists());
    assert_eq!(entries(&root.join("current")), 0, "current holds no entry");
    // no half unpacked version and no temporary leftovers: only the complete folder of the version may exist
    let versions: Vec<_> = fs::read_dir(root.join("packages/tasks"))
        .map(|d| d.map(|e| e.unwrap().file_name()).collect())
        .unwrap_or_default();
    assert!(
        versions.is_empty() || versions == ["1.0.0"],
        "leftovers: {versions:?}"
    );
    if !versions.is_empty() {
        assert_eq!(
            fs::read_to_string(root.join("packages/tasks/1.0.0/bin/prog")).unwrap(),
            "v1"
        );
    }
}

#[test]
#[cfg_attr(
    not(windows),
    ignore = "checks the open Windows outcome (run on Windows only)"
)]
fn a_deb_install_ends_with_the_open_private_dir_error_and_writes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("deb");
    let mut fake = Fake::default();
    let mut s = Deb::new(&dir, vec![b"DEBBYTES".to_vec()], &mut fake);
    let r = run_update(
        &Update {
            package: &verified("tasks", Version::new(1, 0, 0)),
            previous: None,
        },
        &mut s,
        &Idle,
    );
    drop(s);
    assert_open(r, "private-dir");
    assert_eq!(entries(&dir), 0, "no file was written");
    assert_eq!(fake.calls, 0, "the package manager was not called");
}

#[test]
#[cfg_attr(
    not(windows),
    ignore = "checks the open Windows outcome (run on Windows only)"
)]
fn an_rpm_install_ends_with_the_open_private_dir_error_and_writes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("rpm");
    let mut fake = Fake::default();
    let mut s = Rpm::new(&dir, vec![b"RPMBYTES".to_vec()], &mut fake);
    let r = run_update(
        &Update {
            package: &verified("tasks", Version::new(1, 0, 0)),
            previous: None,
        },
        &mut s,
        &Idle,
    );
    drop(s);
    assert_open(r, "private-dir");
    assert_eq!(entries(&dir), 0, "no file was written");
    assert_eq!(fake.calls, 0, "the package manager was not called");
}
