use anyhow::Context;
use clap::{Parser, Subcommand};
use clap_derive::Args;
use std::path::PathBuf;
use tracing::debug;

use super::ReferenceFormat;
use super::color::ColorChoice;
use super::logger::install_logger;

// Release builds are stamped from the Git tag, so the manifest version stays at
// 0.0.0 and nothing has to be committed to cut a release.
const VERSION: &str = match option_env!("CRABWERK_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

/// A CLI to interact with packs
#[derive(Parser, Debug)]
#[command(author, version = VERSION, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,

    /// Path for the root of the project
    #[arg(long, default_value = ".")]
    project_root: PathBuf,

    /// Path to the configuration file to read, instead of looking for
    /// `crabwerk.yml` in the project root. A relative path is resolved against
    /// the project root.
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// Run with performance debug mode
    #[arg(short, long)]
    debug: bool,

    /// When to colour the output. `auto` colours a terminal only, and obeys NO_COLOR
    #[arg(long, global = true, value_name = "WHEN", default_value = "auto")]
    color: ColorChoice,

    /// Run with the experimental parser, which gets constant definitions directly from the AST
    #[arg(short, long)]
    experimental_parser: bool,

    /// Print to console when files begin and finish processing (to identify files that panic when processing files concurrently)
    #[arg(short, long)]
    print_files: bool,

    /// Globally disable enforce_dependency
    #[arg(long)]
    disable_enforce_dependencies: bool,

    /// Globally disable enforce_folder_privacy
    #[arg(long)]
    disable_enforce_folder_privacy: bool,

    /// Globally disable enforce_layers
    #[arg(long)]
    disable_enforce_layers: bool,

    /// Globally disable enforce_privacy
    #[arg(long)]
    disable_enforce_privacy: bool,

    /// Globally disable enforce_visibility
    #[arg(long)]
    disable_enforce_visibility: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    #[clap(about = "Run check, validate, and lint")]
    All,

    #[clap(about = "Set up crabwerk in this project")]
    Init {
        /// Generate packwerk compatible packwerk.yml instead of crabwerk.yml
        #[arg(long)]
        use_packwerk: bool,
    },

    #[clap(
        about = "Copy a packwerk.yml written for the gem to the crabwerk.yml that crabwerk reads"
    )]
    MigrateConfig,

    #[clap(about = "Create a new pack")]
    Create { name: String },

    #[clap(about = "Look for violations in the codebase")]
    Check {
        /// Ignore recorded violations when reporting violations
        #[arg(long)]
        ignore_recorded_violations: bool,

        /// Output results as JSON
        #[arg(long)]
        json: bool,

        files: Vec<String>,
    },

    #[clap(about = "Update package_todo.yml files with the current violations")]
    Update {
        /// Files to scope the update to (merge mode). Without files, replaces all package_todo.yml files.
        files: Vec<String>,

        /// Expand file arguments to their owning pack(s), updating all files in those packs
        #[arg(long)]
        pack: bool,

        /// Only update violations for this constant (e.g. "::Foo")
        #[arg(long)]
        constant: Option<String>,

        /// Only update violations of this type (e.g. "dependency", "privacy")
        #[arg(long)]
        violation_type: Option<String>,

        /// Only update violations where the defining pack matches (e.g. "packs/bar")
        #[arg(long)]
        defining_pack: Option<String>,
    },

    #[clap(about = "Look for validation errors in the codebase")]
    Validate {
        /// Output results as JSON
        #[arg(long)]
        json: bool,
    },

    #[clap(about = "Add a dependency from one pack to another")]
    AddDependency {
        /// The pack that depends on another pack
        from: String,

        /// The pack that is depended on
        to: String,
    },

    #[clap(
        about = "Add missing dependencies for the pack that defines the constant"
    )]
    UpdateDependenciesForConstant {
        /// Update every pack that references this constant
        constant: String,
    },

    #[clap(
        about = "Check for dependencies that when removed produce no violations."
    )]
    CheckUnnecessaryDependencies {
        #[arg(long)]
        auto_correct: bool,
    },

    #[clap(about = "Add everything a pack depends on (may cause cycles)")]
    AddDependencies { pack_name: String },

    #[clap(about = "Lint package.yml and package_todo.yml files")]
    Lint,

    #[clap(
        about = "Expose monkey patches of the Ruby stdlib, gems your app uses, and your application itself"
    )]
    ExposeMonkeyPatches(ExposeMonkeyPatchesArgs),

    #[clap(
        about = "List packs based on configuration in crabwerk.yml (for debugging purposes)"
    )]
    ListPacks,

    #[clap(about = "List packs that depend on a pack")]
    ListPackDependencies {
        /// The pack that is depended on
        pack: String,
    },

    #[clap(
        about = "List analyzed files based on configuration in crabwerk.yml (for debugging purposes)"
    )]
    ListIncludedFiles,

    #[clap(
        about = "List the constants that crabwerk sees and where it sees them (for debugging purposes)"
    )]
    ListDefinitions(ListDefinitionsArgs),

    #[clap(
        about = "List constant references and their definition files (for test selection)"
    )]
    ListReferences(ListReferencesArgs),

    #[clap(about = "Print the path to the package.yml that owns a file")]
    ForFile {
        /// The file to find the owning package.yml for
        file: String,
    },

    #[clap(about = "Remove a dependency from one pack to another")]
    RemoveDependency {
        /// The pack that currently depends on another pack
        from: String,

        /// The pack that is currently depended on
        to: String,
    },

    #[clap(about = "Move files to a pack")]
    Move {
        /// The destination pack (e.g. packs/animals)
        destination: String,

        /// One or more file or directory paths to move
        #[arg(required = true)]
        paths: Vec<String>,
    },
}

