use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;
use ruby_inflector::case::{
    CamelOptions, to_case_camel_like, to_class_case as to_class_case_original,
};

// This is a list of plural to singular words that are not handled by the inflector
// The plural words are
const CLASS_CASE_TO_SINGULAR: [(&str, &str); 4] = [
    ("Censuse", "Census"),
    ("Leafe", "Leave"),
    ("Lefe", "Leave"),
    ("Daum", "Datum"),
];

// The two patterns `camelize` matches, and the lowercase acronym index it
// looks words up in, are the same for a whole run, while `camelize` itself is
// called once per Ruby file. Rebuilding them per call made inferring
// constants from file names cost more than parsing the Ruby.
static LEADING_LOWERCASE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^[a-z\\d]*").unwrap());
static UNDERSCORE_OR_SLASH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("(?:_|(/))([a-z\\d]*)").unwrap());

/// The acronyms declared in `config/initializers/inflections.rb`, indexed the
/// way [`camelize`] reads them: by their lowercase form, which is what a
/// file name holds.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Acronyms {
    declared: HashSet<String>,
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

        Self {
            declared,
            by_lowercase,
        }
    }
}

// See https://github.com/whatisinternet/Inflector/pull/87
// Note that as of the PR that adds this comment, we are now using https://github.com/alexevanczuk/ruby_inflector,
// so that we have an easier time making this inflector more specific to ruby applications (for now)
pub fn to_class_case(
    s: &str,
    should_singularize: bool,
    acronyms: &Acronyms,
) -> String {
    let options = CamelOptions {
        new_word: true,
        last_char: ' ',
        first_word: false,
        injectable_char: ' ',
        has_seperator: false,
        inverted: false,
    };

    let mut class_name = if should_singularize {
        to_class_case_original(s, &acronyms.declared)
    } else {
        to_case_camel_like(s, options, &acronyms.declared)
    };

    if let Some(prefix) = class_name.strip_suffix("Statuse") {
        class_name = format!("{}Status", prefix);
    }
    if let Some(prefix) = class_name.strip_suffix("Statu") {
        class_name = format!("{}Status", prefix);
    }

    for (plural, singular) in CLASS_CASE_TO_SINGULAR {
        if class_name.contains(plural) {
            class_name = class_name.replace(plural, singular);
        }
    }

    class_name
}

pub fn camelize(s: &str, acronyms: &Acronyms) -> String {
    // Meant to emulate https://github.com/rails/rails/blob/e88857bbb9d4e1dd64555c34541301870de4a45b/activesupport/lib/active_support/inflector/methods.rb#L69
    //
    // def camelize(term, uppercase_first_letter = true)
    //   string = term.to_s
    //   # String#camelize takes a symbol (:upper or :lower), so here we also support :lower to keep the methods consistent.
    //   if !uppercase_first_letter || uppercase_first_letter == :lower
    //     string = string.sub(inflections.acronyms_camelize_regex) { |match| match.downcase! || match }
    //   else
    //     string = string.sub(/^[a-z\d]*/) { |match| inflections.acronyms[match] || match.capitalize! || match }
    //   end
    //   string.gsub!(/(?:_|(\/))([a-z\d]*)/i) do
    //     word = $2
    //     substituted = inflections.acronyms[word] || word.capitalize! || word
    //     $1 ? "::#{substituted}" : substituted
    //   end
    //   string
    // end

    // Replace the beginning of the word, matched with lowercase letters, with either a matching inflection or a capitalized version of the word
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

/// Capitalizes the first character in s.
fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| {
        f.to_uppercase().collect::<String>() + c.as_str()
    })
}

// Add tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trivial() {
        let actual = to_class_case("my_string", false, &Acronyms::default());
        let expected = "MyString";
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_digits() {
        let actual =
            to_class_case("my_string_401k_thing", false, &Acronyms::default());
        let expected = "MyString401kThing";
        assert_eq!(expected, actual);
    }

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

    #[test]
    fn test_to_class_case() {
        let tests = vec![
            ("my_string", false, "MyString"),
            ("censuses", true, "Census"),
            ("lefe", true, "Leave"),
            ("leaves", false, "Leaves"),
            ("daum", true, "Datum"),
            ("statuss", false, "Statuss"),
            ("statuses", true, "Status"),
            ("censuse", true, "Census"),
        ];

        for (input, should_singularize, expected) in tests {
            let actual =
                to_class_case(input, should_singularize, &Acronyms::default());
            assert_eq!(
                expected, actual,
                "Failed for input: {}, and singularize: {}",
                input, should_singularize
            );
        }
    }
}
