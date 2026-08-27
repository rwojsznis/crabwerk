use assert_cmd::Command;
#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;
use predicates::prelude::*;
use serde_json::Value;
use std::error::Error;

pub fn output_text(output: Vec<u8>) -> String {
    String::from_utf8_lossy(&output).to_string()
}

#[test]
fn test_check() -> Result<(), Box<dyn Error>> {
    let output = Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--debug")
        .arg("check")
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let output_text = output_text(output);

    assert!(output_text.contains("2 violation(s) detected:"));
    assert!(output_text.contains("packs/foo/app/services/foo.rb:3:4\nDependency violation: `::Bar` belongs to `packs/bar`, but `packs/foo/package.yml` does not specify a dependency on `packs/bar`."));
    assert!(output_text.contains("packs/foo/app/services/foo.rb:3:4\nPrivacy violation: `::Bar` is private to `packs/bar`, but referenced from `packs/foo`"));

    Ok(())
}

#[test]
fn test_check_enforce_privacy_disabled() -> Result<(), Box<dyn Error>> {
    let output = Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--debug")
        .arg("--disable-enforce-privacy")
        .arg("check")
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let output_text = output_text(output);

    assert!(output_text.contains("1 violation(s) detected:"));
    assert!(output_text.contains("packs/foo/app/services/foo.rb:3:4\nDependency violation: `::Bar` belongs to `packs/bar`, but `packs/foo/package.yml` does not specify a dependency on `packs/bar`."));

    Ok(())
}

#[test]
fn test_check_enforce_dependency_disabled() -> Result<(), Box<dyn Error>> {
    let output = Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--debug")
        .arg("--disable-enforce-dependencies")
        .arg("check")
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let output_text = output_text(output);

    assert!(output_text.contains("1 violation(s) detected:"));
    assert!(output_text.contains("packs/foo/app/services/foo.rb:3:4\nPrivacy violation: `::Bar` is private to `packs/bar`, but referenced from `packs/foo`"));

    Ok(())
}

#[test]
fn test_check_with_single_file() -> Result<(), Box<dyn Error>> {
    let output = Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--debug")
        .arg("check")
        .arg("packs/foo/app/services/foo.rb")
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let output_text = output_text(output);

    assert!(output_text.contains("2 violation(s) detected:"));
    assert!(output_text.contains("packs/foo/app/services/foo.rb:3:4\nDependency violation: `::Bar` belongs to `packs/bar`, but `packs/foo/package.yml` does not specify a dependency on `packs/bar`."));
    assert!(output_text.contains("packs/foo/app/services/foo.rb:3:4\nPrivacy violation: `::Bar` is private to `packs/bar`, but referenced from `packs/foo`"));

    Ok(())
}

#[test]
fn test_check_with_single_file_experimental_parser()
-> Result<(), Box<dyn Error>> {
    let output = Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--debug")
        .arg("--experimental-parser")
        .arg("check")
        .arg("packs/foo/app/services/foo.rb")
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let output_text = output_text(output);

    assert!(output_text.contains("2 violation(s) detected:"));
    assert!(output_text.contains("packs/foo/app/services/foo.rb:3:4\nDependency violation: `::Bar` belongs to `packs/bar`, but `packs/foo/package.yml` does not specify a dependency on `packs/bar`."));
    assert!(output_text.contains("packs/foo/app/services/foo.rb:3:4\nPrivacy violation: `::Bar` is private to `packs/bar`, but referenced from `packs/foo`"));

    Ok(())
}

#[test]
fn test_check_with_package_todo_file() -> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/contains_package_todo")
        .arg("--debug")
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains("No violations detected!"));

    Ok(())
}

#[test]
fn test_check_with_package_todo_file_ignoring_recorded_violations()
-> Result<(), Box<dyn Error>> {
    let output = Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/contains_package_todo")
        .arg("--debug")
        .arg("check")
        .arg("--ignore-recorded-violations")
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let output_text = output_text(output);
    assert!(output_text.contains("2 violation(s) detected:"));
    assert!(output_text.contains("packs/foo/app/services/foo.rb:3:4\nDependency violation: `::Bar` belongs to `packs/bar`, but `packs/foo/package.yml` does not specify a dependency on `packs/bar`."));
    assert!(output_text.contains("packs/foo/app/services/other_foo.rb:3:4\nDependency violation: `::Bar` belongs to `packs/bar`, but `packs/foo/package.yml` does not specify a dependency on `packs/bar`."));

    Ok(())
}

