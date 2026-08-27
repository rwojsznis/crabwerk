use regex::Regex;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

// `is_ignored` runs once per reference per checker, so a rule is tested many
// thousands of times against a rule set that never changes. Each distinct rule
// is therefore compiled once per thread; a thread-local cache keeps the hot
// path free of locks, which matters because the callers run under rayon.
thread_local! {
    static COMPILED_RULES: RefCell<HashMap<String, Option<Regex>>> =
        RefCell::new(HashMap::new());
}

pub fn is_ignored(rules: &HashSet<String>, path: &str) -> anyhow::Result<bool> {
    // The allow-list (a `!` prefix) takes precedence over the deny-list.
    if rules
        .iter()
        .filter_map(|rule| rule.strip_prefix('!'))
        .any(|rule| is_match(rule, path))
    {
        return Ok(false);
    }

    Ok(rules
        .iter()
        .filter(|rule| !rule.starts_with('!'))
        .any(|rule| is_match(rule, path)))
}

fn is_match(rule: &str, path: &str) -> bool {
    COMPILED_RULES.with_borrow_mut(|cache| {
        if let Some(compiled) = cache.get(rule) {
            return compiled.as_ref().is_some_and(|r| r.is_match(path));
        }

        // A rule that is not a valid glob never matches. Caching the failure
        // keeps that decision as cheap as a successful one.
        let compiled = fnmatch_regex2::glob_to_regex(rule).ok();
        let matched = compiled.as_ref().is_some_and(|r| r.is_match(path));
        cache.insert(rule.to_owned(), compiled);
        matched
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[macro_export]
    macro_rules! test_ignore {
        ($name:ident, $rules:expr, $path:expr, $expected:expr) => {
            #[test]
            fn $name() {
                assert_eq!(
                    is_ignored($rules, $path).unwrap(),
                    $expected,
                    "Testing path: {}",
                    $path
                );
            }
        };
    }

    #[macro_export]
    macro_rules! ignored {
        ($name:ident, $rules:expr, $path:expr) => {
            test_ignore!($name, $rules, $path, true);
        };
    }

    #[macro_export]
    macro_rules! not_ignored {
        ($name:ident, $rules:expr, $path:expr) => {
            test_ignore!($name, $rules, $path, false);
        };
    }

    ignored!(
        foo1,
        &HashSet::from(["packs/foo/**/*".to_string()]),
        "packs/foo/app/services/my.rb"
    );
    ignored!(
        foo2,
        &HashSet::from(["**/*".to_string()]),
        "logs/monday/foo.bar"
    );

    not_ignored!(
        nofoo1,
        &HashSet::from(["*/**".to_string(), "!packs/foo/**".to_string()]),
        "packs/foo/app/services/my.rb"
    );

    #[test]
    fn test_is_match() {
        assert!(is_match("foo", "foo"));
        assert!(!is_match("foo", "bar"));
        assert!(is_match("foo*", "foobar"));
        assert!(is_match("packs/foo/**", "packs/foo/app/services/my.rb"));
        assert!(is_match("packs/foo/**/*", "packs/foo/app/services/my.rb"));
    }

    // A rule that is not a valid glob never matches, rather than panicking.
    #[test]
    fn test_is_match_with_an_invalid_glob() {
        assert!(!is_match("[", "["));
        assert!(!is_match("[a-", "packs/foo/app/services/my.rb"));
    }

    // The compiled rules are cached, so the same rule must still answer per
    // path, and one rule's compilation must not answer for another's.
    #[test]
    fn test_is_match_is_stable_across_repeated_calls() {
        for _ in 0..3 {
            assert!(is_match("packs/foo/**", "packs/foo/app/services/my.rb"));
            assert!(!is_match("packs/foo/**", "packs/bar/app/services/my.rb"));
            assert!(is_match("packs/bar/**", "packs/bar/app/services/my.rb"));
            assert!(!is_match("[a-", "packs/bar/app/services/my.rb"));
        }
    }

    #[test]
    fn test_is_ignored_is_stable_across_repeated_calls() {
        let rules =
            HashSet::from(["*/**".to_string(), "!packs/foo/**".to_string()]);

        for _ in 0..3 {
            assert!(
                !is_ignored(&rules, "packs/foo/app/services/my.rb").unwrap()
            );
            assert!(
                is_ignored(&rules, "packs/bar/app/services/my.rb").unwrap()
            );
        }
    }

    #[test]
    fn test_is_ignored_with_an_invalid_glob() {
        assert!(
            !is_ignored(
                &HashSet::from(["[".to_string()]),
                "packs/foo/app/services/my.rb"
            )
            .unwrap()
        );
    }
}
