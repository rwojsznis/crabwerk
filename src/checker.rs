// Module declarations
mod dependency;
pub mod layer;

mod common_test;
mod folder_privacy;
pub mod pack_checker;
mod privacy;
pub mod reference;
mod visibility;

// Internal imports
use crate::Configuration;
use crate::pack::Pack;
use crate::pack::write_pack_to_disk;
use crate::package_todo;

use anyhow::Context;
use anyhow::bail;
// External imports
use rayon::prelude::IntoParallelRefIterator;
use rayon::prelude::ParallelIterator;
use reference::Reference;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::{collections::HashSet, path::PathBuf};
use tracing::debug;

use super::Sigil;
use super::reference_extractor::get_all_references_and_sigils;

pub struct UpdateOptions {
    pub files: Vec<String>,
    pub expand_to_pack: bool,
    pub constant_name: Option<String>,
    pub violation_type: Option<String>,
    pub defining_pack_name: Option<String>,
}

impl UpdateOptions {
    pub const fn is_scoped(&self) -> bool {
        !self.files.is_empty()
            || self.constant_name.is_some()
            || self.violation_type.is_some()
            || self.defining_pack_name.is_some()
    }
}

#[derive(PartialEq, Clone, Eq, Hash, Debug, Serialize)]
pub struct ViolationIdentifier {
    pub violation_type: String,
    pub strict: bool,
    pub file: String,
    pub constant_name: String,
    pub referencing_pack_name: String,
    pub defining_pack_name: String,
}
impl ViolationIdentifier {
    // Every list of identifiers is collected from a HashSet or a rayon
    // iterator, so this key is what makes the printed and serialized output
    // independent of the iteration and thread order.
    fn sort_key(&self) -> (&str, &str, &str, &str, &str, bool) {
        (
            &self.file,
            &self.constant_name,
            &self.violation_type,
            &self.referencing_pack_name,
            &self.defining_pack_name,
            self.strict,
        )
    }
}

#[derive(PartialEq, Clone, Eq, Hash, Debug, Serialize)]
pub struct Violation {
    /// The explanation only. The location line above it belongs to the
    /// report, so that colour is added where the report is written and no
    /// consumer has to remove it again.
    pub message: String,
    pub identifier: ViolationIdentifier,
    pub source_location: crate::SourceLocation,
}

impl Violation {
    /// `path/to/file.rb:36:0`, as `packwerk` writes it above the explanation.
    fn location(&self, color: bool) -> String {
        let file = if color {
            format!("\x1b[36m{}\x1b[0m", self.identifier.file)
        } else {
            self.identifier.file.clone()
        };

        format!(
            "{}:{}:{}",
            file, self.source_location.line, self.source_location.column
        )
    }

    /// The whole violation with no colour: what `--json` reports, and the sort
    /// key that keeps the printed order the same whatever `--color` says.
    fn plain_report(&self) -> String {
        format!("{}\n{}", self.location(false), self.message)
    }
}

pub trait CheckerInterface {
    fn check(
        &self,
        reference: &Reference,
        configuration: &Configuration,
        sigils: &HashMap<PathBuf, Vec<Sigil>>,
    ) -> anyhow::Result<Option<Violation>>;

    fn violation_type(&self) -> String;
}

pub trait ValidatorInterface {
    fn validate(&self, configuration: &Configuration) -> Option<Vec<String>>;
}

#[derive(Debug, PartialEq, Eq)]
pub struct CheckAllResult {
    reportable_violations: HashSet<Violation>,
    stale_violations: Vec<ViolationIdentifier>,
    strict_mode_violations: Vec<ViolationIdentifier>,
    color: bool,
}

impl CheckAllResult {
    pub fn has_violations(&self) -> bool {
        !self.reportable_violations.is_empty()
            || !self.stale_violations.is_empty()
            || !self.strict_mode_violations.is_empty()
    }

    pub fn violation_count(&self) -> usize {
        self.reportable_violations.len()
            + self.stale_violations.len()
            + self.strict_mode_violations.len()
    }

