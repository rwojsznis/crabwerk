//! A port of `ActiveSupport::Inflector.singularize` under Rails' default
//! English inflections.
//!
//! The rules are transcribed from Rails'
//! `activesupport/lib/active_support/inflections.rb`, not chosen for English
//! correctness. Where Rails is wrong, matching it is the point: packwerk
//! resolves `has_many :leaves` through `ActiveSupport::Inflector.classify`,
//! so it looks for `Leafe`, and so must we.

use std::sync::LazyLock;

use regex::Regex;

/// The words Rails refuses to inflect. It tests the *last word* of the
/// string, so `two fish` is uncountable while `jellyfish` is not — and an
/// underscore is a word character, which makes `swordfish` and `sales_fish`
/// alike countable.
static UNCOUNTABLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:equipment|information|rice|money|species|series|fish|sheep|jeans|police)\z",
    )
    .unwrap()
});

/// Rails tries the singular rules in reverse declaration order, and the
/// irregulars are declared last, so they come first here. Within one
/// irregular pair the plural form is tried before the singular form.
///
/// The first rule that matches wins; nothing falls through to a second.
static RULES: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (r"(?i)(z)ombies$", "${1}ombie"),
        (r"(?i)(z)ombie$", "${1}ombie"),
        (r"(?i)(m)oves$", "${1}ove"),
        (r"(?i)(m)ove$", "${1}ove"),
        (r"(?i)(s)exes$", "${1}ex"),
        (r"(?i)(s)ex$", "${1}ex"),
        (r"(?i)(c)hildren$", "${1}hild"),
        (r"(?i)(c)hild$", "${1}hild"),
        (r"(?i)(m)en$", "${1}an"),
        (r"(?i)(m)an$", "${1}an"),
        (r"(?i)(p)eople$", "${1}erson"),
        (r"(?i)(p)erson$", "${1}erson"),
        (r"(?i)(database)s$", "${1}"),
        (r"(?i)(quiz)zes$", "${1}"),
        (r"(?i)(matr)ices$", "${1}ix"),
        (r"(?i)(vert|ind)ices$", "${1}ex"),
        (r"(?i)^(ox)en", "${1}"),
        (r"(?i)(alias|status)(es)?$", "${1}"),
        (r"(?i)(octop|vir)(us|i)$", "${1}us"),
        (r"(?i)^(a)x[ie]s$", "${1}xis"),
        (r"(?i)(cris|test)(is|es)$", "${1}is"),
        (r"(?i)(shoe)s$", "${1}"),
        (r"(?i)(o)es$", "${1}"),
        (r"(?i)(bus)(es)?$", "${1}"),
        (r"(?i)^(m|l)ice$", "${1}ouse"),
        (r"(?i)(x|ch|ss|sh)es$", "${1}"),
        (r"(?i)(m)ovies$", "${1}ovie"),
        (r"(?i)(s)eries$", "${1}eries"),
        (r"(?i)([^aeiouy]|qu)ies$", "${1}y"),
        (r"(?i)([lr])ves$", "${1}f"),
        (r"(?i)(tive)s$", "${1}"),
        (r"(?i)(hive)s$", "${1}"),
        (r"(?i)([^f])ves$", "${1}fe"),
        (r"(?i)(^analy)(sis|ses)$", "${1}sis"),
        (
            r"(?i)((a)naly|(b)a|(d)iagno|(p)arenthe|(p)rogno|(s)ynop|(t)he)(sis|ses)$",
            "${1}sis",
        ),
        (r"(?i)([ti])a$", "${1}um"),
        (r"(?i)(n)ews$", "${1}ews"),
        (r"(?i)(ss)$", "${1}"),
        (r"(?i)s$", ""),
    ]
    .into_iter()
    .map(|(rule, replacement)| (Regex::new(rule).unwrap(), replacement))
    .collect()
});

