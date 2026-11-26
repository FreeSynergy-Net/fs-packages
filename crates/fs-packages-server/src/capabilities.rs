//! What this server can do, said honestly (provider rule, ADR 0045).
//!
//! Pattern: Value object — one [`Capabilities`] built once; `capabilities`, `supportReport` and every
//! refusal answer from it, so they cannot disagree with the methods.

use fs_interfaces::Unsupported;
use fs_interfaces::provider::Capabilities;
use fs_rpc::unsupported;

use crate::command::StoreCommand;

/// The contract version this server speaks.
pub(crate) const CONTRACT_VERSION: &str = "store 0.1 (draft)";

/// The capabilities of the contract `store`: reads work; writes are deprecated (K7) and jobs are read through the form source.
pub(crate) fn capabilities() -> Capabilities {
    let mut c = Capabilities::new("fs-packages");
    c.supported("source.read").supported("package.read");
    // K7: the write methods of the contract are deprecated and refused for good; the same keys are commands of the form source.
    for id in StoreCommand::ALL
        .into_iter()
        .filter(|c| *c != StoreCommand::CancelJob)
        .map(StoreCommand::key)
    {
        c.unsupported(
            id,
            format!("deprecated: use the command `{id}` of the form source"),
        );
    }
    c.unsupported(
        "job.read",
        "deprecated: read the records `job` through the form source",
    );
    c.unsupported(
        "job.subscribe",
        "deprecated: watch the records `job` through the form source",
    );
    c
}

/// The refusal for `capability`, as the provider rule wants it.
pub(crate) fn refuse(capability: &str) -> capnp::Error {
    unsupported(
        &capabilities()
            .why_not(capability)
            .unwrap_or_else(|| Unsupported::new(capability, "not possible")),
    )
}
