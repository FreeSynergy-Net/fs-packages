//! The server as a command: what `fs-packages serve` runs.
//!
//! Pattern: Command (one subcommand of the single `fs-packages` program, ADR 0063) — reads its options,
//! serves until a signal. Returns the exit code; the binary exits with it. The server runs on fs-rpc's caller-aware path
//! (`run_until_signal_callers`): every connection is asked from the system who is on it, and the store module (`StoreCore`) answers
//! `FormSource` behind `Guarded` next to the contract `store`. No package manager is attached yet (`NoSystem`): a write of a person is
//! refused with `unsupported` until `PackageKit` (STW-3) is there.

use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use fs_rpc::{ServerIdentity, default_socket, run_until_signal_callers};

use crate::{LocalStore, NoSystem, StoreCore, StoreGuarded, StoreServer, local_person};

const USAGE: &str = "fs-packages serve [--socket PATH]\nDefault: $XDG_RUNTIME_DIR/freesynergy/store.sock\nReads answer from the files of this machine; writes are commands of the form source for a person (no package manager is attached yet: `unsupported`).";

/// The socket named by `--socket`, or the default; `Ok(None)` when help was asked for.
fn socket_of(args: impl IntoIterator<Item = OsString>) -> Result<Option<PathBuf>, String> {
    let mut args = args.into_iter();
    let mut socket = None;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--help" | "-h") => return Ok(None),
            Some("--socket") => {
                let path = args.next().filter(|p| !p.is_empty());
                socket = Some(PathBuf::from(path.ok_or("--socket needs a path")?));
            }
            _ => return Err(format!("unknown argument {}", arg.display())),
        }
    }
    socket
        .or_else(|| default_socket("store"))
        .map(Some)
        .ok_or_else(|| "no socket: set --socket, or $XDG_RUNTIME_DIR".to_owned())
}

/// Run the server with `args` (the words after `serve`); the exit code: 0 after a signal or `--help`,
/// 2 for a mistake in the arguments, 1 when serving failed.
#[must_use]
pub fn serve_command(args: impl IntoIterator<Item = OsString>) -> i32 {
    let socket = match socket_of(args) {
        Ok(Some(socket)) => socket,
        Ok(None) => {
            println!("{USAGE}");
            return 0;
        }
        Err(problem) => {
            eprintln!("fs-packages serve: {problem}");
            return 2;
        }
    };
    // The identity lets a client of another version replace this server (`connect_matching`) and lets the store
    // end it after an update (`Control.stop`).
    let identity = ServerIdentity::new("fs-packages", env!("CARGO_PKG_VERSION"));
    // The person "me" of the module person, read once and BEFORE the loop (it may start that service); without it only the form source stays off.
    let person = local_person(std::env::current_exe().ok().as_deref());
    let served = run_until_signal_callers(&socket, identity, move || async move {
        let view: Arc<dyn crate::StoreView> = Arc::new(LocalStore::open_default());
        let root = StoreServer::new(view.clone()).into_client().client;
        // `Guarded::new` runs inside the `LocalSet` of the loop.
        Ok::<_, io::Error>(StoreGuarded::new(
            root,
            StoreCore::new(view, NoSystem),
            person,
            &|line| {
                eprintln!("{line}");
            },
        ))
    });
    match served {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("fs-packages serve: {e}");
            1
        }
    }
}
