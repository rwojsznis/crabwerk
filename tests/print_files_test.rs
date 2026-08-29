#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;
use assert_cmd::prelude::*;
use std::{error::Error, process::Command};

#[test]
fn test_print_files_is_not_a_valid_option() -> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--print-files")
        .arg("list-definitions")
        .assert()
        .failure();

    Ok(())
}
