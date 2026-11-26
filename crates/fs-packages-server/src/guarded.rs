//! The store served under `Guarded`: `FormSource` next to the contract `store` on ONE capability.
//!
//! Pattern: Decorator (on `PerConnection`) + Factory per connection - the root of `store.capnp` (reads) is handed out as before; next to it
//! the guarded [`StoreCore`] answers `FormSource` (`with_form_source`). Ports: the actor is `OsUserIsPerson` (the transport admits the
//! user of the service only), the person is the owner "me" of the module person (`LocalPerson`, read once). Without that person the form
//! source does not start: the contract `store` runs on and the reason is reported.

use std::path::Path;

use capnp::capability::Client;
use fs_interfaces::owner::OwnerProblem;
use fs_rpc::{
    Caller, Guarded, LocalPerson, OsUserIsPerson, PerConnection, Ports, with_form_source,
};

use crate::core::StoreCore;
use crate::system::SystemPackages;

/// The store per connection: the contract `store` and, when the owner is known, `FormSource` behind `Guarded`.
pub struct StoreGuarded<S: SystemPackages> {
    root: Client,
    form: Option<Guarded<StoreCore<S>>>,
}

impl<S: SystemPackages> StoreGuarded<S> {
    /// `root` (the capability of `store.capnp`) with `core` behind `Guarded` when `person` is known; else `report` is told why not.
    ///
    /// # Panics
    /// Outside a `LocalSet` (`Guarded::new` registers the feed of the module).
    pub fn new(
        root: Client,
        core: StoreCore<S>,
        person: Result<LocalPerson, OwnerProblem>,
        report: &dyn Fn(&str),
    ) -> Self {
        let form = match person {
            Ok(person) => Some(Guarded::new(core, Ports::new(OsUserIsPerson, person))),
            Err(why) => {
                report(&format!(
                    "fs-packages: {why}; the form source does not start, the contract store runs on"
                ));
                None
            }
        };
        Self { root, form }
    }

    /// The module behind the wrapper, if the form source runs (for a test that looks at what the manager was asked; never a way around the rules).
    #[must_use]
    pub fn core(&self) -> Option<&std::rc::Rc<StoreCore<S>>> {
        self.form.as_ref().map(Guarded::core)
    }

    /// Does this serve `FormSource`?
    #[must_use]
    pub fn serves_form(&self) -> bool {
        self.form.is_some()
    }
}

impl<S: SystemPackages> PerConnection for StoreGuarded<S> {
    fn bootstrap(&mut self, caller: &Caller) -> Client {
        match &self.form {
            Some(guarded) => with_form_source(self.root.clone(), guarded.connect(caller)),
            None => self.root.clone(),
        }
    }
}

/// The person "me" of the module person that runs next to `running` (usual socket), read ONCE.
///
/// # Errors
/// The [`OwnerProblem`] (`owner-no-me`, `owner-unreachable`, `owner-invalid`); an unknown place of `running` is `unreachable`.
pub fn local_person(running: Option<&Path>) -> Result<LocalPerson, OwnerProblem> {
    let owner = running
        .and_then(fs_person_service::PersonServiceOwner::standard)
        .ok_or_else(|| OwnerProblem::Unreachable {
            reason: "the place of the module person is unknown (no absolute path of this program)"
                .into(),
        })?;
    LocalPerson::new(&owner)
}
