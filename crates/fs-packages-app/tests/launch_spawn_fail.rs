//! When the program cannot be started after the log pump was, the pump is picked up (waited for) and does not stay a zombie until the store ends.
//!
//! Pattern: Integration test with stub scripts; a file that is found but not executable makes `spawn` fail.

#![cfg(unix)]

mod launch_stage;

use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use fs_packages_app::launcher::{Launcher, ProcessLauncher};

use launch_stage::{alive, guard, read_pid, stage, wait_until};

#[test]
fn a_failed_start_picks_up_the_pump() {
    let _g = guard();
    let s = stage("spawn-fail");
    let pid_file = s.dir.join("pump.pid");
    s.stub(
        "pump-with-pid",
        &format!("echo $$ > '{}'\nexec cat >> \"$2\"", pid_file.display()),
    );
    // Found on the path, not executable: `spawn` says permission denied.
    let program = s.dir.join("bin").join("fs-stub-noexec");
    std::fs::write(&program, "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o644)).unwrap();
    let l = ProcessLauncher::new(s.dir.join("bin"))
        .with_pump(s.dir.join("bin").join("pump-with-pid"))
        .with_state_dir(s.state())
        .with_observe(Duration::from_millis(100));
    let result = l.start("fs-stub-noexec");
    assert!(result.is_err(), "the program could not be started");
    let pid = read_pid(&pid_file);
    assert!(
        wait_until(|| !alive(pid)),
        "the pump ended and was picked up (a zombie would still be in /proc)"
    );
}
