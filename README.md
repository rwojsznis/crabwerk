<img width="400" height="266" alt="crabwerk-logo" src="https://github.com/user-attachments/assets/1d0ed88d-ea82-4a11-b6a7-c007b71d7872" />

# crabwerk
[![GitHub Release](https://img.shields.io/github/v/release/rwojsznis/crabwerk)](https://github.com/rwojsznis/crabwerk/releases/latest)
[![codecov](https://codecov.io/gh/rwojsznis/crabwerk/graph/badge.svg?token=70FFG3ZC0C)](https://codecov.io/gh/rwojsznis/crabwerk)
[![CI](https://github.com/rwojsznis/crabwerk/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/rwojsznis/crabwerk/actions/workflows/ci.yml?query=branch%3Amain)

> [!NOTE]
> tldr: Rust fork of [packs](https://github.com/alexevanczuk/packs), which is a Rust-native port of [packwerk](https://github.com/Shopify/packwerk), with a few minor changes: improved performance, removed cache support, Prism instead of lib-ruby-parser, distributed as a binary. Up to 90x faster than packwerk - for all your git hooks and CI needs.

## Why?

Because _I could_ (thanks LLMs). If `packs` or `packwerk` works for you - there is no _real_ reason to use this fork. General maintenance was done, test coverage was improved, performance was tweaked - mostly around precompiling regexes, some leftovers were removed, a few panics were addressed, dependencies were updated, and binaries for both Linux and macOS are distributed with each release.

| Tool                     |       Mean [s] | Min [s] | Max [s] | vs packwerk |
| :----------------------- | -------------: | ------: | ------: | ----------: |
| packwerk                 | 11.479 ± 1.273 |  10.591 |  13.606 |          1× |
| packs 0.2.40, no cache   |  0.253 ± 0.012 |   0.237 |   0.264 |      45.42× |
| packs 0.2.40, cold cache |  0.517 ± 0.004 |   0.514 |   0.523 |       22.2× |
| packs 0.2.40, warm cache |   0.18 ± 0.004 |   0.174 |   0.184 |      63.66× |
| crabwerk                 |  0.125 ± 0.004 |   0.122 |   0.132 |      91.53× |

(powered by [hyperfine](https://github.com/sharkdp/hyperfine) - ran on a real production project, full check; idle MacBook Pro M1 Max, Ruby 4.0.6, Rust 1.98.0)

## How to migrate
1. Grab a binary from the [releases page](https://github.com/rwojsznis/crabwerk/releases) (nowadays [I recommend using mise](https://mise.jdx.dev/dev-tools/backends/github.html) for a local env)
2. Migrate existing config via `crabwerk migrate-config`
3. Check if it works by calling `crabwerk check`
4. Run `crabwerk update` if you have `package_todo.yml` files to refresh their YAML syntax
5. Remove `packs`/`packwerk` leftovers if you're happy with the results

## Commands

```
A CLI for working with packs (modular code organization) in Ruby codebases.

Usage: crabwerk [OPTIONS] <COMMAND>

Commands:
  all                               Run check and validate
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
