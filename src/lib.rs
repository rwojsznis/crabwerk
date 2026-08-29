// Currently there are no supported library APIs for crabwerk. The public API is the CLI.
// This may change in the future! Please file an issue if you have a use case for a library API.
pub mod cli;

pub(crate) mod checker;
pub(crate) mod color;
pub(crate) mod configuration;
pub(crate) mod constant_resolver;
pub(crate) mod dependencies;
pub(crate) mod ignored;
pub(crate) mod monkey_patch_detection;
pub mod pack;
pub(crate) mod parsing;
pub(crate) mod raw_configuration;
pub(crate) mod walk_directory;

mod constant_dependencies;
mod file_utils;
mod pack_set;
mod package_todo;
mod reference_extractor;
pub mod yaml;

use crate::pack::Pack;
use crate::pack::write_pack_to_disk;

pub(crate) use self::checker::Violation;
pub(crate) use self::pack_set::PackSet;
pub(crate) use self::parsing::ParsedDefinition;
pub(crate) use self::parsing::UnresolvedReference;
pub(crate) use self::parsing::process_files;
pub(crate) use self::parsing::ruby::experimental::get_experimental_constant_resolver;
pub(crate) use self::parsing::ruby::zeitwerk::get_zeitwerk_constant_resolver;
pub(crate) use self::reference_extractor::get_all_references_and_sigils;
use anyhow::bail;
pub(crate) use configuration::Configuration;
pub(crate) use package_todo::PackageTodo;

