use std::collections::HashMap;

use super::CheckerInterface;
use super::pack_checker::PackChecker;
use crate::checker::Reference;
use crate::{Configuration, Violation};

pub struct Checker {}

impl CheckerInterface for Checker {
    fn check(
        &self,
        reference: &Reference,
        configuration: &Configuration,
        _sigils: &HashMap<std::path::PathBuf, Vec<crate::Sigil>>,
    ) -> anyhow::Result<Option<Violation>> {
        let pack_checker =
            PackChecker::new(configuration, reference, &self.violation_type())?;
        if !pack_checker.checkable()? {
            return Ok(None);
        }
        let defining_pack = pack_checker.defining_pack.unwrap();
        if defining_pack.visible_to.as_ref().is_some_and(|visible_to| {
            visible_to.contains(&pack_checker.referencing_pack.name)
        }) {
            return Ok(None);
        }

        let message = format!(
            "Visibility violation: `{}` belongs to `{}`, which is not visible to `{}`",
            reference.constant_name,
            defining_pack.name,
            pack_checker.referencing_pack.name,
        );

        Ok(Some(Violation {
            message,
            identifier: pack_checker.violation_identifier(),
            source_location: reference.source_location.clone(),
        }))
    }

    fn violation_type(&self) -> String {
        "visibility".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::{
        checker::common_test::tests::{
            TestChecker, build_expected_violation, default_defining_pack,
            default_referencing_pack, test_check,
        },
        pack::EnforcementGlobsIgnore,
    };

    use super::*;
    use crate::{
        pack::{CheckerSetting, Pack},
        *,
    };

    #[test]
    fn referencing_and_defining_pack_are_identical() -> anyhow::Result<()> {
        let mut test_checker = TestChecker {
            reference: None,
            configuration: None,
            referenced_constant_name: Some(String::from("::Bar")),
            defining_pack: Some(Pack {
                name: "packs/bar".to_owned(),
                enforce_visibility: Some(CheckerSetting::True),
                ..default_defining_pack()
            }),
            referencing_pack: Pack {
                name: "packs/bar".to_owned(),
                relative_path: PathBuf::from("packs/bar"),
                ..default_referencing_pack()
            },
            ..Default::default()
        };
        test_check(&Checker {}, &mut test_checker)
    }

    #[test]
    fn test_with_violation() -> anyhow::Result<()> {
        let mut test_checker = TestChecker {
            reference: None,
            configuration: None,
            referenced_constant_name: Some(String::from("::Bar")),
            defining_pack: Some(Pack {
                name: "packs/bar".to_owned(),
                enforce_visibility: Some(CheckerSetting::True),
                ..default_defining_pack()
            }),
            referencing_pack: Pack{
                relative_path: PathBuf::from("packs/foo"),
                ..default_referencing_pack()},
            expected_violation: Some(build_expected_violation(
                "Visibility violation: `::Bar` belongs to `packs/bar`, which is not visible to `packs/foo`".to_string(),
                "visibility".to_string(), false)),
        };
        test_check(&Checker {}, &mut test_checker)
    }

    #[test]
    fn test_with_enforcement_globs_ignore() -> anyhow::Result<()> {
        let mut test_checker = TestChecker {
            reference: None,
            configuration: None,
            referenced_constant_name: Some(String::from("::Bar")),
            defining_pack: Some(Pack {
                name: "packs/bar".to_owned(),
                enforce_visibility: Some(CheckerSetting::True),
                enforcement_globs_ignore: Some(vec![EnforcementGlobsIgnore {
                    enforcements: HashSet::from(["visibility".to_string()]),
                    ignores: HashSet::from(["packs/foo/**".to_string()]),
                    reason: "foo is deprecated".to_string(),
                }]),
                ..default_defining_pack()
            }),
            referencing_pack: Pack {
                relative_path: PathBuf::from("packs/foo"),
                ..default_referencing_pack()
            },
            ..Default::default()
        };
        test_check(&Checker {}, &mut test_checker)
    }

    #[test]
    fn test_with_strict_violation() -> anyhow::Result<()> {
        let mut test_checker = TestChecker {
            reference: None,
            configuration: None,
            referenced_constant_name: Some(String::from("::Bar")),
            defining_pack: Some(Pack {
                name: "packs/bar".to_owned(),
                enforce_visibility: Some(CheckerSetting::Strict),
                ..default_defining_pack()
            }),
            referencing_pack: Pack{
                relative_path: PathBuf::from("packs/foo"),
                ..default_referencing_pack()},
            expected_violation: Some(build_expected_violation(
                "Visibility violation: `::Bar` belongs to `packs/bar`, which is not visible to `packs/foo`".to_string(),
                "visibility".to_string(), true)),
        };
        test_check(&Checker {}, &mut test_checker)
    }

    #[test]
    fn reference_is_not_a_visibility_violation() -> anyhow::Result<()> {
        let mut visible_to = HashSet::new();
        visible_to.insert(String::from("packs/foo"));

        let mut test_checker = TestChecker {
            reference: None,
            configuration: None,
            referenced_constant_name: Some(String::from("::Bar")),
            defining_pack: Some(Pack {
                name: "packs/bar".to_owned(),
                enforce_visibility: Some(CheckerSetting::True),
                visible_to: Some(visible_to),
                ..default_defining_pack()
            }),
            referencing_pack: Pack {
                relative_path: PathBuf::from("packs/foo"),
                ..default_referencing_pack()
            },
            ..Default::default()
        };
        test_check(&Checker {}, &mut test_checker)
    }
}
