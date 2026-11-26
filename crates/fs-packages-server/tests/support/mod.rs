//! The world of `tests/guarded.rs` and `tests/socket.rs`: a store view in memory, the module behind `Guarded` with the ports of the program,
//! the calls over the form source. Pattern: Test double helpers (in-process, no socket unless a test opens one).
#![allow(dead_code, clippy::cast_possible_truncation)]

pub mod lax;

use std::future::Future;
use std::sync::Arc;

use capnp::capability::FromClientHook;
use fs_interfaces::formsource_capnp::form_source;
use fs_interfaces::owner::fake::{FixedOwner, person};
use fs_packages_core::{Details, Format, Kind};
use fs_packages_server::{
    FakeSystem, PackageInfo, SourceInfo, StoreCore, StoreGuarded, StoreServer, StoreView,
    SystemPackages,
};
use fs_rpc::{Caller, LocalPerson, PerConnection};

/// A person: the transport names the id of the service's own user. A bot: an id the system could not turn into an actor.
pub const PERSON_ID: &str = "1000";
pub const BOT_ID: &str = "not an id";

/// What the view knows: two sources, one offered package, nothing installed.
pub struct Memory;

impl StoreView for Memory {
    fn sources(&self) -> Result<Vec<SourceInfo>, String> {
        let s = |id: &str, reference| SourceInfo {
            id: id.into(),
            url: format!("https://example.invalid/{id}"),
            enabled: true,
            reference,
        };
        Ok(vec![s("reference", true), s("extra", false)])
    }
    fn available(
        &self,
    ) -> Result<(Vec<PackageInfo>, Vec<fs_packages_server::SkippedInfo>), String> {
        Ok((vec![package("tool")], Vec::new()))
    }
    fn installed(&self) -> Result<Vec<PackageInfo>, String> {
        Ok(Vec::new())
    }
}

fn package(name: &str) -> PackageInfo {
    PackageInfo {
        name: name.into(),
        kind: Kind::Program,
        version: "1.0.0".into(),
        format: Format::Rpm,
        source: "reference".into(),
        facets: Vec::new(),
        summary: "a tool".into(),
        details: Details::default(),
        locked: None,
        icon_sha256: String::new(),
    }
}

pub fn run<F: Future>(f: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&rt, f)
}

pub fn local_person() -> LocalPerson {
    LocalPerson::new(&FixedOwner(person(
        "urn:uuid:018f3a52-7c1e-7b3a-9d4e-5a6b7c8d9e01",
    )))
    .unwrap()
}

/// The program's service over `system`, with the real root of `store.capnp` over an in-memory view.
pub fn service<S: SystemPackages>(system: S) -> StoreGuarded<S> {
    let root = StoreServer::new(Arc::new(Memory)).into_client().client;
    StoreGuarded::new(
        root,
        StoreCore::new(Arc::new(Memory), system),
        Ok(local_person()),
        &|_| {},
    )
}

pub fn fake_service() -> StoreGuarded<FakeSystem> {
    service(FakeSystem::new())
}

pub fn connect(s: &mut impl PerConnection, id: &str) -> form_source::Client {
    FromClientHook::new(s.bootstrap(&Caller::new(id, None)).hook)
}

#[derive(Debug)]
pub struct Wrote {
    pub failed: bool,
    pub status: u16,
}

pub async fn invoke(
    c: &form_source::Client,
    cmd: &str,
    record: &str,
    list: &[(&str, &str)],
) -> Wrote {
    let mut req = c.invoke_request();
    req.get().set_command(cmd);
    if !record.is_empty() {
        req.get().set_record(record);
    }
    let mut out = req.get().init_args(list.len() as u32);
    for (i, (p, t)) in list.iter().enumerate() {
        let mut e = out.reborrow().get(i as u32);
        e.set_path(p);
        e.set_text(t);
    }
    let r = req.send().promise.await.unwrap();
    let a = r.get().unwrap();
    Wrote {
        failed: a.get_failed(),
        status: a.get_problem().unwrap().get_status(),
    }
}

/// The ids of the rows of the form `form@1.0.0`.
pub async fn ids(c: &form_source::Client, form: &str) -> Vec<String> {
    let mut req = c.page_request();
    req.get().set_form(format!("{form}@1.0.0"));
    req.get().set_limit(100);
    let r = req.send().promise.await.unwrap();
    let page = r.get().unwrap().get_page().unwrap();
    page.get_records()
        .unwrap()
        .iter()
        .map(|rec| rec.get_id().unwrap().to_string().unwrap())
        .collect()
}

/// The arguments of a call: `(name, text)`.
pub type Args = &'static [(&'static str, &'static str)];

/// What the manager was asked, by method name.
pub fn asked(s: &StoreGuarded<FakeSystem>) -> Vec<&'static str> {
    s.core()
        .unwrap()
        .system()
        .calls()
        .into_iter()
        .map(|c| c.0)
        .collect()
}

/// One call per key of K7 with the arguments it needs: `(key, record, args)`.
pub const WRITES: [(&str, &str, Args); 8] = [
    ("package.install", "tool", &[]),
    ("package.update", "tool", &[("step", "run")]),
    ("package.uninstall", "tool", &[]),
    ("source.add", "", &[("url", "https://example.invalid/new")]),
    ("source.enable", "extra", &[("enabled", "false")]),
    ("source.remove", "extra", &[]),
    ("source.refresh", "extra", &[]),
    ("job.cancel", "fake-1", &[]),
];
