//! The log pump keeps exactly the last 64 KB of what it reads, at every moment (the file is never longer than the limit while the pump
//! reads), and never stops reading: a failing log must not block or kill the program that writes into the pipe.
//!
//! Pattern: Unit test of the pump against a reader that looks at the file between two reads; files in the test target (never /tmp).

#![cfg(unix)]

use std::cell::RefCell;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use fs_packages_app::launch_log::{LOG_MAX, open_existing};
use fs_packages_app::launch_pump::pump;

fn file(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("launch-pump");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    fs::write(&path, "").unwrap();
    path
}

/// Hands out `pieces` one by one and notes the size of the log before each read.
struct Feed {
    pieces: Vec<Vec<u8>>,
    log: PathBuf,
    sizes: Rc<RefCell<Vec<u64>>>,
}

impl Read for Feed {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.sizes
            .borrow_mut()
            .push(fs::metadata(&self.log).unwrap().len());
        if self.pieces.is_empty() {
            return Ok(0);
        }
        let piece = self.pieces.remove(0);
        buf[..piece.len()].copy_from_slice(&piece);
        Ok(piece.len())
    }
}

fn bytes(from: usize, len: usize) -> Vec<u8> {
    (from..from + len)
        .map(|i| b'a' + u8::try_from(i % 26).unwrap())
        .collect()
}

#[test]
fn the_log_holds_the_last_64_kb_at_every_moment() {
    let log = file("long.log");
    let mut all = Vec::new();
    let mut pieces = Vec::new();
    for round in 0..40usize {
        let piece = bytes(all.len(), 3000 + round * 111);
        all.extend_from_slice(&piece);
        pieces.push(piece);
    }
    assert!(all.len() > 3 * LOG_MAX, "much more than the limit");
    let sizes = Rc::new(RefCell::new(Vec::new()));
    let input = Feed {
        pieces,
        log: log.clone(),
        sizes: Rc::clone(&sizes),
    };
    pump(open_existing(&log), input).unwrap();
    assert_eq!(fs::read(&log).unwrap(), all[all.len() - LOG_MAX..]);
    let sizes = sizes.borrow();
    assert!(sizes.iter().all(|s| *s <= LOG_MAX as u64), "{sizes:?}");
    assert!(sizes.contains(&(LOG_MAX as u64)), "it did fill up");
}

#[test]
fn a_short_output_stays_whole() {
    let log = file("short.log");
    let input = std::io::Cursor::new(b"line one\nline two\n".to_vec());
    pump(open_existing(&log), input).unwrap();
    assert_eq!(fs::read_to_string(&log).unwrap(), "line one\nline two\n");
}

/// Everything that can fail on the log side leaves the reading intact.
struct Counting {
    left: usize,
    read: usize,
}

impl Read for Counting {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.left == 0 {
            return Ok(0);
        }
        let n = self.left.min(buf.len()).min(1000);
        buf[..n].fill(b'x');
        self.left -= n;
        self.read += n;
        Ok(n)
    }
}

#[test]
fn a_log_that_cannot_be_opened_does_not_stop_the_reading() {
    let mut input = Counting {
        left: 250_000,
        read: 0,
    };
    let missing = Path::new(env!("CARGO_TARGET_TMPDIR")).join("launch-pump-none/x.log");
    let result = pump(open_existing(&missing), &mut input);
    assert!(result.is_err(), "told");
    assert_eq!(input.left, 0, "the input was read to the end");
}

#[test]
fn a_log_that_stops_taking_writes_does_not_stop_the_reading() {
    // /dev/full takes the open and refuses every write (disk full).
    let mut input = Counting {
        left: 250_000,
        read: 0,
    };
    let full = std::fs::OpenOptions::new().write(true).open("/dev/full");
    let Ok(full) = full else { return };
    let result = pump(Ok(full), &mut input);
    assert!(result.is_err(), "told");
    assert_eq!(input.left, 0, "the input was read to the end");
}

#[test]
fn a_symbolic_link_is_not_followed_by_the_pump() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("launch-pump");
    fs::create_dir_all(&dir).unwrap();
    let foreign = dir.join("foreign-for-pump.txt");
    fs::write(&foreign, "keep\n").unwrap();
    let link = dir.join("link-for-pump.log");
    let _ = fs::remove_file(&link);
    std::os::unix::fs::symlink(&foreign, &link).unwrap();
    let input = std::io::Cursor::new(b"overwrite?\n".to_vec());
    assert!(pump(open_existing(&link), input).is_err());
    assert_eq!(fs::read_to_string(&foreign).unwrap(), "keep\n");
}

/// One line per read, as a program that prints line by line.
struct Lines {
    left: usize,
    line: Vec<u8>,
}

impl Read for Lines {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.left == 0 {
            return Ok(0);
        }
        self.left -= 1;
        buf[..self.line.len()].copy_from_slice(&self.line);
        Ok(self.line.len())
    }
}

#[test]
fn line_by_line_output_is_rewritten_in_bundles_not_per_line() {
    use fs_packages_app::launch_pump::{REWRITE_BYTES, pump_with};
    let log = file("lines.log");
    let line = b"0123456789 a log line of forty bytes....\n".to_vec();
    let input = Lines {
        left: 50_000,
        line: line.clone(),
    };
    let total = 50_000 * line.len();
    let start = std::time::Instant::now();
    let (result, stats) = pump_with(open_existing(&log), input, |_| {});
    let took = start.elapsed();
    result.unwrap();
    // the content is still exactly the end of the output
    let kept = fs::read(&log).unwrap();
    assert_eq!(kept.len(), LOG_MAX);
    assert!(kept.ends_with(&line));
    // before: ~50 000 rewrites of 64 KB (3 GB); now at most one per 16 KB of new bytes (+ time + the end)
    let most = total / REWRITE_BYTES + 3 + usize::try_from(took.as_millis() / 100).unwrap();
    assert!(
        stats.rewrites <= most,
        "{} rewrites for {total} bytes (at most {most})",
        stats.rewrites
    );
    assert!(stats.rewrites >= 1);
}

#[test]
fn a_burst_followed_by_silence_is_written_after_the_waiting_time() {
    use fs_packages_app::launch_pump::{REWRITE_EVERY, pump_with};
    // 60 000 bytes fill nearly all; 10 000 more overflow (less than the byte limit: a rewrite waits); then comes one more read. The wait
    // callback is asked before that read, and the rewrite is done by then: the log has its full size before the read, not only at the end.
    let log = file("quiet.log");
    let sizes = Rc::new(RefCell::new(Vec::new()));
    let input = Feed {
        pieces: vec![
            bytes(0, 40_000),
            bytes(40_000, 20_000),
            bytes(60_000, 10_000),
        ],
        log: log.clone(),
        sizes: Rc::clone(&sizes),
    };
    let mut waited = Vec::new();
    let (result, _) = pump_with(open_existing(&log), input, |left| {
        waited.push(left);
        std::thread::sleep(left);
    });
    result.unwrap();
    assert!(!waited.is_empty(), "asked while a rewrite was waiting");
    assert!(waited.iter().all(|w| *w <= REWRITE_EVERY));
    assert_eq!(
        sizes.borrow()[3],
        LOG_MAX as u64,
        "written after the waiting time, before the next output: {:?}",
        sizes.borrow()
    );
}
