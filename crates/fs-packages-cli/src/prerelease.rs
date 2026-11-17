//! The pre-release mark in the command line: `available` and `check` append the word in the language of the person.

/// The word that marks a pre-release in the language of the person (`FS_LANG`, else `LANG`): `Vorabversion` for German, else `pre-release`.
fn prerelease_word(lang: &str) -> &'static str {
    if lang.to_ascii_lowercase().starts_with("de") {
        "Vorabversion"
    } else {
        "pre-release"
    }
}

/// The marker word in the environment's language.
pub(crate) fn prerelease_label() -> &'static str {
    prerelease_word(&fs_packages_core::language::from_environment())
}

/// One line of `available`: `name version`, with `(pre-release)` after a pre-release (`<release type="development">`).
pub(crate) fn available_line(e: &fs_packages_catalog::CatalogEntry) -> String {
    marked_line(e, &fs_packages_core::language::from_environment())
}

pub(crate) fn marked_line(e: &fs_packages_catalog::CatalogEntry, lang: &str) -> String {
    let line = format!("{} {}", e.name.as_str(), e.version);
    if e.details.prerelease {
        format!("{line} ({})", prerelease_word(lang))
    } else {
        line
    }
}

#[cfg(test)]
mod tests {
    use super::marked_line;

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
    fn available_marks_only_prereleases_in_both_languages() {
        assert_eq!(
            marked_line(&entry(true), "en_US.UTF-8"),
            "a 0.0.3 (pre-release)"
        );
        assert_eq!(
            marked_line(&entry(true), "de_DE.UTF-8"),
            "a 0.0.3 (Vorabversion)"
        );
        assert_eq!(marked_line(&entry(false), "de"), "a 0.0.3");
    }
}
