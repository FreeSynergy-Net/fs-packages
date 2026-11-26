//! The records of the three forms of the store module (`package`, `source`, `job`) and the refusals of the module.
//!
//! Pattern: Adapter - the plain values of [`StoreView`](crate::StoreView) and [`Job`] to the values (`RecordView`, `Refusal`) of the module
//! contract; nothing is stored or decided here. Texts are values or codes, never sentences.

use std::collections::BTreeSet;
use std::fmt::Debug;

use fs_interfaces::module::{RecordView, Refusal, Value};

use crate::command::StoreCommand::{
    AddSource, CancelJob, EnableSource, Install, Refresh, RemoveSource, Uninstall, Update,
};
use crate::system::{Job, SystemFault};
use crate::view::{PackageInfo, SourceInfo};

/// The form of the packages (offered and installed ones).
pub const PACKAGE: &str = "package";
/// The form of the catalog sources.
pub const SOURCE: &str = "source";
/// The form of the jobs.
pub const JOB: &str = "job";

/// A refusal of the contract made of its kind, status and a code.
pub(crate) fn refuse(kind: &str, status: u16, detail: &str) -> Refusal {
    let mut r = Refusal::default();
    kind.clone_into(&mut r.kind);
    r.status = status;
    detail.clone_into(&mut r.detail);
    r
}

/// The refusal of a package manager that said no: `unsupported` is 501, any other code a conflict with the state of the system.
pub(crate) fn refusal_of(fault: &SystemFault) -> Refusal {
    if fault.code == "unsupported" {
        refuse("unsupported", 501, &fault.code)
    } else {
        refuse("conflict", 409, &fault.code)
    }
}

/// A closed set of words (`Kind`, `Format`) as the lower-case word the form shows.
fn word(w: &impl Debug) -> String {
    format!("{w:?}").to_ascii_lowercase()
}

fn record(id: &str, may: &[&str], values: &[(&str, String)]) -> RecordView {
    let mut v = RecordView::default();
    id.clone_into(&mut v.id);
    v.may = may.iter().map(|k| (*k).to_owned()).collect();
    v.values = values.iter().map(|(p, t)| Value::new(p, t)).collect();
    v
}

/// The rows of `package`: what the sources offer, then what is installed and offered by no source. `installed` is the truth of the system.
pub(crate) fn packages(offered: &[PackageInfo], installed: &[PackageInfo]) -> Vec<RecordView> {
    let have: BTreeSet<&str> = installed.iter().map(|p| p.name.as_str()).collect();
    let offered_names: BTreeSet<&str> = offered.iter().map(|p| p.name.as_str()).collect();
    let row = |p: &PackageInfo, is_installed: bool| {
        let may: &[&str] = if is_installed {
            &[Update.key(), Uninstall.key()]
        } else {
            &[Install.key()]
        };
        record(
            &p.name,
            may,
            &[
                ("name", p.name.clone()),
                ("version", p.version.clone()),
                ("kind", word(&p.kind)),
                ("format", word(&p.format)),
                ("source", p.source.clone()),
                ("summary", p.summary.clone()),
                ("installed", is_installed.to_string()),
            ],
        )
    };
    let mut rows: Vec<_> = offered
        .iter()
        .map(|p| row(p, have.contains(p.name.as_str())))
        .collect();
    rows.extend(
        installed
            .iter()
            .filter(|p| !offered_names.contains(p.name.as_str()))
            .map(|p| row(p, true)),
    );
    rows
}

/// The rows of `source`.
pub(crate) fn sources(list: &[SourceInfo]) -> Vec<RecordView> {
    list.iter()
        .map(|s| {
            record(
                &s.id,
                &[EnableSource.key(), RemoveSource.key(), Refresh.key()],
                &[
                    ("id", s.id.clone()),
                    ("url", s.url.clone()),
                    ("enabled", s.enabled.to_string()),
                    ("reference", s.reference.to_string()),
                ],
            )
        })
        .collect()
}

/// The rows of `job`: the fields of a `PackageKit` transaction.
pub(crate) fn jobs(list: &[Job]) -> Vec<RecordView> {
    list.iter()
        .map(|j| {
            let may: &[&str] = if j.allow_cancel {
                &[CancelJob.key()]
            } else {
                &[]
            };
            record(
                &j.id,
                may,
                &[
                    ("role", j.role.nick().to_owned()),
                    ("status", j.status.nick().to_owned()),
                    ("percentage", j.percentage.to_string()),
                    ("allow_cancel", j.allow_cancel.to_string()),
                ],
            )
        })
        .collect()
}

/// The commands the set of a form offers without a record.
pub(crate) fn set_commands(form: &str) -> Vec<String> {
    let keys: &[&str] = match form {
        PACKAGE => &[Update.key()],
        SOURCE => &[AddSource.key(), Refresh.key()],
        _ => &[],
    };
    keys.iter().map(|k| (*k).to_owned()).collect()
}
