//! The command line of FreeSynergy.Packages as a library: `fs-packages` (crate `fs-packages`) calls [`run`].
//! Pattern: Command; see `commands`.
#![deny(missing_docs)]

mod background;
pub use background::notification_lines;
mod batch;
mod commands;
mod locked;
mod own;
mod plan;
mod prerelease;
mod register;
mod relink;
mod source_list;
mod tuf;
mod updates;

pub use commands::run;
