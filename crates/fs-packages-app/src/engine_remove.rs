//! Pattern: Adapter (taking a program away, with the same steps as `fs-packages uninstall` of the CLI; the CLI keeps
//! its code inside its own crate, so the steps are repeated here on purpose).

use std::path::Path;

use fs_packages_engine::{ProcRunningCheck, SystemCommander};
use fs_packages_sources::Registry;

/// rpm or deb: `pkexec dnf remove` / `pkexec apt-get remove`; else the unpacked package is deleted. Then the commands are unlinked and the
/// register forgets the program. The data of the person is never removed.
pub(crate) fn uninstall(root: &Path, name: &str) -> Result<(), String> {
    if fs_packages_engine::native_installed_version(name).is_some() {
        fs_packages_engine::remove_native(name, &mut SystemCommander)
            .map_err(|e| format!("{name}: {e}"))?;
    } else {
        let running = ProcRunningCheck::new(root);
        fs_packages_engine::remove(root, name, &running).map_err(|e| format!("{name}: {e:?}"))?;
    }
    let mut registry =
        Registry::load(&root.join("installed.toml")).map_err(|e| format!("{e:?}"))?;
    let facets = registry
        .get(name)
        .map(|e| e.facets.clone())
        .unwrap_or_default();
    // The files are gone, so the register forgets the program in any case; leftovers are then said, not swallowed.
    let mut problems = fs_packages_engine::settle_remove(
        root,
        name,
        &facets,
        &fs_packages_engine::default_bin_dir(),
        &fs_packages_engine::default_apps_dir(),
    );
    if let Err(e) = fs_packages_sources::ManifestStore::new(root).forget(name) {
        problems.push(format!("{name}: the list of files was not removed: {e:?}"));
    }
    registry.forget(name);
    registry.save().map_err(|e| format!("{e:?}"))?;
    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "the program is taken away, but leftovers remain: {}",
            problems.join("; ")
        ))
    }
}
