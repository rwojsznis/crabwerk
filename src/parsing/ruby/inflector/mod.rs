//! The parts of Rails' inflector that constant resolution needs, ported from
//! `ActiveSupport::Inflector`.
//!
//! This used to wrap the `ruby_inflector` crate. That crate is a port of the
//! same Rails code, but it applies the rules case-sensitively, ships an
//! English uncountable-word list in place of Rails' ten words, and omits the
//! irregulars entirely — so `has_many :people` resolved to `People` rather
//! than `Person`. A local port is small enough to keep honest.

mod singularize;

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;

pub use singularize::singularize;

// The two patterns `camelize` matches, and the lowercase acronym index it
// looks words up in, are the same for a whole run, while `camelize` itself is
// called once per Ruby file. Rebuilding them per call made inferring
// constants from file names cost more than parsing the Ruby.
static LEADING_LOWERCASE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^[a-z\\d]*").unwrap());
static UNDERSCORE_OR_SLASH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("(?:_|(/))([a-z\\d]*)").unwrap());

// Rails strips a leading schema name, as in `public.users`, before it
// inflects a table name.
static SCHEMA_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s).*\.").unwrap());

/// The acronyms declared in `config/initializers/inflections.rb`, indexed the
/// way [`camelize`] reads them: by their lowercase form, which is what a
/// file name holds.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Acronyms {
    by_lowercase: HashMap<String, String>,
}

impl Acronyms {
    /// The declared spelling of `word` when it names an acronym — which may
    /// differ from `word` in case, as `FacTory` does — and otherwise `word`
    /// capitalized.
    fn spelling_of(&self, word: &str) -> String {
        self.by_lowercase
            .get(word)
            .map_or_else(|| capitalize(word), String::to_owned)
    }
}

impl From<HashSet<String>> for Acronyms {
    fn from(declared: HashSet<String>) -> Self {
        let by_lowercase = declared
            .iter()
            .map(|acronym| (acronym.to_lowercase(), acronym.to_owned()))
            .collect();

        Self { by_lowercase }
    }
}

/// `ActiveSupport::Inflector.classify`, which is what packwerk's
/// `AssociationInspector` calls to turn `has_many :companies` into a
/// reference to `Company`.
pub fn classify(table_name: &str, acronyms: &Acronyms) -> String {
    let without_schema = SCHEMA_PREFIX.replace(table_name, "");

    camelize(&singularize(&without_schema), acronyms)
}

pub fn camelize(s: &str, acronyms: &Acronyms) -> String {
    // Match ActiveSupport's acronym-aware `camelize` behavior.
    let new_string = LEADING_LOWERCASE
        .replace(s, |caps: &regex::Captures| {
            acronyms.spelling_of(caps.get(0).unwrap().as_str())
        })
        .into_owned();

    UNDERSCORE_OR_SLASH
        .replace_all(&new_string, |caps: &regex::Captures| {
            let matched_slash = caps.get(1);
            let capitalized_word =
                acronyms.spelling_of(caps.get(2).unwrap().as_str());

            if matched_slash.is_some() {
                format!("::{}", capitalized_word)
            } else {
                capitalized_word
            }
        })
        .into_owned()
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| {
        f.to_uppercase().collect::<String>() + c.as_str()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fn_test_camelizing_case_retained() {
        let acronyms = Acronyms::from(HashSet::from([String::from("FacTory")]));

        let actual = camelize("my_factory", &acronyms);
        let expected = "MyFacTory";
        assert_eq!(expected, actual);
    }

    #[test]
    fn camelizes_an_acronym_in_every_path_segment() {
        let acronyms = Acronyms::from(HashSet::from([
            String::from("API"),
            String::from("CSV"),
        ]));

        let actual = camelize("api/csv/some_thing", &acronyms);

        assert_eq!("API::CSV::SomeThing", actual);
    }

    // Every expectation below is what
    // `ActiveSupport::Inflector.classify` returns for the same input.
    #[test]
    fn matches_active_support_classify() {
        let cases = [
            ("companies", "Company"),
            ("blog_posts", "BlogPost"),
            ("my_string", "MyString"),
            ("my_string_401k_things", "MyString401kThing"),
            // The irregulars, which the crate had no rules for.
            ("people", "Person"),
            ("children", "Child"),
            ("men", "Man"),
            ("women", "Woman"),
            // Rails singularizes these correctly only because it matches
            // case-insensitively. The crate applied its rules to the already
            // camelized word, so it reached none of them and fell through to
            // "drop the trailing s".
            ("statuses", "Status"),
            ("status", "Status"),
            ("aliases", "Alias"),
            ("buses", "Bus"),
            ("indices", "Index"),
            ("matrices", "Matrix"),
            ("analyses", "Analysis"),
            ("movies", "Movie"),
            ("mice", "Mouse"),
            // Rails anchors the `(m|l)ice` rule at the start of the word.
            // Unanchored, this one turns into `Polouse`.
            ("police", "Police"),
            // Rails is wrong here, and so are we, on purpose.
            ("leaves", "Leafe"),
            ("censuses", "Censuse"),
            ("athletics", "Athletic"),
            ("pasta", "Pastum"),
            ("data", "Datum"),
            // Uncountable.
            ("species", "Species"),
            ("series", "Series"),
        ];

        for (input, expected) in cases {
            assert_eq!(
                expected,
                classify(input, &Acronyms::default()),
                "classify({:?})",
                input
            );
        }
    }

    #[test]
    fn classify_applies_acronyms() {
        let acronyms = Acronyms::from(HashSet::from([String::from("API")]));

        assert_eq!("APIKey", classify("api_keys", &acronyms));
    }

    #[test]
    fn classify_drops_a_leading_schema_name() {
        assert_eq!("User", classify("public.users", &Acronyms::default()));
    }
}
