use assert_cmd::prelude::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

fn cargo_coil() -> Command {
    Command::cargo_bin("cargo-coil").unwrap()
}

#[test]
fn new_and_doctor_work_non_interactively() {
    let workspace = tempdir().unwrap();
    let root = workspace.path().join("my-store");

    cargo_coil()
        .current_dir(workspace.path())
        .args([
            "new",
            root.to_str().unwrap(),
            "--non-interactive",
            "--name",
            "my-store",
            "--display-name",
            "My Store",
            "--default-locale",
            "en-GB",
            "--locale",
            "fr-FR",
            "--framework-version",
            "0.1.0",
        ])
        .assert()
        .success();

    assert!(root.join(".coil/project.toml").exists());
    assert!(root.join("Cargo.toml").exists());

    cargo_coil()
        .current_dir(workspace.path())
        .args(["doctor", "--root", root.to_str().unwrap()])
        .assert()
        .success();

    let descriptor = fs::read_to_string(root.join(".coil/project.toml")).unwrap();
    assert!(descriptor.contains("framework_version = \"0.1.0\""));
}

#[test]
fn init_apply_module_site_and_locale_commands_work_non_interactively() {
    let workspace = tempdir().unwrap();
    let root = workspace.path().join("shop");
    fs::create_dir_all(&root).unwrap();

    cargo_coil()
        .current_dir(workspace.path())
        .args([
            "init",
            "--root",
            root.to_str().unwrap(),
            "--non-interactive",
            "--name",
            "shop",
            "--display-name",
            "Shop",
            "--default-locale",
            "en-GB",
            "--framework-version",
            "0.1.0",
        ])
        .assert()
        .success();

    cargo_coil()
        .current_dir(workspace.path())
        .args(["module", "add", "--root", root.to_str().unwrap(), "memberships"])
        .assert()
        .success();

    cargo_coil()
        .current_dir(workspace.path())
        .args(["module", "remove", "--root", root.to_str().unwrap(), "memberships"])
        .assert()
        .success();

    cargo_coil()
        .current_dir(workspace.path())
        .args([
            "site",
            "add",
            "--root",
            root.to_str().unwrap(),
            "eu",
            "--display-name",
            "EU Store",
            "--brand-name",
            "Shop",
            "--canonical-domain",
            "eu.shop.localhost",
            "--domain",
            "www.eu.shop.localhost",
            "--default-locale",
            "fr-FR",
        ])
        .assert()
        .success();

    cargo_coil()
        .current_dir(workspace.path())
        .args([
            "locale",
            "add",
            "--root",
            root.to_str().unwrap(),
            "de-DE",
            "--site",
            "eu",
        ])
        .assert()
        .success();

    cargo_coil()
        .current_dir(workspace.path())
        .args(["apply", "--root", root.to_str().unwrap()])
        .assert()
        .success();

    cargo_coil()
        .current_dir(workspace.path())
        .args(["doctor", "--root", root.to_str().unwrap()])
        .assert()
        .success();

    let descriptor = fs::read_to_string(root.join(".coil/project.toml")).unwrap();
    assert!(descriptor.contains("id = \"eu\""));
    assert!(descriptor.contains("\"fr-FR\""));
    assert!(descriptor.contains("\"de-DE\""));
    assert!(!descriptor.contains("\"memberships\""));
}

#[test]
fn dev_no_watch_runs_docker_infra_and_customer_binary_with_expected_env() {
    let workspace = tempdir().unwrap();
    let root = workspace.path().join("my-store");

    cargo_coil()
        .current_dir(workspace.path())
        .args([
            "new",
            root.to_str().unwrap(),
            "--non-interactive",
            "--name",
            "my-store",
            "--display-name",
            "My Store",
            "--default-locale",
            "en-GB",
            "--framework-version",
            "0.1.0",
        ])
        .assert()
        .success();

    let fake_bin_dir = workspace.path().join("fake-bin");
    fs::create_dir_all(&fake_bin_dir).unwrap();
    let log_path = workspace.path().join("dev.log");
    let docker_bin = write_fake_tool(&fake_bin_dir, "docker");
    let cargo_bin = write_fake_tool(&fake_bin_dir, "cargo");

    cargo_coil()
        .current_dir(workspace.path())
        .env("COIL_DEV_DOCKER_BIN", &docker_bin)
        .env("COIL_DEV_CARGO_BIN", &cargo_bin)
        .env("COIL_TEST_LOG", &log_path)
        .args(["dev", "--root", root.to_str().unwrap(), "--no-watch"])
        .assert()
        .success();

    let log = fs::read_to_string(log_path).unwrap();
    assert!(log.contains("ARGS:compose up -d postgres redis"));
    assert!(log.contains("ARGS:run -p my-store -- --config platform.dev.toml up"));
    assert!(log.contains("ENV:DATABASE_URL=postgres://coil:coil@127.0.0.1:15432/my-store"));
    assert!(log.contains("ENV:REDIS_URL=redis://127.0.0.1:16379/0"));
    assert!(log.contains("ENV:COIL_COOKIE_SECRET=local-development-cookie-secret"));
    assert!(log.contains("ENV:COIL_CSRF_SECRET=local-development-csrf-secret"));
}

fn write_fake_tool(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    let script = format!(
        "#!/bin/sh\n\
echo \"ARGS:$*\" >> \"$COIL_TEST_LOG\"\n\
echo \"ENV:DATABASE_URL=$DATABASE_URL\" >> \"$COIL_TEST_LOG\"\n\
echo \"ENV:REDIS_URL=$REDIS_URL\" >> \"$COIL_TEST_LOG\"\n\
echo \"ENV:COIL_COOKIE_SECRET=$COIL_COOKIE_SECRET\" >> \"$COIL_TEST_LOG\"\n\
echo \"ENV:COIL_CSRF_SECRET=$COIL_CSRF_SECRET\" >> \"$COIL_TEST_LOG\"\n\
exit 0\n"
    );
    fs::write(&path, script).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).unwrap();
    path
}
