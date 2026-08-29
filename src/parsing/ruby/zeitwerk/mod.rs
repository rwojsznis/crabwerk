mod constant_resolver;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use rayon::prelude::{ParallelBridge, ParallelIterator};

use crate::{
    PackSet,
    constant_resolver::{
        ConstantDefinition, ConstantResolver, ConstantResolverConfiguration,
    },
    file_utils::expand_glob,
    pack::Pack,
};

use self::constant_resolver::ZeitwerkConstantResolver;

use super::{inflector, inflector::Acronyms};

pub fn get_zeitwerk_constant_resolver(
    pack_set: &PackSet,
    configuration: &ConstantResolverConfiguration,
) -> anyhow::Result<Box<dyn ConstantResolver + Send + Sync>> {
    let constants = inferred_constants_from_pack_set(pack_set, configuration)?;

    ZeitwerkConstantResolver::create(constants, configuration.absolute_root)
}

#[derive(Debug)]
struct PackNamespaceSettings {
    automatic_pack_namespace: bool,
    automatic_pack_namespace_exclusions: HashSet<PathBuf>,
}

fn get_pack_namespace_settings(pack: &Pack) -> PackNamespaceSettings {
    pack.client_keys
        .get("metadata")
        .and_then(|metadata| {
            if let serde_json::Value::Object(map) = metadata {
                let automatic_pack_namespace = map
                    .get("automatic_pack_namespace")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);

                let automatic_pack_namespace_exclusions: HashSet<PathBuf> = map
                    .get("automatic_pack_namespace_exclusions")
                    .and_then(serde_json::Value::as_array)
                    .map(|seq| {
                        seq.iter()
                            .filter_map(|v| {
                                v.as_str().map(|s| {
                                    let mut full_path = pack.yml.clone();
                                    full_path.pop();
                                    full_path.push(s);
                                    full_path
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                Some(PackNamespaceSettings {
                    automatic_pack_namespace,
                    automatic_pack_namespace_exclusions,
                })
            } else {
                None
            }
        })
        .unwrap_or(PackNamespaceSettings {
            automatic_pack_namespace: false,
            automatic_pack_namespace_exclusions: HashSet::new(),
        })
}

fn inferred_constants_from_pack_set(
    pack_set: &PackSet,
    configuration: &ConstantResolverConfiguration,
) -> anyhow::Result<Vec<ConstantDefinition>> {
    let mut full_autoload_roots: HashMap<PathBuf, String> = HashMap::new();
    for pack in &pack_set.packs {
        let PackNamespaceSettings {
            automatic_pack_namespace,
            automatic_pack_namespace_exclusions,
        } = get_pack_namespace_settings(pack);

        for path in pack.default_autoload_roots()? {
            let namespace = if automatic_pack_namespace
                && !automatic_pack_namespace_exclusions.contains(&path)
            {
                format!(
                    "::{}",
                    inflector::camelize(
                        pack.last_name(),
                        configuration.acronyms
                    )
                )
            } else {
                String::from("")
            };

            full_autoload_roots.insert(path, namespace);
        }
    }

    // Explicit roots take precedence over inferred roots.
    for (rel_path, ns) in configuration.autoload_roots {
        let abs_path = configuration.absolute_root.join(rel_path);
        let ns = if ns == "::Object" {
            String::from("")
        } else {
            ns.to_owned()
        };
        for path in expand_glob(&abs_path.to_string_lossy())? {
            full_autoload_roots.insert(path, ns.clone());
        }
    }

    inferred_constants_from_autoload_paths(configuration, full_autoload_roots)
}

fn inferred_constants_from_autoload_paths(
    configuration: &ConstantResolverConfiguration,
    full_autoload_roots: HashMap<PathBuf, String>,
) -> anyhow::Result<Vec<ConstantDefinition>> {
    let autoload_paths_to_their_globbed_files = full_autoload_roots
        .keys()
        .par_bridge()
        .map(|absolute_autoload_path| {
            let glob_path = absolute_autoload_path.join("**/*.rb");
            let files = expand_glob(&glob_path.to_string_lossy())?;

            Ok((absolute_autoload_path, files))
        })
        .collect::<anyhow::Result<HashMap<&PathBuf, Vec<PathBuf>>>>()?;

    // The most specific autoload root owns files under nested roots.
    let mut file_to_longest_path: HashMap<&PathBuf, &PathBuf> = HashMap::new();

    for (autoload_path, files) in &autoload_paths_to_their_globbed_files {
        for file in files {
            let current_longest_path = file_to_longest_path
                .entry(file)
                .or_insert_with(|| autoload_path);

            if autoload_path.components().count()
                > current_longest_path.components().count()
            {
                *current_longest_path = autoload_path;
            }
        }
    }

    Ok(file_to_longest_path
        .into_iter()
        .par_bridge()
        .map(|(absolute_path_of_definition, absolute_autoload_path)| {
            let default_namespace =
                full_autoload_roots.get(absolute_autoload_path).unwrap();
            inferred_constant_from_file(
                absolute_path_of_definition,
                absolute_autoload_path,
                configuration.acronyms,
                default_namespace,
            )
        })
        .collect::<Vec<ConstantDefinition>>())
}

fn inferred_constant_from_file(
    absolute_path: &Path,
    absolute_autoload_path: &PathBuf,
    acronyms: &Acronyms,
    default_namespace: &String,
) -> ConstantDefinition {
    let relative_path =
        absolute_path.strip_prefix(absolute_autoload_path).unwrap();

    let relative_path = relative_path.with_extension("");

    let relative_path_str = relative_path.to_str().unwrap();
    let camelized_path = inflector::camelize(relative_path_str, acronyms);
    let fully_qualified_name =
        format!("{}::{}", default_namespace, camelized_path);

    ConstantDefinition {
        fully_qualified_name,
        absolute_path_of_definition: absolute_path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configuration;

    use crate::test_util::{
        SIMPLE_APP, get_absolute_root,
        get_zeitwerk_constant_resolver_for_fixture,
    };
    use pretty_assertions::assert_eq;

    #[test]
    fn unnested_reference_to_unnested_constant() {
        assert_eq!(
            vec![ConstantDefinition {
                fully_qualified_name: "::Foo".to_string(),
                absolute_path_of_definition: get_absolute_root(SIMPLE_APP)
                    .join("packs/foo/app/services/foo.rb")
            }],
            get_zeitwerk_constant_resolver_for_fixture(SIMPLE_APP)
                .unwrap()
                .resolve(&String::from("Foo"), &[])
                .unwrap()
        );
    }

    #[test]
    fn constant_in_overridden_namespace() {
        assert_eq!(
            vec![ConstantDefinition {
                fully_qualified_name: "::Company::Widget".to_string(),
                absolute_path_of_definition: get_absolute_root(SIMPLE_APP)
                    .join("app/company_data/widget.rb")
            }],
            get_zeitwerk_constant_resolver_for_fixture(SIMPLE_APP)
                .unwrap()
                .resolve(&String::from("Widget"), &["Company"])
                .unwrap()
        );
    }

    #[test]
    fn nested_reference_to_unnested_constant() {
        let absolute_root = get_absolute_root(SIMPLE_APP);
        let resolver =
            get_zeitwerk_constant_resolver_for_fixture(SIMPLE_APP).unwrap();

        assert_eq!(
            vec![ConstantDefinition {
                fully_qualified_name: "::Foo".to_string(),
                absolute_path_of_definition: absolute_root
                    .join("packs/foo/app/services/foo.rb")
            }],
            resolver
                .resolve(&String::from("Foo"), &["Foo", "Bar", "Baz"])
                .unwrap()
        );
    }

    #[test]
    fn nested_reference_to_nested_constant() {
        let absolute_root = get_absolute_root(SIMPLE_APP);
        let resolver =
            get_zeitwerk_constant_resolver_for_fixture(SIMPLE_APP).unwrap();
        assert_eq!(
            vec![ConstantDefinition {
                fully_qualified_name: "::Foo::Bar".to_string(),
                absolute_path_of_definition: absolute_root
                    .join("packs/foo/app/services/foo/bar.rb")
            }],
            resolver.resolve("Bar", &["Foo"]).unwrap()
        );
    }

    #[test]
    fn nested_reference_to_global_constant() {
        let absolute_root = get_absolute_root(SIMPLE_APP);
        let resolver =
            get_zeitwerk_constant_resolver_for_fixture(SIMPLE_APP).unwrap();

        assert_eq!(
            vec![ConstantDefinition {
                fully_qualified_name: "::Bar".to_string(),
                absolute_path_of_definition: absolute_root
                    .join("packs/bar/app/services/bar.rb")
            }],
            resolver.resolve("::Bar", &["Foo"]).unwrap()
        );
    }

    #[test]
    fn nested_reference_to_constant_defined_within_another_file() {
        let absolute_root = get_absolute_root(SIMPLE_APP);
        let resolver =
            get_zeitwerk_constant_resolver_for_fixture(SIMPLE_APP).unwrap();
        assert_eq!(
            vec![ConstantDefinition {
                fully_qualified_name: "::Bar::BAR".to_string(),
                absolute_path_of_definition: absolute_root
                    .join("packs/bar/app/services/bar.rb")
            }],
            resolver.resolve(&String::from("::Bar::BAR"), &[]).unwrap()
        );
    }

    #[test]
    fn inflected_constant() {
        let app = "tests/fixtures/app_with_inflections";
        let absolute_root = get_absolute_root(app);
        let resolver = get_zeitwerk_constant_resolver_for_fixture(app).unwrap();

        assert_eq!(
            vec![ConstantDefinition {
                fully_qualified_name: "::MyModule::SomeAPIClass".to_string(),
                absolute_path_of_definition: absolute_root
                    .join("app/services/my_module/some_api_class.rb")
            }],
            resolver
                .resolve(&String::from("::MyModule::SomeAPIClass"), &[])
                .unwrap()
        );

        assert_eq!(
            vec![ConstantDefinition {
                fully_qualified_name: "::MyModule::SomeCSVClass".to_string(),
                absolute_path_of_definition: absolute_root
                    .join("app/services/my_module/some_csv_class.rb")
            }],
            resolver
                .resolve(&String::from("::MyModule::SomeCSVClass"), &[])
                .unwrap()
        );
    }

    #[test]
    fn test_file_map() {
        let absolute_root = &PathBuf::from("tests/fixtures/simple_app")
            .canonicalize()
            .expect("Could not canonicalize path");

        let configuration = configuration::get(absolute_root, &0).unwrap();

        let constant_resolver = get_zeitwerk_constant_resolver(
            &configuration.pack_set,
            &configuration.constant_resolver_configuration(),
        )
        .unwrap();
        let actual_constant_map = constant_resolver
            .fully_qualified_constant_name_to_constant_definition_map();

        let mut expected_constant_map = HashMap::new();
        expected_constant_map.insert(
            String::from("::Foo::Bar"),
            vec![ConstantDefinition {
                fully_qualified_name: "::Foo::Bar".to_owned(),
                absolute_path_of_definition: absolute_root
                    .join("packs/foo/app/services/foo/bar.rb"),
            }],
        );

        expected_constant_map.insert(
            "::Bar".to_owned(),
            vec![ConstantDefinition {
                fully_qualified_name: "::Bar".to_owned(),
                absolute_path_of_definition: absolute_root
                    .join("packs/bar/app/services/bar.rb"),
            }],
        );
        expected_constant_map.insert(
            "::Baz".to_owned(),
            vec![ConstantDefinition {
                fully_qualified_name: "::Baz".to_owned(),
                absolute_path_of_definition: absolute_root
                    .join("packs/baz/app/services/baz.rb"),
            }],
        );
        expected_constant_map.insert(
            "::Foo".to_owned(),
            vec![ConstantDefinition {
                fully_qualified_name: "::Foo".to_owned(),
                absolute_path_of_definition: absolute_root
                    .join("packs/foo/app/services/foo.rb"),
            }],
        );
        expected_constant_map.insert(
            "::SomeConcern".to_owned(),
            vec![ConstantDefinition {
                fully_qualified_name: "::SomeConcern".to_owned(),
                absolute_path_of_definition: absolute_root
                    .join("packs/bar/app/models/concerns/some_concern.rb"),
            }],
        );
        expected_constant_map.insert(
            "::SomeRootClass".to_owned(),
            vec![ConstantDefinition {
                fully_qualified_name: "::SomeRootClass".to_owned(),
                absolute_path_of_definition: absolute_root
                    .join("app/services/some_root_class.rb"),
            }],
        );
        expected_constant_map.insert(
            "::Company::Widget".to_owned(),
            vec![ConstantDefinition {
                fully_qualified_name: "::Company::Widget".to_owned(),
                absolute_path_of_definition: absolute_root
                    .join("app/company_data/widget.rb"),
            }],
        );

        assert_eq!(&expected_constant_map, actual_constant_map);
    }

    use std::collections::HashMap;
    use std::path::PathBuf;
}
