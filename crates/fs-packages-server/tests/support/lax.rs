//! The wrong rules of the counter-proof: `AllowAll`, and a decorator that puts them on the real module. Pattern: Decorator (test).
#![allow(
    clippy::unused_async_trait_impl,
    clippy::result_large_err,
    clippy::unnecessary_literal_bound
)]

use std::sync::Arc;

use capnp::capability::FromClientHook;
use fs_interfaces::formsource_capnp::form_source;
use fs_interfaces::module::{
    Applied, Ask, Blob, ChangeFeed, ChangePage, ChangesQuery, Context, FeedBreak, Follow,
    HiddenReason, Invocation, ModuleCore, Page, PageQuery, Refusal, RightsRules, StepRef, Verdict,
};
use fs_packages_server::{StoreCore, SystemPackages};
use fs_rpc::{Caller, Guarded, OsUserIsPerson, PerConnection, Ports};

use super::{Memory, local_person};

/// The rules say yes to everything.
pub struct AllowAll;

impl RightsRules for AllowAll {
    fn may(&self, _: &Ask<'_>) -> Verdict {
        Verdict::Allow
    }
    fn hidden(&self, _: &Context, _: &str, _: &str) -> Option<HiddenReason> {
        None
    }
}

/// The module `M` with the rules of `AllowAll`; everything else is `M`.
pub struct Lax<M: ModuleCore> {
    inner: M,
    rules: AllowAll,
}

impl<M: ModuleCore> Lax<M> {
    pub fn inner(&self) -> &M {
        &self.inner
    }
}

impl<M: ModuleCore> ModuleCore for Lax<M> {
    fn level(&self) -> u32 {
        self.inner.level()
    }
    fn provider(&self) -> &str {
        self.inner.provider()
    }
    fn rules(&self) -> &dyn RightsRules {
        &self.rules
    }
    async fn page(&self, cx: &Context, query: PageQuery) -> Result<Page, Refusal> {
        self.inner.page(cx, query).await
    }
    async fn blob(&self, cx: &Context, record: &str, field: &str) -> Result<Blob, Refusal> {
        self.inner.blob(cx, record, field).await
    }
    async fn apply(&self, cx: &Context, command: Invocation) -> Result<Applied, Refusal> {
        self.inner.apply(cx, command).await
    }
    async fn revert(&self, cx: &Context, change: &str, step: StepRef) -> Result<Applied, Refusal> {
        self.inner.revert(cx, change, step).await
    }
    async fn changes(&self, cx: &Context, query: ChangesQuery) -> Result<ChangePage, Refusal> {
        self.inner.changes(cx, query).await
    }
    fn feed(&self, sink: ChangeFeed) {
        self.inner.feed(sink);
    }
    fn on_feed_break(&self, sink: FeedBreak) {
        self.inner.on_feed_break(sink);
    }
    fn follows(&self, cx: &Context, form: &str, scope: &str) -> Result<Follow, Refusal> {
        self.inner.follows(cx, form, scope)
    }
}

/// The wrapper with the SAME ports as the program, over the real module with the wrong rules.
pub fn lax<S: SystemPackages>(system: S) -> Guarded<Lax<StoreCore<S>>> {
    let core = Lax {
        inner: StoreCore::new(Arc::new(Memory), system),
        rules: AllowAll,
    };
    Guarded::new(core, Ports::new(OsUserIsPerson, local_person()))
}

pub fn connect(g: &mut Guarded<impl ModuleCore>, id: &str) -> form_source::Client {
    FromClientHook::new(g.bootstrap(&Caller::new(id, None)).hook)
}
