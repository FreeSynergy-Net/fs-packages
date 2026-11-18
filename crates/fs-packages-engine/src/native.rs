//! Pattern: none - two plain helper functions that the Strategies `Rpm` and `Deb` share (write the verified bytes into a
//! private directory; the command that hands them to the package manager stays in each Strategy).
//! The person confirms root themselves: `pkexec` asks for it, we never see a password.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Writes `files` below `dir` (created with mode 0700) as `<name>-<version>-<pid>-<n>.<ext>`: names nobody can guess,
/// created exclusively with mode 0600 and never through a link, so there is nothing to swap after the check.
/// Returns every path that was tried (also after a failure, so the caller can delete them) and the outcome.
pub(crate) fn write_private(
    dir: &Path,
    stem: &str,
    ext: &str,
    files: &[Vec<u8>],
) -> (Vec<PathBuf>, Result<(), String>) {
    let mut paths = Vec::new();
    // Absolute, so the package manager reads a path (a name with a `/` is a file to apt) and no relative root confuses it.
    let dir = &std::path::absolute(dir).unwrap_or_else(|_| dir.to_owned());
    let prepared = fs::create_dir_all(dir).and_then(|()| crate::platform::make_private_dir(dir));
    if let Err(e) = prepared {
        return (paths, Err(e.to_string()));
    }
    for (n, bytes) in files.iter().enumerate() {
        let file = dir.join(format!("{stem}-{}-{n}.{ext}", std::process::id()));
        let _ = fs::remove_file(&file);
        let one = crate::platform::private_file_options(
            fs::OpenOptions::new().write(true).create_new(true),
        )
        .and_then(|options| options.open(&file))
        .and_then(|mut f| f.write_all(bytes));
        paths.push(file);
        if let Err(e) = one {
            return (paths, Err(e.to_string()));
        }
    }
    (paths, Ok(()))
}

/// The paths as the text a command line needs.
pub(crate) fn names(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|f| f.to_string_lossy().into_owned())
        .collect()
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[test]
    fn the_folder_is_0700_and_the_files_are_0600() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("stage");
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
        let (paths, result) = write_private(&dir, "p", "rpm", &[vec![1], vec![2]]);
        assert_eq!(result, Ok(()));
        assert_eq!(paths.len(), 2);
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&dir), 0o700);
        assert!(paths.iter().all(|p| mode(p) == 0o600));
    }

    #[test]
    fn a_stale_link_at_the_path_is_replaced_not_written_through() {
        let tmp = tempfile::tempdir().unwrap();
        let (paths, _) = write_private(tmp.path(), "p", "rpm", &[vec![1]]);
        let foreign = tmp.path().join("foreign");
        fs::write(&foreign, b"keep").unwrap();
        let _ = fs::remove_file(&paths[0]);
        std::os::unix::fs::symlink(&foreign, &paths[0]).unwrap();
        // The next write removes the link first and creates a new file: the foreign file stays as it is. (This is the
        // `remove_file` before `create_new`; that `O_NOFOLLOW` itself holds is tested in `platform::tests_unix`.)
        let (_, result) = write_private(tmp.path(), "p", "rpm", &[vec![9]]);
        assert_eq!(result, Ok(()));
        assert_eq!(fs::read(&foreign).unwrap(), b"keep");
    }
}