#[test]
fn test_check_with_experimental_parser() -> Result<(), Box<dyn Error>> {
    let output = Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--experimental-parser")
        .arg("--debug")
        .arg("check")
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let output_text = output_text(output);

    assert!(output_text.contains("2 violation(s) detected:"));
    assert!(output_text.contains("packs/foo/app/services/foo.rb:3:4\nDependency violation: `::Bar` belongs to `packs/bar`, but `packs/foo/package.yml` does not specify a dependency on `packs/bar`."));
    assert!(output_text.contains("packs/foo/app/services/foo.rb:3:4\nPrivacy violation: `::Bar` is private to `packs/bar`, but referenced from `packs/foo`"));

    Ok(())
}

#[test]
fn test_check_with_stale_violations() -> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/contains_stale_violations")
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "There were stale violations found, please run `crabwerk update`",
        ));

    Ok(())
}

#[test]
fn test_check_with_stale_violations_when_file_no_longer_exists()
-> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/contains_stale_violations_no_file")
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "There were stale violations found, please run `crabwerk update`",
        ));

    Ok(())
}

#[test]
fn test_check_with_relationship_violations() -> Result<(), Box<dyn Error>> {
    // Tests that associations with explicit class_name (using .name) are correctly resolved
    // The fixture has:
    //   has_many :censuses       -> Census
    //   has_many :tacos          -> Taco
    //   belongs_to :my_widget, class_name: Census.name  -> Census (NOT MyWidget)
    // Plus a direct reference to Census in the class_name argument itself
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/app_with_rails_relationships")
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains("4 violation(s) detected:"))
        .stdout(predicate::str::contains("Privacy violation: `::Taco` is private to `packs/baz`, but referenced from `packs/bar`"))
        .stdout(predicate::str::contains("Privacy violation: `::Census` is private to `packs/baz`, but referenced from `packs/bar`"));

    Ok(())
}

#[test]
fn test_check_without_stale_violations() -> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/contains_package_todo")
        .arg("check")
        .assert()
        .success()
        .stdout(
            predicate::str::contains(
                "There were stale violations found, please run `crabwerk update`",
            )
            .not(),
        );

    Ok(())
}

#[test]
fn test_check_with_strict_mode() -> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/uses_strict_mode")
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "packs/foo cannot have privacy violations on packs/bar because strict mode is enabled for privacy violations in the enforcing pack's package.yml file",
        ))
        .stdout(predicate::str::contains(
            "packs/foo cannot have dependency violations on packs/bar because strict mode is enabled for dependency violations in the enforcing pack's package.yml file",
        ));

    Ok(())
}

#[test]
fn test_check_output_is_deterministic() -> Result<(), Box<dyn Error>> {
    let mut outputs: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    for _ in 0..20 {
        let output = Command::new(cargo_bin!("crabwerk"))
            .arg("--project-root")
            .arg("tests/fixtures/uses_strict_mode")
            .arg("check")
            .assert()
            .failure()
            .get_output()
            .stdout
            .clone();

        outputs.insert(output_text(output));
    }

    assert_eq!(
        outputs.len(),
        1,
        "`check` produced {} distinct outputs over 20 runs: {:?}",
        outputs.len(),
        outputs
    );

    Ok(())
}

#[test]
fn test_check_json_output() -> Result<(), Box<dyn Error>> {
    let output = Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("check")
        .arg("--json")
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["status"], "failure");

    let violations = json["violations"].as_array().unwrap();
    assert_eq!(violations.len(), 2);

    // Violations are sorted by message, dependency comes before privacy
    let dep = &violations[0];
    assert_eq!(dep["file"], "packs/foo/app/services/foo.rb");
    assert_eq!(dep["line"], 3);
    assert_eq!(dep["column"], 4);
    assert_eq!(dep["violation_type"], "dependency");
    assert_eq!(dep["constant_name"], "::Bar");
    assert_eq!(dep["referencing_pack_name"], "packs/foo");
    assert_eq!(dep["defining_pack_name"], "packs/bar");
    assert_eq!(dep["strict"], false);
    assert!(
        dep["message"]
            .as_str()
            .unwrap()
            .contains("Dependency violation")
    );
    // No ANSI escape codes in JSON message
    assert!(!dep["message"].as_str().unwrap().contains("\x1b"));

    let priv_v = &violations[1];
    assert_eq!(priv_v["violation_type"], "privacy");
    assert!(
        priv_v["message"]
            .as_str()
            .unwrap()
            .contains("Privacy violation")
    );

    assert!(json["stale_violations"].as_array().unwrap().is_empty());
    assert!(
        json["strict_mode_violations"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    Ok(())
}

#[test]
fn test_check_json_no_violations() -> Result<(), Box<dyn Error>> {
    let output = Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/contains_package_todo")
        .arg("check")
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["status"], "success");
    assert!(json["violations"].as_array().unwrap().is_empty());

    Ok(())
}

