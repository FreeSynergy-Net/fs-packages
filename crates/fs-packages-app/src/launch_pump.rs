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

use crate::launch_log::{LOG_MAX, open_existing};

/// The first argument of the pump: `fs-packages log-pump <log file>`.
pub const PUMP_ARG: &str = "log-pump";
/// The most that is read at once (a pipe holds 64 KB).
const CHUNK: usize = 64 * 1024;

/// Reads `input` until it ends and keeps the last [`LOG_MAX`] bytes in `log`, which must be empty at the start. When `log` is an error
/// (it could not be opened) or a write fails, the input is still read to the end and thrown away.
///
/// # Errors
/// The first error of the log or of the input; the input was read to the end anyway.
pub fn pump(log: io::Result<File>, mut input: impl Read) -> io::Result<()> {
    let (mut file, mut failure) = match log {
        Ok(file) => (Some(file), None),
        Err(e) => (None, Some(e)),
    };
    // Invariant: while `file` is there, its content is exactly `tail`.
    let mut tail: Vec<u8> = Vec::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
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
        let written = if overflow == 0 {
            append(out, old as u64, &buf[..n])
        } else {
            rewrite(out, &tail)
        };
        if let Err(e) = written {
            failure.get_or_insert(e);
            file = None;
        }
    }
    failure.map_or(Ok(()), Err)
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
    match pump(open_existing(Path::new(path)), io::stdin().lock()) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("fs-packages {PUMP_ARG}: {e}");
            1
        }
    }
}
