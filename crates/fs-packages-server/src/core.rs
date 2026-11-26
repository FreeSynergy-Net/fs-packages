//! The store as a module of the contract `module_v1`: the forms `package`, `source` and `job`, the commands of K7.
//!
//! Pattern: Command (the enum `StoreCommand`, one `match` in `apply`) inside a Template Method: `StoreCore` delivers only reading and changing, the
//! order check -> change -> report stands once in `fs_rpc::Guarded`; Strategy - [`StoreRules`] are its rights. Deviation: not a Template Method
//! in the strict sense (the wrapper is generic, there is no base class).
//!
//! It checks NO right (the trait says so): that is the job of `StoreRules`, asked by the wrapper before `page` and `apply`. So it is safe ONLY
//! behind `Guarded` (never hand it out bare). Every write goes to the port [`SystemPackages`]; the catalog and signature check in front of it
//! and the job records in fs-objects come with STW-2b/4a (the jobs are held in memory until then). Level 1: no `write_by` log, so no
//! `changes`, no `revert`, no feed.

#![allow(clippy::unused_async_trait_impl, clippy::unnecessary_literal_bound)] // the trait is async and says `&str`
#![allow(clippy::result_large_err)] // `Refusal` is the one error type of the contract `ModuleCore`

use std::sync::{Arc, Mutex};

use fs_interfaces::forms::parse_key;
use fs_interfaces::module::{
    Applied, Blob, ChangeFeed, ChangePage, ChangesQuery, Context, Invocation, ModuleCore, Page,
    PageQuery, Refusal, RightsRules, StepRef,
};

use crate::command::StoreCommand;
use crate::rows::{
    JOB, PACKAGE, SOURCE, jobs, packages, refusal_of, refuse, set_commands, sources,
};
use crate::rules::StoreRules;
use crate::system::{Job, Step, SystemPackages, locked};
use crate::view::StoreView;

/// The level this module speaks (no `write_by` log yet).
pub const LEVEL: u32 = 1;

/// Pattern: Template Method (steps) - the store as a module; see the module doc.
pub struct StoreCore<S: SystemPackages> {
    view: Arc<dyn StoreView>,
    system: S,
    jobs: Mutex<Vec<Job>>,
    rules: StoreRules,
}

impl<S: SystemPackages> StoreCore<S> {
    /// The store that reads through `view` and writes through `system`.
    #[must_use]
    pub fn new(view: Arc<dyn StoreView>, system: S) -> Self {
        Self {
            view,
            system,
            jobs: Mutex::new(Vec::new()),
            rules: StoreRules,
        }
    }

    /// The manager of the system the writes go to (for a test that looks at what it was asked).
    #[must_use]
    pub fn system(&self) -> &S {
        &self.system
    }

    /// A read of the view off the RPC thread (a catalog may be cloned from Git).
    async fn read<T: Send + 'static>(
        &self,
        f: impl FnOnce(&dyn StoreView) -> Result<T, String> + Send + 'static,
    ) -> Result<T, Refusal> {
        let view = self.view.clone();
        tokio::task::spawn_blocking(move || f(view.as_ref()))
            .await
            .map_err(|_| refuse("unavailable", 503, "read cut short"))?
            .map_err(|_| refuse("unavailable", 503, "store unreadable"))
    }

    fn remember(&self, job: &Job) -> Applied {
        let mut jobs = locked(&self.jobs);
        match jobs.iter_mut().find(|j| j.id == job.id) {
            Some(known) => known.clone_from(job),
            None => jobs.push(job.clone()),
        }
        let mut done = Applied::default();
        job.id.clone_into(&mut done.record);
        "started".clone_into(&mut done.text);
        done
    }

    fn cancellable(&self, id: &str) -> Result<(), Refusal> {
        let jobs = locked(&self.jobs);
        match jobs.iter().find(|j| j.id == id) {
            None => Err(refuse("unknown-record", 404, "no such job")),
            Some(j) if !j.allow_cancel => Err(refuse("conflict", 409, "not-cancellable")),
            Some(_) => Ok(()),
        }
    }
}

/// The record a command is about: `required` or not; an empty text is none.
fn record_of(command: &Invocation, required: bool) -> Result<&str, Refusal> {
    match command.record.as_deref().filter(|r| !r.is_empty()) {
        Some(r) => Ok(r),
        None if required => Err(refuse("bad-arguments", 400, "a record is required")),
        None => Ok(""),
    }
}

