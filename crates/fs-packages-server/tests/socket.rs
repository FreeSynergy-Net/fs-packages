//! A real socket: the guarded store behind `serve_callers`, asked through the generated clients. The socket lives in a runtime directory of
//! its own (mode 0700, made fresh, never the one of the user's session): a probe must not touch the session of a person.
#![cfg(unix)]

mod support;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use fs_interfaces::formsource_capnp::form_source;
use fs_interfaces::store_capnp::store;
use fs_rpc::{bind_socket, connect, serve_callers};
use support::{fake_service, ids, invoke};

/// A fresh runtime directory below `$FS_TEST_RUNTIME_ROOT` (default: the temp dir), checked BEFORE use: new, set to mode 0700 (checked), and neither the
/// session's `$XDG_RUNTIME_DIR` nor anything below `/run/user`.
fn own_runtime_dir() -> tempfile::TempDir {
    let root =
        std::env::var_os("FS_TEST_RUNTIME_ROOT").map_or_else(std::env::temp_dir, PathBuf::from);
    let dir = tempfile::Builder::new()
        .prefix("stw2a-xdg-")
        .tempdir_in(root)
        .unwrap();
    let path = dir.path().canonicalize().unwrap();
    assert!(
        !path.starts_with("/run/user"),
        "never the session of the user: {}",
        path.display()
    );
    if let Some(session) = std::env::var_os("XDG_RUNTIME_DIR") {
        assert_ne!(
            Path::new(&session).canonicalize().ok().as_deref(),
            Some(path.as_path()),
            "collision with the session"
        );
    }
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700, "the runtime dir must be private");
    dir
}

#[tokio::test(flavor = "current_thread")]
async fn the_contract_and_the_form_source_answer_on_one_socket_and_the_caller_comes_from_the_system()
 {
    let runtime = own_runtime_dir();
    let dir = runtime.path().join("freesynergy");
    std::fs::create_dir(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = dir.join("store.sock");
    assert!(!path.exists(), "no collision at the socket path");
    tokio::task::LocalSet::new()
        .run_until(async {
            let bound = bind_socket(&path).await.unwrap();
            let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
            let serving = tokio::task::spawn_local(serve_callers(bound, fake_service(), async {
                let _ = stopped.await;
            }));
            // The contract `store` on the socket, as before.
            let hub = connect::<store::Client>(&path).await.unwrap();
            let caps = hub
                .client
                .capabilities_request()
                .send()
                .promise
                .await
                .unwrap();
            assert!(caps.get().unwrap().has_capabilities());
            // The form source on the same socket: the system says the caller is the own user, so a person.
            let form = connect::<form_source::Client>(&path).await.unwrap();
            let done = invoke(&form.client, "package.install", "tool", &[]).await;
            assert!(!done.failed, "{done:?}");
            assert_eq!(
                ids(&form.client, "job").await,
                ["fake-1"],
                "the job is a record"
            );
            assert_eq!(
                invoke(&form.client, "package.nope", "tool", &[])
                    .await
                    .status,
                403
            );
            drop((hub, form));
            let _ = stop.send(());
            serving.await.unwrap().unwrap();
        })
        .await;
}
