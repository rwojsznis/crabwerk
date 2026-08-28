use std::{collections::HashSet, path::Path, sync::LazyLock};

use regex::Regex;
use tracing::warn;

use super::inflector::Acronyms;

// `inflect.acronym 'API'` in config/initializers/inflections.rb, where the
// receiver is whatever the block parameter was named. Matching the call form
// rather than the substring ".acronym" keeps comments and non-literal
// arguments out of the set.
static ACRONYM_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^\s*[A-Za-z_][A-Za-z0-9_]*\.acronym\s*\(?\s*['"]([^'"]+)['"]"#,
    )
    .unwrap()
});

/// The acronyms declared in `config/initializers/inflections.rb`, which
/// [`camelize`](super::inflector::camelize) needs to match the Rails
/// inflections.
///
/// A file that cannot be read gives an empty set and a warning: a constant
/// camelized without the acronyms is better than no answer at all.
pub fn get_acronyms_from_disk(inflections_path: &Path) -> Acronyms {
    let mut acronyms: HashSet<String> = HashSet::new();

    if !inflections_path.exists() {
        return acronyms.into();
    }

    let inflections_file = match std::fs::read_to_string(inflections_path) {
        Ok(contents) => contents,
        Err(error) => {
            warn!(
                "Could not read {}: {}. Continuing without its acronyms.",
                inflections_path.display(),
                error
            );
            return acronyms.into();
        }
    };

    for line in inflections_file.lines() {
        if let Some(captures) = ACRONYM_CALL.captures(line) {
            acronyms.insert(captures[1].to_string());
        }
    }

    acronyms.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    fn acronyms_from(contents: &str) -> Acronyms {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inflections.rb");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        get_acronyms_from_disk(&path)
    }

    #[test]
    fn ignores_a_comment_that_names_acronym() {
        let acronyms = acronyms_from(
            r#"
# Configure inflections; see the .acronym docs for details
ActiveSupport::Inflector.inflections do |inflect|
  inflect.acronym "API"
end
"#,
        );

        assert_eq!(
            acronyms,
            Acronyms::from(HashSet::from(["API".to_string()]))
        );
    }

    #[test]
    fn reads_both_quote_styles_and_a_parenthesized_call() {
        let acronyms = acronyms_from(
            r#"
ActiveSupport::Inflector.inflections do |inflect|
  inflect.acronym 'API'
  inflect.acronym "CSV"
  inflect.acronym("PDF")
end
"#,
        );

        assert_eq!(
            acronyms,
            Acronyms::from(HashSet::from([
                "API".to_string(),
                "CSV".to_string(),
                "PDF".to_string()
            ]))
        );
    }

    #[test]
    fn ignores_a_non_literal_argument() {
        let acronyms = acronyms_from("  inflect.acronym API_NAME\n");

        assert_eq!(acronyms, Acronyms::default());
    }

    #[test]
    fn returns_an_empty_set_for_a_missing_file() {
        let dir = tempdir().unwrap();

        let acronyms = get_acronyms_from_disk(&dir.path().join("nope.rb"));

        assert_eq!(acronyms, Acronyms::default());
    }

    #[test]
    fn returns_an_empty_set_for_an_unreadable_file() {
        let dir = tempdir().unwrap();

        let acronyms = get_acronyms_from_disk(dir.path());

        assert_eq!(acronyms, Acronyms::default());
    }
}
