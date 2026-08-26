#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;
use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::{error::Error, process::Command};
mod common;

#[test]
fn test_greet() -> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("packs"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("greet")
        .assert()
        .success()
        .stdout(predicate::str::contains("Welcome to packs"));

    common::teardown();
    Ok(())
}