/// The arguments by name; any name outside `allowed` or given twice is refused.
fn args<'a, const N: usize>(
    command: &'a Invocation,
    allowed: [&str; N],
) -> Result<[&'a str; N], Refusal> {
    let mut out = [""; N];
    let mut seen = [false; N];
    for a in &command.args {
        let at = allowed
            .iter()
            .position(|n| *n == a.path)
            .filter(|i| !seen[*i])
            .ok_or_else(|| refuse("bad-arguments", 400, "unknown or doubled argument"))?;
        (out[at], seen[at]) = (a.text.as_str(), true);
    }
    Ok(out)
}

impl<S: SystemPackages> ModuleCore for StoreCore<S> {
    fn level(&self) -> u32 {
        LEVEL
    }

    fn provider(&self) -> &str {
        "fs-packages"
    }

    fn rules(&self) -> &dyn RightsRules {
        &self.rules
    }

    async fn page(&self, _cx: &Context, query: PageQuery) -> Result<Page, Refusal> {
        let form = parse_key(&query.form)
            .filter(|(n, v)| [PACKAGE, SOURCE, JOB].contains(n) && (v.major, v.minor) == (1, 0))
            .map(|(n, _)| n)
            .ok_or_else(|| refuse("unknown-form", 404, "no such form"))?;
        if !query.scope.is_empty() {
            return Err(refuse("unknown-scope", 404, "the store has no scopes"));
        }
        let records = match form {
            PACKAGE => {
                let (offered, _skipped) = self.read(|v| v.available()).await?;
                packages(&offered, &self.read(|v| v.installed()).await?)
            }
            SOURCE => sources(&self.read(|v| v.sources()).await?),
            _ => jobs(&locked(&self.jobs)),
        };
        let mut page = Page::default();
        page.total = Some(records.len() as u64);
        page.records = records;
        page.may = set_commands(form);
        Ok(page)
    }

    async fn blob(&self, _cx: &Context, _record: &str, _field: &str) -> Result<Blob, Refusal> {
        Err(refuse(
            "unsupported",
            501,
            "the store has no binary field yet",
        ))
    }

    async fn apply(&self, _cx: &Context, command: Invocation) -> Result<Applied, Refusal> {
        use StoreCommand as C;
        let known = C::of_key(&command.command)
            .ok_or_else(|| refuse("unknown-command", 404, "no such command"))?;
        let started = match known {
            C::Install => {
                let [source] = args(&command, ["source"])?;
                self.system
                    .install(record_of(&command, true)?, source)
                    .await
            }
            C::Update => {
                let [word] = args(&command, ["step"])?;
                let step = Step::of_word(word)
                    .ok_or_else(|| refuse("bad-arguments", 400, "step is check, offer or run"))?;
                self.system.update(record_of(&command, false)?, step).await
            }
            C::Uninstall => {
                args(&command, [])?;
                self.system.uninstall(record_of(&command, true)?).await
            }
            C::AddSource => {
                let [url] = args(&command, ["url"])?;
                if command.record.as_deref().is_some_and(|r| !r.is_empty()) || url.is_empty() {
                    return Err(refuse("bad-arguments", 400, "only a url"));
                }
                self.system.add_source(url).await
            }
            C::EnableSource => {
                let [enabled] = args(&command, ["enabled"])?;
                let enabled = enabled
                    .parse()
                    .map_err(|_| refuse("bad-arguments", 400, "enabled is true or false"))?;
                self.system
                    .enable_source(record_of(&command, true)?, enabled)
                    .await
            }
            C::RemoveSource => {
                args(&command, [])?;
                self.system.remove_source(record_of(&command, true)?).await
            }
            C::Refresh => {
                args(&command, [])?;
                self.system.refresh(record_of(&command, false)?).await
            }
            C::CancelJob => {
                args(&command, [])?;
                let id = record_of(&command, true)?;
                self.cancellable(id)?;
                self.system.cancel(id).await
            }
        };
        let job = started.map_err(|fault| refusal_of(&fault))?;
        let mut done = self.remember(&job);
        if known == C::CancelJob {
            "cancelled".clone_into(&mut done.text);
        }
        Ok(done)
    }

    async fn revert(
        &self,
        _cx: &Context,
        _change: &str,
        _step: StepRef,
    ) -> Result<Applied, Refusal> {
        Err(refuse(
            "unsupported",
            501,
            "level 1: no log to take a change back from",
        ))
    }

    async fn changes(&self, _cx: &Context, _query: ChangesQuery) -> Result<ChangePage, Refusal> {
        Ok(ChangePage::default())
    }

    fn feed(&self, _sink: ChangeFeed) {
        // Level 1: no change feed (jobs are read again by `page`; the feed comes with the job records of STW-2b/4a).
    }
}
