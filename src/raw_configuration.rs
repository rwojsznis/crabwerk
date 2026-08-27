use std::{
    collections::{HashMap, HashSet},
    fmt,
    fs::File,
    path::{Path, PathBuf},
};

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, SeqAccess, Visitor, value},
};

pub(crate) const CONFIG_FILE_NAME: &str = "packwerk.yml";
pub(crate) const CRABWERK_CONFIG_FILE_NAME: &str = "crabwerk.yml";

// See: Setting up the configuration file
// https://github.com/Shopify/packwerk/blob/main/USAGE.md#setting-up-the-configuration-file
#[derive(Debug, Deserialize, Serialize)]
pub struct RawConfiguration {
    // List of patterns for folder paths to include
    #[serde(default = "default_include")]
    pub include: Vec<String>,

    // List of patterns for folder paths to exclude
    #[serde(default = "default_exclude")]
    pub exclude: Vec<String>,

    // Patterns to find package configuration files
    #[serde(
        default = "default_package_paths",
        deserialize_with = "string_or_vec"
    )]
    pub package_paths: Vec<String>,

    // List of custom associations, if any
    #[serde(default = "default_custom_associations")]
    pub custom_associations: Vec<String>,

    // Architecture layers
    #[serde(default)]
    pub layers: Vec<String>,

    // Experimental parser
    #[serde(default)]
    pub experimental_parser: bool,

    // Ignored monkey patches
    #[serde(default)]
    pub ignored_definitions: HashMap<String, HashSet<PathBuf>>,

    // Extra autoload roots, each mapped to the namespace it defines. The gem
    // takes its load paths from a booted Rails app instead, so this key has no
    // packwerk equivalent and there is nothing to keep parity with.
    #[serde(default)]
    pub autoload_roots: HashMap<PathBuf, String>,

    // Relative path to inflections file
    #[serde(default)]
    pub inflections_path: Option<PathBuf>,

    // Use crabwerk copy
    #[serde(default)]
    pub crabwerk_first_mode: bool,
}

/// Resolve a `--config` argument against the project root.
///
/// A relative path is taken to be relative to the root, so that
/// `--project-root some/app --config packwerk.yml` names the config inside the
/// app rather than one next to the shell's working directory.
pub fn absolute_config_path(
    absolute_root: &Path,
    config_path: &Path,
) -> PathBuf {
    if config_path.is_absolute() {
        config_path.to_path_buf()
    } else {
        absolute_root.join(config_path)
    }
}

pub fn get(
    absolute_root: &Path,
    config_path: Option<&Path>,
) -> anyhow::Result<(RawConfiguration, Option<PathBuf>)> {
    if let Some(config_path) = config_path {
        let absolute_config_path =
            absolute_config_path(absolute_root, config_path);
        if !absolute_config_path.exists() {
            anyhow::bail!(
                "There is no configuration file at: {}",
                absolute_config_path.display(),
            );
        }
        let mut config = get_from_file_that_exists(&absolute_config_path)?;
        // Only the packwerk gem's own file name means packwerk-first; any other
        // named file is read as a crabwerk configuration.
        config.crabwerk_first_mode = absolute_config_path
            .file_name()
            .is_none_or(|name| name != CONFIG_FILE_NAME);
        return Ok((config, Some(absolute_config_path)));
    }

    let absolute_path_to_crabwerk_yml =
        absolute_root.join(CRABWERK_CONFIG_FILE_NAME);

    if absolute_path_to_crabwerk_yml.exists() {
        let mut config =
            get_from_file_that_exists(&absolute_path_to_crabwerk_yml)?;
        config.crabwerk_first_mode = true;
        return Ok((config, Some(absolute_path_to_crabwerk_yml)));
    }

    // A `packwerk.yml` that is left behind is an error rather than a fallback:
    // reading the defaults instead would silently discard the layers, the
    // include globs and the autoload roots that the file configures.
    let absolute_path_to_packwerk_yml = absolute_root.join(CONFIG_FILE_NAME);
    if absolute_path_to_packwerk_yml.exists() {
        anyhow::bail!(
            "Found `{}` at: {}\n\
             crabwerk does not read `packwerk.yml`. Run `crabwerk migrate-config` \
             to copy it to `crabwerk.yml`, or name it with `--config {}`.",
            CONFIG_FILE_NAME,
            absolute_path_to_packwerk_yml.display(),
            CONFIG_FILE_NAME,
        );
    }

    Ok((
        RawConfiguration {
            crabwerk_first_mode: true,
            ..RawConfiguration::default()
        },
        None,
    ))
}

