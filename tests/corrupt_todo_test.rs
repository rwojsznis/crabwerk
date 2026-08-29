use assert_cmd::Command;
#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;
use predicates::prelude::*;

#[test]
fn test_check_with_corrupt_todo() -> anyhow::Result<()> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/contains_corrupt_todo")
        .arg("check")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to deserialize the package_todo.yml"))
        .stderr(predicate::str::contains("Try deleting the file and running the `update` command to regenerate it"));

    Ok(())
}
