//! Pattern: none (the three honest progress sentences of an install, in English and German; the window and the command line say the same).
//! The language is the person's (`FS_LANG`, else `LANG`), like the rest of the words.

/// The language of the person's environment is German.
#[must_use]
pub fn german() -> bool {
    crate::language::from_environment()
        .to_ascii_lowercase()
        .starts_with("de")
}

/// While the catalog of a source is fetched.
#[must_use]
pub fn catalog_fetching(de: bool) -> &'static str {
    if de {
        "Katalog wird geholt\u{2026}"
    } else {
        "Fetching the catalog\u{2026}"
    }
}

/// While the package file is downloaded; `size` is the size the catalog states (for example `14 MiB`).
#[must_use]
pub fn downloading(de: bool, size: &str) -> String {
    if de {
        format!("Lade {size}\u{2026}")
    } else {
        format!("Downloading {size}\u{2026}")
    }
}

/// While the install command runs: for a package of the system the system asks for the password, and that must not look like a hang.
#[must_use]
pub fn installing_password(de: bool) -> &'static str {
    if de {
        "Installiere (Passwort-Abfrage m\u{f6}glich)\u{2026}"
    } else {
        "Installing (a password prompt may appear)\u{2026}"
    }
}

/// 13631488 -> "13 MiB"; below 10 MiB one decimal ("4.2 MiB"); below 1 MiB in KiB.
#[must_use]
pub fn size_text(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * 1024;
    if bytes >= 10 * MIB {
        format!("{} MiB", bytes / MIB)
    } else if bytes >= MIB {
        format!("{}.{} MiB", bytes / MIB, (bytes % MIB) * 10 / MIB)
    } else {
        format!("{} KiB", bytes.div_ceil(KIB).max(1))
    }
}

/// Installed, but no source offers it any more (it was taken out of the catalog): the hint on its tile and in the sweep of `update --all`.
#[must_use]
pub fn no_longer_offered(de: bool) -> &'static str {
    if de {
        "Wird nicht mehr angeboten"
    } else {
        "No longer offered"
    }
}