fn get_from_file_that_exists(
    absolute_path_to_config: &Path,
) -> anyhow::Result<RawConfiguration> {
    let mut file = File::open(absolute_path_to_config).map_err(|e| {
        anyhow::Error::new(e).context(format!(
            "Could not open configuration file at: {}",
            absolute_path_to_config.display(),
        ))
    })?;

    let mut contents = String::new();
    std::io::Read::read_to_string(&mut file, &mut contents).map_err(|e| {
        anyhow::Error::new(e).context(format!(
            "Could not read configuration file at: {}",
            absolute_path_to_config.display(),
        ))
    })?;

    parse(&contents, absolute_path_to_config)
}

pub fn parse(
    contents: &str,
    absolute_path_to_config: &Path,
) -> anyhow::Result<RawConfiguration> {
    serde_yaml::from_str(contents).map_err(|e| {
        anyhow::Error::new(e).context(format!(
            "Could not parse configuration file at: {}",
            absolute_path_to_config.display(),
        ))
    })
}

// Normally if a key is not set, serde will use the default value for that type.
// If there is no `crabwerk.yml` at all, we use `RawConfiguration::default()` to get the default,
// So this implementation of default ensures that the default is the same as the serde default.
impl Default for RawConfiguration {
    fn default() -> Self {
        // Deserialize an empty string to get the default RawConfiguration
        // We used to use #[derive(Default)] on the RawConfiguration.
        // However, that doesn't use the defaults fed to serde
        serde_yaml::from_str("").unwrap()
    }
}

fn default_include() -> Vec<String> {
    vec![
        String::from("**/*.rb"),
        String::from("**/*.rake"),
        String::from("**/*.erb"),
    ]
}

fn default_exclude() -> Vec<String> {
    // `log`, `public` and `sorbet` are not in packwerk's default. They are
    // here, and not hardcoded in the directory walk, so that a repository
    // with Ruby in them can ask for it back.
    vec![String::from(
        "{bin,log,node_modules,public,script,sorbet,tmp,vendor}/**/*",
    )]
}

fn default_package_paths() -> Vec<String> {
    vec![String::from("**/*")]
}

const fn default_custom_associations() -> Vec<String> {
    vec![]
}

