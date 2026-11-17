//! Pattern: none (plain functions: where the log of a started program lives, how it is made private, kept short and read).
//!
//! The log of a program `fs-tasks` is `<state>/fs-packages/launch-fs-tasks.log` with `<state>` = `$XDG_STATE_HOME` (an absolute path) or
//! `~/.local/state`. One file per program, wiped at each start, 0600 in a folder of 0700; never longer than [`LOG_MAX`] bytes (the end stays).
//! The name of the program is the only part that comes from outside, so it is reduced to `[a-z0-9-]` before it is part of a path.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// The most bytes a log keeps; a longer one loses its beginning.
pub const LOG_MAX: usize = 64 * 1024;
/// How many of the last lines of a log a message on the tile shows.
pub const SHOWN_LINES: usize = 10;
/// The longest line a message shows (characters); the rest is cut.
const LINE_MAX: usize = 200;

/// The name of a program as part of a file name: lower case letters, digits and `-`; every other character (a path separator, a dot, a
/// space) becomes `-`. A name that is empty after that is `program`.
#[must_use]
pub fn log_name(command: &str) -> String {
    let name: String = command
        .chars()
        .map(|c| match c.to_ascii_lowercase() {
            c @ ('a'..='z' | '0'..='9' | '-') => c,
            _ => '-',
        })
        .collect();
    if name.is_empty() {
        "program".to_owned()
    } else {
        name
    }
}

/// The folder of the logs: `$XDG_STATE_HOME/fs-packages` (only when absolute, as the standard says), else `~/.local/state/fs-packages`.
/// `None` when there is no home.
#[must_use]
pub fn state_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .map(|h| h.join(".local").join("state"))
        })?;
    Some(base.join("fs-packages"))
}

/// The log file of `command` in `dir`.
#[must_use]
pub fn log_file(dir: &Path, command: &str) -> PathBuf {
    dir.join(format!("launch-{}.log", log_name(command)))
}

/// Makes `dir` (and what is missing above it) with mode 0700, never wider; a folder that exists and is open for others is closed. The
/// folder itself must not be a symbolic link (checked with `symlink_metadata`, before anything is changed on it): a link there could point
/// the log, and the closing of the rights, at a place that is not ours.
///
/// # Errors
/// The cause when the folder cannot be made or closed, or is a symbolic link or not a folder.
pub fn create_private_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
        check_real_dir(dir)?;
        let mode = fs::metadata(dir)?.permissions().mode();
        if mode & 0o077 != 0 {
            fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(dir)?;
        check_real_dir(dir)
    }
}

/// `dir` is a real folder: not a symbolic link, not something else.
fn check_real_dir(dir: &Path) -> io::Result<()> {
    let kind = fs::symlink_metadata(dir)?.file_type();
    if kind.is_symlink() {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the log folder is a symbolic link",
        ))
    } else if !kind.is_dir() {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the log folder is not a folder",
        ))
    } else {
        Ok(())
    }
}

/// Opens the log of a new start: made 0600 (an old file is set to 0600 as well) and wiped. A symbolic link at the path is never followed
/// (`O_NOFOLLOW`): the open fails instead of wiping a file somewhere else.
///
/// # Errors
/// The cause when the file cannot be made or wiped, or the path is a symbolic link.
pub fn open_fresh(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    Ok(file)
}

/// Opens the existing log for the log pump (read and write, never following a symbolic link, nothing is made or wiped).
///
/// # Errors
/// The cause when the file is not there, or is a symbolic link.
pub fn open_existing(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    options.open(path)
}

/// The last `count` lines of the log, each without control characters and cut to a sane length; empty lines are left out.
#[must_use]
pub fn last_lines(path: &Path, count: usize) -> Vec<String> {
    let Ok(mut file) = File::open(path) else {
        return Vec::new();
    };
    let len = file.metadata().map_or(0, |m| m.len());
    let max = LOG_MAX as u64;
    if file.seek(SeekFrom::Start(len.saturating_sub(max))).is_err() {
        return Vec::new();
    }
    let mut bytes = Vec::new();
    if file.read_to_end(&mut bytes).is_err() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&bytes);
    let mut lines: Vec<String> = text
        .lines()
        .map(clean)
        .filter(|l| !l.trim().is_empty())
        .collect();
    let from = lines.len().saturating_sub(count);
    lines.drain(..from);
    lines
}

/// A line for the screen: no control characters (an escape sequence of a terminal program), at most [`LINE_MAX`] characters.
fn clean(line: &str) -> String {
    line.chars()
        .map(|c| if c == '\t' { ' ' } else { c })
        .filter(|c| !c.is_control())
        .take(LINE_MAX)
        .collect()
}