use anyhow::Context;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub fn init(absolute_root: &Path, use_packwerk: bool) -> anyhow::Result<()> {
    let command = if use_packwerk { "packwerk" } else { "crabwerk" };
    let root_package = format!("\
# This file represents the root package of the application
# Please validate the configuration using `{} validate` (for Rails applications) or running the auto generated
# test case (for non-Rails projects). You can then use `{} check` to check your code.

# Change to `true` to turn on dependency checks for this package
enforce_dependencies: false

# A list of this package's dependencies
# Note that packages in this list require their own `package.yml` file
# dependencies:
# - \"packages/billing\"
", command, command);

    let crabwerk_config = "\
# See: Setting up the configuration file
# https://github.com/Shopify/packwerk/blob/main/USAGE.md#configuring-packwerk

# List of patterns for folder paths to include
# include:
# - \"**/*.{rb,rake,erb}\"

# List of patterns for folder paths to exclude
# The `.git` directory is always skipped, whatever this list says.
# exclude:
# - \"{bin,log,node_modules,public,script,sorbet,tmp,vendor}/**/*\"

# Patterns to find package configuration files
# package_paths: \"**/\"

# List of custom associations, if any
# custom_associations:
# - \"cache_belongs_to\"

";
    let root_package_path = absolute_root.join("package.yml");
    let crabwerk_config_path = absolute_root.join(if use_packwerk {
        "packwerk.yml"
    } else {
        "crabwerk.yml"
    });

    if root_package_path.exists() {
        println!("`{}` already exists!", root_package_path.display());
        bail!("Could not initialize package.yml")
    }
    if crabwerk_config_path.exists() {
        println!("`{}` already exists!", crabwerk_config_path.display());
        bail!(format!(
            "Could not initialize {}",
            crabwerk_config_path.display()
        ))
    }

    std::fs::write(&root_package_path, root_package).with_context(|| {
        format!("Could not write {}", root_package_path.display())
    })?;
    std::fs::write(&crabwerk_config_path, crabwerk_config).with_context(
        || format!("Could not write {}", crabwerk_config_path.display()),
    )?;

    println!(
        "Created '{}' and '{}'",
        crabwerk_config_path.display(),
        root_package_path.display()
    );
    Ok(())
}

fn create(configuration: &Configuration, name: String) -> anyhow::Result<()> {
    let existing_pack = configuration.pack_set.for_pack(&name);
    if existing_pack.is_ok() {
        println!("`{}` already exists!", name);
        return Ok(());
    }
    let new_pack_path =
        configuration.absolute_root.join(&name).join("package.yml");

    let new_pack = Pack::from_contents(
        &new_pack_path,
        &configuration.absolute_root,
        "enforce_dependencies: true",
        PackageTodo::default(),
    )?;

    write_pack_to_disk(&new_pack)?;

    let readme = format!(
"Welcome to `{}`!

If you're the author, please consider replacing this file with a README.md, which may contain:
- What your pack is and does
- How you expect people to use your pack
- Example usage of your pack's public API and where to find it
- Limitations, risks, and important considerations of usage
- How to get in touch with eng and other stakeholders for questions or issues pertaining to this pack
- What SLAs/SLOs (service level agreements/objectives), if any, your package provides
- When in doubt, keep it simple
- Anything else you may want to include!

README.md should change as your public API changes.",
    new_pack.name
);

    let readme_path = configuration.absolute_root.join(&name).join("README.md");
    std::fs::write(readme_path, readme).context("Failed to write README.md")?;

    println!("Successfully created `{}`!", name);
    Ok(())
}

pub fn check(
    configuration: &Configuration,
    files: Vec<String>,
    json: bool,
) -> anyhow::Result<()> {
    let result = checker::check_all(configuration, files)
        .context("Failed to check files")?;
    if json {
        println!("{}", result.to_json().context("Failed to serialize JSON")?);
        if result.has_violations() {
            std::process::exit(1);
        }
    } else {
        println!("{}", result);
        if result.has_violations() {
            let count = result.violation_count();
            bail!("{} violation(s) found!", count)
        }
    }
    Ok(())
}

pub fn update(
    configuration: &Configuration,
    options: &checker::UpdateOptions,
) -> anyhow::Result<()> {
    checker::update(configuration, options)
}

pub fn add_dependency(
    configuration: &Configuration,
    from: String,
    to: String,
) -> anyhow::Result<()> {
    let pack_set = &configuration.pack_set;

    let from_pack = pack_set
        .for_pack(&from)
        .context(format!("`{}` not found", from))?;

    let to_pack = pack_set
        .for_pack(&to)
        .context(format!("`{}` not found", to))?;

    if from_pack.dependencies.contains(&to_pack.name) {
        println!(
            "`{}` already depends on `{}`!",
            from_pack.name, to_pack.name
        );
        return Ok(());
    }

    let new_from_pack = from_pack.add_dependency(to_pack);

    write_pack_to_disk(&new_from_pack)?;

    // Reloading is simpler than keeping the in-memory pack set in sync. This
    // write command does not need the check path's performance.
    let new_configuration = configuration::get_with_config_path(
        &configuration.absolute_root,
        &configuration.input_files_count,
        configuration.config_file_path.as_deref(),
    )?;
    let validation_result = crate::validate(&new_configuration, false);
    if validation_result.is_err() {
        println!("Added `{}` as a dependency to `{}`!", to, from);
        println!("Warning: This creates a cycle!");
    } else {
        println!("Successfully added `{}` as a dependency to `{}`!", to, from);
    }

    Ok(())
}

pub fn list_included_files(configuration: Configuration) -> anyhow::Result<()> {
    configuration
        .included_files
        .iter()
        .for_each(|f| println!("{}", f.display()));
    Ok(())
}

pub fn validate(
    configuration: &Configuration,
    json: bool,
) -> anyhow::Result<()> {
    if json {
        checker::validate_all_json(configuration)
    } else {
        checker::validate_all(configuration)
    }
}

pub fn remove_dependency(
    configuration: &Configuration,
    from: String,
    to: String,
) -> anyhow::Result<()> {
    let pack_set = &configuration.pack_set;

    let from_pack = pack_set
        .for_pack(&from)
        .context(format!("`{}` not found", from))?;

    let _to_pack = pack_set
        .for_pack(&to)
        .context(format!("`{}` not found", to))?;

    if !from_pack.dependencies.contains(&to) {
        println!("`{}` does not depend on `{}`!", from_pack.name, to);
        return Ok(());
    }

    let mut new_pack = from_pack.clone();
    new_pack.dependencies.remove(&to);
    write_pack_to_disk(&new_pack)?;

    println!(
        "Successfully removed `{}` as a dependency from `{}`!",
        to, from
    );
    Ok(())
}

pub fn configuration(
    project_root: PathBuf,
    input_files_count: &usize,
) -> anyhow::Result<Configuration> {
    let absolute_root = project_root.canonicalize()?;
    configuration::get(&absolute_root, input_files_count)
}

/// Copy a `packwerk.yml` written for the gem to the `crabwerk.yml` that
/// `crabwerk` reads.
///
/// The copy is verbatim, comments included, so that `diff packwerk.yml
/// crabwerk.yml` stays empty while a repo runs both tools side by side. The
/// original is left in place; the packwerk gem still needs it.
pub fn migrate_config(absolute_root: &Path) -> anyhow::Result<()> {
    let packwerk_yml_path =
        absolute_root.join(raw_configuration::CONFIG_FILE_NAME);
    let crabwerk_yml_path =
        absolute_root.join(raw_configuration::CRABWERK_CONFIG_FILE_NAME);

    if !packwerk_yml_path.exists() {
        bail!(
            "There is no `packwerk.yml` at: {}\nNothing to migrate. Use `crabwerk init` to write a new `crabwerk.yml`.",
            packwerk_yml_path.display()
        )
    }
    if crabwerk_yml_path.exists() {
        bail!(
            "`{}` already exists!\nDelete it first if you mean to migrate `packwerk.yml` over it.",
            crabwerk_yml_path.display()
        )
    }

    let contents =
        std::fs::read_to_string(&packwerk_yml_path).context(format!(
            "Could not read configuration file at: {}",
            packwerk_yml_path.display()
        ))?;

    // Parse before writing, so an unparseable configuration is reported here
    // rather than by the next command to run.
    raw_configuration::parse(&contents, &packwerk_yml_path)?;

    std::fs::write(&crabwerk_yml_path, &contents).context(format!(
        "Could not write configuration file at: {}",
        crabwerk_yml_path.display()
    ))?;

    println!(
        "Created `crabwerk.yml` from `packwerk.yml` at: {}",
        crabwerk_yml_path.display()
    );
    println!(
        "`packwerk.yml` was left in place; delete it when you no longer run the packwerk gem."
    );

    Ok(())
}

pub fn check_unnecessary_dependencies(
    configuration: &Configuration,
    auto_correct: bool,
) -> anyhow::Result<()> {
    if auto_correct {
        checker::remove_unnecessary_dependencies(configuration)
    } else {
        checker::check_unnecessary_dependencies(configuration)
    }
}

pub fn add_dependencies(
    configuration: &Configuration,
    pack_name: &str,
) -> anyhow::Result<()> {
    checker::add_all_dependencies(configuration, pack_name)
}

pub fn update_dependencies_for_constant(
    configuration: &Configuration,
    constant_name: &str,
) -> anyhow::Result<()> {
    match constant_dependencies::update_dependencies_for_constant(
        configuration,
        constant_name,
    ) {
        Ok(num_updated) => {
            match num_updated {
                0 => println!(
                    "No dependencies to update for constant '{}'",
                    constant_name
                ),
                1 => println!(
                    "Successfully updated 1 dependency for constant '{}'",
                    constant_name
                ),
                _ => println!(
                    "Successfully updated {} dependencies for constant '{}'",
                    num_updated, constant_name
                ),
            }
            Ok(())
        }
        Err(err) => Err(anyhow::anyhow!(err)),
    }
}

pub fn list(configuration: Configuration) {
    for pack in configuration.pack_set.packs {
        println!("{}", pack.yml.display())
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct ProcessedFile {
    pub absolute_path: PathBuf,
    pub unresolved_references: Vec<UnresolvedReference>,
    pub definitions: Vec<ParsedDefinition>,

    #[serde(default)] // Default to an empty Vec if not present
    pub sigils: Vec<Sigil>,
}

// A sigil is a way to specify some crabwerk specific behavior at the top of a
// file, like `# pack_public: true`. Only the name is kept: the parser matches
// the literal `true` form, so a sigil that is present is a sigil that is on.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct Sigil {
    pub name: String,
}

#[derive(
    Debug, PartialEq, Serialize, Deserialize, Default, Eq, Clone, Hash,
)]
pub struct SourceLocation {
    pub line: usize,
    pub column: usize,
}

