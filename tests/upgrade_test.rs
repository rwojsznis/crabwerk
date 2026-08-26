#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;
use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::{error::Error, process::Command};

// The test binary lives in `target/debug`, never under `$CARGO_HOME/bin`, so
// `upgrade` must refuse to shell out to `cargo install`.
#[test]
fn test_upgrade_refuses_when_not_installed_via_cargo_install(
) -> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("pks"))
        .arg("upgrade")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "`pks upgrade` only works when pks was installed via `cargo install`.",
        ))
        .stderr(predicate::str::contains("Current executable:"))
        .stderr(predicate::str::contains("Expected location:"));

    Ok(())
}

// `upgrade` is handled before configuration is loaded, so it does not need a
// project root with a packwerk.yml/packs.yml in it.
#[test]
fn test_upgrade_does_not_require_configuration() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::TempDir::new()?;

    Command::new(cargo_bin!("pks"))
        .arg("--project-root")
        .arg(temp_dir.path())
        .arg("upgrade")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "`pks upgrade` only works when pks was installed via `cargo install`.",
        ))
        // If configuration had been loaded first, we would see a config error instead
        .stderr(predicate::str::contains("No root pack found").not());

    Ok(())
}

// When CARGO_HOME is not set, the expected install location falls back to
// `$HOME/.cargo/bin`.
#[test]
fn test_upgrade_falls_back_to_home_when_cargo_home_is_unset(
) -> Result<(), Box<dyn Error>> {
    let fake_home = tempfile::TempDir::new()?;

    Command::new(cargo_bin!("pks"))
        .env_remove("CARGO_HOME")
        .env("HOME", fake_home.path())
        .arg("upgrade")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "`pks upgrade` only works when pks was installed via `cargo install`.",
        ))
        .stderr(predicate::str::contains(
            fake_home.path().join(".cargo/bin").to_str().unwrap(),
        ));

    Ok(())
}
