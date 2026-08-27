#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;
use predicates::prelude::*;
use pretty_assertions::assert_eq;
use serial_test::serial;
use std::{collections::HashSet, path::PathBuf};

mod common;

#[test]
#[serial]
fn test_add_constant_dependencies() -> anyhow::Result<()> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/app_with_missing_dependencies")
        .arg("update-dependencies-for-constant")
        .arg("::Bar::Tender")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Successfully updated 1 dependency for constant '::Bar::Tender'",
        ));

    let config = crabwerk::configuration(
        PathBuf::from("tests/fixtures/app_with_missing_dependencies"),
        &0,
    )
    .unwrap();

    let pack = config.pack_set.for_pack("packs/foo").unwrap();
    assert_eq!(pack.dependencies.len(), 0);

    let pack = config.pack_set.for_pack("packs/baz").unwrap();
    assert_eq!(pack.dependencies.len(), 1);

    let mut expected = HashSet::new();
    expected.insert("packs/bar".to_owned());
    assert_eq!(pack.dependencies, expected);
    common::set_up_fixtures();

    Ok(())
}

#[test]
#[serial]
fn test_add_constant_dependencies_no_dependencies() -> anyhow::Result<()> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/app_with_missing_dependencies")
        .arg("update-dependencies-for-constant")
        .arg("::Bar::Nope")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "No dependencies to update for constant '::Bar::Nope'",
        ));

    Ok(())
}

#[test]
#[serial]
fn test_add_constant_dependencies_for_multiple_packs() -> anyhow::Result<()> {
    let temp_dir = tempfile::TempDir::new()?;
    let tmp = temp_dir.path();

    std::fs::write(tmp.join("package.yml"), "enforce_dependencies: true\n")?;
    std::fs::write(tmp.join("crabwerk.yml"), "")?;

    for pack in ["bar", "foo", "baz"] {
        let dir = tmp.join("packs").join(pack).join("app/services");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(
            tmp.join("packs").join(pack).join("package.yml"),
            "enforce_dependencies: true\n",
        )?;
    }

    let tender_dir = tmp.join("packs/bar/app/public/bar");
    std::fs::create_dir_all(&tender_dir)?;
    std::fs::write(
        tender_dir.join("tender.rb"),
        "module Bar\n  class Tender\n  end\nend\n",
    )?;

    // Both `packs/foo` and `packs/baz` reference `::Bar::Tender` without
    // declaring a dependency on `packs/bar`
    std::fs::write(
        tmp.join("packs/foo/app/services/foo.rb"),
        "class Foo\n  def x\n    Bar::Tender\n  end\nend\n",
    )?;
    std::fs::write(
        tmp.join("packs/baz/app/services/baz.rb"),
        "class Baz\n  def x\n    Bar::Tender\n  end\nend\n",
    )?;

    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg(tmp)
        .arg("update-dependencies-for-constant")
        .arg("::Bar::Tender")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Successfully updated 2 dependencies for constant '::Bar::Tender'",
        ));

    for pack in ["foo", "baz"] {
        let contents = std::fs::read_to_string(
            tmp.join("packs").join(pack).join("package.yml"),
        )?;
        assert_eq!(
            contents,
            "enforce_dependencies: true\ndependencies:\n- packs/bar\n",
            "unexpected package.yml for packs/{}",
            pack
        );
    }

    Ok(())
}
