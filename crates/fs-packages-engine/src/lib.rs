//! Engine of FreeSynergy.Packages: the parts that touch the machine. Pattern: Strategy implementations
//! (`TarGz` for `InstallStrategy`, `ProcRunningCheck` for `RunningCheck`). No root needed.
#![deny(missing_docs)]

mod after;
mod batch;
mod catalog_fetch;
mod deb;
mod end;
mod ending;
mod facets;
mod icon_cache;
mod icons;
mod links;
mod lock;
mod mode;
mod native;
pub mod net;
mod own_files;
mod own_judge;
mod owner;
mod prepare;
mod proc;
mod remove;
mod retire;
mod rpm;
mod servers;
pub mod service;
mod targz;
pub mod timing;

pub use after::{Settled, settle_install, settle_install_with, settle_remove};
pub use batch::{CommitObserver, Observer};
pub use catalog_fetch::{CATALOG_FRESH_FOR, CatalogCache, ORPHAN_AFTER, with_clone};
pub use deb::{Deb, remove_deb, remove_native};
pub use end::{CALL_LIMIT, Ended, end_server, end_servers, server_info, socket_of, stop_servers};
pub use facets::{
    Form, default_apps_dir, ensure_own_menu_entry, link_facets, link_facets_for, unlink_facets,
};
pub use icon_cache::{
    KEEP_UNUSED, Wanted, cache_dir, cache_dir_from_environment, cached_bytes, cached_icon,
    fetch_into_cache, fetch_missing, prune,
};
pub use icons::store_icon;
pub use links::{default_bin_dir, default_system_dir, link_programs, unlink_programs};
pub use lock::PackageLock;
pub use mode::{
    System, deb_installed_version, detect_system, format_for, native_installed_version, own_exe,
    preferred_format, restart, rpm_installed_version, system_from_os_release,
    without_deleted_suffix,
};
pub use own_files::{Report, Verdict, adopt_missing, collect, record_install, verify};
pub use owner::{Owner, owner, system_owner};
pub use prepare::{Downloaded, Plan, Prepared};
pub use proc::ProcRunningCheck;
pub use remove::{RemoveError, remove};
pub use retire::check_links;
pub use rpm::{Commander, Rpm, SystemCommander, remove_rpm};
pub use servers::{Server, find_servers, is_alive, is_server_of, program_of, start_server};
pub use targz::TarGz;
