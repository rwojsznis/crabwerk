use assert_cmd::Command;
#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;
use std::error::Error;

fn stderr_of(args: &[&str]) -> String {
    let output = Command::new(cargo_bin!("crabwerk"))
        .args(args)
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();

    String::from_utf8_lossy(&output).to_string()
}

#[test]
fn test_missing_project_root_reports_an_error() -> Result<(), Box<dyn Error>> {
    let stderr = stderr_of(&[
        "--project-root",
        "tests/fixtures/does_not_exist",
        "check",
    ]);

    assert!(
        stderr.starts_with("Error:"),
        "expected an `Error:` line, got: {}",
        stderr
    );
    assert!(
        stderr.contains("tests/fixtures/does_not_exist"),
        "expected the message to name the path, got: {}",
        stderr
    );

    Ok(())
}

#[test]
fn test_invalid_glob_in_configuration_reports_an_error()
-> Result<(), Box<dyn Error>> {
    let stderr = stderr_of(&[
        "--project-root",
        "tests/fixtures/app_with_invalid_glob",
        "check",
    ]);

    assert!(
        stderr.starts_with("Error:"),
        "expected an `Error:` line, got: {}",
        stderr
    );
    assert!(
        stderr.contains("**/*.{rb,erb"),
        "expected the message to name the pattern, got: {}",
        stderr
    );

    Ok(())
}
