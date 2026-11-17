//! Pattern: Strategy (how the store tells that a started program shows a sign of life: a function the launcher can be given, so tests
//! decide the answer).
//!
//! The store cannot see the windows of another program, and it never reaches into another session. What it can read is its OWN child (and
//! the children of that): on Linux `/proc/<pid>/maps` says which libraries the process has loaded. A program that opens a window loads the
//! display and graphics libraries of the system at that moment; one that is still waiting for something before it gets that far has not.
//! A program that is linked to those libraries from its start always counts as alive (so the store never says "no sign" wrongly for it,
//! only too rarely). Where nothing can be read (not Linux, no right) the answer is "unknown", and then nothing is reported.

use std::sync::Arc;

/// Asks for the process `pid` (a child of the store): `Some(true)` a sign of life, `Some(false)` none yet, `None` cannot tell.
pub type SignProbe = Arc<dyn Fn(u32) -> Option<bool> + Send + Sync>;

/// The libraries a program has loaded once it talks to a display or a graphics driver.
const DISPLAY_LIBS: [&str; 6] = [
    "libwayland-client",
    "libxcb.so",
    "libX11.so",
    "libvulkan",
    "libEGL",
    "libGL.so",
];

/// The probe of this system.
#[must_use]
pub fn system_probe() -> SignProbe {
    Arc::new(|pid| tree_shows_sign(pid, 3))
}

/// True when `maps` (the text of `/proc/<pid>/maps`) lists a display or graphics library.
#[must_use]
pub fn maps_show_sign(maps: &str) -> bool {
    DISPLAY_LIBS.iter().any(|lib| maps.contains(lib))
}

/// The process and its children down to `depth` levels (a starter script that starts the real program is a child of the process).
fn tree_shows_sign(pid: u32, depth: u32) -> Option<bool> {
    let maps = std::fs::read_to_string(format!("/proc/{pid}/maps")).ok()?;
    if maps_show_sign(&maps) {
        return Some(true);
    }
    if depth == 0 {
        return Some(false);
    }
    for child in children(pid) {
        if tree_shows_sign(child, depth - 1) == Some(true) {
            return Some(true);
        }
    }
    Some(false)
}

/// The children of `pid`, from `/proc/<pid>/task/<tid>/children`; empty when it cannot be read.
fn children(pid: u32) -> Vec<u32> {
    let Ok(tasks) = std::fs::read_dir(format!("/proc/{pid}/task")) else {
        return Vec::new();
    };
    tasks
        .flatten()
        .filter_map(|t| std::fs::read_to_string(t.path().join("children")).ok())
        .flat_map(|text| {
            text.split_whitespace()
                .filter_map(|n| n.parse().ok())
                .collect::<Vec<u32>>()
        })
        .collect()
}
