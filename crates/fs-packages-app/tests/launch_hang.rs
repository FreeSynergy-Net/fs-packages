//! A program that runs but shows no sign of life is reported once (as a hint, not an error) and can be stopped; the store ends only the
//! process it started itself, by its handle; a program with a sign of life, or one that cannot be judged, is never reported.
//!
//! Pattern: Integration test with stub scripts and a probe that answers as the test says (the real probe reads Linux `/proc`).

#![cfg(unix)]

mod launch_stage;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use fs_packages_app::launch_life::maps_show_sign;
use fs_packages_app::launch_watch::LaunchReport;
use fs_packages_app::launcher::Launcher;

use launch_stage::{alive, guard, read_pid, stage, wait_report, wait_until};

fn pid_stub(s: &launch_stage::Stage, name: &str) -> std::path::PathBuf {
    let pid = s.dir.join(format!("{name}.pid"));
    s.stub(
        name,
        &format!("echo $$ > '{}'\nexec sleep 60", pid.display()),
    );
    pid
}

#[test]
fn a_program_without_a_sign_of_life_is_reported_once_and_can_be_stopped() {
    let _g = guard();
    let s = stage("hang");
    let pid = pid_stub(&s, "fs-stub-hang");
    let l = s.launcher(Duration::from_millis(100), Some(false));
    l.start("fs-stub-hang").unwrap();
    let reports = wait_report(&l);
    let [LaunchReport::NoSignOfLife { command, seconds }] = reports.as_slice() else {
        panic!("one hint expected, got {reports:?}");
    };
    assert_eq!(command, "fs-stub-hang");
    assert!(*seconds < 10, "{seconds}");
    assert_eq!(reports[0].code(), "launch-no-sign-of-life");
    std::thread::sleep(Duration::from_millis(1500));
    assert!(l.take_reports().is_empty(), "told once, not at every check");
    let pid = read_pid(&pid);
    assert!(alive(pid));
    assert!(l.stop("fs-stub-hang"), "the child the store started");
    assert!(wait_until(|| !alive(pid)), "the child is gone and reaped");
    assert!(
        wait_until(|| !l.stop("fs-stub-hang")),
        "nothing left to stop"
    );
}

#[test]
fn a_sign_of_life_after_the_hint_takes_the_hint_back() {
    let _g = guard();
    let s = stage("recover");
    pid_stub(&s, "fs-stub-slow");
    let alive_now = Arc::new(AtomicBool::new(false));
    let seen = Arc::clone(&alive_now);
    let l = s
        .launcher(Duration::from_millis(100), None)
        .with_probe(Arc::new(move |_| Some(seen.load(Ordering::SeqCst))));
    l.start("fs-stub-slow").unwrap();
    assert!(matches!(
        wait_report(&l).as_slice(),
        [LaunchReport::NoSignOfLife { .. }]
    ));
    alive_now.store(true, Ordering::SeqCst);
    let reports = wait_report(&l);
    assert!(
        matches!(reports.as_slice(), [LaunchReport::SignOfLife { command }] if command == "fs-stub-slow"),
        "{reports:?}"
    );
    assert!(l.stop("fs-stub-slow"));
}

#[test]
fn a_program_with_a_sign_of_life_or_not_to_be_judged_is_not_reported() {
    let _g = guard();
    for (name, answer) in [("fs-stub-alive", Some(true)), ("fs-stub-unknown", None)] {
        let s = stage(name);
        pid_stub(&s, name);
        let l = s.launcher(Duration::from_millis(100), answer);
        l.start(name).unwrap();
        std::thread::sleep(Duration::from_millis(1800));
        assert!(l.take_reports().is_empty(), "{name}");
        assert!(l.stop(name));
    }
}

#[test]
fn stop_never_touches_another_process() {
    let _g = guard();
    let s = stage("foreign");
    pid_stub(&s, "fs-stub-mine");
    let l = s.launcher(Duration::from_millis(100), Some(true));
    // A process of this test that is not the store's program: it must survive every stop.
    let mut other = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .unwrap();
    assert!(!l.stop("sleep"), "not started by the launcher");
    assert!(!l.stop("fs-stub-mine"), "not started yet");
    assert!(!l.stop("../fs-stub-mine"));
    assert!(alive(other.id()));
    other.kill().unwrap();
    other.wait().unwrap();
}

#[test]
fn the_real_probe_reads_the_loaded_libraries() {
    let wayland = "7f00-7f01 r-xp 00000000 08:01 1 /usr/lib64/libwayland-client.so.0.22.0";
    let vulkan = "7f00-7f01 r-xp 00000000 08:01 1 /usr/lib64/libvulkan.so.1";
    let plain = "7f00-7f01 r-xp 00000000 08:01 1 /usr/lib64/libc.so.6\n7f02-7f03 r--p 0 08:01 2 /usr/bin/fs-tasks";
    assert!(maps_show_sign(wayland));
    assert!(maps_show_sign(vulkan));
    assert!(!maps_show_sign(plain));
    assert!(!maps_show_sign(""));
}
