use super::checker::layer::Layers;

use super::{
    PackSet, constant_resolver::ConstantResolverConfiguration,
    raw_configuration, raw_configuration::RawConfiguration, walk_directory,
    walk_directory::WalkDirectoryResult,
};

use std::collections::HashMap;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
use tracing::debug;
use walk_directory::walk_directory;

#[derive(Debug)]
pub struct Configuration {
    pub included_files: HashSet<PathBuf>,
    pub input_files_count: usize, // Helpful for optimizations in privacy checker
    pub absolute_root: PathBuf,
    pub config_file_path: Option<PathBuf>,
    pub pack_set: PackSet,
    pub layers: Layers,
    pub experimental_parser: bool,
    pub ignored_definitions: HashMap<String, HashSet<PathBuf>>,
    pub autoload_roots: HashMap<PathBuf, String>,
    pub inflections_path: PathBuf,
    pub custom_associations: Vec<String>,
    // Note that it'd probably be better to use the logger library, `tracing` (see logger.rs)
    // and configure logging in one place. As the complexity of how/why we want to see different logs
    // grows, we can refactor this.
    pub print_files: bool,
    pub crabwerk_first_mode: bool,
    /// Whether a printed report may carry ANSI colour codes. Resolved from
    /// `--color`, the terminal and `NO_COLOR`; off unless the CLI turns it on.
    pub color: bool,
    pub ignore_recorded_violations: bool,
    pub disable_enforce_dependencies: bool,
    pub disable_enforce_folder_privacy: bool,
    pub disable_enforce_layers: bool,
    pub disable_enforce_privacy: bool,
    pub disable_enforce_visibility: bool,
}

impl Configuration {
    /// The files the walk found that the user's path arguments select. An
    /// empty argument list selects every file.
    ///
    /// A directory argument is resolved against `included_files` rather than
    /// by globbing the filesystem a second time. A second glob can disagree
    /// with the walk — `**/*.*` needs a literal dot, so it drops `Gemfile`
    /// and `Rakefile`, which the walk includes — and it would have to repeat
    /// the `include` and `exclude` rules to agree about anything else.
    pub(crate) fn intersect_files(
        &self,
        input_files: Vec<String>,
    ) -> HashSet<PathBuf> {
        if input_files.is_empty() {
            return self.included_files.clone();
        }

        let mut selected = HashSet::new();
        for input_file in input_files {
            let path = PathBuf::from(&input_file);
            let absolute_path = if path.is_absolute() {
                path
            } else {
                self.absolute_root.join(path)
            };

            if absolute_path.is_dir() {
                selected.extend(
                    self.included_files
                        .iter()
                        .filter(|file| file.starts_with(&absolute_path))
                        .cloned(),
                );
            } else if self.included_files.contains(&absolute_path) {
                selected.insert(absolute_path);
            }
        }

        selected
    }

    /// How to name the configuration file in a message that asks the user to
    /// edit it.
    ///
    /// This is not always `crabwerk.yml`: `--config` can name any file, and a
    /// project with no configuration at all has none to name.
    pub(crate) fn config_file_name(&self) -> String {
        match &self.config_file_path {
            Some(path) => path
                .strip_prefix(&self.absolute_root)
                .unwrap_or(path)
                .display()
                .to_string(),
            None => raw_configuration::CRABWERK_CONFIG_FILE_NAME.to_string(),
        }
    }

    pub(crate) const fn constant_resolver_configuration(
        &self,
    ) -> ConstantResolverConfiguration<'_> {
        ConstantResolverConfiguration {
            absolute_root: &self.absolute_root,
            autoload_roots: &self.autoload_roots,
            inflections_path: &self.inflections_path,
        }
    }
}

pub fn get(
    absolute_root: &Path,
    input_files_count: &usize,
) -> anyhow::Result<Configuration> {
    get_with_config_path(absolute_root, input_files_count, None)
}

pub fn get_with_config_path(
    absolute_root: &Path,
    input_files_count: &usize,
    config_path: Option<&Path>,
) -> anyhow::Result<Configuration> {
    debug!("Beginning to build configuration");

    let (raw_config, config_file_path) =
        raw_configuration::get(absolute_root, config_path)?;
    let walk_directory_result =
        walk_directory(absolute_root.to_path_buf(), &raw_config)?;

    from_raw(
        absolute_root,
        raw_config,
        config_file_path,
        walk_directory_result,
        input_files_count,
    )
}