pub fn singularize(word: &str) -> String {
    if word.is_empty() || UNCOUNTABLE.is_match(word) {
        return word.to_owned();
    }

    RULES
        .iter()
        .find(|(rule, _)| rule.is_match(word))
        .map_or_else(
            || word.to_owned(),
            |(rule, replacement)| rule.replace(word, *replacement).into_owned(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every expectation below is what ActiveSupport 8.1 returns for the same
    // input. Several read as wrong English — `leaves` to `leafe`, `censuses`
    // to `censuse`, `pasta` to `pastum` — and are kept because packwerk
    // resolves associations through the same rules.
    #[test]
    fn matches_active_support_singularize() {
        let cases = [
            // the plain plurals an app actually writes
            ("companies", "company"),
            ("posts", "post"),
            ("blog_posts", "blog_post"),
            ("comments", "comment"),
            ("users", "user"),
            ("addresses", "address"),
            ("categories", "category"),
            ("replies", "reply"),
            ("ladies", "lady"),
            ("soliloquies", "soliloquy"),
            ("glasses", "glass"),
            ("witches", "witch"),
            ("dishes", "dish"),
            ("potatoes", "potato"),
            // the irregulars, which the crate had no rules for at all
            ("people", "person"),
            ("person", "person"),
            ("children", "child"),
            ("child", "child"),
            ("men", "man"),
            ("man", "man"),
            ("women", "woman"),
            ("sexes", "sex"),
            ("sex", "sex"),
            ("moves", "move"),
            ("move", "move"),
            ("zombies", "zombie"),
            ("zombie", "zombie"),
            // the words Rails refuses to inflect
            ("equipment", "equipment"),
            ("information", "information"),
            ("rice", "rice"),
            ("money", "money"),
            ("species", "species"),
            ("series", "series"),
            ("fish", "fish"),
            ("sheep", "sheep"),
            ("jeans", "jeans"),
            ("police", "police"),
            // words Rails inflects even though English would not
            ("athletics", "athletic"),
            ("chaos", "chao"),
            ("crossroads", "crossroad"),
            ("economics", "economic"),
            ("gallows", "gallow"),
            ("mathematics", "mathematic"),
            ("measles", "measle"),
            ("mumps", "mump"),
            ("pasta", "pastum"),
            ("physics", "physic"),
            ("tennis", "tenni"),
            ("news", "news"),
            // the -sis and -ves families
            ("analyses", "analysis"),
            ("theses", "thesis"),
            ("crises", "crisis"),
            ("diagnoses", "diagnosis"),
            ("knives", "knife"),
            ("archives", "archive"),
            ("motives", "motive"),
            ("halves", "half"),
            ("wolves", "wolf"),
            ("calves", "calf"),
            ("shelves", "shelf"),
            ("leaves", "leafe"),
            ("eaves", "eafe"),
            ("hives", "hive"),
            // the rest of the rule table
            ("oxen", "ox"),
            ("boxes", "box"),
            ("quizzes", "quiz"),
            ("movies", "movie"),
            ("buses", "bus"),
            ("wishes", "wish"),
            ("pitches", "pitch"),
            ("mice", "mouse"),
            ("minibuses", "minibus"),
            ("snowshoes", "snowshoe"),
            ("axes", "axis"),
            ("octopi", "octopus"),
            ("aliases", "alias"),
            ("indices", "index"),
            ("matrices", "matrix"),
            ("databases", "database"),
            ("statuses", "status"),
            ("status", "status"),
            ("shoes", "shoe"),
            ("viruses", "viruse"),
            ("codebases", "codebasis"),
            ("dice", "dice"),
            ("feet", "feet"),
            ("geese", "geese"),
            ("teeth", "teeth"),
            ("yeses", "yese"),
            // words no rule reaches
            ("bacon", "bacon"),
            ("glass", "glass"),
            ("access", "access"),
            ("goodnews", "goodnews"),
        ];

        for (plural, expected) in cases {
            assert_eq!(
                expected,
                singularize(plural),
                "singularize({:?})",
                plural
            );
        }
    }

    #[test]
    fn keeps_the_case_of_what_it_matched() {
        assert_eq!("Company", singularize("Companies"));
        assert_eq!("Status", singularize("Statuses"));
        assert_eq!("Person", singularize("PEOPLE"));
    }

    #[test]
    fn leaves_text_before_the_match_alone() {
        assert_eq!("foo bar", singularize("foo bars"));
    }

    #[test]
    fn an_uncountable_word_must_be_the_last_word() {
        assert_eq!("police", singularize("police"));
        assert_eq!("traffic police", singularize("traffic police"));
        assert_eq!("policeman", singularize("policemen"));
    }

    #[test]
    fn an_empty_string_is_unchanged() {
        assert_eq!("", singularize(""));
    }
}
