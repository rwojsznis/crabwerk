//! Every YAML read and write goes through here, so that the rules that decide
//! what a bare scalar means are set once.
//!
//! packwerk reads its YAML with Ruby's Psych, which follows YAML 1.1: `yes`,
//! `no`, `on` and `off` are booleans, and a leading zero means octal. The
//! options below pick those rules, because a `package.yml` value that the gem
//! reads as `true` must not arrive here as the string `"yes"`.

use serde::{Serialize, de::DeserializeOwned};
use serde_saphyr::{DeserializeError, Options, SerializeError};

/// The one difference left from Psych: Psych keeps a lone `y` or `n` as a
/// string, and this reads them as booleans. It shows only in a `package.yml`
/// key that crabwerk does not name itself, and no such key means anything to
/// either tool.
fn options() -> Options {
    let mut options = Options::default();
    options.legacy_octal_numbers = true;
    options
}

pub fn from_str<T: DeserializeOwned>(
    contents: &str,
) -> Result<T, DeserializeError> {
    serde_saphyr::from_str_with_options(contents, options())
}

pub fn to_string<T: Serialize>(value: &T) -> Result<String, SerializeError> {
    serde_saphyr::to_string(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// The expected column is what `ruby -ryaml -e 'p YAML.load(...)'` prints.
    /// Change a row only when Ruby's answer changes.
    #[test]
    fn test_scalars_resolve_the_way_psych_resolves_them() {
        let cases = [
            ("yes", Value::Bool(true)),
            ("no", Value::Bool(false)),
            ("on", Value::Bool(true)),
            ("off", Value::Bool(false)),
            ("NO", Value::Bool(false)),
            ("true", Value::Bool(true)),
            ("012", Value::from(10)),
            ("1_000", Value::from(1000)),
            ("0x1F", Value::from(31)),
            ("1.5", Value::from(1.5)),
            ("1.2.3", Value::from("1.2.3")),
            ("2024-01-01", Value::from("2024-01-01")),
        ];

        for (scalar, expected) in cases {
            let parsed: Value =
                from_str(&format!("value: {scalar}\n")).unwrap();

            assert_eq!(
                parsed["value"], expected,
                "`{scalar}` did not resolve the way Psych resolves it"
            );
        }
    }
}
