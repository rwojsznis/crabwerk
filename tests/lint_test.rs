#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn setup_project(tmp: &Path) {
    fs::write(tmp.join("package.yml"), "enforce_dependencies: false\n")
        .unwrap();
    fs::write(tmp.join("crabwerk.yml"), "").unwrap();
}

fn write(tmp: &Path, relative_path: &str, contents: &str) {
    let full_path = tmp.join(relative_path);
    fs::create_dir_all(full_path.parent().unwrap()).unwrap();
    fs::write(full_path, contents).unwrap();
}

fn crabwerk_lint(tmp: &Path) -> assert_cmd::assert::Assert {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg(tmp)
        .arg("lint")
        .assert()
}

#[test]
fn test_lint_normalizes_package_yml() {
    let tmp_dir = TempDir::new().unwrap();
    let tmp = tmp_dir.path();
    setup_project(tmp);

    write(tmp, "packs/bar/package.yml", "enforce_dependencies: true\n");
    write(tmp, "packs/baz/package.yml", "enforce_dependencies: true\n");
    // Dependencies out of order, keys out of the canonical order
    write(
        tmp,
        "packs/foo/package.yml",
        "dependencies:\n- packs/baz\n- packs/bar\nenforce_privacy: true\nenforce_dependencies: true\n",
    );

    crabwerk_lint(tmp).success();

    let linted = fs::read_to_string(tmp.join("packs/foo/package.yml")).unwrap();
    assert_eq!(
        linted,
        "enforce_dependencies: true\nenforce_privacy: true\ndependencies:\n- packs/bar\n- packs/baz\n"
    );
}

#[test]
fn test_lint_normalizes_package_todo_yml() {
    let tmp_dir = TempDir::new().unwrap();
    let tmp = tmp_dir.path();
    setup_project(tmp);

    write(tmp, "packs/bar/package.yml", "enforce_dependencies: true\n");
    write(tmp, "packs/bar/app/services/bar.rb", "class Bar; end\n");
    write(tmp, "packs/foo/package.yml", "enforce_dependencies: true\n");
    write(
        tmp,
        "packs/foo/app/services/foo.rb",
        "class Foo\n  def call\n    Bar\n  end\nend\n",
    );
    // No header comment, violations out of order
    write(
        tmp,
        "packs/foo/package_todo.yml",
        "---\npacks/bar:\n  \"::Bar\":\n    violations:\n    - privacy\n    - dependency\n    files:\n    - packs/foo/app/services/foo.rb\n",
    );

    crabwerk_lint(tmp).success();

    let linted =
        fs::read_to_string(tmp.join("packs/foo/package_todo.yml")).unwrap();
    // The regeneration header is added back
    assert!(linted
        .contains("You can regenerate this file using the following command:"));
    assert!(linted.contains("# crabwerk update"));
    // Violations are sorted
    let dependency_index = linted.find("- dependency").unwrap();
    let privacy_index = linted.find("- privacy").unwrap();
    assert!(dependency_index < privacy_index);
}

#[test]
fn test_lint_leaves_packs_without_a_todo_alone() {
    let tmp_dir = TempDir::new().unwrap();
    let tmp = tmp_dir.path();
    setup_project(tmp);

    write(tmp, "packs/foo/package.yml", "enforce_dependencies: true\n");

    crabwerk_lint(tmp).success();

    assert!(!tmp.join("packs/foo/package_todo.yml").exists());
}
