//! Pattern: none (the words of a started program that failed or has a log, in English and German; sentences with named placeholders,
//! the values are put in by `words::fill`).

use crate::words_lang::Lang;

/// The program ended with a failure right after the start. Placeholders: `{name}`, `{code}`.
#[must_use]
pub fn ended_with_code(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "{name} ended right after the start (exit code {code}). The end of its log:",
        Lang::De => {
            "{name} wurde gleich nach dem Start beendet (Exit-Code {code}). Das Ende des Protokolls:"
        }
    }
}

/// A signal ended the program right after the start. Placeholder: `{name}`.
#[must_use]
pub fn ended_by_signal(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "{name} was stopped by the system right after the start. The end of its log:",
        Lang::De => {
            "{name} wurde gleich nach dem Start vom System gestoppt. Das Ende des Protokolls:"
        }
    }
}

/// The failed program wrote nothing.
#[must_use]
pub fn no_output(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "It wrote nothing.",
        Lang::De => "Es hat nichts ausgegeben.",
    }
}

/// The log could not be written. Placeholders: `{name}`, `{cause}`.
#[must_use]
pub fn log_unwritable(lang: Lang) -> &'static str {
    match lang {
        Lang::En => {
            "The log of {name} could not be written ({cause}). It was started anyway; its output is lost."
        }
        Lang::De => {
            "Das Protokoll von {name} konnte nicht geschrieben werden ({cause}). Es wurde trotzdem gestartet; seine Ausgabe geht verloren."
        }
    }
}

/// The button that opens the log of a program.
#[must_use]
pub fn open_log(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Open log",
        Lang::De => "Protokoll \u{f6}ffnen",
    }
}

/// The log could not be opened. Placeholder: `{detail}`.
#[must_use]
pub fn open_log_failed(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "The log could not be opened: {detail}",
        Lang::De => "Das Protokoll konnte nicht ge\u{f6}ffnet werden: {detail}",
    }
}