pub(crate) fn list_definitions(
    configuration: &Configuration,
    ambiguous: bool,
) -> anyhow::Result<()> {
    let constant_resolver = if configuration.experimental_parser {
        let processed_files: Vec<ProcessedFile> =
            process_files(&configuration.included_files, configuration)?;

        get_experimental_constant_resolver(
            &configuration.absolute_root,
            &processed_files,
            &configuration.ignored_definitions,
        )
    } else {
        if ambiguous {
            bail!("Ambiguous mode is not supported for the Zeitwerk parser");
        }
        get_zeitwerk_constant_resolver(
            &configuration.pack_set,
            &configuration.constant_resolver_configuration(),
        )?
    };

    let constant_definition_map = constant_resolver
        .fully_qualified_constant_name_to_constant_definition_map();

    for (name, definitions) in constant_definition_map {
        if ambiguous && definitions.len() == 1 {
            continue;
        }

        for definition in definitions {
            let relative_path = definition
                .absolute_path_of_definition
                .strip_prefix(&configuration.absolute_root)?;

            println!("{:?} is defined at {:?}", name, relative_path);
        }
    }
    Ok(())
}

/// How `list-references` writes its map.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum ReferenceFormat {
    #[default]
    Json,
    Text,
}

pub(crate) fn list_references(
    configuration: &Configuration,
    format: ReferenceFormat,
    output_file: Option<&Path>,
) -> anyhow::Result<()> {
    let (references, _sigils) = get_all_references_and_sigils(
        configuration,
        &configuration.included_files,
    )?;

    // `BTreeMap`, not `HashMap`: the output is stored and diffed by whatever
    // selects tests from it, so it must not depend on hash order.
    let mut reference_map: BTreeMap<String, BTreeMap<String, String>> =
        BTreeMap::new();

    for reference in references {
        // A reference the resolver could not place has no file to select on.
        if let Some(relative_defining_file) = reference.relative_defining_file {
            reference_map
                .entry(reference.relative_referencing_file)
                .or_default()
                .insert(reference.constant_name, relative_defining_file);
        }
    }

    let output = match format {
        ReferenceFormat::Json => serde_json::to_string_pretty(&reference_map)?,
        ReferenceFormat::Text => {
            let mut lines = Vec::new();
            for (source_file, constants) in &reference_map {
                lines.push(format!("{}:", source_file));
                for (const_name, def_file) in constants {
                    lines.push(format!("  {} => {}", const_name, def_file));
                }
            }
            lines.join("\n")
        }
    };

    if let Some(path) = output_file {
        std::fs::write(path, output)?;
        println!("Reference map written to: {}", path.display());
    } else {
        println!("{}", output);
    }

    Ok(())
}

