#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;
use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::{error::Error, process::Command};
mod common;

#[test]
fn test_automatic_namespaces_with_zeitwerk_parser() -> Result<(), Box<dyn Error>>
{
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/app_with_automatic_namespaces")
        .arg("list-definitions")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"::FooRecord\" is defined at \"packs/foo/app/models/foo_record.rb\""
        ))
        .stdout(predicate::str::contains(
            "\"::Foo::Creator\" is defined at \"packs/foo/app/services/creator.rb\""
        ));
    Ok(())
}

#[test]
fn test_automatic_namespaces_with_experimental_parser()
-> Result<(), Box<dyn Error>> {
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/app_with_automatic_namespaces")
        // Experimental parser works without issues
        .arg("--experimental-parser")
        .arg("list-definitions")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"::FooRecord\" is defined at \"packs/foo/app/models/foo_record.rb\""
        ))
        .stdout(predicate::str::contains(
            "\"::Foo::Creator\" is defined at \"packs/foo/app/services/creator.rb\""
        ));
    Ok(())
}

#[test]
fn test_automatic_namespace_uses_the_configured_acronyms()
-> Result<(), Box<dyn Error>> {
    // The namespace an automatic pack gets is its directory name camelized,
    // and camelizing reads the app's inflections: `packs/api` is `::API`.
    Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/app_with_automatic_namespaces")
        .arg("list-definitions")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"::API::Client\" is defined at \"packs/api/app/services/client.rb\"",
        ));
    Ok(())
}
