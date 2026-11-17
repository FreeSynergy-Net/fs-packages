//! `fs-packages`: one binary for everything (ADR 0063). Pattern: Facade over the crates that stay libraries.
//!
//! ```text
//! fs-packages                         the window (feature `gui`, on by default)
//! fs-packages [--size WxH] [--snapshot file.png]
//! fs-packages sources | available | install <name> | update <name> | installed
//! fs-packages serve [--socket PATH]  the server: store.capnp on a Unix socket (reads only for now)
//! fs-packages log-pump <log>          (internal) keeps the log of a started program at 64 KB, reading the standard input
//! ```

use std::ffi::OsString;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match dispatch(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("fs-packages: {message}");
            ExitCode::FAILURE
        }
    }
}

/// True when this process runs as root. The store works in the person's own folders and asks for root itself (`pkexec`)
/// only for an rpm; started as root it would read and write `/root/.local/share/fs-packages`, a second, different store.
fn is_root() -> bool {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata("/proc/self").is_ok_and(|m| m.uid() == 0)
}

fn dispatch(args: &[String]) -> Result<(), String> {
    if is_root() && std::env::var_os("FS_PACKAGES_ALLOW_ROOT").is_none() {
        return Err("do not run fs-packages as root (sudo): it works in your own folders and asks for the password itself when an rpm needs it. Running as root would use /root/.local/share/fs-packages, a different store than yours. (FS_PACKAGES_ALLOW_ROOT=1 overrides.)".into());
    }
    match args.first().map(String::as_str) {
        None => window(&[]),
        Some("version" | "--version" | "-V") => {
            let mode = match fs_packages_engine::preferred_format() {
                fs_packages_core::Format::Rpm => "installed by rpm: programs come as rpm",
                fs_packages_core::Format::Deb => "installed by deb: programs come as deb",
                fs_packages_core::Format::Exe => {
                    "Windows: programs come as setup.exe (Unsupported: the store cannot install it yet)"
                }
                _ => "not a system package: programs come as tar.gz",
            };
            println!("fs-packages {} ({mode})", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("gui") => window(&args[1..]),
        Some("log-pump") => log_pump(&args[1..]),
        Some(flag) if flag.starts_with("--") && flag != "--trust-key" => window(args),
        Some("serve") => {
            match fs_packages_server::serve_command(args[1..].iter().map(OsString::from)) {
                0 => Ok(()),
                code => Err(format!("serve ended with code {code}")),
            }
        }
        Some(_) => fs_packages_cli::run(args),
    }
}

#[cfg(feature = "gui")]
fn window(args: &[String]) -> Result<(), String> {
    // The first start puts the store into the person's menu (never over a file that is not ours); not for a picture or
    // for `--demo` (a test of the window must not change the person's folders).
    if !args.iter().any(|a| a == "--snapshot" || a == "--demo") {
        let _ = fs_packages_engine::ensure_own_menu_entry(&fs_packages_engine::default_apps_dir());
    }
    fs_packages_app::gui::run(args)
}

/// The log pump of a program the store started (internal; see `fs_packages_app::launch_pump`): it ends with the pipe.
#[cfg(feature = "gui")]
fn log_pump(args: &[String]) -> Result<(), String> {
    match fs_packages_app::launch_pump::run(args) {
        0 => Ok(()),
        code => Err(format!("log-pump ended with code {code}")),
    }
}

#[cfg(not(feature = "gui"))]
fn log_pump(_args: &[String]) -> Result<(), String> {
    Err(
        "this build has no window (built with --no-default-features), so there is no log pump"
            .into(),
    )
}

#[cfg(not(feature = "gui"))]
fn window(_args: &[String]) -> Result<(), String> {
    Err("this build has no window (built with --no-default-features); use a subcommand, e.g. `fs-packages installed`".into())
}