fn expose_monkey_patches(
    configuration: &Configuration,
    rubydir: &PathBuf,
    gemdir: &PathBuf,
) -> anyhow::Result<()> {
    println!(
        "{}",
        monkey_patch_detection::expose_monkey_patches(
            configuration,
            rubydir,
            gemdir,
        )?
    );
    Ok(())
}

fn list_dependencies(
    configuration: &Configuration,
    pack_name: String,
) -> anyhow::Result<()> {
    println!("Pack dependencies for {}\n", pack_name);
    let dependencies =
        dependencies::find_dependencies(configuration, &pack_name)?;
    println!("Explicit ({}):", dependencies.explicit.len());
    if dependencies.explicit.is_empty() {
        println!("- None");
    } else {
        for dependency in dependencies.explicit {
            println!("- {}", dependency);
        }
    }
    println!("\nImplicit (violations) ({}):", dependencies.implicit.len());
    if dependencies.implicit.is_empty() {
        println!("- None");
    } else {
        let mut dependent_packs_with_violations =
            dependencies.implicit.keys().collect::<Vec<_>>();
        dependent_packs_with_violations.sort();
        for dependent in dependent_packs_with_violations {
            println!("- {}", dependent);
            let mut violation_types =
                dependencies.implicit[dependent].iter().collect::<Vec<_>>();
            violation_types.sort();
            for (violation_type, count) in violation_types {
                println!("  - {}: {}", violation_type, count);
            }
        }
    }
    Ok(())
}