#[test]
fn test_check_ambiguous_constant_definition() -> Result<(), Box<dyn Error>> {
    let assert = Command::new(cargo_bin!("crabwerk"))
        // anyhow appends a backtrace to the `Error:` line when the ambient
        // `RUST_BACKTRACE` asks for one, as CI does. This test is about the
        // message, so the child is told not to.
        .env("RUST_LIB_BACKTRACE", "0")
        .arg("--project-root")
        .arg("tests/fixtures/app_with_ambiguous_constants")
        .arg("check")
        .assert()
        .code(1);

    let stderr = output_text(assert.get_output().stderr.clone());
    // anyhow pads the blank line inside the cause, so compare trimmed lines.
    let lines: Vec<&str> = stderr.lines().map(|line| line.trim_end()).collect();

    assert_eq!(
        lines,
        vec![
            "Error: Failed to check files",
            "",
            "Caused by:",
            "    Ambiguous constant definition:",
            "",
            "    \"Foo\" could refer to any of",
            "      packs/a/app/services/foo.rb",
            "      packs/b/app/services/foo.rb",
        ]
    );

    Ok(())
}

#[test]
fn test_check_private_constants() -> Result<(), Box<dyn Error>> {
    let assert = Command::new(cargo_bin!("crabwerk"))
        // See `test_check_ambiguous_constant_definition`: the exact stderr is
        // asserted, so the backtrace must not depend on the environment.
        .env("RUST_LIB_BACKTRACE", "0")
        .arg("--project-root")
        .arg("tests/fixtures/app_with_private_constants")
        .arg("check")
        .assert()
        .code(1);

    let stdout = output_text(assert.get_output().stdout.clone());

    assert!(stdout.contains("2 violation(s) detected:"));
    assert!(stdout.contains("Privacy violation: `::Bar::Private` is private to `packs/bar`, but referenced from `packs/foo`"));
    assert!(stdout.contains("Privacy violation: `::Bar::Private::Nested` is private to `packs/bar`, but referenced from `packs/foo`"));
    assert!(!stdout.contains("::Bar::Other"));

    // The checker must not leak debug output for every private reference.
    let stderr = output_text(assert.get_output().stderr.clone());
    assert_eq!(stderr, "Error: 2 violation(s) found!\n");

    Ok(())
}

#[test]
fn test_check_custom_public_path() -> Result<(), Box<dyn Error>> {
    let assert = Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/app_with_custom_public_path")
        .arg("check")
        .assert()
        .code(1);

    let stdout = output_text(assert.get_output().stdout.clone());

    // `public_path` is pack-relative, as in packwerk, and the root pack's
    // default public folder is not prefixed with the pack path.
    assert!(stdout.contains("2 violation(s) detected:"));
    assert!(stdout.contains("Privacy violation: `::Bar::Private` is private to `packs/bar`, but referenced from `packs/foo`"));
    assert!(stdout.contains(
        "Privacy violation: `::RootPrivate` is private to `.`, but referenced from `packs/foo`"
    ));
    assert!(!stdout.contains("`::Bar` is private"));
    assert!(!stdout.contains("::RootThing"));

    Ok(())
}

#[test]
fn test_check_writes_no_color_when_stdout_is_not_a_terminal()
-> Result<(), Box<dyn Error>> {
    let output = Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("check")
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let raw = String::from_utf8_lossy(&output).to_string();

    assert!(raw.contains("2 violation(s) detected:"));
    assert!(
        !raw.contains('\x1b'),
        "expected no ANSI escape codes, got {:?}",
        raw
    );

    Ok(())
}

#[test]
fn test_check_color_always_writes_color() -> Result<(), Box<dyn Error>> {
    let output = Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--color")
        .arg("always")
        .arg("check")
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let raw = String::from_utf8_lossy(&output).to_string();

    assert!(
        raw.contains("\x1b[36mpacks/foo/app/services/foo.rb\x1b[0m:3:4"),
        "expected a coloured location, got {:?}",
        raw
    );

    Ok(())
}

#[test]
fn test_check_color_never_writes_no_color() -> Result<(), Box<dyn Error>> {
    let output = Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--color")
        .arg("never")
        .arg("check")
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let raw = String::from_utf8_lossy(&output).to_string();

    assert!(raw.contains("2 violation(s) detected:"));
    assert!(
        !raw.contains('\x1b'),
        "expected no ANSI escape codes, got {:?}",
        raw
    );

    Ok(())
}

// `--json` output is read by other programs, so colour must never reach it,
// even when the user asks for colour.
#[test]
fn test_check_json_is_never_coloured() -> Result<(), Box<dyn Error>> {
    let output = Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--color")
        .arg("always")
        .arg("check")
        .arg("--json")
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let raw = String::from_utf8_lossy(&output).to_string();
    assert!(
        !raw.contains('\x1b'),
        "expected no ANSI escape codes, got {:?}",
        raw
    );

    let json: Value = serde_json::from_slice(&output)?;
    let violations = json["violations"].as_array().unwrap();
    assert_eq!(violations.len(), 2);

    Ok(())
}
