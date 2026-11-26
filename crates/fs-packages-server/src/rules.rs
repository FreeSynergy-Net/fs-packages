//! The rights of the store: every write is a person's, reading is free.
//!
//! Pattern: Strategy - `StoreRules` is the `RightsRules` of ONE data kind (the store); `fs_rpc::Guarded` asks it before every read and every
//! change, the module (`StoreCore`) never checks a right itself. Fail closed: only `Actor::Person` writes; every other actor (`Bot` and any
//! one added later: `!=`, never `== Bot`) is refused, and a key the rules do not know is refused for everybody.

use fs_interfaces::module::{Actor, Ask, Context, HiddenReason, RightsRules, Verdict};
use fs_rpc::{READ_COMMAND, REVERT_COMMAND, WATCH_COMMAND};

use crate::command::StoreCommand;

/// The reason code of a refusal because the actor is no person (a code, never a sentence).
pub const PERSON_ONLY: &str = "person-only";
/// The reason code of a key the rules do not know.
pub const UNKNOWN: &str = "unknown";

/// The rules of the store (ENTSCHEID-store-k7-befehle, table 4). A unit value: nothing to configure, so nothing that could be configured open.
#[derive(Clone, Copy, Debug)]
pub struct StoreRules;

impl RightsRules for StoreRules {
    fn may(&self, ask: &Ask<'_>) -> Verdict {
        let key = ask.command;
        if key == READ_COMMAND || key == WATCH_COMMAND {
            return Verdict::Allow;
        }
        // A write installs code or changes a trust anchor: a person only (ADR 0054 Z. 206/213). Taking a change back is a write too.
        if key == REVERT_COMMAND || StoreCommand::of_key(key).is_some() {
            return if ask.who.actor == Actor::Person {
                Verdict::Allow
            } else {
                Verdict::deny(PERSON_ONLY)
            };
        }
        Verdict::deny(UNKNOWN)
    }

    fn hidden(&self, _: &Context, _: &str, _: &str) -> Option<HiddenReason> {
        None
    }
}

#[cfg(test)]
mod tests {
    use fs_interfaces::module::{Actor, Ask, Caller, Context, RightsRules, Verdict};
    use fs_interfaces::owner::fake::person;
    use fs_rpc::{READ_COMMAND, REVERT_COMMAND, WATCH_COMMAND};
    use fs_types::Utc;

    use super::{PERSON_ONLY, StoreRules, UNKNOWN};
    use crate::command::StoreCommand;

    fn cx(actor: Actor) -> Context {
        let who = Caller::Connection {
            system_id: "1000".into(),
            system_name: None,
        };
        Context::from_connection(
            who,
            actor,
            person("urn:uuid:018f3a52-7c1e-7b3a-9d4e-5a6b7c8d9e01"),
        )
    }

    fn asked(actor: Actor, key: &str) -> Verdict {
        let who = cx(actor);
        StoreRules.may(&Ask::new(&who, key, Some("tool"), Utc::from_millis(0)))
    }

    #[test]
    fn reading_is_for_every_actor_a_write_is_for_a_person_only() {
        for actor in [Actor::Person, Actor::Bot] {
            assert!(asked(actor, READ_COMMAND).is_allowed());
            assert!(asked(actor, WATCH_COMMAND).is_allowed());
        }
        for key in StoreCommand::ALL
            .map(StoreCommand::key)
            .into_iter()
            .chain([REVERT_COMMAND])
        {
            assert!(asked(Actor::Person, key).is_allowed(), "{key}");
            assert_eq!(asked(Actor::Bot, key), Verdict::deny(PERSON_ONLY), "{key}");
        }
    }

    #[test]
    fn an_unknown_key_is_refused_for_everybody_and_nothing_is_hidden() {
        for actor in [Actor::Person, Actor::Bot] {
            assert_eq!(asked(actor, "package.frobnicate"), Verdict::deny(UNKNOWN));
            assert_eq!(StoreRules.hidden(&cx(actor), "tool", "version"), None);
        }
    }
}
