#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;
use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::{error::Error, process::Command};

#[test]
fn test_list_pack_dependencies_with_explicit_dependencies()
-> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("list-pack-dependencies")
        .arg("packs/baz")
        .assert()
        .success()
        .stdout(predicate::str::contains("Explicit (1):"))
        .stdout(predicate::str::contains("packs/foo"));

    Ok(())
}

#[test]
fn list_pack_dependencies_with_implicit_dependencies()
-> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/contains_package_todo")
        .arg("list-pack-dependencies")
        .arg("packs/bar")
        .assert()
        .success()
        .stdout(predicate::str::contains("Explicit (0):"))
        .stdout(predicate::str::contains("packs/foo"))
        // `::Bar` is recorded against two files in the `packs/foo` todo file,
        // and each file is one violation.
        .stdout(predicate::str::contains("dependency: 2"));

    Ok(())
}
