//! The log is never made through a symbolic link: a link in place of the log file, or in place of the log folder, makes the log
//! unwritable (told with the cause), the file or folder it points to stays as it was, and the program is started anyway.
//!
//! Pattern: Integration test with stub scripts; the "foreign" file and folder belong to the test and stand for anything that is not the log.

#![cfg(unix)]

mod launch_stage;

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::time::Duration;

use fs_packages_app::launch_log::{create_private_dir, log_file, open_fresh};
use fs_packages_app::launch_watch::LaunchReport;
use fs_packages_app::launcher::Launcher;

use launch_stage::{guard, stage, wait_report, wait_until};

fn mode(path: &std::path::Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn a_link_in_place_of_the_log_file_never_wipes_what_it_points_to() {
    let _g = guard();
    let s = stage("link-file");
    let mark = s.dir.join("started");
    s.stub("fs-stub-link", &format!("echo ok > '{}'", mark.display()));
    let foreign = s.dir.join("foreign.txt");
    fs::write(&foreign, "do not touch\n").unwrap();
    fs::set_permissions(&foreign, fs::Permissions::from_mode(0o644)).unwrap();
    create_private_dir(&s.state()).unwrap();
    symlink(&foreign, log_file(&s.state(), "fs-stub-link")).unwrap();
    let l = s.launcher(Duration::from_secs(3), Some(true));
    l.start("fs-stub-link").unwrap();
    let reports = wait_report(&l);
    let [LaunchReport::LogUnwritable { cause, .. }] = reports.as_slice() else {
        panic!("one report expected, got {reports:?}");
    };
    assert!(cause.contains("launch-fs-stub-link.log"), "{cause}");
    assert_eq!(fs::read_to_string(&foreign).unwrap(), "do not touch\n");
    assert_eq!(mode(&foreign), 0o644, "its rights stay");
    assert!(
        wait_until(|| mark.exists()),
        "the program was started anyway"
    );
}

#[test]
fn a_link_in_place_of_the_log_folder_is_refused_and_the_target_stays_as_it_was() {
    let _g = guard();
    let s = stage("link-dir");
    let mark = s.dir.join("started");
    s.stub("fs-stub-dir", &format!("echo ok > '{}'", mark.display()));
    let real = s.dir.join("somewhere-else");
    fs::create_dir_all(&real).unwrap();
    fs::set_permissions(&real, fs::Permissions::from_mode(0o755)).unwrap();
    fs::create_dir_all(s.state().parent().unwrap()).unwrap();
    symlink(&real, s.state()).unwrap();
    let l = s.launcher(Duration::from_secs(3), Some(true));
    l.start("fs-stub-dir").unwrap();
    let reports = wait_report(&l);
    let [LaunchReport::LogUnwritable { cause, .. }] = reports.as_slice() else {
        panic!("one report expected, got {reports:?}");
    };
    assert!(cause.contains("symbolic link"), "{cause}");
    assert_eq!(mode(&real), 0o755, "the rights of the target stay");
    assert_eq!(
        fs::read_dir(&real).unwrap().count(),
        0,
        "nothing was put there"
    );
    assert!(
        wait_until(|| mark.exists()),
        "the program was started anyway"
    );
}

#[test]
fn the_functions_refuse_a_link_themselves() {
    let s = stage("link-fn");
    let foreign = s.dir.join("foreign.txt");
    fs::write(&foreign, "keep\n").unwrap();
    let link = s.dir.join("log-link");
    symlink(&foreign, &link).unwrap();
    assert!(open_fresh(&link).is_err());
    assert_eq!(fs::read_to_string(&foreign).unwrap(), "keep\n");
    let dir_link = s.dir.join("dir-link");
    symlink(s.dir.join("bin"), &dir_link).unwrap();
    assert!(create_private_dir(&dir_link).is_err());
    // A real file and a real folder still work.
    assert!(open_fresh(&s.dir.join("fresh.log")).is_ok());
    assert!(create_private_dir(&s.dir.join("own")).is_ok());
}
