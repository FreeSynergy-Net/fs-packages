//! Pattern: none (the words under a card: the state badge and the line with date and size).

use super::cards::{Card, CardState};
use crate::words;

impl super::store::StoreModel {
    /// `name` is installed, every source was read and none offers it. False while a source could not be read, or when nothing is offered.
    pub(super) fn offer_is_gone(&self, name: &str) -> bool {
        self.installed.iter().any(|r| r.name == name)
            && !self.available.iter().any(|r| r.package.name == name)
            && !self.offers.is_empty()
            && !self
                .problems
                .iter()
                .any(|p| matches!(p, super::types::Problem::Unreadable { .. }))
    }
}

impl Card {
    /// Text of the description panel: the description, or a sentence saying there is none.
    #[must_use]
    pub fn description_text(&self) -> String {
        if self.no_longer_offered {
            let hint = fs_packages_core::progress_words::no_longer_offered(
                fs_packages_core::progress_words::german(),
            );
            return if self.description.trim().is_empty() {
                hint.to_owned()
            } else {
                format!("{hint}. {}", self.description.trim())
            };
        }
        if self.description.trim().is_empty() {
            words::NO_DESCRIPTION.to_owned()
        } else {
            self.description.trim().to_owned()
        }
    }

    /// The state in words, for the badge on the card.
    #[must_use]
    pub fn state_word(&self) -> &'static str {
        match self.state {
            CardState::NotInstalled => "Available to install",
            CardState::Installed => "Installed",
            CardState::UpdateAvailable => "Update available",
        }
    }

    /// The state of a running job in a word, for the badge ("Installing", "Waiting", ...).
    #[must_use]
    pub fn working_word(&self) -> &'static str {
        use crate::backend::Phase;
        match self.working {
            Some(super::cards::Working::Job(_, Phase::Pending)) => "Waiting",
            Some(super::cards::Working::Job(_, Phase::Verified)) => "Verified",
            Some(super::cards::Working::Job(_, Phase::Installing)) => "Installing",
            Some(super::cards::Working::Job(_, _)) => "Working",
            Some(super::cards::Working::Removing | super::cards::Working::Asking) => "Removing",
            None => "",
        }
    }

    /// `0.1.0 installed -> 0.2.0 released`, or `Version 0.7.0`: the first line under the badge of a tile.
    #[must_use]
    pub fn version_line_long(&self) -> String {
        match (self.installed, self.offered, self.state) {
            (Some(old), Some(new), CardState::UpdateAvailable) => {
                format!("{old} installed \u{2192} {new} released")
            }
            (Some(v), _, _) | (None, Some(v), _) => format!("Version {v}"),
            (None, None, _) => String::new(),
        }
    }

    /// The line `0.7.0 - 2026-11-09 - 13 MiB`, with `unknown` where the catalog is silent.
    #[must_use]
    pub fn meta_line(&self) -> String {
        if self.no_longer_offered {
            // nothing is known about date and size any more: say what is true
            return fs_packages_core::progress_words::no_longer_offered(
                fs_packages_core::progress_words::german(),
            )
            .to_owned();
        }
        let date = if self.released.is_empty() {
            crate::words_front::UNKNOWN
        } else {
            self.released.as_str()
        };
        format!("{date} - {}", self.size)
    }

    /// How far a running job is, as a share 0..1 and the sentence under the bar. While the release file is read it is bytes (what the
    /// server said, nothing guessed); else the phase. `None` when the card is not busy with a job.
    #[must_use]
    pub fn progress(&self) -> Option<(f32, String)> {
        use crate::backend::Phase;
        let super::cards::Working::Job(_, phase) = self.working? else {
            return None;
        };
        let de = fs_packages_core::progress_words::german();
        if phase == Phase::Pending
            && let Some(bytes) = self.bytes
        {
            let line = match bytes.total {
                Some(total) => crate::words_msg::download_of(
                    de,
                    &super::front_text::size_text(bytes.done),
                    &super::front_text::size_text(total),
                    bytes.share().map(|s| (s * 100.0).round()),
                ),
                None => {
                    crate::words_msg::download_so_far(de, &super::front_text::size_text(bytes.done))
                }
            };
            return Some((bytes.share().unwrap_or(0.1), line));
        }
        let (share, line) = match phase {
            // Before the first byte: the catalog is fetched (and the size of the file is known from it), so say so.
            Phase::Pending => (
                0.1,
                fs_packages_core::progress_words::catalog_fetching(de).to_owned(),
            ),
            Phase::Verified => (0.45, crate::words_msg::verified_waiting(de).to_owned()),
            Phase::Installing => (
                0.8,
                if self.asks_password {
                    fs_packages_core::progress_words::installing_password(de).to_owned()
                } else {
                    crate::words_msg::installing(de).to_owned()
                },
            ),
            Phase::Installed => (1.0, crate::words_msg::installed(de).to_owned()),
            Phase::Failed | Phase::RolledBack => (1.0, crate::words_msg::going_back(de).to_owned()),
        };
        Some((share, line))
    }
}

/// Why "Open" is locked for a package of this kind: a component or a theme has no window of its own. `None` for everything else.
#[must_use]
pub(super) fn open_blocked(kind: Option<fs_packages_core::Kind>) -> Option<&'static str> {
    use fs_packages_core::Kind;
    matches!(kind, Some(Kind::Component | Kind::Theme))
        .then(|| crate::words_front::no_window(crate::words_front::Lang::current()))
}

/// Why "Install" is locked: the package carries a reason (`locked`), and the words say it for its kind in the language of the program. `None`
/// when it can be installed.
#[must_use]
pub(super) fn install_blocked(
    locked: Option<&str>,
    kind: Option<fs_packages_core::Kind>,
) -> Option<String> {
    use fs_packages_core::Kind;
    let reason = locked?;
    let lang = crate::words_front::Lang::current();
    if let Some(wanted) = reason.strip_prefix(fs_packages_catalog::NEEDS_STORE) {
        return Some(crate::words_lang::needs_store(lang).replace("{version}", wanted));
    }
    Some(
        match kind {
            Some(Kind::Server | Kind::IconTheme) => crate::words_lang::coming_soon(lang),
            _ => crate::words_lang::not_supported(lang),
        }
        .to_owned(),
    )
}

/// How many cards of width `item` with `gap` between them fit in a row of `room` pixels (at least one).
#[must_use]
pub fn columns(room: f32, item: f32, gap: f32) -> usize {
    let fit = ((room + gap) / (item + gap)).floor();
    if fit < 1.0 {
        return 1;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let fit = fit as usize;
    fit
}
