//! What the packages program hands to a program it starts: the command and nothing else (no `--facet`, no source), so
//! the program starts as its desktop entry starts it and is not put into a reading mode by an argument of its own.
//!
//! Pattern: Unit test of the pure argument builder; nothing is started.

use std::ffi::OsString;
use std::path::Path;

use fs_packages_app::launcher::setsid_words;

#[test]
fn starting_a_program_passes_no_argument_of_its_own() {
    let words = setsid_words(Path::new("/usr/bin/fs-text"), None);
    let expected: Vec<OsString> = vec!["-f".into(), "/usr/bin/fs-text".into()];
    assert_eq!(words, expected);
}

#[test]
fn only_the_opener_gets_its_address() {
    let words = setsid_words(Path::new("/usr/bin/xdg-open"), Some("https://example.org"));
    assert_eq!(words.len(), 3);
    assert_eq!(words[2], OsString::from("https://example.org"));
}
