//! Pattern: none (three small functions). The few file operations that exist only on Unix: a symbolic link, the executable bit
//! and the private modes. Windows: not ported yet ("Windows port open"), the functions say so or do nothing instead of guessing.

use std::fs;
use std::io;
use std::path::Path;

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
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "symbolic links are not ported to Windows yet",
        ))
    }
}

/// True when the file has an executable bit; on Windows nothing counts as executable yet (port open).
pub(crate) fn is_executable(meta: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::PermissionsExt::mode(&meta.permissions()) & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        false
    }
}

/// Closes the folder to everyone but the owner (mode 0700); no-op where there is no such mode.
///
/// # Errors
/// `io::Error` when the rights cannot be set.
pub(crate) fn make_private_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        fs::set_permissions(dir, std::os::unix::fs::PermissionsExt::from_mode(0o700))
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
        Ok(())
    }
}

/// Options of a file only the owner reads (mode 0600) and that is never opened through a link (`O_NOFOLLOW`); on Windows plain options.
pub(crate) fn private_file_options(options: &mut fs::OpenOptions) -> &mut fs::OpenOptions {
    #[cfg(unix)]
    {
        std::os::unix::fs::OpenOptionsExt::custom_flags(
            std::os::unix::fs::OpenOptionsExt::mode(options, 0o600),
            0x2_0000, // O_NOFOLLOW
        )
    }
    #[cfg(not(unix))]
    {
        options
    }
}