    // The report is the sort key a user sees, but two violations can share
    // one, so the identifier breaks the tie.
    fn sorted_reportable_violations(&self) -> Vec<&Violation> {
        let mut keyed: Vec<(String, &Violation)> = self
            .reportable_violations
            .iter()
            .map(|v| (v.plain_report(), v))
            .collect();
        keyed.sort_by(|(a_report, a), (b_report, b)| {
            a_report.cmp(b_report).then_with(|| {
                a.identifier.sort_key().cmp(&b.identifier.sort_key())
            })
        });
        keyed.into_iter().map(|(_, v)| v).collect()
    }

    fn sorted(
        identifiers: &[ViolationIdentifier],
    ) -> Vec<&ViolationIdentifier> {
        let mut sorted: Vec<&ViolationIdentifier> =
            identifiers.iter().collect();
        sorted.sort_by_key(|v| v.sort_key());
        sorted
    }

    pub fn to_json(&self) -> serde_json::Result<String> {
        let output = CheckAllJsonOutput {
            status: if self.has_violations() {
                "failure"
            } else {
                "success"
            },
            violations: self
                .sorted_reportable_violations()
                .into_iter()
                .map(JsonViolation::from)
                .collect(),
            stale_violations: Self::sorted(&self.stale_violations),
            strict_mode_violations: Self::sorted(&self.strict_mode_violations),
        };
        serde_json::to_string(&output)
    }

    fn write_violations(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if !self.reportable_violations.is_empty() {
            let sorted_violations = self.sorted_reportable_violations();

            writeln!(f, "{} violation(s) detected:", sorted_violations.len())?;

            for violation in sorted_violations {
                writeln!(
                    f,
                    "{}\n{}\n",
                    violation.location(self.color),
                    violation.message
                )?;
            }
        }

        if !self.stale_violations.is_empty() {
            writeln!(
                f,
                "There were stale violations found, please run `crabwerk update`"
            )?;
        }

        if !self.strict_mode_violations.is_empty() {
            for v in Self::sorted(&self.strict_mode_violations) {
                let error_message = build_strict_violation_message(v);
                writeln!(f, "{}", error_message)?;
            }
        }
        Ok(())
    }
}

impl Display for CheckAllResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.has_violations() {
            self.write_violations(f)
        } else {
            write!(f, "No violations detected!")
        }
    }
}

#[derive(Serialize)]
struct CheckAllJsonOutput<'a> {
    status: &'a str,
    violations: Vec<JsonViolation>,
    stale_violations: Vec<&'a ViolationIdentifier>,
    strict_mode_violations: Vec<&'a ViolationIdentifier>,
}

#[derive(Serialize)]
struct JsonViolation {
    message: String,
    file: String,
    line: usize,
    column: usize,
    violation_type: String,
    strict: bool,
    constant_name: String,
    referencing_pack_name: String,
    defining_pack_name: String,
}

impl From<&Violation> for JsonViolation {
    fn from(v: &Violation) -> Self {
        Self {
            message: v.plain_report(),
            file: v.identifier.file.clone(),
            line: v.source_location.line,
            column: v.source_location.column,
            violation_type: v.identifier.violation_type.clone(),
            strict: v.identifier.strict,
            constant_name: v.identifier.constant_name.clone(),
            referencing_pack_name: v.identifier.referencing_pack_name.clone(),
            defining_pack_name: v.identifier.defining_pack_name.clone(),
        }
    }
}

struct CheckAllBuilder<'a> {
    configuration: &'a Configuration,
    found_violations: &'a FoundViolations,
}
#[derive(Debug)]
struct FoundViolations {
    absolute_paths: HashSet<PathBuf>,
    violations: HashSet<Violation>,
}

impl<'a> CheckAllBuilder<'a> {
    const fn new(
        configuration: &'a Configuration,
        found_violations: &'a FoundViolations,
    ) -> Self {
        Self {
            configuration,
            found_violations,
        }
    }

