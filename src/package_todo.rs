use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize, Serializer, ser::SerializeMap};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::debug;

use anyhow::Context;

use super::{Configuration, Violation, pack::Pack};

#[derive(Debug, Default)]
pub struct UpdateStats {
    pub violations_added: usize,
    pub violations_removed: usize,
    pub files_changed: usize,
    pub files_added: usize,
    pub files_deleted: usize,
}

impl UpdateStats {
    pub const fn is_empty(&self) -> bool {
        self.violations_added == 0
            && self.violations_removed == 0
            && self.files_changed == 0
            && self.files_added == 0
            && self.files_deleted == 0
    }
}

fn count_violations(package_todo: &PackageTodo) -> usize {
    package_todo
        .violations_by_defining_pack
        .values()
        .flat_map(|by_constant| by_constant.values())
        .map(|group| group.files.len())
        .sum()
}

#[derive(PartialEq, Debug, Eq, Deserialize, Serialize, Default, Clone)]
pub struct ViolationGroup {
    #[serde(rename = "violations", serialize_with = "serialize_sorted_set")]
    pub violation_types: HashSet<String>,
    #[serde(serialize_with = "serialize_sorted_set")]
    pub files: HashSet<String>,
}

fn serialize_sorted_set<S>(
    set: &HashSet<String>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut sorted_files: Vec<&String> = set.iter().collect();
    sorted_files.sort();
    sorted_files.serialize(serializer)
}

#[derive(PartialEq, Eq, Debug, Deserialize, Serialize, Default, Clone)]
pub struct PackageTodo {
    #[serde(flatten, serialize_with = "serialize_violations_by_defining_pack")]
    pub violations_by_defining_pack:
        BTreeMap<String, BTreeMap<String, ViolationGroup>>,
}

fn serialize_violations_by_defining_pack<S>(
    map: &BTreeMap<String, BTreeMap<String, ViolationGroup>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut map_serializer = serializer.serialize_map(Some(map.len()))?;

    for (key, value) in map {
        let mut quoted_sorted_violations_by_constant: BTreeMap<
            String,
            ViolationGroup,
        > = BTreeMap::new();
        for (constant_name, violation_group) in value {
            // HACK: This is the first part of a hack (search `HACK:` for more)
            let quoted_constant_name = format!("#{}#", constant_name);

            // The issue is that I have not been able to figure out how to get serde to serialize
            // a String key with double quotes.
            // When I tried this:
            // let quoted_constant_name = format!("\"{}\"", constant_name);
            // serde_yaml would escape the quotes, so I would get this:
            // '\"::Bar\"'
            // (uncomment the above and run tests to reproduce)
            quoted_sorted_violations_by_constant
                .insert(quoted_constant_name, violation_group.clone());
        }
        let modified_key = if key == &String::from(".") {
            String::from("#.#")
        } else {
            key.to_owned()
        };

        map_serializer.serialize_entry(
            &modified_key,
            &quoted_sorted_violations_by_constant,
        )?;
    }

    map_serializer.end()
}

