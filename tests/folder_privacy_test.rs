#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;
use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::{error::Error, process::Command};

#[test]
fn test_check() -> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/folder_privacy_violations")
        .arg("--debug")
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains("Folder Privacy violation: `::Foo` belongs to `packs/foos/foo`, which is private to `packs/baz` as it is not a sibling pack or parent pack."));

    Ok(())
}

#[test]
fn test_check_enforce_folder_privacy_disabled() -> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/folder_privacy_violations")
        .arg("--debug")
        .arg("--disable-enforce-folder-privacy")
        .arg("check")
        .assert()
        .success();

    Ok(())
}

#[test]
fn test_invisible_pack_violation_with_deprecated_enforce_folder_visibility()
-> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/folder_visibility_violations")
        .arg("--debug")
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains("Folder Privacy violation: `::Foo` belongs to `packs/foos/foo`, which is private to `packs/baz` as it is not a sibling pack or parent pack."));

    Ok(())
}

// `packs/foo_bar/nested` is not below `packs/foo`, even though its path
// starts with the same characters.
#[test]
fn test_check_pack_name_that_is_a_prefix_of_another()
-> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/folder_privacy_prefix_pack_names")
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains("Folder Privacy violation: `::Bar` belongs to `packs/foo_bar/nested`, which is private to `packs/foo` as it is not a sibling pack or parent pack."))
        .stdout(predicate::str::contains("Folder Privacy violation: `::Quux` belongs to `packs/qux/nested`, which is private to `packs/foo` as it is not a sibling pack or parent pack."));

    Ok(())
}
