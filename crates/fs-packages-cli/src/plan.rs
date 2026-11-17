//! Pattern: Strategy (which of the wanted packages go into the batch: what is not installed, locked, no longer offered, already up to
//! date, or a pre-release for a stable install in the sweep is left out with a note). The sources are asked through a function, so a test
//! needs no network.

use std::path::Path;

use fs_packages_catalog::CatalogEntry;

use crate::batch::Want;
use crate::commands::installed;
use crate::locked::Outcome;

/// What the batch found out about one wanted package before downloading.
pub(crate) struct Found {
    pub name: String,
    pub url: String,
    pub entry: CatalogEntry,
    pub catalog: Vec<u8>,
    pub sig: Vec<u8>,
}

/// No source gave an entry: the last problem, and whether every source could be read (then the package is no longer offered).
pub(crate) struct Miss {
    pub last: String,
    pub all_read: bool,
}

/// What the sources say about one want.
pub(crate) type Lookup<'a> = dyn Fn(&Want) -> Result<Result<Found, Miss>, String> + 'a;

/// Decides for every want whether it goes into the batch; returns the packages to install and the problems found. `only_newer` makes
/// it an update; `sweep` marks `update --all`.
pub(crate) fn plan_batch(
    root: &Path,
    wants: &[Want],
    only_newer: bool,
    sweep: bool,
    look: &Lookup<'_>,
) -> Result<(Vec<Found>, Vec<String>), String> {
    let mut problems = Vec::new();
    let mut found = Vec::new();
    for want in wants {
        let name = want.name.as_str();
        let current = installed(root, name);
        if only_newer && current.is_none() {
            problems.push(format!("{name} is not installed; use install"));
            continue;
        }
        let hit = match look(want)? {
            Ok(hit) => hit,
            Err(miss) => {
                // Every source was read and none lists it: taken out of the catalog. The sweep says so and goes on.
                let mut last = miss.last;
                if miss.all_read {
                    match crate::locked::outcome_unoffered(name, sweep) {
                        Outcome::Skip(note) => {
                            println!("{note}");
                            continue;
                        }
                        Outcome::Fail(why) => last = why,
                        Outcome::Proceed => {}
                    }
                }
                problems.push(last);
                continue;
            }
        };
        match crate::locked::outcome(name, hit.entry.locked.as_deref(), sweep) {
            Outcome::Proceed => {}
            Outcome::Skip(note) => {
                println!("{note}");
                continue;
            }
            Outcome::Fail(why) => {
                problems.push(why);
                continue;
            }
        }
        if only_newer && current.is_some_and(|c| hit.entry.version <= c) {
            println!(
                "{name} is up to date ({})",
                current.map_or_else(String::new, |c| c.to_string())
            );
            continue;
        }
        if crate::prerelease::skips_prerelease(root, name, &hit.entry, sweep && only_newer) {
            continue;
        }
        found.push(hit);
    }
    Ok((found, problems))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(version: &str, dev: bool) -> CatalogEntry {
        let t = if dev { r#"type="development""# } else { "" };
        let xml = format!(
            r#"<components version="1.0"><component><id>a</id><releases><release version="{version}" {t}><artifacts><artifact type="binary"><location>l</location><checksum type="sha256">3</checksum></artifact></artifacts></release></releases><custom><value key="fs:signature">s</value></custom></component></components>"#
        );
        fs_packages_catalog::parse_catalog(&xml)
            .unwrap()
            .entries
            .remove(0)
    }

    /// A store with `a` installed as 0.0.1; the register says whether that release is a pre-release.
    fn store(prerelease: bool) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("packages/a/0.0.1");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir_all(tmp.path().join("current")).unwrap();
        std::os::unix::fs::symlink(&dir, tmp.path().join("current/a")).unwrap();
        std::fs::write(
            tmp.path().join("installed.toml"),
            format!(
                "[[package]]\nname='a'\nversion='0.0.1'\nsource='s'\nprerelease={prerelease}\n"
            ),
        )
        .unwrap();
        tmp
    }

    fn plan(root: &Path, offered: &CatalogEntry, only_newer: bool, sweep: bool) -> Vec<String> {
        let want = [Want {
            name: "a".into(),
            source: None,
        }];
        let (found, _) = plan_batch(root, &want, only_newer, sweep, &|w| {
            Ok(Ok(Found {
                name: w.name.clone(),
                url: "u".into(),
                entry: offered.clone(),
                catalog: Vec::new(),
                sig: Vec::new(),
            }))
        })
        .unwrap();
        found.into_iter().map(|f| f.name).collect()
    }

    #[test]
    fn the_sweep_never_plans_a_prerelease_for_a_stable_install() {
        let stable = store(false);
        let dev = entry("0.0.2", true);
        // stable install, pre-release offered: the sweep leaves it out; named by the person and a stable offer go in
        assert_eq!(plan(stable.path(), &dev, true, true), Vec::<String>::new());
        assert_eq!(plan(stable.path(), &dev, true, false), ["a"]);
        assert_eq!(
            plan(stable.path(), &entry("0.0.2", false), true, true),
            ["a"]
        );
        // a pre-release install follows pre-releases
        let early = store(true);
        assert_eq!(plan(early.path(), &dev, true, true), ["a"]);
    }

    #[test]
    fn an_offer_that_is_not_newer_is_left_out_in_an_update() {
        let stable = store(false);
        assert_eq!(
            plan(stable.path(), &entry("0.0.1", false), true, true),
            Vec::<String>::new()
        );
    }
}