pub fn package_todos_for_pack_name(
    violations_by_responsible_pack_name: HashMap<String, Vec<Violation>>,
) -> HashMap<String, PackageTodo> {
    let mut ret = HashMap::new();

    // package_todo.yml groups violations by the defining pack.
    for (responsible_pack_name, mut violations) in
        violations_by_responsible_pack_name
    {
        let mut violations_by_defining_pack: BTreeMap<
            String,
            BTreeMap<String, ViolationGroup>,
        > = BTreeMap::new();
        // Stable output prevents parallel collection from changing the file.
        violations.sort_by(|a, b| {
            a.identifier
                .defining_pack_name
                .cmp(&b.identifier.defining_pack_name)
                .then_with(|| {
                    a.identifier.constant_name.cmp(&b.identifier.constant_name)
                })
                .then_with(|| a.identifier.file.cmp(&b.identifier.file))
        });

        for violation in violations {
            let defining_pack_name =
                violation.identifier.defining_pack_name.to_owned();
            let existing_violations_by_constant_group =
                violations_by_defining_pack
                    .entry(defining_pack_name)
                    .or_default();

            let violation_group = existing_violations_by_constant_group
                .entry(violation.identifier.constant_name.to_owned())
                .or_default();

            violation_group
                .files
                .insert(violation.identifier.file.to_owned());
            violation_group
                .violation_types
                .insert(violation.identifier.violation_type.to_owned());
        }

        let package_todo = PackageTodo {
            violations_by_defining_pack,
        };

        ret.insert(responsible_pack_name, package_todo);
    }

    ret
}
pub fn write_violations_to_disk(
    configuration: &Configuration,
    violations: HashSet<Violation>,
) -> anyhow::Result<UpdateStats> {
    debug!("Starting writing violations to disk");
    // The referencing pack owns the todo entry for every current checker.
    let mut violations_by_responsible_pack: HashMap<String, Vec<Violation>> =
        HashMap::new();
    for violation in violations {
        if violation.identifier.strict {
            continue;
        }
        let referencing_pack_name =
            violation.identifier.referencing_pack_name.to_owned();
        violations_by_responsible_pack
            .entry(referencing_pack_name)
            .or_default()
            .push(violation);
    }

    let package_todos_by_pack_name =
        package_todos_for_pack_name(violations_by_responsible_pack);

    let violations_added = AtomicUsize::new(0);
    let violations_removed = AtomicUsize::new(0);
    let files_changed = AtomicUsize::new(0);
    let files_added = AtomicUsize::new(0);
    let files_deleted = AtomicUsize::new(0);

    let all_packs = &configuration.pack_set.packs;
    let results: Vec<anyhow::Result<()>> = all_packs
        .par_iter()
        .map(|p| {
            let new_package_todo = package_todos_by_pack_name.get(&p.name);
            let old_count = count_violations(&p.package_todo);
            let old_exists =
                !p.package_todo.violations_by_defining_pack.is_empty();

            match new_package_todo {
                Some(package_todo) => {
                    let new_count = count_violations(package_todo);
                    match new_count.cmp(&old_count) {
                        std::cmp::Ordering::Greater => {
                            violations_added.fetch_add(
                                new_count - old_count,
                                Ordering::Relaxed,
                            );
                        }
                        std::cmp::Ordering::Less => {
                            violations_removed.fetch_add(
                                old_count - new_count,
                                Ordering::Relaxed,
                            );
                        }
                        std::cmp::Ordering::Equal => {}
                    }

                    if old_exists {
                        if &p.package_todo != package_todo {
                            files_changed.fetch_add(1, Ordering::Relaxed);
                        }
                    } else {
                        files_added.fetch_add(1, Ordering::Relaxed);
                    }

                    write_package_todo_to_disk(
                        p,
                        package_todo,
                        configuration.crabwerk_first_mode,
                    )
                }
                None => {
                    if old_exists {
                        violations_removed
                            .fetch_add(old_count, Ordering::Relaxed);
                        files_deleted.fetch_add(1, Ordering::Relaxed);
                    }
                    delete_package_todo_from_disk(p)
                }
            }
        })
        .collect();

    collect_write_failures(results)?;

    debug!("Finished writing violations to disk");

    Ok(UpdateStats {
        violations_added: violations_added.load(Ordering::Relaxed),
        violations_removed: violations_removed.load(Ordering::Relaxed),
        files_changed: files_changed.load(Ordering::Relaxed),
        files_added: files_added.load(Ordering::Relaxed),
        files_deleted: files_deleted.load(Ordering::Relaxed),
    })
}

fn merge_package_todo(base: &PackageTodo, new: &PackageTodo) -> PackageTodo {
    let mut merged = base.clone();
    for (defining_pack, constants) in &new.violations_by_defining_pack {
        let existing_constants = merged
            .violations_by_defining_pack
            .entry(defining_pack.clone())
            .or_default();
        for (constant_name, violation_group) in constants {
            let existing_group =
                existing_constants.entry(constant_name.clone()).or_default();
            existing_group
                .files
                .extend(violation_group.files.iter().cloned());
            existing_group
                .violation_types
                .extend(violation_group.violation_types.iter().cloned());
        }
    }
    merged
}