#[derive(Debug, Args)]
struct ListDefinitionsArgs {
    /// Show constants with multiple definitions only
    #[arg(short, long)]
    ambiguous: bool,
}

#[derive(Debug, Args)]
struct ListReferencesArgs {
    /// Output format
    #[arg(short, long, default_value = "json")]
    format: ReferenceFormat,

    /// Output file path
    #[arg(short, long)]
    out: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct ExposeMonkeyPatchesArgs {
    /// An absolute path to the directory containing Ruby source code (for extracting definitions from Ruby stdlib)
    /// Example: /Users/alex.evanczuk/.rbenv/versions/3.2.2/lib/ruby/3.2.0/
    #[arg(short, long)]
    rubydir: PathBuf,

    /// An absolute path to the directory containing your gems (for extracting definitions from gem source code)
    /// Example: /Users/alex.evanczuk/.rbenv/versions/3.2.2/lib/ruby/gems/3.2.0/gems/
    #[arg(short, long)]
    gemdir: PathBuf,
}

impl Args {
    fn absolute_project_root(&self) -> anyhow::Result<PathBuf> {
        self.project_root
            .canonicalize()
            .map_err(anyhow::Error::from)
    }
}

pub fn run() -> anyhow::Result<()> {
    let args = Args::parse();
    let absolute_root = args.absolute_project_root().with_context(|| {
        format!(
            "Could not resolve --project-root {}",
            args.project_root.display()
        )
    })?;

    install_logger(args.debug);

    // Two commands run in directories whose configuration `crabwerk` cannot
    // load: `init` runs where there is no configuration yet, and
    // `migrate-config` runs where the only configuration is a `packwerk.yml`,
    // which is an error to load. Both are handled here, before the load below,
    // and return without it.
    if let Command::Init { use_packwerk } = args.command {
        crate::init(&absolute_root, use_packwerk)?;
        println!(
            "Successfully initialized crabwerk{} in this directory!",
            if use_packwerk { "/packwerk" } else { "" }
        );
        return Ok(());
    }

    if let Command::MigrateConfig = args.command {
        return crate::migrate_config(&absolute_root);
    }

    // Input filesize TBD
    let mut configuration = crate::configuration::get_with_config_path(
        &absolute_root,
        &0,
        args.config.as_deref(),
    )?;

    configuration.color = args.color.enabled();

    if args.print_files {
        configuration.print_files = true;
    }

    if args.experimental_parser {
        debug!("Using experimental parser");
        configuration.experimental_parser = true;
    }

    if args.disable_enforce_dependencies {
        configuration.disable_enforce_dependencies = true;
    }

    if args.disable_enforce_folder_privacy {
        configuration.disable_enforce_folder_privacy = true;
    }

    if args.disable_enforce_layers {
        configuration.disable_enforce_layers = true;
    }

    if args.disable_enforce_privacy {
        configuration.disable_enforce_privacy = true;
    }

    if args.disable_enforce_visibility {
        configuration.disable_enforce_visibility = true;
    }

    match args.command {
        Command::All => {
            let check_result = crate::check(&configuration, vec![], false);
            let validate_result = crate::validate(&configuration, false);
            let lint_result = crate::lint(&configuration);

            check_result.and(validate_result).and(lint_result)
        }
        Command::Init { .. } => {
            unreachable!("handled before the configuration load")
        }
        Command::MigrateConfig => {
            unreachable!("handled before the configuration load")
        }
        Command::ListPacks => {
            crate::list(configuration);
            Ok(())
        }
        Command::ListPackDependencies { pack } => {
            crate::list_dependencies(&configuration, pack)
        }
        Command::AddDependency { from, to } => {
            crate::add_dependency(&configuration, from, to)
        }
        Command::ListIncludedFiles => crate::list_included_files(configuration),
        Command::Check {
            ignore_recorded_violations,
            json,
            files,
        } => {
            configuration.ignore_recorded_violations =
                ignore_recorded_violations;
            configuration.input_files_count = files.len();
            crate::check(&configuration, files, json)
        }
        Command::Update {
            files,
            pack,
            constant,
            violation_type,
            defining_pack,
        } => crate::update(
            &configuration,
            &crate::checker::UpdateOptions {
                files,
                expand_to_pack: pack,
                constant_name: constant,
                violation_type,
                defining_pack_name: defining_pack,
            },
        ),
        Command::Validate { json } => crate::validate(&configuration, json),
        Command::CheckUnnecessaryDependencies { auto_correct } => {
            crate::check_unnecessary_dependencies(&configuration, auto_correct)
        }
        Command::AddDependencies { pack_name } => {
            crate::add_dependencies(&configuration, &pack_name)
        }
        Command::UpdateDependenciesForConstant { constant } => Ok(
            crate::update_dependencies_for_constant(&configuration, &constant)?,
        ),
        Command::ListDefinitions(args) => {
            let ambiguous = args.ambiguous;
            crate::list_definitions(&configuration, ambiguous)
        }
        Command::ListReferences(args) => crate::list_references(
            &configuration,
            args.format,
            args.out.as_deref(),
        ),
        Command::ExposeMonkeyPatches(args) => crate::expose_monkey_patches(
            &configuration,
            &args.rubydir,
            &args.gemdir,
        ),
        Command::Lint => crate::lint(&configuration),
        Command::Create { name } => crate::create(&configuration, name),
        Command::ForFile { file } => crate::for_file(&configuration, file),
        Command::RemoveDependency { from, to } => {
            crate::remove_dependency(&configuration, from, to)
        }
        Command::Move { destination, paths } => {
            crate::move_to_pack(&configuration, &destination, paths)
        }
    }
}
