use sophia_config::ApplicationCatalogConfig;
use sophia_session::application_catalog::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let p = std::env::temp_dir().join(format!(
            "sophia-catalog-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn entry(&self, name: &str, contents: &str) {
        std::fs::write(self.0.join(name), contents).unwrap();
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn environment() -> ApplicationCatalogEnvironment {
    ApplicationCatalogEnvironment {
        search_path: vec!["/usr/bin".into(), "/bin".into()],
        locale: "sr_RS.UTF-8@latin".into(),
        current_desktop: vec!["Sophia".into()],
    }
}
fn config(sources: Vec<PathBuf>) -> ApplicationCatalogConfig {
    ApplicationCatalogConfig {
        name: "installed".into(),
        sources,
        applications: vec![],
        terminal: None,
        terminal_arguments: vec![],
    }
}
fn wait(worker: &mut ApplicationCatalogWorker) -> ApplicationCatalogWorkerResult {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        if let Some(result) = worker.poll() {
            return result;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
#[test]
fn exec_is_argv_and_field_codes_never_become_shell_evaluation() {
    let args = desktop_exec_arguments(
        r#"/bin/echo "hello world" %c %% %i %F"#,
        "Title;$(touch nope)",
        Some("editor"),
        Path::new("/x.desktop"),
    )
    .unwrap();
    assert_eq!(
        args,
        vec![
            "/bin/echo",
            "hello world",
            "Title;$(touch nope)",
            "%",
            "--icon",
            "editor"
        ]
    );
    for bad in [
        "/bin/echo ; touch nope",
        "/bin/echo $(touch nope)",
        "/bin/echo %Z",
        "/bin/echo %F %u",
        r#"/bin/echo "%c""#,
        "FOO=x /bin/echo",
    ] {
        assert!(
            desktop_exec_arguments(bad, "name", None, Path::new("/x.desktop")).is_none(),
            "{bad}"
        );
    }
}
#[test]
fn priority_visibility_locale_and_unsupported_activation_are_explicit() {
    let user = Directory::new();
    let system = Directory::new();
    system.entry(
        "hidden.desktop",
        "[Desktop Entry]\nType=Application\nName=Masked\nExec=/bin/true\n",
    );
    user.entry(
        "hidden.desktop",
        "[Desktop Entry]\nType=Application\nHidden=true\n",
    );
    user.entry("localized.desktop","[Desktop Entry]\nType=Application\nName=Base\nName[sr_RS@latin]=Lokalno\nExec=/bin/true\nOnlyShowIn=Sophia;\n");
    user.entry(
        "dbus.desktop",
        "[Desktop Entry]\nType=Application\nName=Bus\nExec=/bin/true\nDBusActivatable=true\n",
    );
    user.entry(
        "terminal.desktop",
        "[Desktop Entry]\nType=Application\nName=Terminal app\nExec=/bin/true\nTerminal=true\n",
    );
    user.entry("missing.desktop","[Desktop Entry]\nType=Application\nName=Missing\nExec=/bin/true\nTryExec=missing-sophia-test-executable\n");
    let catalog = build_application_catalog(
        &config(vec![user.0.clone(), system.0.clone()]),
        &[],
        &environment(),
    )
    .unwrap();
    assert_eq!(catalog.entries.len(), 3);
    assert!(
        catalog
            .entries
            .iter()
            .any(|e| e.descriptor.label == "Lokalno" && e.descriptor.available)
    );
    assert_eq!(
        catalog
            .entries
            .iter()
            .filter(|e| !e.descriptor.available)
            .count(),
        2
    );
}
#[test]
fn worker_rejects_changed_entry_and_new_priority_mask_at_dispatch() {
    let user = Directory::new();
    let system = Directory::new();
    system.entry(
        "app.desktop",
        "[Desktop Entry]\nType=Application\nName=App\nExec=/bin/true\n",
    );
    let mut worker = ApplicationCatalogWorker::start(
        config(vec![user.0.clone(), system.0.clone()]),
        vec![],
        environment(),
    )
    .unwrap();
    assert!(worker.refresh(1));
    assert!(!worker.refresh(2));
    let ApplicationCatalogWorkerResult::Built(1, Ok(catalog)) = wait(&mut worker) else {
        panic!("build failed");
    };
    let entry = catalog.entries[0].clone();
    assert!(worker.verify(2, entry.clone()));
    assert!(matches!(
        wait(&mut worker),
        ApplicationCatalogWorkerResult::Verified(2, Ok(_))
    ));
    user.entry(
        "app.desktop",
        "[Desktop Entry]\nType=Application\nHidden=true\n",
    );
    assert!(worker.verify(3, entry.clone()));
    assert!(matches!(
        wait(&mut worker),
        ApplicationCatalogWorkerResult::Verified(3, Err(_))
    ));
    std::fs::remove_file(user.0.join("app.desktop")).unwrap();
    system.entry(
        "app.desktop",
        "[Desktop Entry]\nType=Application\nName=App\nExec=/bin/false\n",
    );
    assert!(revalidate_catalog_entry(&entry).is_err());
}
#[test]
fn terminal_adapter_is_explicit_and_sources_cannot_escape_via_symlinks_or_fifos() {
    let source = Directory::new();
    let other = Directory::new();
    other.entry(
        "outside.desktop",
        "[Desktop Entry]\nType=Application\nName=Outside\nExec=/bin/true\n",
    );
    std::os::unix::fs::symlink(
        other.0.join("outside.desktop"),
        source.0.join("escape.desktop"),
    )
    .unwrap();
    rustix::fs::mknodat(
        rustix::fs::CWD,
        source.0.join("fifo.desktop"),
        rustix::fs::FileType::Fifo,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        0,
    )
    .unwrap();
    source.entry(
        "terminal.desktop",
        "[Desktop Entry]\nType=Application\nName=Terminal\nExec=/bin/true\nTerminal=true\n",
    );
    let mut config = config(vec![source.0.clone()]);
    config.terminal = Some("terminal".into());
    config.terminal_arguments = vec!["--".into()];
    let registry = vec![RegisteredCatalogApplication {
        name: "terminal".into(),
        command: ApplicationLaunchCommand {
            executable: "/bin/echo".into(),
            arguments: vec!["prefix".into()],
            working_directory: None,
        },
    }];
    let catalog = build_application_catalog(&config, &registry, &environment()).unwrap();
    assert_eq!(catalog.entries.len(), 1);
    assert_eq!(
        catalog.entries[0].command.as_ref().unwrap().arguments,
        vec!["prefix", "--", "/bin/true"]
    );
}
