//! The commands of the store module: one list, one `match`.
//!
//! Pattern: Command - the keys of K7 (`<record kind>.<verb>`, never the module name) are the variants of [`StoreCommand`]; `StoreCore::apply`
//! turns a key into a variant and matches once. Deviation from the textbook: no `invert` (undo only where the system's package manager
//! carries it, STW-4a).

/// The commands a person gives the store (ENTSCHEID-store-k7-befehle, table 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreCommand {
    /// `package.install`: record = the package, argument `source` (empty = best checked offer).
    Install,
    /// `package.update`: record = the package (empty = all), argument `step` = `check` | `offer` | `run`.
    Update,
    /// `package.uninstall`: record = the package.
    Uninstall,
    /// `source.add`: argument `url`.
    AddSource,
    /// `source.enable`: record = the source, argument `enabled`.
    EnableSource,
    /// `source.remove`: record = the source.
    RemoveSource,
    /// `source.refresh`: record = the source (empty = all active ones).
    Refresh,
    /// `job.cancel`: record = the job.
    CancelJob,
}

impl StoreCommand {
    /// All of them, in the order of the table.
    pub const ALL: [Self; 8] = [
        Self::Install,
        Self::Update,
        Self::Uninstall,
        Self::AddSource,
        Self::EnableSource,
        Self::RemoveSource,
        Self::Refresh,
        Self::CancelJob,
    ];

    /// The key on the wire.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Install => "package.install",
            Self::Update => "package.update",
            Self::Uninstall => "package.uninstall",
            Self::AddSource => "source.add",
            Self::EnableSource => "source.enable",
            Self::RemoveSource => "source.remove",
            Self::Refresh => "source.refresh",
            Self::CancelJob => "job.cancel",
        }
    }

    /// The command of `key`; `None` for any other key (the caller says `unknown-command`).
    #[must_use]
    pub fn of_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.key() == key)
    }
}

#[cfg(test)]
mod tests {
    use super::StoreCommand;

    #[test]
    fn every_key_finds_its_command_and_no_other_key_does() {
        for c in StoreCommand::ALL {
            assert_eq!(StoreCommand::of_key(c.key()), Some(c));
        }
        assert_eq!(
            StoreCommand::of_key("packages.update"),
            None,
            "no module name in a key"
        );
        assert_eq!(StoreCommand::of_key(""), None);
    }
}
