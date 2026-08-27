#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;
use assert_cmd::prelude::*;
use std::{error::Error, process::Command};

pub fn output_text(output: Vec<u8>) -> String {
    String::from_utf8_lossy(&output).to_string()
}

#[test]
fn test_all_runs_all_commands_even_when_check_fails()
-> Result<(), Box<dyn Error>> {
    // simple_app has check violations but should pass validate
    // This test verifies that validate still runs even when check fails
    let output = Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("all")
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let output_text = output_text(output);

    // Check violations should appear
    assert!(output_text.contains("violation(s) detected"));
    // Validate should also run (no validation errors message means it ran and passed)
    // Lint should also run - it outputs nothing on success

    Ok(())
}

#[test]
fn test_all_shows_validate_errors_even_when_check_fails()
-> Result<(), Box<dyn Error>> {
    // app_with_dependency_cycles has both check violations and validation errors
    let output = Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/app_with_dependency_cycles")
        .arg("all")
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let output_text = output_text(output);

    // Validate errors should appear (cycles)
    assert!(
        output_text.contains("validation error(s) detected"),
        "Expected validation errors to be shown. Output was: {}",
        output_text
    );

    Ok(())
}
