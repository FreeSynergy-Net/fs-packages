//! Pattern: none (a stop watch for the phases of an install: catalog clone, download, checks, install command).
//! Off unless `FS_PACKAGES_TIMING` is set; then each phase says on the error stream how long it took, so a slow install can be taken apart.

use std::time::{Duration, Instant};

/// True when the person (or a test) asked for the phase times.
#[must_use]
pub fn enabled() -> bool {
    std::env::var_os("FS_PACKAGES_TIMING").is_some_and(|v| !v.is_empty())
}

/// The line that is printed for a phase.
#[must_use]
pub fn line(label: &str, took: Duration) -> String {
    format!("timing: {label}: {:.2} s", took.as_secs_f64())
}

/// Runs `f` and, when timing is on, says how long it took.
pub fn timed<T>(label: &str, f: impl FnOnce() -> T) -> T {
    if !enabled() {
        return f();
    }
    let start = Instant::now();
    let out = f();
    eprintln!("{}", line(label, start.elapsed()));
    out
}