fn move_to_pack(
    configuration: &Configuration,
    destination: &str,
    paths: Vec<String>,
) -> anyhow::Result<()> {
    let dest_pack = configuration
        .pack_set
        .for_pack(destination)
        .context(format!("Cannot move to '{}': pack not found", destination))?;

    // Check if destination pack uses automatic_pack_namespace
    if let Some(serde_json::Value::Object(map)) =
        dest_pack.client_keys.get("metadata")
    {
        let has_auto_namespace = map
            .get("automatic_pack_namespace")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if has_auto_namespace {
            bail!(
                "Cannot move files into '{}': pack has automatic_pack_namespace enabled. \
                 Files in this pack are automatically namespaced under the pack's module, \
                 so moved files would need to be wrapped in that namespace to work correctly.",
                destination
            );
        }
    }

    let dest_relative_path = dest_pack.relative_path.clone();

    // Expand input paths: if a path is a directory, glob all files within it
    let mut source_files: Vec<PathBuf> = Vec::new();
    for path_str in &paths {
        let path_str = path_str.trim_end_matches('/');
        let absolute_path = configuration.absolute_root.join(path_str);
        if absolute_path.is_dir() {
            // `**/*` rather than `**/*.*`: the latter needs a literal dot, so
            // it leaves `Gemfile` and `Rakefile` behind. `move` works on disk
            // and must take every file, not only the ones the walk includes.
            let pattern = absolute_path.join("**/*");
            let entries = glob::glob(pattern.to_str().unwrap())
                .context("Failed to glob")?;
            for entry in entries.flatten() {
                if entry.is_dir() {
                    continue;
                }
                let relative = entry
                    .strip_prefix(&configuration.absolute_root)
                    .unwrap()
                    .to_path_buf();
                let filename =
                    relative.file_name().unwrap().to_string_lossy().to_string();
                if filename != "package.yml" && filename != "package_todo.yml" {
                    source_files.push(relative);
                }
            }
        } else {
            source_files.push(PathBuf::from(path_str));
        }
    }

    // Compute file move operations
    struct FileMoveOperation {
        origin: PathBuf,
        destination: PathBuf,
    }

    let mut operations: Vec<FileMoveOperation> = Vec::new();

    for source_file in &source_files {
        let source_str = source_file.to_string_lossy().to_string();

        // Find the origin pack. `PackSet::packs` is sorted longest name
        // first, so the most nested pack that owns the path comes first.
        let origin_pack = configuration.pack_set.packs.iter().find(|p| {
            p.name != "."
                && (source_str
                    .starts_with(&format!("{}/", p.relative_path.display()))
                    || source_str == p.relative_path.to_string_lossy())
        });

        // The path with its origin pack prefix removed, so it can be re-rooted
        // under the destination pack. Files outside any pack keep their path.
        let within_pack = origin_pack.map_or_else(
            || source_str.clone(),
            |origin| {
                let origin_prefix =
                    format!("{}/", origin.relative_path.display());
                source_str
                    .strip_prefix(&origin_prefix)
                    .unwrap_or(&source_str)
                    .to_string()
            },
        );
        let dest_path = dest_relative_path.join(&within_pack);

        operations.push(FileMoveOperation {
            origin: source_file.clone(),
            destination: dest_path.clone(),
        });

        // Auto-detect corresponding spec file
        let spec_origin_within_pack = compute_spec_path(&within_pack);

        if let Some(spec_relative) = spec_origin_within_pack {
            let spec_origin = origin_pack.map_or_else(
                || PathBuf::from(&spec_relative),
                |origin| origin.relative_path.join(&spec_relative),
            );
            let spec_dest = dest_relative_path.join(&spec_relative);

            operations.push(FileMoveOperation {
                origin: spec_origin,
                destination: spec_dest,
            });
        }
    }

    println!("{}", "=".repeat(100));
    println!("File Operations");

    let mut moved_pairs: Vec<(String, String)> = Vec::new();

    for op in &operations {
        let origin_abs = configuration.absolute_root.join(&op.origin);
        let dest_abs = configuration.absolute_root.join(&op.destination);
        let origin_exists = origin_abs.exists();
        let dest_exists = dest_abs.exists();

        if origin_exists && dest_exists {
            println!(
                "[SKIP] Not moving {}, {} already exists",
                op.origin.display(),
                op.destination.display()
            );
        } else if origin_exists && !dest_exists {
            if let Some(parent) = dest_abs.parent() {
                std::fs::create_dir_all(parent)?;
            }
            move_file(&origin_abs, &dest_abs).with_context(|| {
                format!(
                    "Failed to move {} to {}. {}",
                    op.origin.display(),
                    op.destination.display(),
                    describe_moved_files(&moved_pairs)
                )
            })?;
            println!(
                "Moving file {} to {}",
                op.origin.display(),
                op.destination.display()
            );
            moved_pairs.push((
                op.origin.to_string_lossy().to_string(),
                op.destination.to_string_lossy().to_string(),
            ));
        } else if !origin_exists && dest_exists {
            println!(
                "[SKIP] Not moving {}, does not exist, ({} already exists)",
                op.origin.display(),
                op.destination.display()
            );
        }
        // Packwerk is silent when neither path exists.
    }

    let rubocop_todo_path =
        configuration.absolute_root.join(".rubocop_todo.yml");
    if !moved_pairs.is_empty() && rubocop_todo_path.exists() {
        rewrite_rubocop_todo(&rubocop_todo_path, &moved_pairs)?;
    }

    Ok(())
}

