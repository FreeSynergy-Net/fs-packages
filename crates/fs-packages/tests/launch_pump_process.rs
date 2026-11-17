//! The real log pump (`fs-packages log-pump`, this crate's binary) keeps the log of a started program at 64 KB although the program
//! that was started has ended long ago and the store's launcher is gone: a background process that the program left behind writes 300 KB
//! afterwards, and the log holds its last 64 KB, ending with its last line.
//!
//! Pattern: Integration test with the real binary and a stub program in a folder of the test target (never /tmp).

#![cfg(all(unix, feature = "gui"))]

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use fs_packages_app::launch_log::{LOG_MAX, log_file};
use fs_packages_app::launch_watch::LaunchReport;
use fs_packages_app::launcher::{Launcher, ProcessLauncher};

fn wait_until(mut what: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(30);
    while Instant::now() < end {
        if what() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    what()
}

#[test]
fn the_log_stays_within_64_kb_after_the_started_program_and_the_launcher_are_gone() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("launch-real-pump");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("bin")).unwrap();
    let mark = dir.join("done");
    let stub = dir.join("bin").join("fs-stub-flood");
    let mut file = fs::File::create(&stub).unwrap();
    // The program ends at once; a background process of it writes 300 KB three seconds later.
    write!(
        file,
        "#!/bin/sh\n( sleep 3\nhead -c 300000 /dev/zero | tr '\\000' x >&2\necho >&2\necho tail-end >&2\necho ok > '{}' ) &\necho early >&2\nexit 0\n",
        mark.display()
    )
    .unwrap();
    file.sync_all().unwrap();
    drop(file);
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();

    let state = dir.join("state").join("fs-packages");
    let launcher = ProcessLauncher::new(dir.join("bin"))
        .with_pump(env!("CARGO_BIN_EXE_fs-packages").into())
        .with_probe(Arc::new(|_| Some(true)))
        .with_state_dir(state.clone());
    launcher.start("fs-stub-flood").unwrap();
    let mut reports = Vec::new();
    assert!(wait_until(|| {
        reports.extend(launcher.take_reports());
        !reports.is_empty()
    }));
    assert!(
        matches!(&reports[0], LaunchReport::EndedAtOnce { lines, .. } if lines == &["early"]),
        "{reports:?}"
    );
    // The store is closed from here on: nothing of the launcher is left.
    drop(launcher);
    assert!(wait_until(|| mark.exists()), "the background process wrote");
    let log = log_file(&state, "fs-stub-flood");
    assert!(
        wait_until(|| fs::read(&log).is_ok_and(|b| b.ends_with(b"tail-end\n"))),
        "the end of the output is in the log"
    );
    let bytes = fs::read(&log).unwrap();
    assert_eq!(bytes.len(), LOG_MAX, "exactly the last 64 KB of 300 KB");
    assert!(bytes.starts_with(b"x"), "the beginning was cut off");
    assert_eq!(
        fs::metadata(&log).unwrap().permissions().mode() & 0o777,
        0o600
    );
}
