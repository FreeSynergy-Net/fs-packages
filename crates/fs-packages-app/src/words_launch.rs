//! Pattern: none (the words of a started program that failed, ended at once, shows no sign of life or has a log, in English and German; sentences with named placeholders,
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

/// The program ended right after the start without a failure (exit code 0). A starter that hands over to another process looks the same.
/// Placeholder: `{name}`.
#[must_use]
pub fn ended_at_once(lang: Lang) -> &'static str {
    match lang {
        Lang::En => {
            "{name} ended right after the start, without an error (exit code 0). If it handed over to another process, that is fine; if no window came up, the end of its log may tell why:"
        }
        Lang::De => {
            "{name} beendete sich gleich nach dem Start ohne Fehler (Exit-Code 0). Hat es an einen anderen Prozess \u{fc}bergeben, ist das in Ordnung; kam kein Fenster, sagt das Ende des Protokolls vielleicht warum:"
        }
    }
}

/// The program runs but shows no sign of life. Placeholders: `{name}`, `{seconds}`.
#[must_use]
pub fn no_sign_of_life(lang: Lang) -> &'static str {
    match lang {
        Lang::En => {
            "{name} has been running for {seconds} s and shows no sign of life yet (no window). It may be waiting for something; its log may tell. Not responding: stop it?"
        }
        Lang::De => {
            "{name} l\u{e4}uft seit {seconds} s und zeigt noch kein Lebenszeichen (kein Fenster). Es wartet vielleicht auf etwas; das Protokoll sagt es eventuell. Reagiert nicht: beenden?"
        }
    }
}

/// The button that stops the program the store started.
#[must_use]
pub fn stop_program(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Stop it",
        Lang::De => "Beenden",
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
