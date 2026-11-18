# fs-packages

Installs and updates FreeSynergy packages and itself. Design: see DESIGN.md; changes: CHANGELOG.md.

## The window and its frontends

`fs-packages` (without arguments) opens the window. Its **frontend** is chosen in the window: menu **View** (German: **Ansicht**), entries
"Frontend: ...". The choice is remembered (the setting is still called `face` in `settings.toml`, value `"storefront"` or `"compact"`, in the
data folder of the store, `FS_PACKAGES_ROOT` or `~/.local/share/fs-packages`; the name stays so that saved settings keep working).
Tiles, details and list are **views** of a frontend, not frontends.

| Frontend | What it is |
|---|---|
| Storefront (default) | Craft look: tiles or a table, navigation at the left, start page, package page |
| Compact | a list at the left, the page of the package at the right (platform look) |

`--variant b|c` wears a frontend for one run only (it wins over the saved choice and is not saved). `--version` / `-V` prints the version.
The look (Craft or platform theme) is chosen once at the start: a frontend chosen in the menu changes the layout at once, its own look
follows at the next start.

### Reload, messages, install question, navigation (0.0.16)

- **Reload** is the round-arrow button (`view-refresh`, tooltip "Reload catalog" / "Katalog neu laden") at the right of the top bar. A source that cannot be read is told in a message and what was shown stays. The reload button fetches the catalogs again; otherwise a fetched catalog is reused for 10 minutes, and only one fetch runs at a time (lock `sources/.lock`). Windows: the lock is `File::lock` of the standard library, the same as elsewhere.
- **Messages** at the top go away by themselves (a hint after 6 s, an error after 20 s), each has a close button; menu **Packages > Messages** (German: **Meldungen**) lists every message of the session.
- **Install** is one click on "Install", then the question (what is added, size, source, rights), then **Install now**. Nothing asks again; for a system install (rpm/deb) the question says that the system asks for your password next.
- **Start** is the overview (updates, new, not-supported entries) and has no number. **Programs**, **Frontends**, **Components**, **Themes**, ... list ALL packages of their kind; the number is what the list shows.

### Starting a program: its log, hints, stop (0.0.20 - 0.0.22)

"Open" starts the program as a child of its own; its output goes through a small helper (`fs-packages log-pump`, internal) to `~/.local/state/fs-packages/launch-<name>.log` (`$XDG_STATE_HOME` is honoured; folder 0700 and not a symbolic link, file 0600 and never opened through a link, one file per program, wiped at each start). The helper keeps the log at **64 KB at most at all times**, also when the program forks away and after the store is closed. A program that fails within 3 seconds shows its exit code and the last 10 log lines on its tile; one that ends within 3 seconds without an error says so; one that still runs after 8 seconds without a sign of life (no window) is told as a hint with **Stop it** (only the process the store started). "Open log" (German: "Protokoll öffnen") opens the file. The same notes show on the package page and in the compact variant. Measuring the speed of the window: `docs/measuring.md`. **Limits (read before you rely on it):** the hint "no sign of life" looks for display libraries in the started program, so a program that has none (a terminal program, a server) gets the hint after 8 seconds although it is fine; ignore it. **Stop it** ends only the process the store started itself, not programs that a starter script started from it (the script's children keep running). The pump rewrites the 64 KB of a full log at most every 100 ms or every 16 KB of new output (a program that prints line by line no longer makes it rewrite the whole file for each line), so the file may lack the last 100 ms of output; it is complete when the output ends.

## Commands and the install form

The commands of a program and its facets (`~/.local/bin/<name>`) follow the form that is active: `/usr/bin` for an rpm/deb,
`current/<name>/` for a package the store unpacked. `fs-packages verify [name]` also checks that no command starts another version than
`installed.toml` says (exit code not zero, with the repair); `fs-packages relink [name]` makes the commands again for the active form.

License: MIT OR Apache-2.0.

## Windows: test files that do not run there (not visible in the Windows log)

These 18 test files start with `#![cfg(unix)]`. On Windows they are not compiled, so the Windows log shows neither `ok` nor `ignored` for them: the Windows port is open (symbolic links, modes, processes). Tests that are only `ignored` on Windows are listed with their reason in the log and in CHANGELOG.md. The wanted Windows outcome of an install (`unsupported-on-windows{symlink}` / `{private-dir}`, nothing left behind) is checked by [`crates/fs-packages-engine/tests/windows_unsupported.rs`](crates/fs-packages-engine/tests/windows_unsupported.rs), which runs on Windows only.

## Windows: Testdateien, die dort nicht laufen (im Windows-Log nicht sichtbar)

Diese 18 Testdateien beginnen mit `#![cfg(unix)]`. Unter Windows werden sie nicht übersetzt, das Windows-Log zeigt dafür weder `ok` noch `ignored`: der Windows-Port ist offen (Symlinks, Modi, Prozesse). Tests, die unter Windows nur `ignored` sind, stehen mit Grund im Log und im CHANGELOG.md. Das gewollte Windows-Ergebnis einer Installation (`unsupported-on-windows{symlink}` / `{private-dir}`, nichts bleibt zurück) prüft [`crates/fs-packages-engine/tests/windows_unsupported.rs`](crates/fs-packages-engine/tests/windows_unsupported.rs), nur unter Windows.

- [`crates/fs-packages-app/tests/installed_detect.rs`](crates/fs-packages-app/tests/installed_detect.rs)
- [`crates/fs-packages-app/tests/launch_hang.rs`](crates/fs-packages-app/tests/launch_hang.rs)
- [`crates/fs-packages-app/tests/launch_log.rs`](crates/fs-packages-app/tests/launch_log.rs)
- [`crates/fs-packages-app/tests/launch_pump.rs`](crates/fs-packages-app/tests/launch_pump.rs)
- [`crates/fs-packages-app/tests/launch_spawn_fail.rs`](crates/fs-packages-app/tests/launch_spawn_fail.rs)
- [`crates/fs-packages-app/tests/launch_symlink.rs`](crates/fs-packages-app/tests/launch_symlink.rs)
- [`crates/fs-packages-cli/tests/links.rs`](crates/fs-packages-cli/tests/links.rs)
- [`crates/fs-packages-cli/tests/own.rs`](crates/fs-packages-cli/tests/own.rs)
- [`crates/fs-packages-cli/tests/verify_broken.rs`](crates/fs-packages-cli/tests/verify_broken.rs)
- [`crates/fs-packages-engine/tests/after.rs`](crates/fs-packages-engine/tests/after.rs)
- [`crates/fs-packages-engine/tests/cancel_more.rs`](crates/fs-packages-engine/tests/cancel_more.rs)
- [`crates/fs-packages-engine/tests/end_server.rs`](crates/fs-packages-engine/tests/end_server.rs)
- [`crates/fs-packages-engine/tests/facets.rs`](crates/fs-packages-engine/tests/facets.rs)
- [`crates/fs-packages-engine/tests/links.rs`](crates/fs-packages-engine/tests/links.rs)
- [`crates/fs-packages-engine/tests/own_files.rs`](crates/fs-packages-engine/tests/own_files.rs)
- [`crates/fs-packages-engine/tests/servers.rs`](crates/fs-packages-engine/tests/servers.rs)
- [`crates/fs-packages-engine/tests/switch.rs`](crates/fs-packages-engine/tests/switch.rs)
- [`crates/fs-packages-tuf-publish/tests/publish.rs`](crates/fs-packages-tuf-publish/tests/publish.rs)
