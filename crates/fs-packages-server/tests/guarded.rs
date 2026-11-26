//! The store under `Guarded` (STW-2a): a person writes through every key of K7, a bot writes through none BECAUSE of `StoreRules` (with rules
//! "everything is allowed" the tests of the bot turn red), reading is for both, an unknown key is for nobody. Pattern: none (tests).
#![allow(
    clippy::unused_async_trait_impl,
    clippy::unnecessary_literal_bound,
    clippy::result_large_err
)]

mod support;

use fs_packages_server::FakeSystem;
use support::{BOT_ID, PERSON_ID, WRITES, asked, connect, fake_service, ids, invoke, lax, run};

/// A job must exist before it can be cancelled: the person starts one first.
async fn start_job(c: &fs_interfaces::formsource_capnp::form_source::Client) {
    assert!(!invoke(c, "package.install", "tool", &[]).await.failed);
}

#[test]
fn a_person_writes_through_every_key() {
    run(async {
        let mut s = fake_service();
        let person = connect(&mut s, PERSON_ID);
        start_job(&person).await; // fake-1, so `job.cancel` has a job
        for (key, record, args) in &WRITES[1..] {
            let done = invoke(&person, key, record, args).await;
            assert!(!done.failed, "{key}: {done:?}");
        }
        let asked: Vec<_> = s
            .core()
            .unwrap()
            .system()
            .calls()
            .into_iter()
            .map(|c| c.0)
            .collect();
        assert_eq!(
            asked,
            [
                "install",
                "update",
                "uninstall",
                "add_source",
                "enable_source",
                "remove_source",
                "refresh",
                "cancel"
            ]
        );
    });
}

#[test]
fn a_bot_writes_through_no_key_and_the_manager_is_never_asked() {
    run(async {
        let mut s = fake_service();
        let bot = connect(&mut s, BOT_ID);
        for (key, record, args) in WRITES {
            let refused = invoke(&bot, key, record, args).await;
            assert_eq!(
                (refused.failed, refused.status),
                (true, 403),
                "{key}: {refused:?}"
            );
        }
        assert!(
            s.core().unwrap().system().calls().is_empty(),
            "the manager was not asked"
        );
    });
}

#[test]
fn an_unknown_key_is_for_nobody() {
    run(async {
        let mut s = fake_service();
        for id in [PERSON_ID, BOT_ID] {
            let c = connect(&mut s, id);
            for key in ["package.frobnicate", "packages.update", ""] {
                let r = invoke(&c, key, "tool", &[]).await;
                assert_eq!((r.failed, r.status), (true, 403), "{id} {key:?}: {r:?}");
            }
        }
        assert_eq!(asked(&s), Vec::<&str>::new());
    });
}

#[test]
fn a_bad_argument_of_a_person_is_a_400_and_asks_nobody() {
    run(async {
        let mut s = fake_service();
        let person = connect(&mut s, PERSON_ID);
        for (key, record, args) in [
            ("package.install", "", &[][..]),
            ("package.update", "tool", &[("step", "now")]),
            ("package.update", "tool", &[]),
            ("source.add", "", &[("url", "")]),
            ("source.enable", "extra", &[("enabled", "maybe")]),
            ("package.install", "tool", &[("force", "yes")]),
        ] {
            let r = invoke(&person, key, record, args).await;
            assert_eq!((r.failed, r.status), (true, 400), "{key}: {r:?}");
        }
        assert_eq!(asked(&s), Vec::<&str>::new());
    });
}

#[test]
fn reading_is_for_a_person_and_a_bot() {
    run(async {
        let mut s = fake_service();
        for id in [PERSON_ID, BOT_ID] {
            let c = connect(&mut s, id);
            assert_eq!(ids(&c, "package").await, ["tool"], "{id}");
            assert_eq!(ids(&c, "source").await, ["reference", "extra"], "{id}");
            assert!(ids(&c, "job").await.is_empty(), "{id}");
        }
    });
}

#[test]
fn a_job_is_a_record_and_only_a_person_stops_it() {
    run(async {
        let mut s = fake_service();
        let (person, bot) = (connect(&mut s, PERSON_ID), connect(&mut s, BOT_ID));
        start_job(&person).await;
        assert_eq!(ids(&bot, "job").await, ["fake-1"], "a bot reads the job");
        assert_eq!(invoke(&bot, "job.cancel", "fake-1", &[]).await.status, 403);
        assert_eq!(
            invoke(&person, "job.cancel", "fake-9", &[]).await.status,
            404,
            "no such job"
        );
        assert!(!invoke(&person, "job.cancel", "fake-1", &[]).await.failed);
        assert_eq!(
            invoke(&person, "job.cancel", "fake-1", &[]).await.status,
            409,
            "stopped already"
        );
    });
}

#[test]
fn a_manager_that_says_no_is_a_refusal_not_a_success() {
    run(async {
        let mut s = support::service(FakeSystem::failing("not-offered"));
        let person = connect(&mut s, PERSON_ID);
        let r = invoke(&person, "package.install", "tool", &[]).await;
        assert_eq!((r.failed, r.status), (true, 409), "{r:?}");
        assert!(
            ids(&person, "job").await.is_empty(),
            "no job without a manager that took it"
        );
        let mut none = support::service(fs_packages_server::NoSystem);
        let person = connect(&mut none, PERSON_ID);
        assert_eq!(
            invoke(&person, "package.install", "tool", &[]).await.status,
            501
        );
    });
}

/// The counter-proof: with rules "everything is allowed" the bot installs (the facts of the bot test turn around).
#[test]
fn counter_rules_that_allow_everything_let_the_bot_in() {
    run(async {
        let mut g = lax::lax(FakeSystem::new());
        let bot = lax::connect(&mut g, BOT_ID);
        for (key, record, args) in &WRITES[..7] {
            let done = invoke(&bot, key, record, args).await;
            assert!(!done.failed, "with AllowAll the bot does {key}: {done:?}");
        }
        assert_eq!(
            g.core().inner().system().calls().len(),
            7,
            "the manager was asked"
        );
    });
}
