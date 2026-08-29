#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;
use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::{error::Error, process::Command};

// Dropping the acronyms changes how constants resolve, so the user has to see
// it on an ordinary run rather than only when a log level is turned up.
#[test]
fn test_warns_when_acronyms_cannot_be_read() -> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/app_with_unreadable_inflections")
        .arg("check")
        .assert()
        .success()
        .stderr(predicate::str::contains("warning: could not read "))
        .stderr(predicate::str::contains(
            "config/initializers: Is a directory",
        ))
        .stderr(predicate::str::contains("Continuing without its acronyms."));

    Ok(())
}
