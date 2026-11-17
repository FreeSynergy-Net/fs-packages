//! Pattern: Strategy (the word follows the language). The pre-release mark in the command line: `available` and `check` append the word in the language of the person.

/// The word that marks a pre-release in the language of the person (`FS_LANG`, else `LANG`): `Vorabversion` for German, else `pre-release`.
fn prerelease_word(lang: &str) -> &'static str {
    if lang.to_ascii_lowercase().starts_with("de") {
        "Vorabversion"
    } else {
        "pre-release"
    }
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

/// One line of `check`: `name have -> offered`, with the pre-release word after a pre-release offer.
pub(crate) fn check_line(
    name: &str,
    have: &fs_packages_core::Version,
    offered: &fs_packages_catalog::CatalogEntry,
    lang: &str,
) -> String {
    let mark = if offered.details.prerelease {
        format!(" ({})", prerelease_word(lang))
    } else {
        String::new()
    };
    format!("{name} {have} -> {}{mark}", offered.version)
}

/// What an automatic update does with an offer: a stable installed release never moves to a pre-release by itself. `sweep` is true for
/// `update --all` and the timer; a package the person NAMED (`update X`) takes the pre-release, because they asked for it.
pub(crate) fn outcome_prerelease(
    name: &str,
    installed_prerelease: bool,
    offered_prerelease: bool,
    sweep: bool,
    lang: &str,
) -> crate::locked::Outcome {
    use crate::locked::Outcome;
    if !(sweep && offered_prerelease && !installed_prerelease) {
        return Outcome::Proceed;
    }
    let word = prerelease_word(lang);
    if lang.to_ascii_lowercase().starts_with("de") {
        Outcome::Skip(format!(
            "{name}: angeboten wird eine {word}; die automatische Aktualisierung bleibt bei der stabilen Version. Mit `fs-packages update {name}` von Hand übernehmen"
        ))
    } else {
        Outcome::Skip(format!(
            "{name}: a {word} is offered; automatic updates stay on the stable version. Take it by hand with `fs-packages update {name}`"
        ))
    }
}

/// The sweep keeps a stable installed release on its release: true (after telling the person) when the offer is a pre-release.
pub(crate) fn skips_prerelease(
    root: &std::path::Path,
    name: &str,
    offered: &fs_packages_catalog::CatalogEntry,
    sweep: bool,
) -> bool {
    use crate::locked::Outcome;
    let lang = fs_packages_core::language::from_environment();
    let installed = crate::updates::installed_prerelease(root, name);
    match outcome_prerelease(name, installed, offered.details.prerelease, sweep, &lang) {
        Outcome::Skip(note) => {
            println!("{note}");
            true
        }
        Outcome::Proceed | Outcome::Fail(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{check_line, marked_line, outcome_prerelease};
    use crate::locked::Outcome;

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

    #[test]
    fn check_line_carries_the_mark_in_both_languages_only_for_a_prerelease() {
        let have = fs_packages_core::Version::parse("0.0.2").unwrap();
        assert_eq!(
            check_line("a", &have, &entry(true), "de_DE.UTF-8"),
            "a 0.0.2 -> 0.0.3 (Vorabversion)"
        );
        assert_eq!(
            check_line("a", &have, &entry(true), "en_US.UTF-8"),
            "a 0.0.2 -> 0.0.3 (pre-release)"
        );
        assert_eq!(
            check_line("a", &have, &entry(false), "en"),
            "a 0.0.2 -> 0.0.3"
        );
    }

    #[test]
    fn the_automatic_update_never_moves_stable_to_a_prerelease() {
        // stable installed, pre-release offered, sweep: skipped with a note in each language
        assert!(matches!(
            outcome_prerelease("a", false, true, true, "en"),
            Outcome::Skip(m) if m.contains("pre-release") && m.contains("update a")
        ));
        assert!(matches!(
            outcome_prerelease("a", false, true, true, "de"),
            Outcome::Skip(m) if m.contains("Vorabversion") && m.contains("update a")
        ));
        // counter-proofs: named by the person, a pre-release already installed, a stable offer
        assert_eq!(
            outcome_prerelease("a", false, true, false, "en"),
            Outcome::Proceed
        );
        assert_eq!(
            outcome_prerelease("a", true, true, true, "en"),
            Outcome::Proceed
        );
        assert_eq!(
            outcome_prerelease("a", false, false, true, "en"),
            Outcome::Proceed
        );
        assert_eq!(
            outcome_prerelease("a", true, false, true, "en"),
            Outcome::Proceed
        );
    }

    #[test]
    fn the_sweep_skips_a_prerelease_for_a_stable_install_and_the_register_decides() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("installed.toml");
        std::fs::write(&file, "[[package]]\nname='a'\nversion='0.0.1'\nsource='s'\n[[package]]\nname='b'\nversion='0.0.1'\nsource='s'\nprerelease=true\n").unwrap();
        let dev = entry(true);
        // `a` is stable in the register: skipped; `b` is itself a pre-release: goes on; named by the person: goes on
        assert!(super::skips_prerelease(tmp.path(), "a", &dev, true));
        assert!(!super::skips_prerelease(tmp.path(), "b", &dev, true));
        assert!(!super::skips_prerelease(tmp.path(), "a", &dev, false));
        assert!(!super::skips_prerelease(
            tmp.path(),
            "a",
            &entry(false),
            true
        ));
    }
}
