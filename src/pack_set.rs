use anyhow::{Result, bail};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use itertools::Itertools;

use super::{Configuration, checker::ViolationIdentifier, pack::Pack};

#[derive(Default, Debug)]
pub struct PackSet {
    pub packs: Vec<Pack>,
    // An index into `packs`, not a second copy of it: a `Pack` carries its
    // whole `package_todo.yml`, which is large mid-adoption.
    index_by_name: HashMap<String, usize>,
    owning_pack_name_for_file: HashMap<PathBuf, String>,
    // For now, we keep track of all violations so that we can diff them and only
    // present the ones that are not recorded.
    // Eventually, we'll need to rewrite these to disk, in which case we'll need
    // more info in these Violations to aggregate them into package_todo.yml files.
    // We will also likely want to have an optimization that only rewrites the files
    // that have different violations.
    pub all_violations: HashSet<ViolationIdentifier>,
}

#[derive(Debug)]
pub struct PackDependency<'a> {
    // from_pack has a package.yml dependency on to_pack
    pub from_pack: &'a Pack,
    pub to_pack: &'a Pack,
}

impl PackSet {
    pub fn build(
        packs: HashSet<Pack>,
        owning_package_yml_for_file: HashMap<PathBuf, PathBuf>,
    ) -> anyhow::Result<Self> {
        // Name length descending, so that a caller scanning for the pack that
        // owns a path meets the most nested pack first; `move_to_pack` relies
        // on it. The name tiebreak keeps the order stable.
        let packs: Vec<Pack> = packs
            .into_iter()
            .sorted_by(|packa, packb| {
                Ord::cmp(&packb.name.len(), &packa.name.len())
                    .then_with(|| packa.name.cmp(&packb.name))
            })
            .collect();
        let mut index_by_name: HashMap<String, usize> = HashMap::new();
        let mut indexed_packs_by_yml: HashMap<PathBuf, String> = HashMap::new();

        let mut all_violations = HashSet::new();
        for (index, pack) in packs.iter().enumerate() {
            index_by_name.insert(pack.name.clone(), index);
            indexed_packs_by_yml.insert(pack.yml.clone(), pack.name.clone());
            for violation_identifier in pack.all_violations() {
                all_violations.insert(violation_identifier);
            }
        }

        let mut owning_pack_name_for_file: HashMap<PathBuf, String> =
            HashMap::new();

        for (file, package_yml) in owning_package_yml_for_file {
            if let Some(pack_name) = indexed_packs_by_yml.get(&package_yml) {
                owning_pack_name_for_file.insert(file, pack_name.clone());
            }
        }

        if !index_by_name.contains_key(".") {
            bail!(
                "No root pack found. First double check a root pack exists (a package.yml file in the application root). Secondly, double check your configuration file's `package_paths` includes the root pack by using command crabwerk list-packs."
            );
        }

        Ok(Self {
            index_by_name,
            packs,
            all_violations,
            owning_pack_name_for_file,
        })
    }

    pub fn for_file(
        &self,
        absolute_file_path: &Path,
    ) -> anyhow::Result<Option<&Pack>> {
        self.owning_pack_name_for_file
            .get(absolute_file_path)
            .map(|pack_name| self.for_pack(pack_name))
            .transpose()
            .map_err(|_| {
                anyhow::Error::msg(format!(
                    "Walking the directory identified that the following file belongs to a pack, but that pack cannot be found in the packset:\n{}",
                    absolute_file_path.display()
                ))
            })
    }

    pub fn for_pack(&self, pack_name: &str) -> Result<&Pack> {
        // Trim trailing slash on pack_name.
        // Since often the input arg here comes from the command line,
        // a command line auto-completer may add a trailing slash.
        let pack_name = pack_name.trim_end_matches('/');
        if let Some(&index) = self.index_by_name.get(pack_name) {
            Ok(&self.packs[index])
        } else {
            bail!("No pack found '{}'", pack_name)
        }
    }

    // Takes every wanted pack at once, because the file index is keyed by file:
    // answering one pack at a time would rescan the whole repo per pack.
    pub fn files_for_packs(
        &self,
        pack_names: &HashSet<String>,
    ) -> HashSet<PathBuf> {
        self.owning_pack_name_for_file
            .iter()
            .filter(|(_, name)| pack_names.contains(name.as_str()))
            .map(|(path, _)| path.clone())
            .collect()
    }

