//! Pattern: none (tests of the lock across processes: two children of this test binary read at the same moment).

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::tests::read_both;
use super::*;

/// The child of `two_processes_*`: does nothing unless the parent asked.
#[test]
fn child_read() {
    let Some(root) = std::env::var_os("FS_CF_CHILD_ROOT") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    let lock = std::env::var_os("FS_CF_CHILD_NOLOCK").is_none();
    let marker = root.join("active.marker");
    let clone = |_: &str, dest: &Path| -> Result<(), String> {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&marker)
            .map_err(|_| "shallow file could not be locked".to_owned())?;
        fs::create_dir_all(dest).map_err(|e| e.to_string())?;
        fs::write(dest.join("catalog.xml"), "p").map_err(|e| e.to_string())?;
        std::thread::sleep(Duration::from_millis(400));
        fs::write(dest.join("catalog.xml.asc"), "sig").map_err(|e| e.to_string())?;
        fs::remove_file(&marker).map_err(|e| e.to_string())
    };
    with_clone_using(
        &root,
        "https://example.org/p.git",
        &clone,
        lock,
        ORPHAN_AFTER,
        read_both,
    )
    .unwrap();
}

fn two_children(root: &Path, nolock: bool) -> Vec<bool> {
    let exe = std::env::current_exe().unwrap();
    let spawn = || {
        let mut c = Command::new(&exe);
        c.args([
            "--exact",
            "catalog_fetch::tests_process::child_read",
            "--test-threads=1",
        ])
        .env("FS_CF_CHILD_ROOT", root);
        if nolock {
            c.env("FS_CF_CHILD_NOLOCK", "1");
        }
        c.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        c.spawn().unwrap()
    };
    let (mut a, mut b) = (spawn(), spawn());
    vec![a.wait().unwrap().success(), b.wait().unwrap().success()]
}

#[test]
fn two_processes_both_get_a_result() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(two_children(dir.path(), false), vec![true, true]);
}

#[test]
fn two_processes_without_the_lock_collide() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        two_children(dir.path(), true).contains(&false),
        "the mutant must turn red"
    );
}
