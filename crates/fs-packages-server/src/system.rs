//! The package manager of the system, as the store sees it: the port behind every write.
//!
//! The words are `PackageKit`'s (standard first): a job has a `role`, a `status`, a `percentage` and `allow_cancel`. This file holds the port
//! and the test double; the real implementations (`PackageKit` on Linux, the Windows Installer) come with STW-3 and STW-5b.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// The guard of `m`; a lock that another thread poisoned is taken anyway (the data is plain values).
pub(crate) fn locked<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Pattern: Strategy - one implementation per package manager of the system; each implementation is an Adapter of a foreign API.
///
/// Every method starts a job and tells how it stands; what a manager cannot do it refuses with a [`SystemFault`] code (never by branching on
/// the operating system in the server). The store asks it only AFTER the rights were checked (`StoreRules` under `Guarded`).
#[allow(async_fn_in_trait)] // not `dyn`: the module `StoreCore` is generic over it, as `ModuleCore` is over its own parts
pub trait SystemPackages: Send + Sync + 'static {
    /// Install `package`; `source` empty = the best checked offer.
    ///
    /// # Errors
    /// [`SystemFault`] when the manager refuses or is not there.
    async fn install(&self, package: &str, source: &str) -> Result<Job, SystemFault>;
    /// Update `package` (empty = all of ours) as far as `step` says.
    ///
    /// # Errors
    /// As [`install`](Self::install).
    async fn update(&self, package: &str, step: Step) -> Result<Job, SystemFault>;
    /// Remove `package`.
    ///
    /// # Errors
    /// As [`install`](Self::install).
    async fn uninstall(&self, package: &str) -> Result<Job, SystemFault>;
    /// Take over a new catalog source at `url`.
    ///
    /// # Errors
    /// As [`install`](Self::install).
    async fn add_source(&self, url: &str) -> Result<Job, SystemFault>;
    /// Switch the source `id` on or off.
    ///
    /// # Errors
    /// As [`install`](Self::install).
    async fn enable_source(&self, id: &str, enabled: bool) -> Result<Job, SystemFault>;
    /// Drop the source `id`.
    ///
    /// # Errors
    /// As [`install`](Self::install).
    async fn remove_source(&self, id: &str) -> Result<Job, SystemFault>;
    /// Read the source `id` again (empty = all active ones).
    ///
    /// # Errors
    /// As [`install`](Self::install).
    async fn refresh(&self, id: &str) -> Result<Job, SystemFault>;
    /// Stop the job `id`; only while its `allow_cancel` is true.
    ///
    /// # Errors
    /// As [`install`](Self::install).
    async fn cancel(&self, id: &str) -> Result<Job, SystemFault>;
}

/// How far `package.update` goes: look, offer, or do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Only look whether an update exists.
    Check,
    /// Offer it to the person.
    Offer,
    /// Run it.
    Run,
}

impl Step {
    /// The word of the argument `step`.
    #[must_use]
    pub fn of_word(word: &str) -> Option<Self> {
        match word {
            "check" => Some(Self::Check),
            "offer" => Some(Self::Offer),
            "run" => Some(Self::Run),
            _ => None,
        }
    }
}

/// Why a job exists (`PackageKit` `role`, as its nick).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// `install-packages`.
    InstallPackages,
    /// `update-packages`.
    UpdatePackages,
    /// `remove-packages`.
    RemovePackages,
    /// `refresh-cache`.
    RefreshCache,
    /// `repo-set-data`: a source added, switched or dropped.
    RepoSetData,
}

impl Role {
    /// The nick of the role.
    #[must_use]
    pub const fn nick(self) -> &'static str {
        match self {
            Self::InstallPackages => "install-packages",
            Self::UpdatePackages => "update-packages",
            Self::RemovePackages => "remove-packages",
            Self::RefreshCache => "refresh-cache",
            Self::RepoSetData => "repo-set-data",
        }
    }
}

/// Where a job stands (`PackageKit` `status`, as its nick).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// `running`.
    Running,
    /// `finished`.
    Finished,
    /// `cancel`: stopped by the person.
    Cancelled,
}

impl Status {
    /// The nick of the status.
    #[must_use]
    pub const fn nick(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Finished => "finished",
            Self::Cancelled => "cancel",
        }
    }
}

/// One job of the manager, with the fields `PackageKit` gives a transaction. Pattern: Value Object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Job {
    /// The id of the job.
    pub id: String,
    /// Why it runs.
    pub role: Role,
    /// Where it stands.
    pub status: Status,
    /// 0 to 100.
    pub percentage: u8,
    /// May it still be stopped?
    pub allow_cancel: bool,
}

