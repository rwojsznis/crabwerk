use assert_cmd::Command;
#[allow(deprecated)]
use assert_cmd::cargo::cargo_bin;
use std::error::Error;

// The fixture puts a deny-all `**/*` rule and a narrower `!` allow-list rule
// on the same enforcement, so every assertion here has a counterpart: a
// reference the rule ignores, and one the allow-list brings back. Asserting
// the count as well as the text keeps a rule that stops working from being
// invisible.
#[test]
fn test_check() -> Result<(), Box<dyn Error>> {
    let output = Command::new(cargo_bin!("crabwerk"))
        .arg("--project-root")
        .arg("tests/fixtures/simple_app_with_enforcement_globs")
        .arg("--debug")
        .arg("check")
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let output_text = String::from_utf8_lossy(&output).to_string();

    assert!(
        output_text.contains("2 violation(s) detected:"),
        "{output_text}"
    );

    // `packs/product_services/oo_prod/foo` ignores privacy everywhere except
    // under `oo_prod`, and `zoo` is under `oo_prod`.
    assert!(
        output_text.contains(
            "packs/product_services/oo_prod/zoo/app/services/zoo.rb:1:7\nPrivacy violation: `::Foo` is private to `packs/product_services/oo_prod/foo`, but referenced from `packs/product_services/oo_prod/zoo`"
        ),
        "{output_text}"
    );

    // `packs/product_services/oo_prod/zoo` ignores dependencies everywhere
    // except on `ar_prod/baz`, which is where `::Baz` is defined.
    assert!(
        output_text.contains(
            "packs/product_services/oo_prod/zoo/app/services/zoo.rb:7:4\nDependency violation: `::Baz` belongs to `packs/product_services/ar_prod/baz`, but `packs/product_services/oo_prod/zoo/package.yml` does not specify a dependency on `packs/product_services/ar_prod/baz`."
        ),
        "{output_text}"
    );

    // The same file references `::Bar`, which the deny-all side of both rules
    // covers: `ar_prod` is outside the two allow-lists.
    assert!(!output_text.contains("`::Bar`"), "{output_text}");

    Ok(())
}
