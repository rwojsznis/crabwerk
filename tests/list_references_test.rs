#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;
use assert_cmd::prelude::*;
use std::{error::Error, fs, process::Command};
use tempfile::TempDir;

#[test]
fn test_list_references_simple_app() -> Result<(), Box<dyn Error>> {
    let temp_dir = TempDir::new()?;
    let output_file = temp_dir.path().join("references.json");

    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--experimental-parser")
        .arg("list-references")
        .arg("--out")
        .arg(&output_file)
        .assert()
        .success();

    let contents = fs::read_to_string(&output_file)?;
    let json: serde_json::Value = serde_json::from_str(&contents)?;

    let expected: serde_json::Value = serde_json::json!({
        "packs/foo/app/services/foo.rb": {
            "::Bar": "packs/bar/app/services/bar.rb"
        }
    });

    assert_eq!(json, expected);

    Ok(())
}

#[test]
fn test_list_references_namespaced_app() -> Result<(), Box<dyn Error>> {
    let temp_dir = TempDir::new()?;
    let output_file = temp_dir.path().join("references.json");

    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/app_with_namespaced_tests")
        .arg("--experimental-parser")
        .arg("list-references")
        .arg("--out")
        .arg(&output_file)
        .assert()
        .success();

    let contents = fs::read_to_string(&output_file)?;
    let json: serde_json::Value = serde_json::from_str(&contents)?;

    let expected: serde_json::Value = serde_json::json!({
        "spec/models/some_module/some_other_module/some_class_spec.rb": {
            "::SomeModule::SomeOtherModule::SomeClass": "app/models/some_module/some_other_module/some_class.rb"
        }
    });

    assert_eq!(json, expected);

    Ok(())
}

#[test]
fn test_list_references_text_format_to_stdout() -> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--experimental-parser")
        .arg("list-references")
        .arg("--format")
        .arg("text")
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "packs/foo/app/services/foo.rb:\n  ::Bar => packs/bar/app/services/bar.rb",
        ));

    Ok(())
}

#[test]
fn test_list_references_text_format_to_file() -> Result<(), Box<dyn Error>> {
    let temp_dir = TempDir::new()?;
    let output_file = temp_dir.path().join("references.txt");

    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--experimental-parser")
        .arg("list-references")
        .arg("--format")
        .arg("text")
        .arg("--out")
        .arg(&output_file)
        .assert()
        .success()
        .stdout(predicates::str::contains("Reference map written to:"));

    let contents = fs::read_to_string(&output_file)?;
    assert_eq!(
        contents,
        "packs/foo/app/services/foo.rb:\n  ::Bar => packs/bar/app/services/bar.rb"
    );

    Ok(())
}

#[test]
fn test_list_references_json_format_to_stdout() -> Result<(), Box<dyn Error>> {
    let output = Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--experimental-parser")
        .arg("list-references")
        .output()?;

    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        json,
        serde_json::json!({
            "packs/foo/app/services/foo.rb": {
                "::Bar": "packs/bar/app/services/bar.rb"
            }
        })
    );

    Ok(())
}

#[test]
fn test_list_references_with_unsupported_format() -> Result<(), Box<dyn Error>>
{
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--experimental-parser")
        .arg("list-references")
        .arg("--format")
        .arg("xml")
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "Unsupported format: xml. Use 'json' or 'text'",
        ));

    Ok(())
}

// Without `--experimental-parser`, references are resolved with the Zeitwerk
// constant resolver instead.
#[test]
fn test_list_references_with_the_zeitwerk_resolver(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = TempDir::new()?;
    let output_file = temp_dir.path().join("references.json");

    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("list-references")
        .arg("--out")
        .arg(&output_file)
        .assert()
        .success();

    let contents = fs::read_to_string(&output_file)?;
    let json: serde_json::Value = serde_json::from_str(&contents)?;

    assert_eq!(
        json["packs/foo/app/services/foo.rb"]["::Bar"],
        serde_json::json!("packs/bar/app/services/bar.rb")
    );
    assert_eq!(
        json["packs/foo/app/services/foo.rb"]["::Baz"],
        serde_json::json!("packs/baz/app/services/baz.rb")
    );

    Ok(())
}
