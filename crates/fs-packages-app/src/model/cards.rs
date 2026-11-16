//! Pattern: View-model (one card per program: what the card shows and which buttons it has). Pure Rust, no GUI
//! types; the window only draws a [`Card`].

use crate::backend::{JobKind, Phase};
use fs_packages_core::{Kind, Version};

use super::cards_index::Index;
use super::cards_placeholder::{Placeholder, placeholder};
use super::store::StoreModel;
use super::types::Action;

/// Where a program is, as the card tells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardState {
    /// Offered, not on this machine: the button is "Install".
    NotInstalled,
    /// On this machine and up to date: "Open" over the whole width.
    Installed,
    /// On this machine, a newer version is offered: "Open" at half the width, "Update" beside it.
    UpdateAvailable,
}

/// How wide the "Open" button is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenWidth {
    /// The whole width of the button row.
    Full,
    /// Half of it; "Update" takes the other half.
    Half,
}

/// Which buttons a card has (the three-dot menu is always there).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Buttons {
    /// "Install" (not installed).
    pub install: bool,
    /// "Open", and how wide; `None` when the program is not installed.
    pub open: Option<OpenWidth>,
    /// "Update" (next to a half-wide "Open").
    pub update: bool,
}

/// What a card is busy with right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Working {
    /// An install or update job, in this phase.
    Job(JobKind, Phase),
    /// The program is being taken away.
    Removing,
    /// "Uninstall" was chosen and waits for the person's answer.
    Asking,
}

/// One program on the page; the flags are independent facts of the view-model.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    /// Name (shown, and the key of every command).
    pub name: String,
    /// The name a person reads (`FreeSynergy.People`); never `fs-...`.
    pub title: String,
    /// Kind, when a source offers it.
    pub kind: Option<Kind>,
    /// Where it is.
    pub state: CardState,
    /// The offered version, when a source offers it.
    pub offered: Option<Version>,
    /// The installed version.
    pub installed: Option<Version>,
    /// The description of the catalog entry (may be empty).
    pub description: String,
    /// The project page (may be empty).
    pub homepage: String,
    /// Marked "New" by the last check.
    pub is_new: bool,
    /// The cursor is over this card.
    pub hovered: bool,
    /// The description panel shows this card.
    pub selected: bool,
    /// What it is busy with, or waits for.
    pub working: Option<Working>,
    /// Why its last job failed, as a sentence; stays on the card until the next try.
    pub failure: Option<String>,
    /// Its buttons.
    pub buttons: Buttons,
    /// Its stand-in logo.
    pub placeholder: Placeholder,
    /// The stored icon (a PNG file), when the catalog entry has one and the engine kept it; it replaces the stand-in.
    pub icon: Option<std::path::PathBuf>,
    /// The offered release is a pre-release (`type="development"`): shown with the word "Pre-release" / "Vorabversion".
    pub prerelease: bool,
    /// Release date as the catalog gives it; empty when it does not say.
    pub released: String,
    /// Download size in words; `unknown` when the catalog does not say.
    pub size: String,
    /// Signature level in a word; `unknown` when the catalog does not say.
    pub trust: &'static str,
    /// How far the download is (bytes).
    pub bytes: Option<fs_packages_core::Progress>,
    /// "Cancel" can stop the running job now.
    pub can_cancel: bool,
    /// Installed, but every source was read and none offers it any more (it was taken out of the catalog). Never true while a source
    /// could not be read or when nothing is offered at all, so a down source does not say "no longer offered".
    pub no_longer_offered: bool,
    /// The install of this system goes through the system's package tool, which asks for the password (an rpm or deb install).
    pub asks_password: bool,
    /// Why "Open" is locked (no window of its own); `None` when it can be pressed.
    pub open_blocked: Option<&'static str>,
    /// Why "Install" is locked: a kind this store version does not install yet ("Coming soon") or does not know ("not supported by this store
    /// version", or "needs Store 1.2.3" for an entry that wants a newer store). `None` when it can be installed.
    pub install_blocked: Option<String>,
    /// What happened right after the start (first line), then the last lines of its log; empty when nothing is to tell.
    pub launch_note: Vec<String>,
    /// A log of the program exists, so "Open log" shows.
    pub log_available: bool,
    /// The program does not respond (no sign of life): "Stop it" shows.
    pub can_stop: bool,
}

impl Card {
    /// True while "Uninstall" waits for the person's answer.
    #[must_use]
    pub fn is_confirming(&self) -> bool {
        self.working == Some(Working::Asking)
    }

