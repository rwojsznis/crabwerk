#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn setup_project(tmp: &Path) {
    fs::write(tmp.join("package.yml"), "enforce_dependencies: false\n")
        .unwrap();
    fs::write(tmp.join("packs.yml"), "").unwrap();
}

fn write_pack(tmp: &Path, name: &str, contents: &str) {
    let pack_dir = tmp.join(name);
    fs::create_dir_all(&pack_dir).unwrap();
    fs::write(pack_dir.join("package.yml"), contents).unwrap();
}

fn pks_remove_dependency(
    tmp: &Path,
    from: &str,
    to: &str,
) -> assert_cmd::assert::Assert {
    Command::new(cargo_bin!("packs"))
        .arg("--project-root")
        .arg(tmp)
        .arg("remove-dependency")
        .arg(from)
        .arg(to)
        .assert()
}

#[test]
fn test_remove_dependency() {
    let tmp_dir = TempDir::new().unwrap();
    let tmp = tmp_dir.path();
    setup_project(tmp);

    write_pack(tmp, "packs/bar", "enforce_dependencies: true\n");
    write_pack(tmp, "packs/baz", "enforce_dependencies: true\n");
    write_pack(
        tmp,
        "packs/foo",
        "enforce_dependencies: true\ndependencies:\n- packs/bar\n- packs/baz\n",
    );

    pks_remove_dependency(tmp, "packs/foo", "packs/baz")
        .success()
        .stdout(predicate::str::contains(
            "Successfully removed `packs/baz` as a dependency from `packs/foo`!",
        ));

    let foo = fs::read_to_string(tmp.join("packs/foo/package.yml")).unwrap();
    assert_eq!(
        foo,
        "enforce_dependencies: true\ndependencies:\n- packs/bar\n"
    );
}

#[test]
fn test_remove_only_dependency() {
    let tmp_dir = TempDir::new().unwrap();
    let tmp = tmp_dir.path();
    setup_project(tmp);

    write_pack(tmp, "packs/bar", "enforce_dependencies: true\n");
    write_pack(
        tmp,
        "packs/foo",
        "enforce_dependencies: true\ndependencies:\n- packs/bar\n",
    );

    pks_remove_dependency(tmp, "packs/foo", "packs/bar").success();

    let foo = fs::read_to_string(tmp.join("packs/foo/package.yml")).unwrap();
    assert_eq!(foo, "enforce_dependencies: true\n");
}

#[test]
fn test_remove_dependency_that_does_not_exist_is_a_no_op() {
    let tmp_dir = TempDir::new().unwrap();
    let tmp = tmp_dir.path();
    setup_project(tmp);

    write_pack(tmp, "packs/bar", "enforce_dependencies: true\n");
    write_pack(tmp, "packs/foo", "enforce_dependencies: true\n");

    pks_remove_dependency(tmp, "packs/foo", "packs/bar")
        .success()
        .stdout(predicate::str::contains(
            "`packs/foo` does not depend on `packs/bar`!",
        ));

    let foo = fs::read_to_string(tmp.join("packs/foo/package.yml")).unwrap();
    assert_eq!(foo, "enforce_dependencies: true\n");
}

#[test]
fn test_remove_dependency_with_unknown_from_pack() {
    let tmp_dir = TempDir::new().unwrap();
    let tmp = tmp_dir.path();
    setup_project(tmp);

    write_pack(tmp, "packs/bar", "enforce_dependencies: true\n");

    pks_remove_dependency(tmp, "packs/nope", "packs/bar")
        .failure()
        .stderr(predicate::str::contains("`packs/nope` not found"));
}

#[test]
fn test_remove_dependency_with_unknown_to_pack() {
    let tmp_dir = TempDir::new().unwrap();
    let tmp = tmp_dir.path();
    setup_project(tmp);

    write_pack(tmp, "packs/foo", "enforce_dependencies: true\n");

    pks_remove_dependency(tmp, "packs/foo", "packs/nope")
        .failure()
        .stderr(predicate::str::contains("`packs/nope` not found"));
}
