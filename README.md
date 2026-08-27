# crabwerk
![Logo](logo.png)

[![CI](https://github.com/rwojsznis/crabwerk/actions/workflows/ci.yml/badge.svg)](https://github.com/rwojsznis/crabwerk/actions)
[![Security Audit](https://github.com/rwojsznis/crabwerk/actions/workflows/audit.yml/badge.svg)](https://github.com/rwojsznis/crabwerk/actions?query=workflow%3A%22Security+audit%22++)

A 100% Rust implementation of [packwerk](https://github.com/Shopify/packwerk), a gradual modularization platform for Ruby.

# Goals:
## Serve as a drop-in replacement for `packwerk` on most projects
- Currently can serve as a drop-in replacement on Gusto's extra-large Rails monolith
- This is a work in progress! Please see [Verification](#verification) for instructions on how to verify the output of `crabwerk` is the same as `packwerk`.

## Run 20x faster than `packwerk` on most projects
- Currently ~10-20x as fast as the ruby implementation. See [BENCHMARKS.md](https://github.com/rwojsznis/crabwerk/blob/main/BENCHMARKS.md).
- Your mileage may vary!
- Other performance improvements are coming soon!

## Support non-Rails, non-zeitwerk apps
- Currently supports non-Rails apps through an experimental implementation
- Uses the same public API as `packwerk`, but has different behavior.
- See [EXPERIMENTAL_PARSER_USAGE.md](https://github.com/rwojsznis/crabwerk/blob/main/EXPERIMENTAL_PARSER_USAGE.md) for more info

# Usage and Documentation
Once installed and added to your `$PATH`, just call `crabwerk` to see the CLI help message and documentation.

```
A CLI for working with packs (modular code organization) in Ruby codebases.

Usage: crabwerk [OPTIONS] <COMMAND>

Commands:
  all                               Run check, validate, and lint
  init                              Set up crabwerk in this project
  migrate-config                    Copy a packwerk.yml written for the gem to the crabwerk.yml that crabwerk reads
  create                            Create a new pack
  check                             Look for violations in the codebase
  update                            Update package_todo.yml files with the current violations
  validate                          Look for validation errors in the codebase
  add-dependency                    Add a dependency from one pack to another
  update-dependencies-for-constant  Add missing dependencies for the pack that defines the constant
  check-unnecessary-dependencies    Check for dependencies that when removed produce no violations.
  add-dependencies                  Add everything a pack depends on (may cause cycles)
  lint                              Lint package.yml and package_todo.yml files
  expose-monkey-patches             Expose monkey patches of the Ruby stdlib, gems your app uses, and your application itself
  list-packs                        List packs based on configuration in crabwerk.yml (for debugging purposes)
  list-pack-dependencies            List packs that depend on a pack
  list-included-files               List analyzed files based on configuration in crabwerk.yml (for debugging purposes)
  list-definitions                  List the constants that crabwerk sees and where it sees them (for debugging purposes)
  list-references                   List constant references and their definition files (for test selection)
  for-file                          Print the path to the package.yml that owns a file
  remove-dependency                 Remove a dependency from one pack to another
  move                              Move files to a pack
  help                              Print this message or the help of the given subcommand(s)

Options:
      --project-root <PROJECT_ROOT>     Path for the root of the project [default: .]
      --config <CONFIG>                 Path to the configuration file to read, instead of looking for `crabwerk.yml` in the project root. A relative path is resolved against the project root
  -d, --debug                           Run with performance debug mode
      --color <WHEN>                    When to colour the output. `auto` colours a terminal only, and obeys NO_COLOR [default: auto] [possible values: auto, always, never]
  -e, --experimental-parser             Run with the experimental parser, which gets constant definitions directly from the AST
  -p, --print-files                     Print to console when files begin and finish processing (to identify files that panic when processing files concurrently)
      --disable-enforce-dependencies    Globally disable enforce_dependency
      --disable-enforce-folder-privacy  Globally disable enforce_folder_privacy
      --disable-enforce-layers          Globally disable enforce_layers
      --disable-enforce-privacy         Globally disable enforce_privacy
      --disable-enforce-visibility      Globally disable enforce_visibility
  -h, --help                            Print help
  -V, --version                         Print version
```

# Configuration
`crabwerk` reads `crabwerk.yml` in the project root. It does not read `packwerk.yml`: a project that has one is asked to migrate rather than left to wonder which file took effect.

To move a project over, run:

```
crabwerk migrate-config
```

That copies `packwerk.yml` to `crabwerk.yml` verbatim, comments included, and leaves the original in place — the `packwerk` gem still needs it. Delete `packwerk.yml` when you no longer run the gem. Every key `packwerk` accepts, `crabwerk` accepts, so the copy needs no editing.

While a project runs both tools, point `crabwerk` at the gem's file instead of keeping two copies in step:

```
crabwerk --config packwerk.yml check
```

`--config` takes any path, relative to `--project-root` or absolute, and turns off the search for `crabwerk.yml`.

# Installation
Download the prebuilt binary for your platform from the [latest release](https://github.com/rwojsznis/crabwerk/releases), then put it on your `PATH`.

To build from source instead, clone the repository and run `cargo build --release`. The binary is written to `target/release/crabwerk`. If you don't have Rust yet: https://www.rust-lang.org/tools/install

Building from source also needs a C compiler and `libclang`, because the Ruby parser (`ruby-prism`) compiles bundled C sources and generates its bindings with `bindgen`. On macOS the Xcode Command Line Tools provide both (`xcode-select --install`); on Debian or Ubuntu, install `build-essential` and `libclang-dev`. You do not need a Ruby installation. Prebuilt binaries have no such requirement.

# Using with VSCode/RubyMine Extension
`packwerk` has a VSCode Extension: https://github.com/rubyatscale/packwerk-vscode/tree/main

It also has a RubyMine Extension: https://github.com/vinted/packwerk-intellij

Using the extension with `crabwerk` is straightforward and results in a much more responsive experience.

Directions:
- Follow the [Installation](#installation) instructions above
- Follow the [configuration](https://github.com/rubyatscale/packwerk-vscode/tree/main#configuration) directions to configure the extension to use `crabwerk` instead of the ruby gem by setting the executable to `crabwerk check`

# Verification
As `crabwerk` is still a work-in-progress, it's possible it will not produce the same results as the ruby implementation (see [Not Yet Supported](#not-yet-supported)). If so, please file an issue – I'd love to try to support your use case!

Instructions:
- Follow the directions above to install `crabwerk`
- Run `crabwerk --config packwerk.yml update`, so that both tools read the one configuration file and a difference cannot come from a difference in what they read
- Confirm the output of `git diff` is empty
- Please file an issue if it's not!

# New to Rust?
Me too! This is my first Rust project, so I'd love to have feedback, advice, and contributions!

Rust is a low-level language with high-level abstractions, a rich type system, with a focus on memory safety through innovative compile-time checks on memory usage.

If you're new to Rust, don't be intimidated! [https://www.rust-lang.org](https://www.rust-lang.org/learn) has tons of great learning resources.

If you'd like to contribute but don't know where to start, please reach out! I'd love to help you get started.

# Not yet supported
- custom inflections
- custom load paths
- extensible plugin system

# Behavioral differences
There are still some known behavioral differences between `crabwerk` and `packwerk`. If you find any, please file an issue!
- `package_paths` must not end in a slash, e.g. `packs/*/` is not supported, but `packs/*` is.
- A `**` in `package_paths` is supported, but is not a substitute for a single `*`, e.g. `packs/**` is supported and will match `packs/*/*/package.yml`, but will not match `packs/*/package.yml`. `packs/*` must be used to match that.

## Default Namespaces
`crabwerk` supports Zeitwerk default namespaces.

For example, if you're using [`packs-rails`](https://github.com/rubyatscale/packs-rails) and [`automatic_namespaces`](https://github.com/gap777/automatic_namespaces) to configure your default namespaces, and you have
- `packs/foo/app/models/bar.rb` which is configured to define `Foo::Bar`
- `packs/foo/app/domain/baz.rb` which is configured to define `Foo::Baz`

then `crabwerk` will automatically read the configuration as specified in the `automatic_namespaces` gem and should interpret the namespaces correctly. Please file an issue if you find any problems. There is a known limitation here where acronym-based automatic namespaces are not yet supported (feel free to open an issue if you need this).

If you are not using `automatic_namespaces`, you can also explicitly specify the namespaces in `crabwerk.yml`, like so:
```yml
autoload_roots:
  packs/foo/app/models: "::Foo"
  packs/foo/app/domain: "::Foo"
```

## Enforcement Globs Ignore
`enforcement_globs_ignore` can be used to specify gitignore-style rules for not enforcing violations.

### Examples

```yml
# packs/product_services/serv1/foo/package.yml
enforce_privacy: true
enforce_visibility: true

enforcement_globs_ignore:
- enforcements:
  - privacy
  - visiblity
  ignores:
  - "**/*"
  # Enforce incoming privacy and visibility violation references _only_ in `packs/product_services/serv1/**/*`
  - "!packs/product_services/serv1/**/*"
  reason: "It was decided only to fix incoming violations from serv1. See ticket #232"
```

```yml
# packs/pack2/package.yml
enforce_dependencies: true
dependencies:
# not required because of the below enforcement_globs_ignore
# - packs/pack1 
# required because of the enforcement_globs_ignore exception line 
  - packs/pack3 

enforcement_globs_ignore:
- enforcements:
  - dependency
  ignores:
  - "**/*"
  # Enforce outgoing dependency violation references _only_ to `packs/pack3/**/*`
  - "!packs/pack3/**/*"
  reason: "The other dependency violations are fine as those packs will be absorbed into this one."
```

# Benchmarks
See [BENCHMARKS.md](https://github.com/rwojsznis/crabwerk/blob/main/BENCHMARKS.md)

# Kudos
- Current (@gmcgibbon, @rafaelfranca), and Ex-Shopifolks (@exterm, @wildmaples) for open-sourcing and maintaining `packwerk`
- Gusties, and the [Ruby/Rails Modularity Slack Server](https://join.slack.com/t/rubymod/shared_invite/zt-1dgyrxji9-sihGNX43mVh5T6tw18hFaQ), for continued feedback and support
- @mzruya for the initial implementation and Rust inspiration
