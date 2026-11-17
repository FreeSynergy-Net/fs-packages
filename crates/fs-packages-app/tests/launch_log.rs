//! The launcher against stub programs: a start that fails right away is reported with the end of its log; one that ends at once without
//! a failure is reported as a hint; one that runs on, or ends late, is not; the log is private (0600 in 0700), wiped at each start; the
//! name in the path is reduced. (The 64 KB limit and the log pump itself are tested in `launch_pump.rs` and in the tests of the
//! `fs-packages` crate; here the pump is a stub that copies its input.)
//!
//! Pattern: Integration test with stub scripts in a folder of the test target (never /tmp); one mutex and `sync_all` (a script that is
//! still open for writing cannot be executed: ETXTBSY).

#![cfg(unix)]
#![allow(clippy::assert_is_empty)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use fs_packages_app::launch_log::{log_file, log_name};
use fs_packages_app::launch_watch::LaunchReport;
use fs_packages_app::launcher::{Launcher, ProcessLauncher};

static STUBS: Mutex<()> = Mutex::new(());

fn guard() -> std::sync::MutexGuard<'static, ()> {
    STUBS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct Stage {
    dir: PathBuf,
}

fn stage(name: &str) -> Stage {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("launch-{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("bin")).unwrap();
    let stage = Stage { dir };
    // The stand-in for the log pump: `<pump> log-pump <log>` copies its input into the log.
    stage.stub("log-pump-stub", "exec cat >> \"$2\"");
    stage
}

impl Stage {
    fn stub(&self, name: &str, body: &str) {
        use std::io::Write;
        let path = self.dir.join("bin").join(name);
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(format!("#!/bin/sh\n{body}\n").as_bytes())
            .unwrap();
        file.sync_all().unwrap();
        drop(file);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fn state(&self) -> PathBuf {
        self.dir.join("state").join("fs-packages")
    }
    fn launcher(&self, observe: Duration) -> ProcessLauncher {
        ProcessLauncher::new(self.dir.join("bin"))
            .with_pump(self.dir.join("bin").join("log-pump-stub"))
            .with_probe(std::sync::Arc::new(|_| Some(true)))
            .with_state_dir(self.state())
            .with_observe(observe)
    }
}

fn wait_report(l: &ProcessLauncher) -> Vec<LaunchReport> {
    let end = Instant::now() + Duration::from_secs(10);
    while Instant::now() < end {
        let r = l.take_reports();
        if !r.is_empty() {
            return r;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Vec::new()
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn a_program_that_fails_at_once_is_reported_with_the_line_it_wrote() {
    let _g = guard();
    let s = stage("fails");
    s.stub("fs-stub-fail", "echo 'boom: no display' >&2\nexit 3");
    let l = s.launcher(Duration::from_secs(3));
    l.start("fs-stub-fail").unwrap();
    let reports = wait_report(&l);
    let [
        LaunchReport::Failed {
            command,
            code,
            lines,
        },
    ] = reports.as_slice()
    else {
        panic!("one failure expected, got {reports:?}");
    };
    assert_eq!(command, "fs-stub-fail");
    assert_eq!(*code, Some(3));
    assert_eq!(lines, &["boom: no display"]);
}

#[test]
fn a_program_that_runs_on_is_not_reported() {
    let _g = guard();
    let s = stage("runs");
    let pid = s.dir.join("pid");
    s.stub(
        "fs-stub-run",
        &format!(
            "echo hello >&2\necho $$ > '{}'\nexec sleep 30",
            pid.display()
        ),
    );
    let l = s.launcher(Duration::from_millis(600));
    l.start("fs-stub-run").unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    assert!(l.take_reports().is_empty());
    let pid = fs::read_to_string(&pid).unwrap();
    let status = std::process::Command::new("kill")
        .arg(pid.trim())
        .status()
        .unwrap();
    assert!(status.success());
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        l.take_reports().is_empty(),
        "an end after the observation time is not an early failure"
    );
}

#[test]
fn a_failure_after_the_observation_time_is_not_reported() {
    let _g = guard();
    let s = stage("late");
    s.stub("fs-stub-late", "echo late >&2\nsleep 1\nexit 2");
    let l = s.launcher(Duration::from_millis(300));
    l.start("fs-stub-late").unwrap();
    std::thread::sleep(Duration::from_millis(2500));
    assert!(l.take_reports().is_empty());
}

#[test]
fn an_early_clean_end_is_reported_as_a_hint_with_its_lines() {
    let _g = guard();
    let s = stage("clean");
    s.stub("fs-stub-clean", "echo done >&2\nexit 0");
    let l = s.launcher(Duration::from_secs(3));
    l.start("fs-stub-clean").unwrap();
    let reports = wait_report(&l);
    let [LaunchReport::EndedAtOnce { command, lines }] = reports.as_slice() else {
        panic!("one hint expected, got {reports:?}");
    };
    assert_eq!(command, "fs-stub-clean");
    assert_eq!(lines, &["done"]);
    assert_eq!(reports[0].code(), "launch-ended-at-once");
}

#[test]
fn a_clean_end_after_the_observation_time_is_not_reported() {
    let _g = guard();
    let s = stage("clean-late");
    s.stub("fs-stub-clean-late", "sleep 1\nexit 0");
    let l = s.launcher(Duration::from_millis(300));
    l.start("fs-stub-clean-late").unwrap();
    std::thread::sleep(Duration::from_millis(2500));
    assert!(l.take_reports().is_empty());
}

#[test]
fn the_log_is_private_even_when_folder_and_file_were_open() {
    let _g = guard();
    let s = stage("modes");
    s.stub("fs-stub-fail", "echo x >&2\nexit 1");
    let l = s.launcher(Duration::from_secs(3));
    l.start("fs-stub-fail").unwrap();
    wait_report(&l);
    let log = log_file(&s.state(), "fs-stub-fail");
    assert_eq!(mode(&s.state()), 0o700);
    assert_eq!(mode(&log), 0o600);
    // Folder and file open for others (an older version, a wrong umask): the next start closes both.
    fs::set_permissions(s.state(), fs::Permissions::from_mode(0o755)).unwrap();
    fs::set_permissions(&log, fs::Permissions::from_mode(0o644)).unwrap();
    l.start("fs-stub-fail").unwrap();
    wait_report(&l);
    assert_eq!(mode(&s.state()), 0o700);
    assert_eq!(mode(&log), 0o600);
    assert_eq!(l.log_path("fs-stub-fail"), Some(log));
    assert_eq!(l.log_path("fs-stub-other"), None);
}

#[test]
fn a_new_start_overwrites_the_log() {
    let _g = guard();
    let s = stage("again");
    let n = s.dir.join("n");
    s.stub(
        "fs-stub-n",
        &format!("echo \"run $(cat '{}')\" >&2\nexit 1", n.display()),
    );
    let l = s.launcher(Duration::from_secs(3));
    fs::write(&n, "1").unwrap();
    l.start("fs-stub-n").unwrap();
    wait_report(&l);
    fs::write(&n, "2").unwrap();
    l.start("fs-stub-n").unwrap();
    wait_report(&l);
    let text = fs::read_to_string(log_file(&s.state(), "fs-stub-n")).unwrap();
    assert_eq!(text, "run 2\n");
}

#[test]
fn the_name_is_reduced_before_it_is_part_of_a_path() {
    assert_eq!(log_name("fs-tasks"), "fs-tasks");
    assert_eq!(log_name("FS-Tasks"), "fs-tasks");
    assert_eq!(log_name("../../etc/passwd"), "------etc-passwd");
    assert_eq!(log_name("a b.c"), "a-b-c");
    assert_eq!(log_name(""), "program");
    let dir = Path::new("/state/fs-packages");
    assert_eq!(log_file(dir, "../x").parent(), Some(dir));
    let s = stage("name");
    let l = s.launcher(Duration::from_millis(100));
    assert!(l.start("../bin/fs-stub").is_err());
}

#[test]
fn a_log_that_cannot_be_written_is_told_with_its_cause_and_the_program_starts() {
    let _g = guard();
    let s = stage("nolog");
    let mark = s.dir.join("started");
    s.stub("fs-stub-nolog", &format!("echo ok > '{}'", mark.display()));
    // The state folder would have to be made below a file: impossible.
    fs::write(s.dir.join("blocker"), "x").unwrap();
    let l = ProcessLauncher::new(s.dir.join("bin"))
        .with_state_dir(s.dir.join("blocker").join("fs-packages"));
    l.start("fs-stub-nolog").unwrap();
    let reports = wait_report(&l);
    let [LaunchReport::LogUnwritable { command, cause }] = reports.as_slice() else {
        panic!("one report expected, got {reports:?}");
    };
    assert_eq!(command, "fs-stub-nolog");
    assert!(cause.contains("blocker"), "{cause}");
    let end = Instant::now() + Duration::from_secs(5);
    while !mark.exists() && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(mark.exists(), "the program was started anyway");
}