pub fn merge_violations_to_disk(
    configuration: &Configuration,
    violations: HashSet<Violation>,
) -> anyhow::Result<UpdateStats> {
    debug!("Starting merging violations to disk");
    let mut violations_by_responsible_pack: HashMap<String, Vec<Violation>> =
        HashMap::new();
    for violation in violations {
        if violation.identifier.strict {
            continue;
        }
        let referencing_pack_name =
            violation.identifier.referencing_pack_name.to_owned();
        violations_by_responsible_pack
            .entry(referencing_pack_name)
            .or_default()
            .push(violation);
    }

    let new_package_todos =
        package_todos_for_pack_name(violations_by_responsible_pack);

    let violations_added = AtomicUsize::new(0);
    let files_changed = AtomicUsize::new(0);
    let files_added = AtomicUsize::new(0);

    let all_packs = &configuration.pack_set.packs;
    let results: Vec<anyhow::Result<()>> = all_packs
        .par_iter()
        .map(|p| {
            if let Some(new_todo) = new_package_todos.get(&p.name) {
                let old_count = count_violations(&p.package_todo);
                let old_exists =
                    !p.package_todo.violations_by_defining_pack.is_empty();
                let merged = merge_package_todo(&p.package_todo, new_todo);
                let merged_count = count_violations(&merged);

                if merged_count > old_count {
                    violations_added
                        .fetch_add(merged_count - old_count, Ordering::Relaxed);
                }

                if merged != p.package_todo {
                    if old_exists {
                        files_changed.fetch_add(1, Ordering::Relaxed);
                    } else {
                        files_added.fetch_add(1, Ordering::Relaxed);
                    }
                    return write_package_todo_to_disk(
                        p,
                        &merged,
                        configuration.crabwerk_first_mode,
                    );
                }
            }

            Ok(())
        })
        .collect();

    collect_write_failures(results)?;

    debug!("Finished merging violations to disk");

    Ok(UpdateStats {
        violations_added: violations_added.load(Ordering::Relaxed),
        violations_removed: 0,
        files_changed: files_changed.load(Ordering::Relaxed),
        files_added: files_added.load(Ordering::Relaxed),
        files_deleted: 0,
    })
}

/// Lint all package_todo.yml files by reading and rewriting them with proper sorting
pub fn lint_package_todo_yml_files(
    configuration: &Configuration,
) -> anyhow::Result<()> {
    let all_packs = &configuration.pack_set.packs;
    let results: Vec<anyhow::Result<()>> = all_packs
        .par_iter()
        .map(|p| {
            if p.package_todo.violations_by_defining_pack.is_empty() {
                return Ok(());
            }

            write_package_todo_to_disk(
                p,
                &p.package_todo,
                configuration.crabwerk_first_mode,
            )
        })
        .collect();

    collect_write_failures(results)
}

fn serialize_package_todo(
    responsible_pack_name: &String,
    package_todo: &PackageTodo,
    crabwerk_first_mode: bool,
) -> anyhow::Result<String> {
    let package_todo_yml = serde_yaml::to_string(&package_todo)
        .context("Could not serialize the package_todo.yml contents")?;

    // HACK: This is the other part of the hack above (search `HACK:` for more)
    let package_todo_yml = package_todo_yml.replace("'#", "\"");
    let package_todo_yml = package_todo_yml.replace("#'", "\"");
    let header = header(responsible_pack_name, crabwerk_first_mode);
    Ok(header + &package_todo_yml)
}

fn write_package_todo_to_disk(
    responsible_pack: &Pack,
    package_todo: &PackageTodo,
    crabwerk_first_mode: bool,
) -> anyhow::Result<()> {
    let package_todo_yml_absolute_filepath = responsible_pack
        .yml
        .parent()
        .unwrap()
        .join("package_todo.yml");

    let package_todo_yml = serialize_package_todo(
        &responsible_pack.name,
        package_todo,
        crabwerk_first_mode,
    )?;

    std::fs::write(&package_todo_yml_absolute_filepath, package_todo_yml)
        .with_context(|| {
            format!(
                "Could not write {}",
                package_todo_yml_absolute_filepath.display()
            )
        })
}

fn delete_package_todo_from_disk(
    responsible_pack: &Pack,
) -> anyhow::Result<()> {
    let package_todo_yml_absolute_filepath = responsible_pack
        .yml
        .parent()
        .unwrap()
        .join("package_todo.yml");

    if package_todo_yml_absolute_filepath.exists() {
        std::fs::remove_file(&package_todo_yml_absolute_filepath)
            .with_context(|| {
                format!(
                    "Could not delete {}",
                    package_todo_yml_absolute_filepath.display()
                )
            })?;
    }

    Ok(())
}

/// A write that fails part-way through leaves some `package_todo.yml` files
/// rewritten and some not, so every failure is reported rather than only the
/// first one a rayon worker happened to hit.
fn collect_write_failures(
    results: Vec<anyhow::Result<()>>,
) -> anyhow::Result<()> {
    let failures: Vec<String> = results
        .into_iter()
        .filter_map(|result| result.err())
        .map(|error| format!("  {:#}", error))
        .collect();

    if failures.is_empty() {
        return Ok(());
    }

    anyhow::bail!(
        "Could not update {} package_todo.yml file(s). Some files may already have been rewritten:\n{}",
        failures.len(),
        failures.join("\n")
    )
}

