#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;
use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::{error::Error, fs, process::Command};

const LEGACY_FIXTURE: &str = "tests/fixtures/legacy_packwerk_config";

#[test]
fn test_migrate_config_writes_crabwerk_yml() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::TempDir::new()?;
    let tmp = temp_dir.path();
    let packwerk_yml_contents =
        fs::read_to_string(format!("{}/packwerk.yml", LEGACY_FIXTURE))?;
    fs::write(tmp.join("packwerk.yml"), &packwerk_yml_contents)?;
    fs::write(tmp.join("package.yml"), "enforce_dependencies: true\n")?;

    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg(tmp)
        .arg("migrate-config")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Created `crabwerk.yml` from `packwerk.yml`",
        ))
        .stdout(predicate::str::contains("was left in place"));

    // The migration copies the configuration verbatim, so that `diff` between
    // the two files stays empty while a repo runs both tools.
    assert_eq!(
        fs::read_to_string(tmp.join("crabwerk.yml"))?,
        packwerk_yml_contents
    );
    assert!(tmp.join("packwerk.yml").exists());

    Ok(())
}

#[test]
fn test_migrate_config_output_is_read_by_later_commands()
-> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::TempDir::new()?;
    let tmp = temp_dir.path();
    fs::write(
        tmp.join("packwerk.yml"),
        "exclude:\n- \"app/ignored/**/*\"\n",
    )?;
    fs::write(tmp.join("package.yml"), "enforce_dependencies: true\n")?;
    fs::create_dir_all(tmp.join("app/services"))?;
    fs::create_dir_all(tmp.join("app/ignored"))?;
    fs::write(tmp.join("app/services/foo.rb"), "class Foo\nend\n")?;
    fs::write(tmp.join("app/ignored/bar.rb"), "class Bar\nend\n")?;

    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg(tmp)
        .arg("migrate-config")
        .assert()
        .success();

    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg(tmp)
        .arg("list-included-files")
        .assert()
        .success()
        .stdout(predicate::str::contains("app/services/foo.rb"))
        .stdout(predicate::str::contains("app/ignored/bar.rb").not());

    Ok(())
}

#[test]
fn test_migrate_config_when_crabwerk_yml_already_exists()
-> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::TempDir::new()?;
    let tmp = temp_dir.path();
    fs::write(tmp.join("packwerk.yml"), "package_paths: packs/*\n")?;
    fs::write(tmp.join("crabwerk.yml"), "package_paths: components/*\n")?;

    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg(tmp)
        .arg("migrate-config")
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));

    // The existing configuration is never overwritten
    assert_eq!(
        fs::read_to_string(tmp.join("crabwerk.yml"))?,
        "package_paths: components/*\n"
    );

    Ok(())
}

#[test]
fn test_migrate_config_when_there_is_no_packwerk_yml()
-> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::TempDir::new()?;
    let tmp = temp_dir.path();

    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg(tmp)
        .arg("migrate-config")
        .assert()
        .failure()
        .stderr(predicate::str::contains("There is no `packwerk.yml`"));

    assert!(!tmp.join("crabwerk.yml").exists());

    Ok(())
}

#[test]
fn test_migrate_config_when_packwerk_yml_does_not_parse()
-> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::TempDir::new()?;
    let tmp = temp_dir.path();
    fs::write(tmp.join("packwerk.yml"), "package_paths: 5\n")?;

    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg(tmp)
        .arg("migrate-config")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Could not parse"));

    // Nothing is written when the source configuration is invalid
    assert!(!tmp.join("crabwerk.yml").exists());

    Ok(())
}

#[test]
fn test_packwerk_yml_alone_is_an_error() -> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg(LEGACY_FIXTURE)
        .arg("list-included-files")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "crabwerk does not read `packwerk.yml`",
        ))
        .stderr(predicate::str::contains("crabwerk migrate-config"));

    Ok(())
}

#[test]
fn test_config_flag_reads_the_named_file() -> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg(LEGACY_FIXTURE)
        .arg("--config")
        .arg("packwerk.yml")
        .arg("list-included-files")
        .assert()
        .success()
        .stdout(predicate::str::contains("app/services/foo.rb"))
        // `exclude` in the named file is honoured
        .stdout(predicate::str::contains("app/ignored/bar.rb").not());

    Ok(())
}

#[test]
fn test_config_flag_after_the_subcommand() -> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg(LEGACY_FIXTURE)
        .arg("list-included-files")
        .arg("--config")
        .arg("packwerk.yml")
        .assert()
        .success()
        .stdout(predicate::str::contains("app/services/foo.rb"));

    Ok(())
}

#[test]
fn test_config_flag_with_a_missing_file() -> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg(LEGACY_FIXTURE)
        .arg("--config")
        .arg("nope.yml")
        .arg("list-included-files")
        .assert()
        .failure()
        .stderr(predicate::str::contains("nope.yml"));

    Ok(())
}
