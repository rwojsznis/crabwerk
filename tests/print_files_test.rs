#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;
use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::{error::Error, process::Command};

#[test]
fn test_print_files() -> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--print-files")
        .arg("--experimental-parser")
        .arg("list-definitions")
        .assert()
        .success()
        .stdout(predicate::str::contains("Started processing"))
        .stdout(predicate::str::contains("Finished processing"))
        .stdout(predicate::str::contains(
            "simple_app/packs/foo/app/services/foo.rb",
        ));

    Ok(())
}

// Files that are neither Ruby nor ERB are still reported by `--print-files`,
// and produce an empty ProcessedFile rather than an error.
#[test]
fn test_print_files_with_unsupported_file_type() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::TempDir::new()?;
    let tmp = temp_dir.path();
    std::fs::write(tmp.join("package.yml"), "enforce_dependencies: false\n")?;
    std::fs::write(
        tmp.join("crabwerk.yml"),
        "include:\n- \"**/*.rb\"\n- \"**/*.txt\"\n",
    )?;
    std::fs::write(tmp.join("notes.txt"), "not ruby\n")?;

    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg(tmp)
        .arg("--print-files")
        .arg("--experimental-parser")
        .arg("list-definitions")
        .assert()
        .success()
        .stdout(predicate::str::contains("Started processing"))
        .stdout(predicate::str::contains("notes.txt"));

    Ok(())
}
