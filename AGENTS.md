# CLAUDE.md

You must use ASD-STE100 Simplified Technical English (STE) when it doesn't detract from meaning.

## What this is

`crabwerk` is a Rust CLI that finds modularization violations in Ruby
codebases. It is a fork of [`alexevanczuk/packs`](https://github.com/alexevanczuk/packs),
reworked and rebranded. Single crate, one binary (`crabwerk`) plus a library
target that has no public API — [`src/lib.rs`](src/lib.rs) says so at the top,
and the CLI is the contract.

**[`packwerk`](https://github.com/Shopify/packwerk) is the reference
implementation.** The goal is a drop-in replacement that runs 10-20x faster.
`crabwerk` reads the same `packwerk.yml`, the same `package.yml` files and the
same `package_todo.yml` files, and it must produce the same violations. A
difference from the gem is a bug unless there is a written reason for it.

The gem is not vendored here. To compare the two over a real application, use
[`dev/compare.sh`](dev/compare.sh) — it times and diffs `packwerk` (the app's
own bundle), `packs` (the upstream Rust binary) and this repo's build.

Two documents give more context:

- [`README.md`](README.md) — goals, installation, the full CLI help output.
- [`EXPERIMENTAL_PARSER_USAGE.md`](EXPERIMENTAL_PARSER_USAGE.md) — how the
  experimental parser differs from the default one, and why it exists.

## The domain in one paragraph

A *pack* is a directory with a `package.yml` at its root. Each pack declares
what it depends on (`dependencies:`) and how strongly it enforces each rule
(`enforce_dependencies`, `enforce_privacy`, `enforce_visibility`,
`enforce_layers`, `enforce_folder_privacy` — each `false`, `true` or `strict`).
`crabwerk check` parses every Ruby and ERB file, resolves each constant
reference to the file that defines it, maps that file to its owning pack, and
reports a *violation* when the reference breaks a rule. `crabwerk update`
records the current violations in `package_todo.yml` so a codebase can adopt
the rules gradually; a recorded violation is suppressed on later runs.

Five violation types exist, in [`src/checker/pack_checker.rs`](src/checker/pack_checker.rs):
`dependency`, `privacy`, `folder_privacy`, `layer`, `visibility`.

## Layout

| Path | What |
|---|---|
| `src/main.rs` | the entry point; hands off to `cli::run` |
| `src/cli.rs` | clap only: the flags, the subcommands, the dispatch to `lib.rs` |
| `src/lib.rs` | the command bodies — `check`, `update`, `validate`, `lint`, `create`, `move` and the rest |
| `src/raw_configuration.rs` | `packwerk.yml`/`crabwerk.yml` as deserialized, before any resolution |
| `src/configuration.rs` | the resolved config: the pack set, the file list, the resolver choice |
| `src/pack.rs` | one `package.yml`: dependencies, the enforcement settings, the owned files |
| `src/pack_set.rs` | every pack, indexed by name and by owned file |
| `src/package_todo.rs` | read, write and diff `package_todo.yml`; the update statistics |
| `src/walk_directory.rs` | the one directory walk (jwalk); produces the included files and packs |
| `src/ignored.rs` | the glob rules, where a `!` prefix allow-lists over the deny-list |
| `src/file_utils.rs` | glob sets and the extension-to-parser dispatch |
| `src/parsing/ruby/packwerk/` | the default Ruby parser: references from the AST, definitions from file names |
| `src/parsing/ruby/zeitwerk/` | the default constant resolver: Zeitwerk's file-name-to-constant rule |
| `src/parsing/ruby/experimental/` | the `-e` parser and resolver: definitions read from the AST |
| `src/parsing/ruby/inflector_shim.rs` | camelize/underscore, to match Rails inflections |
| `src/parsing/ruby/rails_utils.rs` | `has_many :companies` → a reference to `Company` |
| `src/parsing/erb/` | ERB tags → Ruby, then the same two Ruby parsers |
| `src/constant_resolver.rs` | the `ConstantResolver` trait and `ConstantDefinition` |
| `src/reference_extractor.rs` | parse every file, resolve every reference; the input to the checkers |
| `src/checker.rs` | `check_all`, `validate_all`, `update`; the `Violation` type and the two traits |
| `src/checker/*.rs` | one module per violation type, plus the shared `pack_checker` gate |
| `src/dependencies.rs` | explicit and implicit dependencies, for `list-pack-dependencies` |
| `src/constant_dependencies.rs` | `update-dependencies-for-constant` |
| `src/monkey_patch_detection.rs` | `expose-monkey-patches`: definitions in the stdlib, in gems and in the app |
| `src/logger.rs` | tracing setup; `--debug` turns on the performance spans |
| `tests/` | integration tests, one file per command; `fixtures/` holds small Ruby apps |
| `dev/compare.sh` | the three-way parity and benchmark run against a real application |

Unit tests live in `#[cfg(test)] mod tests` next to the code. Command-level
tests live in `tests/`.

## Workflow — do it in this order

1. **Write the failing test first.** Before any implementation, add a test that
   fails for the reason you expect. Run it and *read the failure* — a test that
   passes immediately, or fails with the wrong message, is not yet a test of
   the thing you meant.
2. **Then write the implementation**, and only enough of it to make that test
   pass.
3. **Check your work.** Run the full suite, not just the new test. If the
   change touches parsing, resolution or the checkers, also run the binary
   against a real application and look at the output; `cargo test` passing is
   necessary, not sufficient.
4. **Run the linters last**, and get them clean before you call the work done.

```bash
cargo test --quiet                                # full suite; show details only on failure
cargo clippy --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items
cargo fmt
```

CI ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) runs those on Linux
and macOS, and adds `cargo machete` (unused dependencies), `cargo audit`, the
doc tests and a coverage run. `RUSTFLAGS: -D warnings` is set for the whole
workflow.

Keep successful test output brief. We care about failures: if the quiet test
run fails, inspect and report the complete failure output.

Do not report a change as finished until every one of them is green. If
something is left failing or unfinished, say so explicitly rather than
narrowing the scope quietly.

## Where tests go

- A parsing rule, a glob rule, a config default → a unit test in that module.
- A CLI flag, an output format, an end-to-end report → `tests/`, in the file
  named for the command.
- A new behaviour that needs an application to reproduce → a new fixture under
  `tests/fixtures/`. Keep it as small as the behaviour allows; the existing
  fixture names say what they hold (`app_with_missing_dependency`,
  `contains_stale_violations`, `uses_strict_mode`).
- Tests that write to a shared fixture must be `#[serial]` — several are, and
  `tests/common/mod.rs` holds the setup and teardown helpers they use.

## Conventions

- Write in-code comments that describe why code or a class does what it does,
  but not what it does. The "what" should be self-evident. Be concise and
  direct.
- **Do not commit any changes yourself.**
- **`max_width` is 80** ([`.rustfmt.toml`](.rustfmt.toml)). The toolchain is
  pinned in [`rust-toolchain.toml`](rust-toolchain.toml); edition 2024.
- **Writing is opt-in.** `check`, `validate`, `lint` and every `list-*` command
  are read-only. `update`, `create`, `move`, `add-dependency`,
  `remove-dependency` and `check-unnecessary-dependencies --auto-correct`
  write. Do not add a command that writes when it is not asked to.
- **Parallelism must not change output.** The walk and the per-file parse run
  under rayon. Results are sorted before they are printed. A change that
  reorders the output is a bug even if the set is the same.
- The manifest version stays at `0.0.0`. Release binaries take their version
  from the Git tag, through `CRABWERK_VERSION` at build time. Do not bump
  `Cargo.toml`.
- Every dependency in `Cargo.toml` carries a comment that says what it is for.
  Keep that habit — `cargo machete` fails CI on an unused one.

## Gotchas

- **`packwerk.yml` wins over `crabwerk.yml`.** When both exist, only
  `packwerk.yml` is read. `crabwerk.yml` alone turns on *crabwerk-first mode*
  (`RawConfiguration::crabwerk_first_mode`), which changes the generated
  messages and the `bin/packwerk` references in them.
- **The default parser infers definitions from file names, not from code.**
  `app/models/foo.rb` defines `::Foo` whether or not it does. This is Zeitwerk
  parity, and it is why `-e`/`experimental_parser: true` exists. The two
  parsers give different answers by design; do not "fix" one to match the
  other.
- **Only Ruby and ERB are parsed.** `get_file_type` in `src/file_utils.rs`
  accepts `.rb`, `.rake`, `.builder`, `.gemspec`, `.ru`, `Gemfile`, `Rakefile`
  and `.erb`. Everything else is skipped, silently.
- `enforce_folder_visibility` is deprecated in favour of
  `enforce_folder_privacy`. Both names are still accepted, and
  `Pack::enforce_folder_privacy()` resolves the fallback — do not read the raw
  field. `enforce_architecture` is *not* the same case: the layer checker reads
  `enforce_layers` only, and `enforce_architecture` survives a `package.yml`
  rewrite as an unknown key without doing anything.
- An enforcement setting is three-valued (`false`, `true`, `strict`), not a
  bool. `strict` refuses recorded violations in `package_todo.yml` as well as
  new ones.
- The `--disable-enforce-*` flags override every pack. They exist to answer
  "what would this rule cost?", so they must not touch anything on disk.
- Exit code 1 covers both "the command found something" and "the tool
  failed" — there is no third code. What separates them is the `Error:` line:
  a found violation under `--json` calls `exit(1)` after clean JSON, while
  every other failure path goes through `bail!`.
- `init` runs before the config is loaded, because there is no config yet. It
  is special-cased early in `cli::run` and then matched a second time to print
  its message. Both places need the change if you touch it.