/// `rename` cannot cross a filesystem, and a pack directory can be a symlink
/// to another mount, so a copy stands in for the rename in that one case.
fn move_file(origin: &Path, destination: &Path) -> anyhow::Result<()> {
    match std::fs::rename(origin, destination) {
        Err(error) if error.kind() == std::io::ErrorKind::CrossesDevices => {
            std::fs::copy(origin, destination)?;
            std::fs::remove_file(origin)?;
            Ok(())
        }
        result => Ok(result?),
    }
}

/// A failure part way through leaves the repository half moved, and nothing
/// puts it back, so the error has to say what already moved.
fn describe_moved_files(moved_pairs: &[(String, String)]) -> String {
    if moved_pairs.is_empty() {
        return String::from("No file moved before this failure.");
    }

    let moved = moved_pairs
        .iter()
        .map(|(origin, destination)| {
            format!("\n  {} -> {}", origin, destination)
        })
        .collect::<String>();

    format!(
        "These files moved before the failure and stay where they are now:{}",
        moved
    )
}

/// The file lists paths under `Exclude:` keys, and a rubocop-generated one is
/// full of comments that a parse-and-serialize round trip would throw away, so
/// the rewrite works line by line. Only a whole list entry counts: a plain
/// string replace would also rewrite `app/foo.rb.bak` when `app/foo.rb` moved.
fn rewrite_rubocop_todo(
    path: &Path,
    moved_pairs: &[(String, String)],
) -> anyhow::Result<()> {
    let contents = std::fs::read_to_string(path)?;
    let mut replacement_counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut rewritten = String::with_capacity(contents.len());

    for line in contents.split_inclusive('\n') {
        let (text, line_ending) = split_line_ending(line);
        match replace_moved_list_entry(text, moved_pairs) {
            Some((origin, replacement)) => {
                *replacement_counts.entry(origin).or_insert(0) += 1;
                rewritten.push_str(&replacement);
            }
            None => rewritten.push_str(text),
        }
        rewritten.push_str(line_ending);
    }

    // Rewriting a file that did not change would still move its mtime, which
    // wakes up file watchers and build caches for nothing.
    if replacement_counts.is_empty() {
        return Ok(());
    }

    // The moved order, not the map order, so that the report follows the moves.
    for (origin, _) in moved_pairs {
        if let Some(count) = replacement_counts.get(origin.as_str()) {
            println!(
                "Replaced {} occurrence(s) of {} in .rubocop_todo.yml",
                count, origin
            );
        }
    }

    std::fs::write(path, rewritten)?;

    Ok(())
}

