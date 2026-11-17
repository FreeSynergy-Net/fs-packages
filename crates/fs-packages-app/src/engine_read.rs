//! Pattern: Cache-aside (reading one source for the engine: one clone at a time, kept in memory, verified before it is believed).

use std::fs;
use std::sync::Arc;

use fs_packages_catalog::parse_catalog_in;
use fs_packages_core::{SignaturePolicy, SignatureVerifier};
use fs_packages_pgp::PgpVerifier;

use crate::engine::EngineBackend;

/// One source as it was fetched, verified and parsed.
#[derive(Debug)]
pub(crate) struct SourceRead {
    pub(crate) catalog: fs_packages_catalog::Catalog,
    pub(crate) bytes: Vec<u8>,
    pub(crate) sig: Vec<u8>,
    /// Unix seconds of the fetch.
    pub(crate) at: u64,
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl EngineBackend {
    /// Clones one source (one clone at a time, in a folder of its own); returns catalog and signature bytes.
    fn fetch_source(&self, url: &str) -> Result<(Vec<u8>, Vec<u8>), String> {
        fs_packages_engine::with_clone(&self.root, url, |work| {
            let read = |file: &str| fs::read(work.join(file)).map_err(|e| format!("{file}: {e}"));
            Ok((read("catalog.xml")?, read("catalog.xml.asc")?))
        })
    }

    /// Catalog of one source after its signature was checked: from memory when it was fetched lately, else fetched now.
    pub(crate) fn read_cached(&self, url: &str) -> Result<Arc<SourceRead>, String> {
        self.catalogs.get_or_fetch(url, || self.read_fresh(url))
    }

    fn read_fresh(&self, url: &str) -> Result<SourceRead, String> {
        let (bytes, sig) = self.fetch_source(url)?;
        let verifier = PgpVerifier::builtin();
        if !SignaturePolicy::default().is_satisfied_by(&verifier.verify(&bytes, Some(&sig))) {
            return Err("the signature of the catalog is not valid".to_owned());
        }
        let catalog = parse_catalog_in(
            &String::from_utf8_lossy(&bytes),
            &fs_packages_core::language::preferred(),
        )
        .map_err(|e| e.0)?;
        fs_packages_sources::ensure_fresh(
            &self.root.join("serials.toml"),
            url,
            catalog.serial,
            catalog.expires,
        )?;
        Ok(SourceRead {
            catalog,
            bytes,
            sig,
            at: unix_now(),
        })
    }

    /// Catalog entries of one source, after its signature was checked.
    pub(crate) fn read_source(
        &self,
        url: &str,
    ) -> Result<(fs_packages_catalog::Catalog, Vec<u8>, Vec<u8>), String> {
        let read = self.read_cached(url)?;
        Ok((read.catalog.clone(), read.bytes.clone(), read.sig.clone()))
    }
}