    /// The version line under the name: the installed one, or the offered one; "old -> new" for an update.
    #[must_use]
    pub fn version_line(&self) -> String {
        match (self.installed, self.offered, self.state) {
            (Some(old), Some(new), CardState::UpdateAvailable) => {
                format!("{old} \u{2192} {new}")
            }
            (Some(v), _, _) | (None, Some(v), _) => v.to_string(),
            (None, None, _) => String::new(),
        }
    }
}

/// Which buttons a state has.
#[must_use]
pub fn buttons_for(state: CardState) -> Buttons {
    match state {
        CardState::NotInstalled => Buttons {
            install: true,
            open: None,
            update: false,
        },
        CardState::Installed => Buttons {
            install: false,
            open: Some(OpenWidth::Full),
            update: false,
        },
        CardState::UpdateAvailable => Buttons {
            install: false,
            open: Some(OpenWidth::Half),
            update: true,
        },
    }
}

impl StoreModel {
    /// One card per program, by name: everything a source offers plus everything installed, never a name twice.
    #[must_use]
    pub fn cards(&self) -> Vec<Card> {
        self.cards_shared().as_ref().clone()
    }

    pub(super) fn compute_cards(&self) -> Vec<Card> {
        self.ui.memo.builds.set(self.ui.memo.builds.get() + 1);
        let mut names: Vec<&str> = self
            .available
            .iter()
            .map(|r| r.package.name.as_str())
            .chain(self.installed.iter().map(|r| r.name.as_str()))
            .collect();
        names.sort_unstable();
        names.dedup();
        let index = Index::of(self);
        names
            .into_iter()
            .map(|name| self.card(name, &index))
            .collect()
    }

    /// The card of `name`, as it is now.
    fn card(&self, name: &str, index: &Index<'_>) -> Card {
        let row = index.available.get(name).copied();
        let installed = index.installed.get(name).map(|r| r.version);
        let state = match (installed, row.map(|r| r.action)) {
            (None, _) => CardState::NotInstalled,
            (Some(_), Some(Action::Update)) => CardState::UpdateAvailable,
            (Some(_), _) => CardState::Installed,
        };
        let working = if self.ui.removing.as_deref() == Some(name) {
            Some(Working::Removing)
        } else if self.ui.confirm.as_deref() == Some(name) {
            Some(Working::Asking)
        } else {
            index
                .running
                .get(name)
                .map(|j| Working::Job(j.kind, j.phase))
        };
        Card {
            name: name.to_owned(),
            title: self.title(name),
            kind: row.map(|r| r.package.kind),
            state,
            offered: row.map(|r| r.package.version),
            installed,
            description: row
                .map(|r| r.package.description.clone())
                .unwrap_or_default(),
            homepage: row.map(|r| r.package.homepage.clone()).unwrap_or_default(),
            is_new: self.is_fresh(name),
            hovered: self.ui.hovered.as_deref() == Some(name),
            selected: self.ui.selected.as_deref() == Some(name),
            working,
            failure: index
                .job
                .get(name)
                .and_then(|j| j.error.as_ref())
                .map(|e| super::text::job_failed_line(&self.title(name), e)),
            buttons: buttons_for(state),
            placeholder: placeholder(name),
            icon: self.backend.icon_path(name),
            prerelease: row.is_some_and(|r| r.package.details.prerelease),
            released: row
                .map(|r| r.package.details.released.clone())
                .unwrap_or_default(),
            size: row.and_then(|r| r.package.details.size).map_or_else(
                || crate::words_front::UNKNOWN.to_owned(),
                super::front_text::size_text,
            ),
            trust: super::front_text::trust_word(row.and_then(|r| r.package.details.trust)),
            bytes: index.running.get(name).and_then(|j| j.progress),
            can_cancel: self.can_cancel(name),
            no_longer_offered: self.offer_is_gone(name),
            asks_password: !matches!(
                self.backend.system_format(),
                fs_packages_core::Format::TarGz
            ),
            open_blocked: super::card_text::open_blocked(row.map(|r| r.package.kind)),
            install_blocked: super::card_text::install_blocked(
                row.and_then(|r| r.package.locked.as_deref()),
                row.map(|r| r.package.kind),
            ),
            launch_note: self.launch_note(name),
            log_available: self.has_log(name),
            can_stop: self.can_stop(name),
        }
    }
}
