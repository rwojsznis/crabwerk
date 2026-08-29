use assert_cmd::Command;
#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;

use regex::Regex;

#[test]
fn test_pack_with_public_api_exposed_via_sigil()
-> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/public_api_sigils")
        .arg("check")
        .output()?;

    let stdout_with_ansi = String::from_utf8_lossy(&output.stdout);

    let ansi_escape =
        Regex::new(r"\x1B\[([0-9]{1,2}(;[0-9]{1,2})?)?[m|K]").unwrap();
    let stdout = ansi_escape.replace_all(&stdout_with_ansi, "");

    let expected_output = r#"1 violation(s) detected:
packs/foo/app/domain/foo/api.rb:7:8
Privacy violation: `::Bar::Api3` is private to `packs/bar`, but referenced from `packs/foo`


"#;

    assert!(!output.status.success());

    assert_eq!(stdout, expected_output, "Unexpected output: {}", stdout);

    Ok(())
}

#[test]
// A scoped check must read the defining file again because the initial parse
// might not include the file that contains the public sigil.
fn test_pack_with_public_api_exposed_via_sigil_with_single_fine_input()
-> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/public_api_sigils")
        .arg("check")
        .arg("packs/foo/app/domain/foo/api.rb")
        .output()?;

    let stdout_with_ansi = String::from_utf8_lossy(&output.stdout);

    let ansi_escape =
        Regex::new(r"\x1B\[([0-9]{1,2}(;[0-9]{1,2})?)?[m|K]").unwrap();
    let stdout = ansi_escape.replace_all(&stdout_with_ansi, "");

    let expected_output = r#"1 violation(s) detected:
packs/foo/app/domain/foo/api.rb:7:8
Privacy violation: `::Bar::Api3` is private to `packs/bar`, but referenced from `packs/foo`


"#;

    assert!(!output.status.success());

    assert_eq!(stdout, expected_output, "Unexpected output: {}", stdout);

    Ok(())
}

#[test]
fn test_pack_with_public_api_exposed_via_sigil_with_experimental_parser()
-> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/public_api_sigils")
        .arg("--experimental-parser")
        .arg("check")
        .output()?;

    let stdout_with_ansi = String::from_utf8_lossy(&output.stdout);

    let ansi_escape =
        Regex::new(r"\x1B\[([0-9]{1,2}(;[0-9]{1,2})?)?[m|K]").unwrap();
    let stdout = ansi_escape.replace_all(&stdout_with_ansi, "");

    let expected_output = r#"1 violation(s) detected:
packs/foo/app/domain/foo/api.rb:7:8
Privacy violation: `::Bar::Api3` is private to `packs/bar`, but referenced from `packs/foo`


"#;

    assert!(!output.status.success());

    assert_eq!(stdout, expected_output, "Unexpected output: {}", stdout);

    Ok(())
}