fn header(responsible_pack_name: &String, crabwerk_first_mode: bool) -> String {
    let command = if crabwerk_first_mode {
        "crabwerk update"
    } else {
        "bin/packwerk update-todo"
    };

    format!("\
# This file contains a list of dependencies that are not part of the long term plan for the
# '{}' package.
# We should generally work to reduce this list over time.
#
# You can regenerate this file using the following command:
#
# {}
---
", responsible_pack_name, command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn construct_violations(
        constant_name: String,
        input_types: Vec<String>,
        input_files: Vec<String>,
    ) -> BTreeMap<String, ViolationGroup> {
        let mut bar_violations = BTreeMap::new();
        let mut files = HashSet::new();
        let mut violation_types = HashSet::new();

        for file in input_files {
            files.insert(file);
        }

        for violation_type in input_types {
            violation_types.insert(violation_type);
        }

        bar_violations.insert(
            constant_name,
            ViolationGroup {
                violation_types,
                files,
            },
        );

        bar_violations
    }

    fn bar_violations() -> BTreeMap<String, ViolationGroup> {
        construct_violations(
            String::from("::Bar"),
            vec![String::from("dependency")],
            vec![String::from("packs/foo/app/services/foo.rb")],
        )
    }

    fn bar_blah_violations() -> BTreeMap<String, ViolationGroup> {
        construct_violations(
            String::from("::BarBlah"),
            vec![String::from("dependency")],
            vec![String::from("packs/foo/app/services/foo.rb")],
        )
    }

    fn baz_violations() -> BTreeMap<String, ViolationGroup> {
        construct_violations(
            String::from("::Baz"),
            vec![String::from("dependency"), String::from("privacy")],
            vec![String::from("packs/foo/app/services/foo.rb")],
        )
    }

    fn example_package_todo(defining_package_name: String) -> PackageTodo {
        let mut violations_by_defining_pack: BTreeMap<
            String,
            BTreeMap<String, ViolationGroup>,
        > = BTreeMap::new();
        let bar_violations = bar_violations();
        let bar_blah_violations = bar_blah_violations();
        let baz_violations = baz_violations();
        let mut merged_map: BTreeMap<String, ViolationGroup> = BTreeMap::new();
        merged_map.extend(bar_violations);
        merged_map.extend(bar_blah_violations);
        merged_map.extend(baz_violations);

        violations_by_defining_pack.insert(defining_package_name, merged_map);

        PackageTodo {
            violations_by_defining_pack,
        }
    }

    #[test]
    fn test_deserialize_trivial_case() {
        let contents: String = String::from(
            "
        # This file contains a list of dependencies that are not part of the long term plan for the
        # 'packs/foo' package.
        # We should generally work to reduce this list over time.
        #
        # You can regenerate this file using the following command:
        #
        # bin/packwerk update-todo
        packs/bar:
            \"::Bar\":
                violations:
                - dependency
                files:
                - packs/foo/app/services/foo.rb
            \"::BarBlah\":
                violations:
                - dependency
                files:
                - packs/foo/app/services/foo.rb
            \"::Baz\":
                violations:
                - dependency
                - privacy
                files:
                - packs/foo/app/services/foo.rb
        ",
        );

        let expected = example_package_todo(String::from("packs/bar"));

        let actual: PackageTodo = serde_yaml::from_str(&contents).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_serialize_trivial_case() {
        let expected: String = String::from(
            "\
# This file contains a list of dependencies that are not part of the long term plan for the
# 'packs/foo' package.
# We should generally work to reduce this list over time.
#
# You can regenerate this file using the following command:
#
# bin/packwerk update-todo
---
packs/bar:
  \"::Bar\":
    violations:
    - dependency
    files:
    - packs/foo/app/services/foo.rb
  \"::BarBlah\":
    violations:
    - dependency
    files:
    - packs/foo/app/services/foo.rb
  \"::Baz\":
    violations:
    - dependency
    - privacy
    files:
    - packs/foo/app/services/foo.rb
",
);

        let actual_package_todo =
            example_package_todo(String::from("packs/bar"));
        let actual = serialize_package_todo(
            &String::from("packs/foo"),
            &actual_package_todo,
            false,
        )
        .unwrap();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_serialize_violation_against_root() {
        let expected: String = String::from(
            "\
# This file contains a list of dependencies that are not part of the long term plan for the
# 'packs/foo' package.
# We should generally work to reduce this list over time.
#
# You can regenerate this file using the following command:
#
# bin/packwerk update-todo
---
\".\":
  \"::Bar\":
    violations:
    - dependency
    files:
    - packs/foo/app/services/foo.rb
  \"::BarBlah\":
    violations:
    - dependency
    files:
    - packs/foo/app/services/foo.rb
  \"::Baz\":
    violations:
    - dependency
    - privacy
    files:
    - packs/foo/app/services/foo.rb
",
);

        let actual_package_todo = example_package_todo(String::from("."));
        let actual = serialize_package_todo(
            &String::from("packs/foo"),
            &actual_package_todo,
            false,
        )
        .unwrap();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_serialize_trivial_case_in_crabwerk_first_mode() {
        let expected: String = String::from(
            "\
# This file contains a list of dependencies that are not part of the long term plan for the
# 'packs/foo' package.
# We should generally work to reduce this list over time.
#
# You can regenerate this file using the following command:
#
# crabwerk update
---
packs/bar:
  \"::Bar\":
    violations:
    - dependency
    files:
    - packs/foo/app/services/foo.rb
  \"::BarBlah\":
    violations:
    - dependency
    files:
    - packs/foo/app/services/foo.rb
  \"::Baz\":
    violations:
    - dependency
    - privacy
    files:
    - packs/foo/app/services/foo.rb
",
);

        let actual_package_todo =
            example_package_todo(String::from("packs/bar"));
        let actual = serialize_package_todo(
            &String::from("packs/foo"),
            &actual_package_todo,
            true,
        )
        .unwrap();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_write_package_todo_to_disk_reports_an_unwritable_path() {
        let pack = Pack::from_contents(
            Path::new("/nope/nothing/here/packs/foo/package.yml"),
            Path::new("/nope/nothing/here"),
            "enforce_dependencies: true\n",
            PackageTodo::default(),
        )
        .unwrap();

        let error = write_package_todo_to_disk(
            &pack,
            &example_package_todo(String::from("packs/bar")),
            true,
        )
        .expect_err("expected an error, not a panic");

        assert!(
            format!("{:#}", error).contains("package_todo.yml"),
            "expected the message to name the file, got: {:#}",
            error
        );
    }

    #[test]
    fn test_merge_package_todo_adds_new_entries() {
        let base = PackageTodo {
            violations_by_defining_pack: {
                let mut map = BTreeMap::new();
                map.insert("packs/bar".to_string(), bar_violations());
                map
            },
        };

        let new = PackageTodo {
            violations_by_defining_pack: {
                let mut map = BTreeMap::new();
                map.insert("packs/bar".to_string(), baz_violations());
                map
            },
        };

        let merged = merge_package_todo(&base, &new);
        let bar_pack = &merged.violations_by_defining_pack["packs/bar"];
        assert!(
            bar_pack.contains_key("::Bar"),
            "original entry should be preserved"
        );
        assert!(bar_pack.contains_key("::Baz"), "new entry should be added");
    }

    #[test]
    fn test_merge_package_todo_merges_files_and_types() {
        let mut base_violations = BTreeMap::new();
        base_violations.insert(
            "::Bar".to_string(),
            ViolationGroup {
                violation_types: HashSet::from(["dependency".to_string()]),
                files: HashSet::from(["file_a.rb".to_string()]),
            },
        );
        let base = PackageTodo {
            violations_by_defining_pack: {
                let mut map = BTreeMap::new();
                map.insert("packs/bar".to_string(), base_violations);
                map
            },
        };

        let mut new_violations = BTreeMap::new();
        new_violations.insert(
            "::Bar".to_string(),
            ViolationGroup {
                violation_types: HashSet::from(["privacy".to_string()]),
                files: HashSet::from(["file_b.rb".to_string()]),
            },
        );
        let new = PackageTodo {
            violations_by_defining_pack: {
                let mut map = BTreeMap::new();
                map.insert("packs/bar".to_string(), new_violations);
                map
            },
        };

        let merged = merge_package_todo(&base, &new);
        let group = &merged.violations_by_defining_pack["packs/bar"]["::Bar"];
        assert!(group.files.contains("file_a.rb"));
        assert!(group.files.contains("file_b.rb"));
        assert!(group.violation_types.contains("dependency"));
        assert!(group.violation_types.contains("privacy"));
    }

    #[test]
    fn test_merge_package_todo_preserves_unrelated_packs() {
        let base = PackageTodo {
            violations_by_defining_pack: {
                let mut map = BTreeMap::new();
                map.insert("packs/existing".to_string(), bar_violations());
                map
            },
        };

        let new = PackageTodo {
            violations_by_defining_pack: {
                let mut map = BTreeMap::new();
                map.insert("packs/new".to_string(), baz_violations());
                map
            },
        };

        let merged = merge_package_todo(&base, &new);
        assert!(
            merged
                .violations_by_defining_pack
                .contains_key("packs/existing")
        );
        assert!(merged.violations_by_defining_pack.contains_key("packs/new"));
    }
}
