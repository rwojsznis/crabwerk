#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;
use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::{error::Error, process::Command};

#[test]
fn test_list_included_files() -> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("list-included-files")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "simple_app/packs/foo/app/services/foo.rb",
        ))
        .stdout(predicate::str::contains(
            "simple_app/packs/bar/app/services/bar.rb",
        ))
        .stdout(predicate::str::contains(
            "simple_app/packs/foo/app/views/foo.erb",
        ))
        .stdout(predicate::str::contains(
            "simple_app/app/services/some_root_class.rb",
        ))
        // `node_modules` and `script` are excluded by the default `exclude` globs
        .stdout(predicate::str::contains("node_modules").not())
        .stdout(predicate::str::contains("script/my_script.rb").not());

    Ok(())
}
