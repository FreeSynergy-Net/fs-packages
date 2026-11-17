//! The pre-release word follows the window language (`FS_LANG`, else `LANG`), the same source as the other translated words: in each
//! language the word of that language appears and no word of the other.

use fs_packages_app::words_front::{Lang, prerelease};

#[test]
fn prerelease_word_follows_the_window_language_only() {
    // SAFETY: the only test of this file, nothing reads the environment in parallel.
    unsafe { std::env::set_var("FS_LANG", "en") };
    assert_eq!(Lang::current(), Lang::En);
    assert_eq!(prerelease(Lang::current()), "Pre-release");
    assert!(!prerelease(Lang::current()).contains("Vorab"));
    unsafe { std::env::set_var("FS_LANG", "de") };
    assert_eq!(Lang::current(), Lang::De);
    assert_eq!(prerelease(Lang::current()), "Vorabversion");
    assert!(!prerelease(Lang::current()).contains("Pre-release"));
}
