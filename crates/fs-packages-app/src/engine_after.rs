//! Pattern: none (what the window does after a good install: the register, the commands, the list of files; and the native formats an entry offers).

use fs_packages_core::Version;

use crate::engine::EngineBackend;

impl EngineBackend {
    /// After a good install: note it in the register and put the commands on the PATH, as the commands do. None of it
    /// may undo the install that already happened, and none of it is swallowed: the problems come back as sentences.
    pub(crate) fn remember(
        &self,
        name: &str,
        version: Version,
        source: &str,
        signer: &str,
        facets: &[String],
    ) -> Vec<String> {
        let path = self.root().join("installed.toml");
        let settled = fs_packages_engine::settle_install(
            self.root(),
            name,
            &version.to_string(),
            facets,
            &fs_packages_engine::default_bin_dir(),
            &fs_packages_engine::default_apps_dir(),
        );
        let mut problems = settled.problems;
        if let Err(e) = fs_packages_engine::record_install(
            self.root(),
            name,
            &version.to_string(),
            source,
            signer,
        ) {
            problems.push(e);
        }
        match fs_packages_sources::Registry::load(&path) {
            Ok(mut registry) => {
                registry.record(name, &version.to_string(), source);
                registry.set_facets(name, settled.made);
                if let Err(e) = registry.save() {
                    problems.push(format!(
                        "{name}: the list of installed programs was not saved: {e:?}"
                    ));
                }
            }
            Err(e) => problems.push(format!(
                "{name}: the list of installed programs cannot be read: {e:?}"
            )),
        }
        problems
    }
}

/// The native formats a catalog entry also offers for this machine.
pub(crate) fn native_formats(
    e: &fs_packages_catalog::CatalogEntry,
) -> Vec<fs_packages_core::Format> {
    let mut out = Vec::new();
    if !e.rpms.is_empty() {
        out.push(fs_packages_core::Format::Rpm);
    }
    if !e.debs.is_empty() {
        out.push(fs_packages_core::Format::Deb);
    }
    out
}
