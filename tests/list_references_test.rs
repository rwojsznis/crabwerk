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

// `clap` rejects the value, so the run ends before any file is parsed.
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
            "invalid value 'xml' for '--format <FORMAT>'",
        ))
        .stderr(predicates::str::contains("[possible values: json, text]"));

    Ok(())
}

// Without `--experimental-parser`, references are resolved with the Zeitwerk
// constant resolver instead.
#[test]
fn test_list_references_with_the_zeitwerk_resolver()
-> Result<(), Box<dyn Error>> {
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

fn assert_output_is_stable(fixture: &str, experimental: bool) {
    let mut outputs = Vec::new();
    for _ in 0..10 {
        let mut command = Command::new(cargo_bin!("crabwerk"));
        command.arg("--project-root").arg(fixture);
        if experimental {
            command.arg("--experimental-parser");
        }
        let output = command
            .arg("list-references")
            .arg("--format")
            .arg("text")
            .output()
            .expect("crabwerk should run");
        assert!(output.status.success());
        outputs.push(output.stdout);
    }

    let distinct = outputs
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len();
    assert_eq!(
        distinct, 1,
        "list-references wrote {} distinct outputs over 10 runs of {}",
        distinct, fixture
    );
}

// `list-references` feeds test selection, so its output is stored and diffed;
// a map that iterates in hash order makes every run look like a change.
#[test]
fn test_list_references_output_is_deterministic() -> Result<(), Box<dyn Error>>
{
    assert_output_is_stable("tests/fixtures/simple_app", false);

    Ok(())
}

// The experimental parser reads definitions from the AST, so a constant that
// two files define has two defining files. Which one the map keeps must not
// depend on the order the files happened to be parsed in.
#[test]
fn test_list_references_output_is_deterministic_for_an_ambiguous_constant()
-> Result<(), Box<dyn Error>> {
    assert_output_is_stable(
        "tests/fixtures/simple_app_with_enforcement_globs",
        true,
    );

    Ok(())
}

#[test]
fn test_list_references_text_output_is_sorted() -> Result<(), Box<dyn Error>> {
    let output = Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("list-references")
        .arg("--format")
        .arg("text")
        .output()?;

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout)?,
        "\
app/company_data/widget.rb:
  ::Company::Widget => app/company_data/widget.rb
app/services/some_root_class.rb:
  ::SomeRootClass => app/services/some_root_class.rb
packs/bar/app/models/concerns/some_concern.rb:
  ::SomeConcern => packs/bar/app/models/concerns/some_concern.rb
packs/bar/app/services/bar.rb:
  ::Bar => packs/bar/app/services/bar.rb
packs/foo/app/services/foo.rb:
  ::Bar => packs/bar/app/services/bar.rb
  ::Baz => packs/baz/app/services/baz.rb
  ::Foo => packs/foo/app/services/foo.rb
packs/foo/app/services/foo/bar.rb:
  ::Foo => packs/foo/app/services/foo.rb
  ::Foo::Bar => packs/foo/app/services/foo/bar.rb
"
    );

    Ok(())
}
