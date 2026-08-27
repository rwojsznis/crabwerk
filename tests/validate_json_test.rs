#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;
use serde_json::Value;
use std::{error::Error, process::Command};

fn validate_json(project_root: &str) -> (bool, Value) {
    let output = Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg(project_root)
        .arg("validate")
        .arg("--json")
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("Could not parse `validate --json` output {stdout:?}: {e}")
    });

    (output.status.success(), json)
}

#[test]
fn test_validate_json_success() -> Result<(), Box<dyn Error>> {
    let (success, json) = validate_json("tests/fixtures/simple_app");

    assert!(success);
    assert_eq!(json["status"], "success");
    assert_eq!(json["validation_errors"].as_array().unwrap().len(), 0);

    Ok(())
}

#[test]
fn test_validate_json_reports_cycles_and_self_dependencies()
-> Result<(), Box<dyn Error>> {
    let (success, json) =
        validate_json("tests/fixtures/app_with_dependency_cycles");

    assert!(!success, "`validate --json` should exit non-zero on errors");
    assert_eq!(json["status"], "failure");

    let errors = json["validation_errors"].as_array().unwrap();
    let error_types: Vec<&str> = errors
        .iter()
        .map(|e| e["error_type"].as_str().unwrap())
        .collect();
    assert!(error_types.contains(&"self_dependency"));
    assert!(error_types.contains(&"cycle"));

    let self_dependency = errors
        .iter()
        .find(|e| e["error_type"] == "self_dependency")
        .unwrap();
    assert_eq!(self_dependency["file"], "packs/baz/package.yml");
    assert!(
        self_dependency["message"]
            .as_str()
            .unwrap()
            .contains("Package cannot list itself as a dependency")
    );

    let cycle = errors.iter().find(|e| e["error_type"] == "cycle").unwrap();
    let cycle_edges = cycle["cycle_edges"].as_array().unwrap();
    assert_eq!(cycle_edges.len(), 2);
    for edge in cycle_edges {
        assert!(edge["from_pack"].is_string());
        assert!(edge["to_pack"].is_string());
        assert!(edge["file"].is_string());
    }

    Ok(())
}

#[test]
fn test_validate_json_reports_unknown_dependency() -> Result<(), Box<dyn Error>>
{
    let (success, json) =
        validate_json("tests/fixtures/references_unknown_pack");

    assert!(!success);
    assert_eq!(json["status"], "failure");

    let errors = json["validation_errors"].as_array().unwrap();
    let configuration_error = errors
        .iter()
        .find(|e| e["error_type"] == "configuration")
        .expect("expected a `configuration` validation error");
    assert!(
        configuration_error["message"]
            .as_str()
            .unwrap()
            .contains("in its dependencies, but that pack cannot be found")
    );
    // `configuration` errors carry no cycle information or owning file
    assert!(configuration_error.get("cycle_edges").is_none());
    assert!(configuration_error.get("file").is_none());

    Ok(())
}

#[test]
fn test_validate_json_reports_layer_errors() -> Result<(), Box<dyn Error>> {
    let (success, json) =
        validate_json("tests/fixtures/app_with_layer_violations_in_yml");

    assert!(!success);
    assert_eq!(json["status"], "failure");

    let errors = json["validation_errors"].as_array().unwrap();
    let layer_messages: Vec<&str> = errors
        .iter()
        .filter(|e| e["error_type"] == "layer")
        .map(|e| e["message"].as_str().unwrap())
        .collect();

    assert!(layer_messages.iter().any(|m| m.contains(
        "'layer' must be specified in 'packs/baz/package.yml' because `enforce_layers` is true or strict."
    )));
    assert!(layer_messages.iter().any(|m| {
        m.contains("Invalid 'layer' option in 'packs/foo/package.yml'.")
    }));

    Ok(())
}
