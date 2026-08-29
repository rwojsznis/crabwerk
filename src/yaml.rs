//! YAML configured to match the scalar rules used by packwerk's Psych parser.

use serde::{Serialize, de::DeserializeOwned};
use serde_saphyr::{DeserializeError, Options, SerializeError};

/// Psych keeps `y` and `n` as strings, but serde-saphyr reads them as booleans.
/// This difference affects only unknown `package.yml` keys.
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

    // Expected values come from `ruby -ryaml -e 'p YAML.load(...)'`.
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