pub fn from_raw(
    absolute_root: &Path,
    raw_config: RawConfiguration,
    config_file_path: Option<PathBuf>,
    walk_directory_result: WalkDirectoryResult,
    input_files_count: &usize,
) -> anyhow::Result<Configuration> {
    let WalkDirectoryResult {
        included_files,
        included_packs,
        owning_package_yml_for_file,
    } = walk_directory_result;

    let absolute_root = absolute_root.to_path_buf();
    let pack_set = PackSet::build(included_packs, owning_package_yml_for_file)?;

    let experimental_parser = raw_config.experimental_parser;

    let layers = Layers {
        layers: raw_config.layers,
    };

    let ignored_definitions = raw_config.ignored_definitions;
    let autoload_roots: HashMap<PathBuf, String> = raw_config.autoload_roots;

    let crabwerk_first_mode = raw_config.crabwerk_first_mode;

    let inflections_path =
        absolute_root.join(raw_config.inflections_path.unwrap_or_else(|| {
            PathBuf::from("config/initializers/inflections.rb")
        }));

    let custom_associations = raw_config
        .custom_associations
        .iter()
        // In packwerk, custom_associations are an array of symbols. We strip the leading : so this configuration is compatible with the rust implementation.
        .map(|a| a.trim_start_matches(':').to_owned())
        .collect();

    debug!("Finished building configuration");

    Ok(Configuration {
        included_files,
        input_files_count: input_files_count.to_owned(),
        absolute_root,
        config_file_path,
        pack_set,
        layers,
        experimental_parser,
        ignored_definitions,
        autoload_roots,
        inflections_path,
        custom_associations,
        print_files: false,
        crabwerk_first_mode,
        color: false,
        ignore_recorded_violations: false,
        disable_enforce_dependencies: false,
        disable_enforce_folder_privacy: false,
        disable_enforce_layers: false,
        disable_enforce_privacy: false,
        disable_enforce_visibility: false,
    })
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{
        PackageTodo, configuration,
        pack::{CheckerSetting, Pack},
    };

    use pretty_assertions::assert_eq;

    #[test]
    fn default_options() {
        let absolute_root = PathBuf::from("tests/fixtures/simple_app");
        let actual = configuration::get(&absolute_root, &0).unwrap();
        assert_eq!(actual.absolute_root, absolute_root);

        let expected_included_files = vec![
            absolute_root.join("frontend/ui_helper.rb"),
            absolute_root.join("packs/bar/app/services/bar.rb"),
            absolute_root.join("packs/foo/app/services/foo.rb"),
            absolute_root.join("packs/foo/app/services/foo/bar.rb"),
            absolute_root.join("packs/foo/app/views/foo.erb"),
            absolute_root.join("packs/baz/app/services/baz.rb"),
            absolute_root.join("packs/bar/app/models/concerns/some_concern.rb"),
            absolute_root.join("app/services/some_root_class.rb"),
            absolute_root.join("app/company_data/widget.rb"),
        ]
        .into_iter()
        .collect::<HashSet<PathBuf>>();
        assert_eq!(actual.included_files, expected_included_files);

        let expected_packs = vec![
            Pack {
                enforce_dependencies: None,
                enforce_privacy: Some(CheckerSetting::True),
                enforce_visibility: None,
                enforce_folder_privacy: None,
                enforce_folder_visibility: None,
                enforce_layers: None,
                owner: None,
                yml: absolute_root.join("packs/bar/package.yml"),
                name: String::from("packs/bar"),
                relative_path: PathBuf::from("packs/bar"),
                dependencies: HashSet::new(),
                visible_to: None,
                package_todo: PackageTodo::default(),
                ignored_dependencies: HashSet::new(),
                ignored_private_constants: HashSet::new(),
                private_constants: HashSet::new(),
                public_path: None,
                layer: None,
                client_keys: HashMap::new(),
                enforcement_globs_ignore: None,
            },
            Pack {
                enforce_dependencies: None,
                enforce_privacy: None,
                enforce_visibility: None,
                enforce_folder_privacy: None,
                enforce_folder_visibility: None,
                enforce_layers: None,
                owner: None,
                yml: absolute_root.join("packs/baz/package.yml"),
                name: String::from("packs/baz"),
                relative_path: PathBuf::from("packs/baz"),
                dependencies: HashSet::new(),
                visible_to: None,
                package_todo: PackageTodo::default(),
                ignored_dependencies: HashSet::new(),
                ignored_private_constants: HashSet::new(),
                private_constants: HashSet::new(),
                public_path: None,
                layer: None,
                client_keys: HashMap::new(),
                enforcement_globs_ignore: None,
            },
            Pack {
                enforce_dependencies: Some(CheckerSetting::True),
                enforce_privacy: Some(CheckerSetting::True),
                enforce_visibility: None,
                enforce_folder_privacy: None,
                enforce_folder_visibility: None,
                enforce_layers: None,
                owner: None,
                yml: absolute_root.join("packs/foo/package.yml"),
                name: String::from("packs/foo"),
                relative_path: PathBuf::from("packs/foo"),
                dependencies: HashSet::from_iter(vec![String::from(
                    "packs/baz",
                )]),
                visible_to: None,
                package_todo: PackageTodo::default(),
                ignored_dependencies: HashSet::new(),
                ignored_private_constants: HashSet::new(),
                private_constants: HashSet::new(),
                public_path: None,

                layer: None,
                client_keys: HashMap::new(),
                enforcement_globs_ignore: None,
            },
            Pack {
                enforce_dependencies: None,
                enforce_privacy: None,
                enforce_visibility: None,
                enforce_folder_privacy: None,
                enforce_folder_visibility: None,
                enforce_layers: None,
                owner: None,
                yml: absolute_root.join("package.yml"),
                name: String::from("."),
                relative_path: PathBuf::from("."),
                dependencies: HashSet::new(),
                visible_to: None,
                package_todo: PackageTodo::default(),
                ignored_dependencies: HashSet::new(),
                ignored_private_constants: HashSet::new(),
                private_constants: HashSet::new(),
                public_path: None,
                layer: None,
                client_keys: HashMap::new(),
                enforcement_globs_ignore: None,
            },
        ];

        assert_eq!(expected_packs, actual.pack_set.packs);
    }

    #[test]
    fn filtered_absolute_paths_with_nonempty_input_paths() {
        let absolute_root = PathBuf::from("tests/fixtures/simple_app");
        let configuration = configuration::get(&absolute_root, &0).unwrap();
        let actual_paths = configuration.intersect_files(vec![
            String::from("packs/foo/app/services/foo.rb"),
            String::from("scripts/my_script.rb"),
            String::from("packs/bar/app/services/bar.rb"),
            String::from("vendor/some_gem/foo.rb"),
        ]);
        let expected_paths = vec![
            absolute_root.join("packs/bar/app/services/bar.rb"),
            absolute_root.join("packs/foo/app/services/foo.rb"),
        ]
        .into_iter()
        .collect::<HashSet<PathBuf>>();
        assert_eq!(actual_paths, expected_paths);
    }

    #[test]
    fn filtered_absolute_paths_with_empty_input_paths() {
        let absolute_root = PathBuf::from("tests/fixtures/simple_app");
        let configuration = configuration::get(&absolute_root, &0).unwrap();
        let actual_paths = configuration.intersect_files(vec![]);
        let expected_paths = vec![
            absolute_root.join("frontend/ui_helper.rb"),
            absolute_root.join("packs/bar/app/services/bar.rb"),
            absolute_root.join("packs/foo/app/services/foo.rb"),
            absolute_root.join("packs/foo/app/services/foo/bar.rb"),
            absolute_root.join("packs/foo/app/views/foo.erb"),
            absolute_root.join("packs/baz/app/services/baz.rb"),
            absolute_root.join("packs/bar/app/models/concerns/some_concern.rb"),
            absolute_root.join("app/services/some_root_class.rb"),
            absolute_root.join("app/company_data/widget.rb"),
        ]
        .into_iter()
        .collect::<HashSet<PathBuf>>();
        assert_eq!(actual_paths, expected_paths);
    }

    #[test]
    fn filtered_absolute_paths_with_directory_input_paths() {
        let absolute_root = PathBuf::from("tests/fixtures/simple_app");
        let configuration = configuration::get(&absolute_root, &0).unwrap();
        let actual_paths =
            configuration.intersect_files(vec![String::from("packs/bar")]);
        let expected_paths = vec![
            absolute_root.join("packs/bar/app/services/bar.rb"),
            absolute_root.join("packs/bar/app/models/concerns/some_concern.rb"),
        ]
        .into_iter()
        .collect::<HashSet<PathBuf>>();
        assert_eq!(actual_paths, expected_paths);
    }

    #[test]
    fn with_symbols_as_custom_associations() {
        let absolute_root = PathBuf::from("tests/fixtures/simple_app");
        let raw = RawConfiguration {
            custom_associations: vec![":my_association".to_owned()],
            ..RawConfiguration::default()
        };

        let included_packs: HashSet<Pack> = vec![Pack {
            name: String::from("."),
            ..Pack::default()
        }]
        .into_iter()
        .collect();
        let walk_directory_result = WalkDirectoryResult {
            included_files: Default::default(),
            included_packs,
            owning_package_yml_for_file: Default::default(),
        };

        let configuration = configuration::from_raw(
            &absolute_root,
            raw,
            None,
            walk_directory_result,
            &0,
        )
        .unwrap();
        let actual_associations = configuration.custom_associations;
        let expected_paths = vec!["my_association".to_owned()];

        assert_eq!(actual_associations, expected_paths);
    }
}
