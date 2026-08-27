#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;
use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::{error::Error, fs, process::Command};

mod common;

#[test]
fn init_pack() -> Result<(), Box<dyn Error>> {
    let directory = "new_app";
    let rel_path = format!("tests/fixtures/{}", directory);
    common::create_new_app(directory);

    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg(rel_path.clone())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Created "))
        .stdout(predicate::str::contains(format!(
            "{}/crabwerk.yml'",
            rel_path
        )))
        .stdout(predicate::str::contains(format!(
            "{}/package.yml'",
            rel_path
        )));

    let expected = "validate the configuration using `crabwerk validate`";
    let actual = fs::read_to_string(format!("{}/package.yml", rel_path))
        .unwrap_or_else(|_| {
            panic!("Could not read file {}/package.yml", rel_path)
        });
    assert!(actual.contains(expected));

    common::delete_new_app(directory);

    Ok(())
}

#[test]
fn init_pack_with_packwerk() -> Result<(), Box<dyn Error>> {
    let directory = "new_app_with_packwerk";
    let rel_path = format!("tests/fixtures/{}", directory);
    common::create_new_app(directory);

    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg(rel_path.clone())
        .arg("init")
        .arg("--use-packwerk")
        .assert()
        .success()
        .stdout(predicate::str::contains("Created "))
        .stdout(predicate::str::contains(format!(
            "{}/packwerk.yml'",
            rel_path
        )))
        .stdout(predicate::str::contains(format!(
            "{}/package.yml'",
            rel_path
        )));

    let expected = "validate the configuration using `packwerk validate`";
    let actual = fs::read_to_string(format!("{}/package.yml", rel_path))
        .unwrap_or_else(|_| {
            panic!("Could not read file {}/package.yml", rel_path)
        });
    assert!(actual.contains(expected));

    common::delete_new_app(directory);

    Ok(())
}

#[test]
fn init_pack_when_package_yml_already_exists() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::TempDir::new()?;
    let tmp = temp_dir.path();
    fs::write(tmp.join("package.yml"), "enforce_dependencies: true\n")?;

    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg(tmp)
        .arg("init")
        .assert()
        .failure()
        .stdout(predicate::str::contains("package.yml` already exists!"))
        .stderr(predicate::str::contains("Could not initialize package.yml"));

    // The config file is not created when initialization bails
    assert!(!tmp.join("crabwerk.yml").exists());

    Ok(())
}

#[test]
fn init_pack_when_crabwerk_yml_already_exists() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::TempDir::new()?;
    let tmp = temp_dir.path();
    fs::write(tmp.join("crabwerk.yml"), "")?;

    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg(tmp)
        .arg("init")
        .assert()
        .failure()
        .stdout(predicate::str::contains("crabwerk.yml` already exists!"))
        .stderr(predicate::str::contains("Could not initialize"));

    // The root package.yml is not created when initialization bails
    assert!(!tmp.join("package.yml").exists());

    Ok(())
}
