//! The rows of the model by name, built once for each card list.
//!
//! Pattern: Index (one lookup per card instead of a scan of every list per card, which was O(N^2)).

use std::collections::HashMap;

use super::jobs::JobView;
use super::store::StoreModel;
use super::types::AvailableRow;
use crate::backend::InstalledRow;

/// The rows by name. The FIRST row of a name wins, as the `find` it replaces did.
pub(super) struct Index<'a> {
    pub(super) available: HashMap<&'a str, &'a AvailableRow>,
    pub(super) installed: HashMap<&'a str, &'a InstalledRow>,
    /// First job of a name.
    pub(super) job: HashMap<&'a str, &'a JobView>,
    /// First job of a name that is not finished.
    pub(super) running: HashMap<&'a str, &'a JobView>,
}

impl<'a> Index<'a> {
    /// Index the lists of `model`.
    pub(super) fn of(model: &'a StoreModel) -> Self {
        let mut available = HashMap::new();
        for r in &model.available {
            available.entry(r.package.name.as_str()).or_insert(r);
        }
        let mut installed = HashMap::new();
        for r in &model.installed {
            installed.entry(r.name.as_str()).or_insert(r);
        }
        let mut job = HashMap::new();
        let mut running = HashMap::new();
        for j in &model.jobs {
            job.entry(j.name.as_str()).or_insert(j);
            if !j.is_finished() {
                running.entry(j.name.as_str()).or_insert(j);
            }
        }
        Self {
            available,
            installed,
            job,
            running,
        }
    }
}
