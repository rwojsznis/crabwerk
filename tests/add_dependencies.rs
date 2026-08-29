#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;
use assert_cmd::prelude::*;
use crabwerk::pack::Pack;
use std::{error::Error, fs, process::Command};
mod common;

#[test]
fn test_check_add_dependencies() -> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/app_with_missing_dependencies")
        .arg("--debug")
        .arg("add-dependencies")
        .arg("packs/baz")
        .assert()
        .success();

    let after_pack: Pack = crabwerk::yaml::from_str(
    &fs::read_to_string("tests/fixtures/app_with_missing_dependencies/packs/baz/package.yml")
        .expect("Failed to read package.yml"),
)
.expect("Failed to deserialize package.yml");

    let expected_dependencies: std::collections::HashSet<String> =
        vec!["packs/bar".to_string()].into_iter().collect();

    assert_eq!(after_pack.dependencies, expected_dependencies);

    common::set_up_fixtures();

    Ok(())
}
