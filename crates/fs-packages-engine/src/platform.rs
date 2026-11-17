//! Pattern: none (three small functions). The few file operations that exist only on Unix: a symbolic link, the executable bit
//! and the private modes. Windows: not ported yet ("Windows port open"): every function says so openly with the error
//! `unsupported-on-windows{feature}` (a code and a parameter, no sentence) and does NOT act, so nothing is ever written unprotected.

use std::fs;
use std::io;
use std::path::Path;

/// The open error of a function that is not ported to Windows: kind `Unsupported`, text `unsupported-on-windows{<feature>}`.
#[cfg(not(unix))]
fn unsupported(feature: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        format!("unsupported-on-windows{{{feature}}}"),
    )
}

/// Makes a symbolic link `link` that points to `target`.
///
/// # Errors
/// `io::Error` from the system; on Windows always `Unsupported` (port open).
pub(crate) fn symlink(target: &Path, link: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(not(unix))]
    {
        let _ = (target, link);
        Err(unsupported("symlink"))
    }
}

/// True when the file has an executable bit.
///
/// # Errors
/// On Windows always `unsupported-on-windows{executable-bit}` (port open): "unknown" is told, not "no".
#[allow(clippy::unnecessary_wraps)] // the error exists on Windows only
pub(crate) fn is_executable(meta: &fs::Metadata) -> io::Result<bool> {
    #[cfg(unix)]
    {
        Ok(std::os::unix::fs::PermissionsExt::mode(&meta.permissions()) & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        Err(unsupported("executable-bit"))
    }
}

/// Closes the folder to everyone but the owner (mode 0700).
///
/// # Errors
/// `io::Error` when the rights cannot be set; on Windows always `unsupported-on-windows{private-dir}`: the caller must not
/// write into the folder then (it would stay unprotected).
pub(crate) fn make_private_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        fs::set_permissions(dir, std::os::unix::fs::PermissionsExt::from_mode(0o700))
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
        Err(unsupported("private-dir"))
    }
}

/// Options of a file only the owner reads (mode 0600) and that is never opened through a link (`O_NOFOLLOW`).
///
/// # Errors
/// On Windows always `unsupported-on-windows{private-file}`: the caller must not write the file then (it would stay unprotected).
#[allow(clippy::unnecessary_wraps)] // the error exists on Windows only
pub(crate) fn private_file_options(
    options: &mut fs::OpenOptions,
) -> io::Result<&mut fs::OpenOptions> {
    #[cfg(unix)]
    {
        Ok(std::os::unix::fs::OpenOptionsExt::custom_flags(
            std::os::unix::fs::OpenOptionsExt::mode(options, 0o600),
            0x2_0000, // O_NOFOLLOW
        ))
    }
    #[cfg(not(unix))]
    {
        let _ = options;
        Err(unsupported("private-file"))
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    fn open(e: &io::Error, feature: &str) -> bool {
        e.kind() == io::ErrorKind::Unsupported
            && e.to_string() == format!("unsupported-on-windows{{{feature}}}")
    }

    #[test]
    fn what_is_not_ported_says_so_openly() {
        let dir = std::env::temp_dir();
        assert!(open(&symlink(&dir, &dir.join("x")).unwrap_err(), "symlink"));
        assert!(open(&make_private_dir(&dir).unwrap_err(), "private-dir"));
        let meta = fs::metadata(&dir).unwrap();
        assert!(open(&is_executable(&meta).unwrap_err(), "executable-bit"));
        let mut options = fs::OpenOptions::new();
        assert!(open(
            &private_file_options(&mut options).err().unwrap(),
            "private-file"
        ));
    }

    #[test]
    fn a_private_file_is_not_written_unprotected() {
        let dir = std::env::temp_dir().join(format!("fs-pkg-priv-{}", std::process::id()));
        let (paths, result) = crate::native::write_private(&dir, "p", "rpm", &[vec![1]]);
        assert!(result.unwrap_err().contains("unsupported-on-windows"));
        assert!(paths.iter().all(|p| !p.exists()));
        let _ = fs::remove_dir_all(&dir);
    }
}