    pub fn build(self) -> anyhow::Result<CheckAllResult> {
        let recorded_violations = &self.configuration.pack_set.all_violations;

        Ok(CheckAllResult {
            reportable_violations: self
                .build_reportable_violations(recorded_violations)
                .into_iter()
                .cloned()
                .collect(),
            stale_violations: self
                .build_stale_violations(recorded_violations)?
                .into_iter()
                .cloned()
                .collect(),
            strict_mode_violations: self
                .build_strict_mode_violations()
                .into_iter()
                .cloned()
                .collect(),
            color: self.configuration.color,
        })
    }

    fn build_reportable_violations(
        &self,
        recorded_violations: &HashSet<ViolationIdentifier>,
    ) -> HashSet<&'a Violation> {
        if self.configuration.ignore_recorded_violations {
            debug!("Filtering recorded violations is disabled in config");
            self.found_violations.violations.iter().collect()
        } else {
            self.found_violations
                .violations
                .iter()
                .filter(|v| !recorded_violations.contains(&v.identifier))
                .collect()
        }
    }

    fn build_stale_violations(
        &self,
        recorded_violations: &'a HashSet<ViolationIdentifier>,
    ) -> anyhow::Result<Vec<&'a ViolationIdentifier>> {
        let found_violation_identifiers: HashSet<&ViolationIdentifier> = self
            .found_violations
            .violations
            .par_iter()
            .map(|v| &v.identifier)
            .collect();
        let relative_files = self
            .found_violations
            .absolute_paths
            .iter()
            .map(|p| {
                p.strip_prefix(&self.configuration.absolute_root)
                    .map_err(|e| {
                        anyhow::Error::new(e).context(format!(
                            "Failed to strip prefix from {:?}",
                            self.configuration.absolute_root
                        ))
                    })
                    .and_then(|path| {
                        path.to_str().ok_or_else(|| {
                            anyhow::Error::new(std::fmt::Error).context(
                                format!(
                                    "Path ({:?}) cannot be converted to &str",
                                    path
                                ),
                            )
                        })
                    })
            })
            .collect::<anyhow::Result<HashSet<&str>>>()?;

        let stale_violations = recorded_violations
            .par_iter()
            .filter(|v_identifier| {
                Self::is_stale_violation(
                    &relative_files,
                    &found_violation_identifiers,
                    v_identifier,
                )
            })
            .collect::<Vec<&ViolationIdentifier>>();
        Ok(stale_violations)
    }

    fn is_stale_violation(
        relative_files: &HashSet<&str>,
        found_violation_identifiers: &HashSet<&ViolationIdentifier>,
        todo_violation_identifier: &ViolationIdentifier,
    ) -> bool {
        let violation_path_exists =
            relative_files.contains(todo_violation_identifier.file.as_str());
        if violation_path_exists {
            !found_violation_identifiers.contains(todo_violation_identifier)
        } else {
            true // The todo violation references a file that no longer exists
        }
    }

    fn build_strict_mode_violations(&self) -> Vec<&'a ViolationIdentifier> {
        self.found_violations
            .violations
            .iter()
            .filter(|v| v.identifier.strict)
            .map(|v| &v.identifier)
            .collect()
    }
}

pub fn check_all(
    configuration: &Configuration,
    files: Vec<String>,
) -> anyhow::Result<CheckAllResult> {
    let checkers = get_checkers(configuration);

    debug!("Intersecting input files with configuration included files");
    let absolute_paths: HashSet<PathBuf> = configuration.intersect_files(files);

    let violations: HashSet<Violation> =
        get_all_violations(configuration, &absolute_paths, &checkers)?;
    let found_violations = FoundViolations {
        absolute_paths,
        violations,
    };
    CheckAllBuilder::new(configuration, &found_violations).build()
}

