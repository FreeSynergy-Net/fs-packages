//! A real server on a real Unix socket, asked through the generated client.

use std::sync::Arc;

use fs_interfaces::store_capnp::{PackageFormat, store};
use fs_packages_server::{LocalStore, StoreServer};
use fs_packages_sources::Registry;
use fs_rpc::{Caller, PerConnection, Shared, bind_socket, connect, serve_callers};

fn rpm_knows_nothing(_: &str) -> Option<String> {
    None
}

fn rpm_knows_tool(name: &str) -> Option<String> {
    (name == "tool").then(|| "2.0.0".to_owned())
}

async fn with_store<F, Fut>(root: &std::path::Path, lookup: fn(&str) -> Option<String>, test: F)
where
    F: FnOnce(store::Client) -> Fut,
    Fut: Future<Output = ()>,
{
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("store.sock");
            let view = Arc::new(LocalStore::new(root).with_rpm_lookup(lookup));
            let root = StoreServer::new(view).into_client();
            let bound = bind_socket(&path).await.unwrap();
            let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
            let serving =
                tokio::task::spawn_local(serve_callers(bound, Shared(root.client), async {
                    let _ = stopped.await;
                }));
            let connection = connect::<store::Client>(&path).await.unwrap();
            test(connection.client.clone()).await;
            drop(connection);
            let _ = stop.send(());
            serving.await.unwrap().unwrap();
        })
        .await;
}

fn register(root: &std::path::Path) {
    let mut registry = Registry::load(&root.join("installed.toml")).unwrap();
    registry.record("tool", "1.0.0", "https://example.invalid/catalog");
    registry.record("other", "0.3.0-rc.1", "https://example.invalid/catalog");
    registry.save().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn installed_comes_from_the_register() {
    let root = tempfile::tempdir().unwrap();
    register(root.path());
    with_store(root.path(), rpm_knows_nothing, |client| async move {
        let answer = client.installed_request().send().promise.await.unwrap();
        let list = answer.get().unwrap().get_list().unwrap();
        assert_eq!(list.len(), 2);
        let first = list.get(0);
        assert_eq!(first.get_name().unwrap().to_str().unwrap(), "tool");
        assert_eq!(first.get_version().unwrap().to_str().unwrap(), "1.0.0");
        assert_eq!(first.get_format().unwrap(), PackageFormat::TarGz);
        let second = list.get(1);
        assert_eq!(
            second.get_version().unwrap().to_str().unwrap(),
            "0.3.0-rc.1"
        );
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn the_system_package_manager_wins_over_the_register() {
    let root = tempfile::tempdir().unwrap();
    register(root.path());
    with_store(root.path(), rpm_knows_tool, |client| async move {
        let answer = client.installed_request().send().promise.await.unwrap();
        let list = answer.get().unwrap().get_list().unwrap();
        let tool = list.get(0);
        assert_eq!(tool.get_version().unwrap().to_str().unwrap(), "2.0.0");
        assert_eq!(tool.get_format().unwrap(), PackageFormat::Rpm);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn capabilities_name_the_reads_and_the_report_the_reasons() {
    let root = tempfile::tempdir().unwrap();
    with_store(root.path(), rpm_knows_nothing, |client| async move {
        let answer = client.capabilities_request().send().promise.await.unwrap();
        let ids = answer
            .get()
            .unwrap()
            .get_capabilities()
            .unwrap()
            .get_ids()
            .unwrap();
        let ids: Vec<_> = ids
            .iter()
            .map(|i| i.unwrap().to_str().unwrap().to_owned())
            .collect();
        assert_eq!(ids, ["source.read", "package.read"]);

        let answer = client
            .support_report_request()
            .send()
            .promise
            .await
            .unwrap();
        let report = answer.get().unwrap().get_report().unwrap();
        let entries = report.get_entries().unwrap();
        let install = entries
            .iter()
            .find(|e| e.get_capability().unwrap().to_str().unwrap() == "package.install")
            .unwrap();
        assert!(!install.get_supported());
        assert_ne!(install.get_reason().unwrap().to_str().unwrap(), "");
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn sources_list_the_reference_source() {
    let root = tempfile::tempdir().unwrap();
    with_store(root.path(), rpm_knows_nothing, |client| async move {
        let answer = client.sources_request().send().promise.await.unwrap();
        let list = answer.get().unwrap().get_list().unwrap();
        assert_ne!(list.len(), 0);
        assert!(list.get(0).get_reference());
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn writes_answer_unsupported_and_change_nothing() {
    let root = tempfile::tempdir().unwrap();
    register(root.path());
    with_store(root.path(), rpm_knows_nothing, |client| async move {
        let mut request = client.install_request();
        request.get().set_name("tool");
        let error = request.send().promise.await.err().unwrap();
        assert_eq!(error.kind, capnp::ErrorKind::Unimplemented);
        assert!(error.to_string().contains("package.install"), "{error}");
        assert!(
            error.to_string().contains("deprecated"),
            "K7: the reason names the form source command: {error}"
        );

        let mut request = client.add_source_request();
        request.get().set_url("https://example.invalid/x");
        assert!(request.send().promise.await.is_err());
        assert!(client.job_request().send().promise.await.is_err());
    })
    .await;
    assert!(!root.path().join("sources.toml").exists());
}

/// Pattern: Adapter (a `PerConnection` that writes down who was asked for) - proves the store's server gets the caller
/// from the system on every connection, the way the write commands will need it (ADR 0054).
struct Noting {
    client: capnp::capability::Client,
    seen: std::rc::Rc<std::cell::RefCell<Vec<Caller>>>,
}

impl PerConnection for Noting {
    fn bootstrap(&mut self, caller: &Caller) -> capnp::capability::Client {
        self.seen.borrow_mut().push(caller.clone());
        self.client.clone()
    }
}

#[tokio::test(flavor = "current_thread")]
async fn the_transport_tells_the_server_who_is_on_each_connection() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("store.sock");
            let root = tempfile::tempdir().unwrap();
            let view = Arc::new(LocalStore::new(root.path()).with_rpm_lookup(rpm_knows_nothing));
            let seen = std::rc::Rc::default();
            let per = Noting {
                client: StoreServer::new(view).into_client().client,
                seen: std::rc::Rc::clone(&seen),
            };
            let bound = bind_socket(&path).await.unwrap();
            let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
            let serving = tokio::task::spawn_local(serve_callers(bound, per, async {
                let _ = stopped.await;
            }));
            for _ in 0..2 {
                let connection = connect::<store::Client>(&path).await.unwrap();
                let answer = connection.client.sources_request().send().promise.await;
                assert!(answer.is_ok());
            }
            let _ = stop.send(());
            serving.await.unwrap().unwrap();
            let seen = seen.borrow();
            assert_eq!(seen.len(), 2, "one caller per connection");
            assert!(seen.iter().all(|c| !c.id().is_empty()));
        })
        .await;
}