fn split_line_ending(line: &str) -> (&str, &str) {
    if let Some(text) = line.strip_suffix("\r\n") {
        (text, "\r\n")
    } else if let Some(text) = line.strip_suffix('\n') {
        (text, "\n")
    } else {
        (line, "")
    }
}

/// Rewrites a YAML list entry, `    - 'app/services/horse.rb'`, when its whole
/// value is a path that moved. The indentation, the quotes and anything after
/// the value are kept as they are.
fn replace_moved_list_entry<'a>(
    line: &str,
    moved_pairs: &'a [(String, String)],
) -> Option<(&'a str, String)> {
    let indentation = &line[..line.len() - line.trim_start().len()];
    let after_marker = line.trim_start().strip_prefix('-')?;
    let spacing =
        &after_marker[..after_marker.len() - after_marker.trim_start().len()];
    if spacing.is_empty() {
        return None;
    }

    let value_onwards = after_marker.trim_start();
    let quote = value_onwards
        .chars()
        .next()
        .filter(|character| *character == '\'' || *character == '"');
    let (value, suffix) = match quote {
        Some(quote) => {
            let after_quote = &value_onwards[1..];
            let end = after_quote.find(quote)?;
            (&after_quote[..end], &after_quote[end..])
        }
        None => match value_onwards.find([' ', '\t', '#']) {
            Some(end) => (&value_onwards[..end], &value_onwards[end..]),
            None => (value_onwards, ""),
        },
    };

    let (origin, destination) = moved_pairs
        .iter()
        .find(|(origin, _)| origin.as_str() == value)?;
    let quote = quote.map(String::from).unwrap_or_default();

    Some((
        origin.as_str(),
        format!(
            "{}-{}{}{}{}",
            indentation, spacing, quote, destination, suffix
        ),
    ))
}

fn compute_spec_path(within_pack_path: &str) -> Option<String> {
    if within_pack_path.starts_with("app/") {
        // app/services/foo/bar.rb -> spec/services/foo/bar_spec.rb
        let without_app = within_pack_path.strip_prefix("app/")?;
        let without_ext = without_app.strip_suffix(".rb")?;
        Some(format!("spec/{}_spec.rb", without_ext))
    } else if within_pack_path.starts_with("lib/") {
        // lib/foo.rb -> spec/lib/foo_spec.rb
        let without_ext = within_pack_path.strip_suffix(".rb")?;
        Some(format!("spec/{}_spec.rb", without_ext))
    } else {
        None
    }
}

