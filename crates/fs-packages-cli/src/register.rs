//! Pattern: Repository helper (what a good install writes into `installed.toml`: version, source, facets, and whether the installed release is a
//! pre-release). The mark is written at EVERY install, in both directions, so no old `prerelease = true` stays after a stable release.

use std::path::Path;

use fs_packages_catalog::CatalogEntry;

/// Notes the install of `entry` in the register.
pub(crate) fn register_install(
    root: &Path,
    name: &str,
    version: &str,
    source: &str,
    entry: &CatalogEntry,
    facets: Vec<String>,
) -> Result<(), String> {
    let mut registry = fs_packages_sources::Registry::load(&root.join("installed.toml"))
        .map_err(|e| format!("{name}: the list of installed programs cannot be read: {e:?}"))?;
    registry.record(name, version, source, entry.details.prerelease);
    registry.set_facets(name, facets);
    registry
        .save()
        .map_err(|e| format!("{name}: the list of installed programs was not saved: {e:?}"))
}

#[cfg(test)]
mod tests {
    use super::register_install;

    fn entry(dev: bool) -> fs_packages_catalog::CatalogEntry {
        let t = if dev { r#"type="development""# } else { "" };
        let xml = format!(
            r#"<components version="1.0"><component><id>a</id><releases><release version="0.0.3" {t}><artifacts><artifact type="binary"><location>l</location><checksum type="sha256">3</checksum></artifact></artifacts></release></releases><custom><value key="fs:signature">s</value></custom></component></components>"#
        );
        fs_packages_catalog::parse_catalog(&xml)
            .unwrap()
            .entries
            .remove(0)
    }

    #[test]
    fn the_mark_follows_every_install_in_both_directions() {
        let tmp = tempfile::tempdir().unwrap();
        let read = || {
            fs_packages_sources::Registry::load(&tmp.path().join("installed.toml"))
                .unwrap()
                .get("a")
                .unwrap()
                .prerelease
        };
        register_install(tmp.path(), "a", "0.0.3", "s", &entry(true), Vec::new()).unwrap();
        assert!(read());
        // the next install is a stable release: the old mark must go
        register_install(tmp.path(), "a", "1.0.0", "s", &entry(false), Vec::new()).unwrap();
        assert!(!read());
        register_install(tmp.path(), "a", "1.1.0-rc.1", "s", &entry(true), Vec::new()).unwrap();
        assert!(read());
    }
}