    // Returns all of the package dependencies in the pack set.
    pub fn all_pack_dependencies<'a>(
        &'a self,
        configuration: &'a Configuration,
    ) -> Result<Vec<PackDependency<'a>>> {
        let mut pack_refs: Vec<PackDependency> = Vec::new();
        for from_pack in &configuration.pack_set.packs {
            // Sorted because `dependencies` is a set: the order the edges reach
            // the dependency graph decides the order of the reported cycles and
            // the paths shown in the strict-transitive errors.
            for dependency_pack_name in from_pack.dependencies.iter().sorted() {
                match configuration.pack_set.for_pack(dependency_pack_name) {
                    Ok(to_pack) => {
                        pack_refs.push(PackDependency { from_pack, to_pack })
                    }
                    Err(_) => {
                        bail!(
                            "{} has '{}' in its dependencies, but that pack cannot be found. Try `crabwerk list-packs` to debug.",
                            from_pack.yml.to_string_lossy(),
                            dependency_pack_name
                        );
                    }
                }
            }
        }
        Ok(pack_refs)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;

    use crate::pack::Pack;

    use super::PackSet;

    fn example_pack_set() -> PackSet {
        let foo_pack = Pack {
            name: "packs/foo".to_string(),
            ..Pack::default()
        };
        let root_pack = Pack {
            name: ".".to_string(),
            ..Pack::default()
        };
        let mut packs = HashSet::new();
        packs.insert(foo_pack);
        packs.insert(root_pack);
        PackSet::build(packs, HashMap::new()).unwrap()
    }

    #[test]
    fn from_pack() {
        let pack_set = example_pack_set();
        let actual_pack = pack_set.for_pack("packs/foo");
        assert!(actual_pack.is_ok());
    }

    #[test]
    fn from_pack_with_slash() {
        let pack_set = example_pack_set();
        let actual_pack = pack_set.for_pack("packs/foo/");
        assert!(actual_pack.is_ok());
    }

    #[test]
    fn from_unknown_pack() {
        let pack_set = example_pack_set();
        let error = pack_set
            .for_pack("packs/nope")
            .expect_err("an unknown pack should be an error");
        assert_eq!(error.to_string(), "No pack found 'packs/nope'");
    }

    #[test]
    fn build_without_a_root_pack() {
        let mut packs = HashSet::new();
        packs.insert(Pack {
            name: "packs/foo".to_string(),
            ..Pack::default()
        });

        let error = PackSet::build(packs, HashMap::new())
            .expect_err("a pack set without a root pack should be an error");
        let message = error.to_string();
        assert!(
            message.starts_with("No root pack found."),
            "unexpected error message: {}",
            message
        );
        // `PackSet` has no way to know which configuration file was read, so
        // the message must not name one.
        assert!(
            !message.contains("packwerk.yml")
                && !message.contains("crabwerk.yml"),
            "the message should not name a configuration file: {}",
            message
        );
    }

    #[test]
    fn for_file_returns_the_owning_pack() {
        let foo_yml = PathBuf::from("packs/foo/package.yml");
        let foo_pack = Pack {
            name: "packs/foo".to_string(),
            yml: foo_yml.clone(),
            ..Pack::default()
        };
        let mut packs = HashSet::new();
        packs.insert(foo_pack);
        packs.insert(Pack {
            name: ".".to_string(),
            yml: PathBuf::from("package.yml"),
            ..Pack::default()
        });

        let file = PathBuf::from("packs/foo/app/services/foo.rb");
        let mut owning_package_yml_for_file = HashMap::new();
        owning_package_yml_for_file.insert(file.clone(), foo_yml);

        let pack_set =
            PackSet::build(packs, owning_package_yml_for_file).unwrap();

        assert_eq!(
            pack_set.for_file(&file).unwrap().map(|p| p.name.as_str()),
            Some("packs/foo")
        );
        assert_eq!(
            pack_set
                .for_file(&PathBuf::from("packs/foo/app/services/other.rb"))
                .unwrap(),
            None
        );
        assert_eq!(
            pack_set.files_for_packs(&HashSet::from(["packs/foo".to_string()])),
            HashSet::from([file])
        );
    }

    #[test]
    fn for_pack_borrows_from_packs_rather_than_a_second_copy() {
        let pack_set = example_pack_set();
        let from_index = pack_set.for_pack("packs/foo").unwrap();
        let from_vec = pack_set
            .packs
            .iter()
            .find(|pack| pack.name == "packs/foo")
            .unwrap();
        assert!(std::ptr::eq(from_index, from_vec));
    }
}