fn for_file(configuration: &Configuration, file: String) -> anyhow::Result<()> {
    let absolute_file_path =
        file_utils::get_absolute_path(file.clone(), configuration);

    match configuration.pack_set.for_file(&absolute_file_path)? {
        Some(pack) => {
            println!("{}", pack.yml.display());
            Ok(())
        }
        None => {
            bail!("No pack found for file: {}", file)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn moved_pairs() -> Vec<(String, String)> {
        vec![(
            String::from("app/services/horse.rb"),
            String::from("packs/animals/app/services/horse.rb"),
        )]
    }

    #[test]
    fn replaces_a_quoted_list_entry() {
        assert_eq!(
            replace_moved_list_entry(
                "    - 'app/services/horse.rb'",
                &moved_pairs()
            )
            .map(|(_, line)| line),
            Some(String::from("    - 'packs/animals/app/services/horse.rb'"))
        );
    }

    #[test]
    fn replaces_an_unquoted_list_entry_and_keeps_its_comment() {
        assert_eq!(
            replace_moved_list_entry(
                "  - app/services/horse.rb # a note",
                &moved_pairs()
            )
            .map(|(_, line)| line),
            Some(String::from(
                "  - packs/animals/app/services/horse.rb # a note"
            ))
        );
    }

    #[test]
    fn leaves_a_longer_path_alone() {
        assert_eq!(
            replace_moved_list_entry(
                "    - 'app/services/horse.rb.bak'",
                &moved_pairs()
            ),
            None
        );
    }

    #[test]
    fn leaves_a_line_that_is_not_a_list_entry_alone() {
        assert_eq!(
            replace_moved_list_entry(
                "  # app/services/horse.rb moved",
                &moved_pairs()
            ),
            None
        );
        assert_eq!(
            replace_moved_list_entry(
                "  Exclude: app/services/horse.rb",
                &moved_pairs()
            ),
            None
        );
    }

    #[test]
    fn test_for_file() {
        let configuration = configuration::get(
            PathBuf::from("tests/fixtures/simple_app")
                .canonicalize()
                .expect("Could not canonicalize path")
                .as_path(),
            &10,
        )
        .unwrap();
        let absolute_file_path = configuration
            .absolute_root
            .join("packs/foo/app/services/foo.rb")
            .canonicalize()
            .expect("Could not canonicalize path");

        assert_eq!(
            String::from("packs/foo"),
            configuration
                .pack_set
                .for_file(&absolute_file_path)
                .unwrap()
                .unwrap()
                .name
        )
    }
}

#[cfg(test)]
mod test_util {
    use crate::configuration::Configuration;
    use crate::parsing::ruby::zeitwerk::get_zeitwerk_constant_resolver;
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;

    use crate::configuration;

    use crate::configuration::from_raw;
    use crate::constant_resolver::ConstantResolver;
    use crate::pack::Pack;
    use crate::raw_configuration::RawConfiguration;
    use crate::walk_directory::WalkDirectoryResult;

    pub const SIMPLE_APP: &str = "tests/fixtures/simple_app";

    pub fn get_absolute_root(fixture_name: &str) -> PathBuf {
        PathBuf::from(fixture_name).canonicalize().unwrap()
    }

    pub fn get_zeitwerk_constant_resolver_for_fixture(
        fixture_name: &str,
    ) -> anyhow::Result<Box<dyn ConstantResolver>> {
        let absolute_root = get_absolute_root(fixture_name);
        let configuration = configuration::get(&absolute_root, &10)?;

        get_zeitwerk_constant_resolver(
            &configuration.pack_set,
            &configuration.constant_resolver_configuration(),
        )
        .map(|resolver| resolver as Box<dyn ConstantResolver>)
    }

    // Note that instead, we could derive the `Default` trait on `Pack`
    // However, there should be no reason the "production" code ever initializes
    // a default Pack directly, so this implementation is test only.
    #[allow(clippy::derivable_impls)]
    impl Default for Pack {
        fn default() -> Self {
            Self {
                yml: Default::default(),
                name: Default::default(),
                relative_path: Default::default(),
                dependencies: Default::default(),
                ignored_dependencies: Default::default(),
                ignored_private_constants: Default::default(),
                private_constants: Default::default(),
                package_todo: Default::default(),
                visible_to: Default::default(),
                public_path: Default::default(),
                layer: Default::default(),
                enforce_dependencies: Default::default(),
                enforce_privacy: Default::default(),
                enforce_visibility: Default::default(),
                enforce_folder_privacy: Default::default(),
                enforce_folder_visibility: None,
                enforce_layers: Default::default(),
                client_keys: Default::default(),
                owner: Default::default(),
                enforcement_globs_ignore: Default::default(),
            }
        }
    }

    impl Default for Configuration {
        fn default() -> Self {
            let default_absolute_root = std::env::current_dir().unwrap();
            let root_pack = Pack {
                name: ".".to_owned(),
                ..Pack::default()
            };

            let included_packs: HashSet<Pack> =
                vec![root_pack].into_iter().collect();

            let walk_directory_result = WalkDirectoryResult {
                included_files: HashSet::new(),
                included_packs,
                owning_package_yml_for_file: HashMap::new(),
            };
            from_raw(
                &default_absolute_root,
                RawConfiguration::default(),
                None,
                walk_directory_result,
                &0,
            )
            .unwrap() // TODO: potentially convert `default` to `new` and return a Result
        }
    }
}