fn validate(configuration: &Configuration) -> Vec<String> {
    debug!("Running validators against packages");
    let validators: Vec<Box<dyn ValidatorInterface + Send + Sync>> = vec![
        Box::new(dependency::Checker {}),
        Box::new(layer::Checker {
            layers: configuration.layers.clone(),
        }),
    ];

    let mut validation_errors: Vec<String> = validators
        .iter()
        .filter_map(|v| v.validate(configuration))
        .flatten()
        .collect();
    validation_errors.dedup();
    debug!("Finished validators against packages");

    validation_errors
}

pub fn build_strict_violation_message(
    violation_identifier: &ViolationIdentifier,
) -> String {
    format!(
        "{} cannot have {} violations on {} because strict mode is enabled for {} violations in the enforcing pack's package.yml file",
        violation_identifier.referencing_pack_name,
        violation_identifier.violation_type,
        violation_identifier.defining_pack_name,
        violation_identifier.violation_type,
    )
}

pub fn validate_all(configuration: &Configuration) -> anyhow::Result<()> {
    let validation_errors = validate(configuration);
    if !validation_errors.is_empty() {
        println!("{} validation error(s) detected:", validation_errors.len());
        for validation_error in validation_errors.iter() {
            println!("{}\n", validation_error);
        }

        bail!("Crabwerk validate failed")
    } else {
        println!("Crabwerk validate succeeded!");
        Ok(())
    }
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct CycleEdge {
    pub from_pack: String,
    pub to_pack: String,
    pub file: String,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub error_type: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cycle_edges: Option<Vec<CycleEdge>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

#[derive(Serialize)]
struct ValidateJsonOutput {
    status: String,
    validation_errors: Vec<ValidationError>,
}

pub fn validate_structured(
    configuration: &Configuration,
) -> Vec<ValidationError> {
    let mut errors = dependency::validate_structured(configuration);

    // Layer validation (string-based, converted to ValidationError)
    let layer_checker = layer::Checker {
        layers: configuration.layers.clone(),
    };
    if let Some(layer_errors) = layer_checker.validate(configuration) {
        for msg in layer_errors {
            errors.push(ValidationError {
                error_type: "layer".to_string(),
                message: msg,
                cycle_edges: None,
                file: None,
            });
        }
    }

    errors.dedup();
    errors
}

pub fn validate_all_json(configuration: &Configuration) -> anyhow::Result<()> {
    let validation_errors = validate_structured(configuration);
    let has_errors = !validation_errors.is_empty();

    let output = ValidateJsonOutput {
        status: if has_errors {
            "failure".to_string()
        } else {
            "success".to_string()
        },
        validation_errors,
    };

    println!(
        "{}",
        serde_json::to_string(&output)
            .context("Failed to serialize validation JSON")?
    );

    if has_errors {
        std::process::exit(1);
    }

    Ok(())
}

pub fn update(
    configuration: &Configuration,
    options: &UpdateOptions,
) -> anyhow::Result<()> {
    if options.expand_to_pack && options.files.is_empty() {
        bail!("--pack requires at least one file argument");
    }

    let checkers = get_checkers(configuration);

    let absolute_paths = if options.is_scoped() {
        resolve_scoped_files(configuration, options)?
    } else {
        configuration.included_files.clone()
    };

    let violations =
        get_all_violations(configuration, &absolute_paths, &checkers)?;

    let violations = if options.is_scoped() {
        filter_violations(violations, options)
    } else {
        violations
    };

    let strict_violations = &violations
        .iter()
        .filter(|v| v.identifier.strict)
        .collect::<Vec<&Violation>>();
    if !strict_violations.is_empty() {
        for violation in strict_violations {
            let strict_message =
                build_strict_violation_message(&violation.identifier);
            println!("{}", strict_message);
        }
        println!(
            "{} strict mode violation(s) detected. These violations must be fixed for `check` to succeed.",
            strict_violations.len()
        );
    }

    let stats = if options.is_scoped() {
        package_todo::merge_violations_to_disk(configuration, violations)?
    } else {
        package_todo::write_violations_to_disk(configuration, violations)?
    };

    if stats.is_empty() {
        println!("No changes to package_todo.yml files.");
    } else {
        let mut parts = Vec::new();
        if stats.violations_added > 0 {
            parts
                .push(format!("{} violation(s) added", stats.violations_added));
        }
        if stats.violations_removed > 0 {
            parts.push(format!(
                "{} violation(s) removed",
                stats.violations_removed
            ));
        }
        if stats.files_changed > 0 {
            parts.push(format!("{} file(s) modified", stats.files_changed));
        }
        if stats.files_added > 0 {
            parts.push(format!("{} file(s) added", stats.files_added));
        }
        if stats.files_deleted > 0 {
            parts.push(format!("{} file(s) deleted", stats.files_deleted));
        }
        println!(
            "Successfully updated package_todo.yml files: {}",
            parts.join(", ")
        );
    }

    Ok(())
}

fn resolve_scoped_files(
    configuration: &Configuration,
    options: &UpdateOptions,
) -> anyhow::Result<HashSet<PathBuf>> {
    let mut files = if options.files.is_empty() {
        configuration.included_files.clone()
    } else {
        configuration.intersect_files(options.files.clone())
    };

    if options.expand_to_pack {
        if options.files.is_empty() {
            bail!("--pack requires at least one file argument");
        }
        let mut pack_names: HashSet<String> = HashSet::new();
        for file in &files {
            if let Some(pack) = configuration.pack_set.for_file(file)? {
                pack_names.insert(pack.name.clone());
            }
        }
        files = configuration.pack_set.files_for_packs(&pack_names);
    }

    Ok(files)
}

fn filter_violations(
    violations: HashSet<Violation>,
    options: &UpdateOptions,
) -> HashSet<Violation> {
    violations
        .into_iter()
        .filter(|v| {
            if let Some(ref constant) = options.constant_name
                && v.identifier.constant_name != *constant
            {
                return false;
            }
            if let Some(ref vtype) = options.violation_type
                && v.identifier.violation_type != *vtype
            {
                return false;
            }
            if let Some(ref defining_pack) = options.defining_pack_name
                && v.identifier.defining_pack_name != *defining_pack
            {
                return false;
            }
            true
        })
        .collect()
}

pub fn remove_unnecessary_dependencies(
    configuration: &Configuration,
) -> anyhow::Result<()> {
    let unnecessary_dependencies = get_unnecessary_dependencies(configuration)?;
    for (pack, dependency_names) in unnecessary_dependencies.iter() {
        remove_reference_to_dependency(pack, dependency_names)?;
    }
    Ok(())
}

pub fn add_all_dependencies(
    configuration: &Configuration,
    pack_name: &str,
) -> anyhow::Result<()> {
    let (references, _sigils) = get_all_references_and_sigils(
        configuration,
        &configuration.included_files,
    )?;

    let from_pack = configuration
        .pack_set
        .for_pack(pack_name)
        .context(format!("`{}` not found", pack_name))?;

    let mut defining_pack_names: HashSet<String> = HashSet::new();

    for reference in references {
        if reference.referencing_pack_name == pack_name
            && let Some(defining_pack_name) = reference.defining_pack_name
            && defining_pack_name != pack_name
        {
            defining_pack_names.insert(defining_pack_name);
        }
    }

    let updated_pack = Pack {
        dependencies: defining_pack_names.clone().into_iter().collect(),
        ..from_pack.to_owned()
    };
    write_pack_to_disk(&updated_pack)?;

    Ok(())
}

pub fn check_unnecessary_dependencies(
    configuration: &Configuration,
) -> anyhow::Result<()> {
    let unnecessary_dependencies = get_unnecessary_dependencies(configuration)?;
    if unnecessary_dependencies.is_empty() {
        Ok(())
    } else {
        for (pack, dependency_names) in unnecessary_dependencies.iter() {
            for dependency_name in dependency_names {
                println!(
                    "{} depends on {} but does not use it",
                    pack.name, dependency_name
                )
            }
        }
        let found_message = if unnecessary_dependencies.len() == 1 {
            "Found 1 unnecessary dependency".to_string()
        } else {
            format!(
                "Found {} unnecessary dependencies",
                unnecessary_dependencies.len()
            )
        };
        bail!(
            "{}. Run command with `--auto-correct` to remove them.",
            found_message,
        );
    }
}

fn get_unnecessary_dependencies(
    configuration: &Configuration,
) -> anyhow::Result<HashMap<Pack, Vec<String>>> {
    let (references, _sigils) = get_all_references_and_sigils(
        configuration,
        &configuration.included_files,
    )?;
    let mut edge_counts: HashMap<(String, String), i32> = HashMap::new();
    for reference in references {
        let defining_pack_name = reference.defining_pack_name;
        if let Some(defining_pack_name) = defining_pack_name {
            let edge_key =
                (reference.referencing_pack_name, defining_pack_name);

            edge_counts
                .entry(edge_key)
                .and_modify(|f| *f += 1)
                .or_insert(1);
        }
    }

    let mut unnecessary_dependencies: HashMap<Pack, Vec<String>> =
        HashMap::new();
    for pack in &configuration.pack_set.packs {
        for dependency_name in &pack.dependencies {
            let edge_key = (pack.name.clone(), dependency_name.clone());
            let edge_count = edge_counts.get(&edge_key).unwrap_or(&0);
            if edge_count == &0 {
                unnecessary_dependencies
                    .entry(pack.clone())
                    .or_default()
                    .push(dependency_name.clone());
            }
        }
    }

    Ok(unnecessary_dependencies)
}

fn get_all_violations(
    configuration: &Configuration,
    absolute_paths: &HashSet<PathBuf>,
    checkers: &Vec<Box<dyn CheckerInterface + Send + Sync>>,
) -> anyhow::Result<HashSet<Violation>> {
    let (references, sigils) =
        get_all_references_and_sigils(configuration, absolute_paths)?;
    debug!("Running checkers on resolved references");

    // Split over references, not over the five checkers: there are only five
    // of them, so the other way round leaves every core past the fifth idle.
    // Each thread accumulates into a `Vec` and the set is built once at the
    // end: deduplicating per thread instead would hash every violation again
    // at each level of the reduce tree, which costs more than it saves.
    let found: Vec<Violation> = references
        .par_iter()
        .try_fold(Vec::new, |mut acc: Vec<Violation>, reference| {
            for checker in checkers {
                if let Some(violation) =
                    checker.check(reference, configuration, &sigils)?
                {
                    acc.push(violation);
                }
            }
            anyhow::Ok(acc)
        })
        .try_reduce(Vec::new, |mut acc, v| {
            acc.extend(v);
            anyhow::Ok(acc)
        })?;
    let violations: HashSet<Violation> = found.into_iter().collect();

    debug!("Finished running checkers");

    Ok(violations)
}

fn get_checkers(
    configuration: &Configuration,
) -> Vec<Box<dyn CheckerInterface + Send + Sync>> {
    vec![
        Box::new(dependency::Checker {}),
        Box::new(privacy::Checker {}),
        Box::new(visibility::Checker {}),
        Box::new(layer::Checker {
            layers: configuration.layers.clone(),
        }),
        Box::new(folder_privacy::Checker {}),
    ]
}

fn remove_reference_to_dependency(
    pack: &Pack,
    dependency_names: &[String],
) -> anyhow::Result<()> {
    let without_dependency = pack
        .dependencies
        .iter()
        .filter(|dependency| !dependency_names.contains(dependency));
    let updated_pack = Pack {
        dependencies: without_dependency.cloned().collect(),
        ..pack.clone()
    };
    write_pack_to_disk(&updated_pack)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use crate::SourceLocation;
    use crate::checker::{
        CheckAllResult, Violation, ViolationIdentifier,
        build_strict_violation_message,
    };
    use std::collections::HashSet;

    #[test]
    fn test_write_violations() {
        let chec_result = CheckAllResult {
            reportable_violations: [
                Violation {
                    message: "Privacy violation: `::Foo::PrivateClass` is private to `foo`, but referenced from `bar`".to_string(),
                    identifier: ViolationIdentifier {
                        violation_type: "Privacy".to_string(),
                        strict: false,
                        file: "foo/bar/file1.rb".to_string(),
                        constant_name: "::Foo::PrivateClass".to_string(),
                        referencing_pack_name: "bar".to_string(),
                        defining_pack_name: "foo".to_string(),
                    },
                    source_location: SourceLocation { line: 10, column: 5 },
                },
                Violation {
                    message: "Dependency violation: `::Foo::AnotherClass` is not allowed to depend on `::Bar::SomeClass`".to_string(),
                    identifier: ViolationIdentifier {
                        violation_type: "Dependency".to_string(),
                        strict: false,
                        file: "foo/bar/file2.rb".to_string(),
                        constant_name: "::Foo::AnotherClass".to_string(),
                        referencing_pack_name: "foo".to_string(),
                        defining_pack_name: "bar".to_string(),
                    },
                    source_location: SourceLocation { line: 15, column: 3 },
                },
            ].into_iter().collect(),
            stale_violations: Vec::new(),
            strict_mode_violations: Vec::new(),
            color: false,
        };

        let expected_output = "2 violation(s) detected:
foo/bar/file1.rb:10:5
Privacy violation: `::Foo::PrivateClass` is private to `foo`, but referenced from `bar`

foo/bar/file2.rb:15:3
Dependency violation: `::Foo::AnotherClass` is not allowed to depend on `::Bar::SomeClass`

";

        let actual = format!("{}", chec_result);

        assert_eq!(actual, expected_output);
    }

    fn strict_identifier(
        file: &str,
        constant_name: &str,
        violation_type: &str,
    ) -> ViolationIdentifier {
        ViolationIdentifier {
            violation_type: violation_type.to_string(),
            strict: true,
            file: file.to_string(),
            constant_name: constant_name.to_string(),
            referencing_pack_name: "packs/foo".to_string(),
            defining_pack_name: "packs/bar".to_string(),
        }
    }

    fn unsorted_strict_result() -> CheckAllResult {
        CheckAllResult {
            reportable_violations: HashSet::new(),
            stale_violations: vec![
                strict_identifier("b.rb", "::Bar", "privacy"),
                strict_identifier("a.rb", "::Baz", "privacy"),
                strict_identifier("a.rb", "::Baz", "dependency"),
            ],
            strict_mode_violations: vec![
                strict_identifier("b.rb", "::Bar", "privacy"),
                strict_identifier("a.rb", "::Baz", "privacy"),
                strict_identifier("a.rb", "::Baz", "dependency"),
            ],
            color: false,
        }
    }

    #[test]
    fn test_write_strict_mode_violations_is_sorted() {
        let expected_output = format!(
            "There were stale violations found, please run `crabwerk update`\n{}\n{}\n{}\n",
            build_strict_violation_message(&strict_identifier(
                "a.rb",
                "::Baz",
                "dependency"
            )),
            build_strict_violation_message(&strict_identifier(
                "a.rb", "::Baz", "privacy"
            )),
            build_strict_violation_message(&strict_identifier(
                "b.rb", "::Bar", "privacy"
            )),
        );

        assert_eq!(format!("{}", unsorted_strict_result()), expected_output);
    }

    #[test]
    fn test_to_json_identifier_lists_are_sorted() {
        let json: serde_json::Value =
            serde_json::from_str(&unsorted_strict_result().to_json().unwrap())
                .unwrap();

        for key in ["stale_violations", "strict_mode_violations"] {
            let keys: Vec<String> = json[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| {
                    format!(
                        "{}:{}",
                        v["file"].as_str().unwrap(),
                        v["violation_type"].as_str().unwrap()
                    )
                })
                .collect();

            assert_eq!(
                keys,
                vec![
                    "a.rb:dependency".to_string(),
                    "a.rb:privacy".to_string(),
                    "b.rb:privacy".to_string(),
                ],
                "{} is not sorted",
                key
            );
        }
    }
}
