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
        (source, signer): (&str, &str),
        facets: &[String],
        prerelease: bool,
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
                registry.record(name, &version.to_string(), source, prerelease);
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

#[cfg(test)]
mod tests {
    use fs_packages_core::Version;

    use crate::engine::EngineBackend;

    /// Every install of the window writes the pre-release mark in both directions: a stable release takes an old mark away.
    #[test]
    fn remember_sets_the_prerelease_mark_in_both_directions() {
        let tmp = tempfile::tempdir().unwrap();
        let backend = EngineBackend::new(tmp.path().to_owned());
        let read = || {
            fs_packages_sources::Registry::load(&tmp.path().join("installed.toml"))
                .unwrap()
                .get("fs-a")
                .unwrap()
                .prerelease
        };
        let v = |s: &str| Version::parse(s).unwrap();
        let first = backend.remember("fs-a", v("0.0.3"), ("s", "k"), &[], true);
        assert!(first.is_empty(), "{first:?}");
        assert!(read());
        let second = backend.remember("fs-a", v("1.0.0"), ("s", "k"), &[], false);
        assert!(second.is_empty(), "{second:?}");
        assert!(!read());
    }
}
