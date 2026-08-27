use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use crate::Configuration;
use anyhow::Context;
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use regex::Regex;

// Compiled once: this runs for every ERB file, and an application can have
// thousands of them.
static ERB_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<%=?-?\s*(.*?)\s*-?%>").unwrap());

#[derive(PartialEq, Eq, Debug)]
pub enum SupportedFileType {
    Ruby,
    Erb,
}

pub fn get_file_type(path: &Path) -> Option<SupportedFileType> {
    let ruby_special_files = ["Gemfile", "Rakefile"];
    let ruby_extensions = vec!["rb", "rake", "builder", "gemspec", "ru"];

    let extension = path.extension();
    // Eventually, we can have crate::parsing::ruby, crate::parsing::erb, etc.
    // These would implement a crate::parsing::interface::Parser trait and can
    // hold the logic for determining if a parser can parse a file.

    let is_ruby_file = ruby_extensions
        .into_iter()
        .any(|ext| extension.is_some_and(|e| e == ext))
        || ruby_special_files.iter().any(|file| path.ends_with(file));

    let is_erb_file = path.extension().is_some_and(|ext| ext == "erb");

    if is_ruby_file {
        Some(SupportedFileType::Ruby)
    } else if is_erb_file {
        Some(SupportedFileType::Erb)
    } else {
        None
    }
}

pub fn build_glob_set(globs: &[String]) -> anyhow::Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();

    for glob in globs {
        let compiled_glob = GlobBuilder::new(glob)
            .literal_separator(true)
            .build()
            .with_context(|| format!("Invalid glob pattern: {}", glob))?;

        builder.add(compiled_glob);
    }

    builder.build().context("Could not build the glob set")
}

/// Paths that cannot be read during the walk are skipped, as they are in
/// [`glob_ruby_files_in_dirs`]; only a pattern the glob crate refuses is an
/// error, because that comes from the configuration.
pub fn expand_glob(pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
    Ok(glob::glob(pattern)
        .with_context(|| format!("Invalid glob pattern: {}", pattern))?
        .flatten()
        .collect())
}

pub fn glob_ruby_files_in_dirs(dirs: Vec<&PathBuf>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for dir in dirs {
        let glob = dir.join("**/*.rb");
        let pattern = glob.to_str().unwrap();
        for path in glob::glob(pattern)
            .expect("Failed to read glob pattern")
            .flatten()
        {
            paths.push(path);
        }
    }

    paths
}

pub fn user_inputted_paths_to_absolute_filepaths(
    absolute_root: &Path,
    input_paths: Vec<String>,
) -> HashSet<PathBuf> {
    input_paths
        .iter()
        .map(PathBuf::from)
        .flat_map(|p| {
            if p.is_absolute() {
                vec![p]
            } else {
                let absolute_path = absolute_root.join(&p);
                if absolute_path.is_dir() {
                    glob::glob(absolute_path.join("**/*.*").to_str().unwrap())
                        .expect("Failed to read glob pattern")
                        .filter_map(Result::ok)
                        .collect::<Vec<_>>()
                } else {
                    vec![absolute_path]
                }
            }
        })
        .collect::<HashSet<_>>()
}

pub fn convert_erb_to_ruby_without_sourcemaps(contents: String) -> String {
    let extracted_contents: Vec<&str> = ERB_TAG
        .captures_iter(&contents)
        .map(|capture| capture.get(1).unwrap().as_str())
        .collect();

    extracted_contents.join("\n")
}

pub fn file_read_contents(path: &Path) -> anyhow::Result<String> {
    fs::read_to_string(path).context(format!(
        "Failed to read contents of {}",
        path.to_string_lossy()
    ))
}

pub fn get_absolute_path(
    path: String,
    configuration: &Configuration,
) -> PathBuf {
    let path = PathBuf::from(path);

    if path.is_absolute() {
        path
    } else {
        configuration.absolute_root.join(path)
    }
}
