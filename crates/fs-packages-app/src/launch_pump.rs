//! Pattern: Pipe and filter (the output of a started program flows through a small process of its own into its log, which never grows
//! beyond [`LOG_MAX`] bytes).
//!
//! The store gives the program a pipe instead of the log file, and starts `fs-packages log-pump <log>` as the other end. The pump
//! outlives the store and the starting process: it ends when the last writer of the pipe is gone. It keeps the last [`LOG_MAX`] bytes of
//! what it reads, so the file holds exactly the end of the output at every moment. A write that fails (disk full) never stops the reading:
//! a program whose output is not read would block, or die of a broken pipe.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use crate::launch_log::{LOG_MAX, open_existing};

/// The first argument of the pump: `fs-packages log-pump <log file>`.
pub const PUMP_ARG: &str = "log-pump";
/// The most that is read at once (a pipe holds 64 KB).
const CHUNK: usize = 64 * 1024;

/// After the log is full, the rewrite of the whole tail is done at most every [`REWRITE_EVERY`] or when [`REWRITE_BYTES`] new bytes are
/// waiting, and when the input ends or goes quiet for that long: line by line output would otherwise rewrite 64 KB for every line.
pub const REWRITE_EVERY: Duration = Duration::from_millis(100);
/// See [`REWRITE_EVERY`].
pub const REWRITE_BYTES: usize = 16 * 1024;

/// What [`pump_with`] did (for the tests and the measuring).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PumpStats {
    /// Whole-tail rewrites after the log was full.
    pub rewrites: usize,
    /// Appends before the log was full.
    pub appends: usize,
}

/// Reads `input` until it ends and keeps the last [`LOG_MAX`] bytes in `log`, which must be empty at the start. When `log` is an error
/// (it could not be opened) or a write fails, the input is still read to the end and thrown away.
///
/// # Errors
/// The first error of the log or of the input; the input was read to the end anyway.
pub fn pump(log: io::Result<File>, input: impl Read) -> io::Result<()> {
    pump_with(log, input, |_| {}).0
}

/// [`pump`], with `wait`: called before a read while a rewrite is waiting, with the time left until it is due; it may block that long
/// until `input` has something (the real pump polls the pipe), so a burst followed by silence is written after [`REWRITE_EVERY`] and not
/// only with the next output. Returns the result and what was written.
pub fn pump_with(
    log: io::Result<File>,
    mut input: impl Read,
    mut wait: impl FnMut(Duration),
) -> (io::Result<()>, PumpStats) {
    let mut stats = PumpStats::default();
    let (mut file, mut failure) = match log {
        Ok(file) => (Some(file), None),
        Err(e) => (None, Some(e)),
    };
    // Invariant: while `file` is there and nothing is `pending`, its content is exactly `tail`.
    let mut tail: Vec<u8> = Vec::new();
    let mut pending = 0usize;
    let mut since = Instant::now();
    let mut buf = vec![0u8; CHUNK];
    loop {
        if pending > 0 {
            let left = REWRITE_EVERY.saturating_sub(since.elapsed());
            if !left.is_zero() {
                wait(left);
            }
            if since.elapsed() >= REWRITE_EVERY {
                flush(
                    &mut file,
                    &mut failure,
                    &tail,
                    &mut pending,
                    &mut since,
                    &mut stats,
                );
            }
        }
        let n = match input.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => {
                failure.get_or_insert(e);
                break;
            }
        };
        let old = tail.len();
        tail.extend_from_slice(&buf[..n]);
        let overflow = tail.len().saturating_sub(LOG_MAX);
        tail.drain(..overflow);
        let Some(out) = file.as_mut() else { continue };
        if overflow == 0 && pending == 0 {
            stats.appends += 1;
            if let Err(e) = append(out, old as u64, &buf[..n]) {
                failure.get_or_insert(e);
                file = None;
            }
        } else {
            pending += n;
            if pending >= REWRITE_BYTES {
                flush(
                    &mut file,
                    &mut failure,
                    &tail,
                    &mut pending,
                    &mut since,
                    &mut stats,
                );
            }
        }
    }
    if pending > 0 {
        flush(
            &mut file,
            &mut failure,
            &tail,
            &mut pending,
            &mut since,
            &mut stats,
        );
    }
    (failure.map_or(Ok(()), Err), stats)
}

/// Writes the whole tail; a failing log is dropped (the reading goes on).
fn flush(
    file: &mut Option<File>,
    failure: &mut Option<io::Error>,
    tail: &[u8],
    pending: &mut usize,
    since: &mut Instant,
    stats: &mut PumpStats,
) {
    *pending = 0;
    *since = Instant::now();
    let Some(out) = file.as_mut() else { return };
    stats.rewrites += 1;
    if let Err(e) = rewrite(out, tail) {
        failure.get_or_insert(e);
        *file = None;
    }
}

fn append(out: &mut File, at: u64, bytes: &[u8]) -> io::Result<()> {
    out.seek(SeekFrom::Start(at))?;
    out.write_all(bytes)
}

fn rewrite(out: &mut File, tail: &[u8]) -> io::Result<()> {
    out.seek(SeekFrom::Start(0))?;
    out.write_all(tail)?;
    out.set_len(tail.len() as u64)
}

/// The command `fs-packages log-pump <log file>`: reads the standard input into the log. The log is opened without following a symbolic
/// link. Returns the exit code.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let [path] = args else {
        eprintln!("fs-packages {PUMP_ARG}: expects one argument, the log file");
        return 2;
    };
    let (result, _) = pump_with(
        open_existing(Path::new(path)),
        io::stdin().lock(),
        wait_on_stdin,
    );
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("fs-packages {PUMP_ARG}: {e}");
            1
        }
    }
}

/// Blocks until the standard input has something to read or `limit` is over.
#[cfg(unix)]
fn wait_on_stdin(limit: Duration) {
    let mut fd = libc::pollfd {
        fd: 0,
        events: libc::POLLIN,
        revents: 0,
    };
    let ms = i32::try_from(limit.as_millis()).unwrap_or(i32::MAX);
    // SAFETY: one valid pollfd, count 1; a failure or an interrupt only ends the wait early.
    unsafe { libc::poll(&raw mut fd, 1, ms) };
}

#[cfg(not(unix))]
fn wait_on_stdin(_limit: Duration) {}
