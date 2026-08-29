#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;
use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::{error::Error, process::Command};

#[test]
fn test_validate_cycle_detection() -> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/app_with_dependency_cycles")
        .arg("validate")
        .assert()
        .failure()
        .stdout(predicate::str::contains("2 validation error(s) detected:"))
        .stdout(predicate::str::contains("strongly connected components"))
        // Cycle path is now shown as "packs/foo -> packs/bar -> packs/foo"
        .stdout(predicate::str::contains(" -> "))
        .stdout(predicate::str::contains(
            "Package cannot list itself as a dependency: packs/baz/package.yml",
        ));

    Ok(())
}

#[test]
fn test_validate_layer() -> Result<(), Box<dyn Error>> {
    let expected_message_1 = String::from(
        "\'layer\' must be specified in \'packs/baz/package.yml\' because `enforce_layers` is true or strict.",
    );
    let expected_message_2 = String::from(
        "Invalid \'layer\' option in \'packs/foo/package.yml\'. `layer` must be one of the layers defined in `crabwerk.yml`",
    );

    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/app_with_layer_violations_in_yml")
        .arg("validate")
        .assert()
        .failure()
        .stdout(predicate::str::contains("2 validation error(s) detected:"))
        .stdout(predicate::str::contains(expected_message_1))
        .stdout(predicate::str::contains(expected_message_2));

    Ok(())
}

#[test]
fn test_validate_with_referencing_unknown_pack() -> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/references_unknown_pack")
        .arg("validate")
        .assert()
        .failure()
        .stdout(predicate::str::contains("has \'packs/unknown-pack\' in its dependencies, but that pack cannot be found"));

    Ok(())
}

#[test]
fn test_validate_output_is_deterministic() -> Result<(), Box<dyn Error>> {
    // Two strict packs both reach the same non-strict pack, so the report has
    // two strict-transitive errors whose order must not depend on hashing.
    assert_one_distinct_output("tests/fixtures/strict_transitive_non_strict")
}

#[test]
fn test_validate_cycle_output_is_deterministic() -> Result<(), Box<dyn Error>> {
    // The cycle message names a starting pack, which must not depend on which
    // node of the cycle happens to come first out of a set.
    assert_one_distinct_output("tests/fixtures/app_with_dependency_cycles")
}

#[test]
fn test_validate_json_output_is_deterministic() -> Result<(), Box<dyn Error>> {
    assert_one_distinct_output_with_args(
        "tests/fixtures/strict_transitive_non_strict",
        &["validate", "--json"],
    )
}

fn assert_one_distinct_output(
    project_root: &str,
) -> Result<(), Box<dyn Error>> {
    assert_one_distinct_output_with_args(project_root, &["validate"])
}

fn assert_one_distinct_output_with_args(
    project_root: &str,
    args: &[&str],
) -> Result<(), Box<dyn Error>> {
    let mut outputs: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    for _ in 0..20 {
        let output = Command::new(cargo_bin!("crabwerk"))
            .arg("--project-root")
            .arg(project_root)
            .args(args)
            .assert()
            .failure()
            .get_output()
            .stdout
            .clone();

        outputs.insert(String::from_utf8_lossy(&output).to_string());
    }

    assert_eq!(
        outputs.len(),
        1,
        "`{}` on {} produced {} distinct outputs over 20 runs: {:?}",
        args.join(" "),
        project_root,
        outputs.len(),
        outputs
    );

    Ok(())
}
