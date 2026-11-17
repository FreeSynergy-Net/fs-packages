//! The engine looks into the icon folder once for a listing, not once for each card for each draw.

use fs_packages_app::backend::StoreBackend;
use fs_packages_app::engine::EngineBackend;

#[test]
fn the_icon_folder_is_read_once_until_the_next_listing() {
    let root = std::env::temp_dir().join(format!("fs-packages-icons-{}", std::process::id()));
    let icons = root.join("icons");
    std::fs::create_dir_all(&icons).unwrap();
    std::fs::write(icons.join("fs-tasks.png"), b"x").unwrap();
    let engine = EngineBackend::new(root.clone());

    assert_eq!(
        engine.icon_path("fs-tasks"),
        Some(icons.join("fs-tasks.png"))
    );
    assert_eq!(engine.icon_path("fs-wiki"), None);
    assert_eq!(engine.icon_path("../x"), None);
    assert_eq!(engine.icon_path(".hidden"), None);

    // the file goes away: the folder is not looked at again before the next listing
    std::fs::remove_file(icons.join("fs-tasks.png")).unwrap();
    assert_eq!(
        engine.icon_path("fs-tasks"),
        Some(icons.join("fs-tasks.png"))
    );
    let _ = engine.available();
    assert_eq!(engine.icon_path("fs-tasks"), None);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_system_format_is_asked_once_not_once_per_card() {
    // `preferred_format` starts `rpm -qf` or `dpkg -S` (about 8 ms): a card asked it each time, 14 cards = 119 ms a draw.
    let dir = std::env::temp_dir().join(format!("fs-packages-format-{}", std::process::id()));
    let backend = fs_packages_app::engine::EngineBackend::new(dir);
    let first = fs_packages_app::backend::StoreBackend::system_format(&backend);
    let t = std::time::Instant::now();
    for _ in 0..200 {
        assert_eq!(
            fs_packages_app::backend::StoreBackend::system_format(&backend),
            first
        );
    }
    assert!(
        t.elapsed() < std::time::Duration::from_millis(100),
        "200 asks took {:?}: a process each?",
        t.elapsed()
    );
}