fn string_or_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringOrVec;

    impl<'de> Visitor<'de> for StringOrVec {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("glob string or list of glob strings")
        }

        fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(vec![s.to_owned()])
        }

        fn visit_seq<S>(self, seq: S) -> Result<Self::Value, S::Error>
        where
            S: SeqAccess<'de>,
        {
            Deserialize::deserialize(value::SeqAccessDeserializer::new(seq))
        }
    }

    deserializer.deserialize_any(StringOrVec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_package_paths_as_string() {
        let raw_configuration_string = String::from("package_paths: '**/*'");
        let raw_configuration =
            serde_yaml::from_str::<RawConfiguration>(&raw_configuration_string)
                .expect("Could not deserialize package_paths as string");

        assert_eq!(raw_configuration.package_paths, vec!["**/*"]);
    }

    // Caching and `autoload_paths` were removed, but a config file left over
    // from an older version still carries the keys. RawConfiguration
    // deliberately has no `deny_unknown_fields` so those configs keep working
    // untouched.
    #[test]
    fn test_deserialize_ignores_removed_keys() {
        let raw_configuration_string = String::from(
            "cache: true\ncache_directory: 'tmp/cache/packwerk'\nautoload_paths:\n- app/models\npackage_paths: packs/*\n",
        );
        let raw_configuration =
            serde_yaml::from_str::<RawConfiguration>(&raw_configuration_string)
                .expect("Removed keys should be ignored, not rejected");

        assert_eq!(raw_configuration.package_paths, vec!["packs/*"]);
    }

    #[test]
    fn test_deserialize_package_paths_as_vec() {
        let raw_configuration_string =
            String::from("package_paths:\n- packs/*\n- components/*");
        let raw_configuration =
            serde_yaml::from_str::<RawConfiguration>(&raw_configuration_string)
                .expect("Could not deserialize package_paths as a vec");

        assert_eq!(
            raw_configuration.package_paths,
            vec!["packs/*", "components/*"]
        );
    }

    #[test]
    fn test_deserialize_package_paths_with_an_unsupported_type() {
        let raw_configuration_string = String::from("package_paths: 5");
        let error =
            serde_yaml::from_str::<RawConfiguration>(&raw_configuration_string)
                .expect_err("package_paths: 5 should not deserialize");

        assert!(
            error
                .to_string()
                .contains("glob string or list of glob strings"),
            "unexpected error message: {}",
            error
        );
    }

    #[test]
    fn test_get_with_no_configuration_file() {
        let temp_dir = tempfile::TempDir::new().unwrap();

        let (raw_configuration, path) = get(temp_dir.path(), None).unwrap();

        assert_eq!(path, None);
        assert!(raw_configuration.crabwerk_first_mode);
        // Falls back to the serde defaults
        assert_eq!(raw_configuration.package_paths, default_package_paths());
        assert_eq!(raw_configuration.include, default_include());
        assert_eq!(raw_configuration.exclude, default_exclude());
    }

    #[test]
    fn test_get_with_crabwerk_yml() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let crabwerk_yml = temp_dir.path().join(CRABWERK_CONFIG_FILE_NAME);
        std::fs::write(&crabwerk_yml, "package_paths: packs/*\n").unwrap();

        let (raw_configuration, path) = get(temp_dir.path(), None).unwrap();

        assert_eq!(path, Some(crabwerk_yml));
        assert!(raw_configuration.crabwerk_first_mode);
        assert_eq!(raw_configuration.package_paths, vec!["packs/*"]);
    }

    #[test]
    fn test_get_with_only_packwerk_yml_is_an_error() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            temp_dir.path().join(CONFIG_FILE_NAME),
            "package_paths: packs/*\n",
        )
        .unwrap();

        let error = get(temp_dir.path(), None)
            .expect_err("a lone packwerk.yml should be an error");

        let message = format!("{:#}", error);
        assert!(
            message.contains("crabwerk does not read `packwerk.yml`")
                && message.contains("crabwerk migrate-config"),
            "unexpected error message: {}",
            message
        );
    }

    #[test]
    fn test_get_ignores_packwerk_yml_when_crabwerk_yml_exists() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            temp_dir.path().join(CONFIG_FILE_NAME),
            "package_paths: from_packwerk/*\n",
        )
        .unwrap();
        let crabwerk_yml = temp_dir.path().join(CRABWERK_CONFIG_FILE_NAME);
        std::fs::write(&crabwerk_yml, "package_paths: from_crabwerk/*\n")
            .unwrap();

        let (raw_configuration, path) = get(temp_dir.path(), None).unwrap();

        assert_eq!(path, Some(crabwerk_yml));
        assert!(raw_configuration.crabwerk_first_mode);
        assert_eq!(raw_configuration.package_paths, vec!["from_crabwerk/*"]);
    }

    #[test]
    fn test_get_with_a_named_packwerk_yml() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let packwerk_yml = temp_dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&packwerk_yml, "package_paths: packs/*\n").unwrap();

        let (raw_configuration, path) =
            get(temp_dir.path(), Some(Path::new(CONFIG_FILE_NAME))).unwrap();

        assert_eq!(path, Some(packwerk_yml));
        // Naming the gem's own file keeps the messages pointing at the gem
        assert!(!raw_configuration.crabwerk_first_mode);
        assert_eq!(raw_configuration.package_paths, vec!["packs/*"]);
    }

    #[test]
    fn test_get_with_a_named_file_under_another_name() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let config = temp_dir.path().join("config").join("my_config.yml");
        std::fs::create_dir_all(config.parent().unwrap()).unwrap();
        std::fs::write(&config, "package_paths: packs/*\n").unwrap();

        let (raw_configuration, path) = get(
            temp_dir.path(),
            Some(Path::new("config").join("my_config.yml").as_path()),
        )
        .unwrap();

        assert_eq!(path, Some(config));
        assert!(raw_configuration.crabwerk_first_mode);
        assert_eq!(raw_configuration.package_paths, vec!["packs/*"]);
    }

    #[test]
    fn test_get_with_a_named_absolute_path() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let config = temp_dir.path().join("elsewhere.yml");
        std::fs::write(&config, "package_paths: packs/*\n").unwrap();

        let (_, path) =
            get(Path::new("/nonexistent_root"), Some(&config)).unwrap();

        assert_eq!(path, Some(config));
    }

    #[test]
    fn test_get_with_a_named_file_that_does_not_exist() {
        let temp_dir = tempfile::TempDir::new().unwrap();

        let error = get(temp_dir.path(), Some(Path::new("nope.yml")))
            .expect_err("a missing named config file should be an error");

        let message = format!("{:#}", error);
        assert!(
            message.contains("There is no configuration file at")
                && message.contains("nope.yml"),
            "unexpected error message: {}",
            message
        );
    }

    #[test]
    fn test_get_with_unparseable_configuration_file() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            temp_dir.path().join(CRABWERK_CONFIG_FILE_NAME),
            "include: [unterminated\n",
        )
        .unwrap();

        let error = get(temp_dir.path(), None)
            .expect_err("an unparseable crabwerk.yml should be an error");

        assert!(
            error
                .to_string()
                .contains("Could not parse configuration file at"),
            "unexpected error message: {}",
            error
        );
    }

    #[test]
    fn test_get_with_an_unreadable_configuration_file() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        // A directory named `crabwerk.yml` exists, so `exists()` is true but
        // `File::open` cannot read it.
        std::fs::create_dir(temp_dir.path().join(CRABWERK_CONFIG_FILE_NAME))
            .unwrap();

        let error = get(temp_dir.path(), None)
            .expect_err("a directory named crabwerk.yml should be an error");

        let message = format!("{:#}", error);
        assert!(
            message.contains("crabwerk.yml"),
            "unexpected error message: {}",
            message
        );
    }
}