/// A refusal of the manager: a code, never a foreign sentence. Pattern: Result Object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemFault {
    /// The code (`unsupported`, `not-offered`, `unknown-job` ...).
    pub code: String,
}

impl SystemFault {
    /// A fault with `code`.
    #[must_use]
    pub fn new(code: &str) -> Self {
        Self { code: code.into() }
    }
}

/// Pattern: Null Object - the manager of a system where none is attached (yet): every call is refused with the code `unsupported`. The
/// server runs with it until `PackageKit` (STW-3) or the Windows Installer (STW-5b) is there, so a write is never faked.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoSystem;

impl NoSystem {
    fn refused() -> Result<Job, SystemFault> {
        Err(SystemFault::new("unsupported"))
    }
}

impl SystemPackages for NoSystem {
    async fn install(&self, _: &str, _: &str) -> Result<Job, SystemFault> {
        Self::refused()
    }
    async fn update(&self, _: &str, _: Step) -> Result<Job, SystemFault> {
        Self::refused()
    }
    async fn uninstall(&self, _: &str) -> Result<Job, SystemFault> {
        Self::refused()
    }
    async fn add_source(&self, _: &str) -> Result<Job, SystemFault> {
        Self::refused()
    }
    async fn enable_source(&self, _: &str, _: bool) -> Result<Job, SystemFault> {
        Self::refused()
    }
    async fn remove_source(&self, _: &str) -> Result<Job, SystemFault> {
        Self::refused()
    }
    async fn refresh(&self, _: &str) -> Result<Job, SystemFault> {
        Self::refused()
    }
    async fn cancel(&self, _: &str) -> Result<Job, SystemFault> {
        Self::refused()
    }
}

/// What a double was asked, in order: `(method, argument)`.
pub type Calls = Vec<(&'static str, String)>;

/// Pattern: Test Double (a recording Fake) - answers every call with a running job that may be cancelled, and remembers what it was asked.
/// [`failing`](Self::failing) makes it refuse everything with one code.
#[derive(Default)]
pub struct FakeSystem {
    calls: Mutex<Calls>,
    fault: Mutex<Option<String>>,
}

impl FakeSystem {
    /// A double that accepts everything.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A double that refuses everything with `code`.
    #[must_use]
    pub fn failing(code: &str) -> Self {
        let this = Self::default();
        *locked(&this.fault) = Some(code.to_owned());
        this
    }

    /// What it was asked so far.
    #[must_use]
    pub fn calls(&self) -> Calls {
        locked(&self.calls).clone()
    }

    fn take(&self, method: &'static str, arg: &str, role: Role) -> Result<Job, SystemFault> {
        let mut calls = locked(&self.calls);
        calls.push((method, arg.to_owned()));
        if let Some(code) = &*locked(&self.fault) {
            return Err(SystemFault::new(code));
        }
        Ok(Job {
            id: format!("fake-{}", calls.len()),
            role,
            status: Status::Running,
            percentage: 0,
            allow_cancel: true,
        })
    }
}

impl SystemPackages for FakeSystem {
    async fn install(&self, package: &str, _source: &str) -> Result<Job, SystemFault> {
        self.take("install", package, Role::InstallPackages)
    }
    async fn update(&self, package: &str, _step: Step) -> Result<Job, SystemFault> {
        self.take("update", package, Role::UpdatePackages)
    }
    async fn uninstall(&self, package: &str) -> Result<Job, SystemFault> {
        self.take("uninstall", package, Role::RemovePackages)
    }
    async fn add_source(&self, url: &str) -> Result<Job, SystemFault> {
        self.take("add_source", url, Role::RepoSetData)
    }
    async fn enable_source(&self, id: &str, _enabled: bool) -> Result<Job, SystemFault> {
        self.take("enable_source", id, Role::RepoSetData)
    }
    async fn remove_source(&self, id: &str) -> Result<Job, SystemFault> {
        self.take("remove_source", id, Role::RepoSetData)
    }
    async fn refresh(&self, id: &str) -> Result<Job, SystemFault> {
        self.take("refresh", id, Role::RefreshCache)
    }
    async fn cancel(&self, id: &str) -> Result<Job, SystemFault> {
        let mut job = self.take("cancel", id, Role::RefreshCache)?;
        id.clone_into(&mut job.id);
        job.status = Status::Cancelled;
        job.allow_cancel = false;
        Ok(job)
    }
}
