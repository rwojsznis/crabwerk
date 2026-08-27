#!/usr/bin/env bash
#
# Compare three implementations on one application:
#
#   1. packwerk  - the Ruby original, from the app's own bundle (bin/packwerk)
#   2. packs     - the upstream Rust binary, fetched with mise
#   3. crabwerk  - this repo's build
#
# All timed runs are cold. crabwerk cannot cache, so the cache directory of the
# other two is cleared before each run to keep the numbers comparable.
#
# Usage: dev/compare.sh [OPTIONS] <APP_DIR>

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

APP=""
RUNS=5
WARMUP=1
PACKS=""
CRABWERK=""
SINGLE_FILE=""
OUTPUT=""
SKIP_RUBY=0
SKIP_PARITY=0

die() {
  echo "error: $*" >&2
  exit 1
}

note() {
  echo "==> $*" >&2
}

usage() {
  sed -n '2,12p' "${BASH_SOURCE[0]}" | sed 's/^#\s\?//'
  cat <<'EOF'

Options:
  --runs N             Timed runs per tool (default: 5)
  --warmup N           Warmup runs per tool (default: 1)
  --packs PATH         packs binary (default: `mise which packs` in the app)
  --crabwerk PATH      crabwerk binary (default: <repo>/target/release/crabwerk)
  --file PATH          File for the single-file benchmark (default: auto)
  --output PATH        Markdown destination (default: <repo>/tmp/compare-<app>.md)
  --skip-ruby          Leave out bin/packwerk
  --skip-parity        Leave out the package_todo.yml agreement check
  -h, --help           Show this message

packs is measured three ways, because it caches by default and crabwerk cannot:
no cache at all, a cold run that writes the cache, and a run that reads a warm
cache.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --runs) RUNS="${2:?}"; shift 2 ;;
    --warmup) WARMUP="${2:?}"; shift 2 ;;
    --packs) PACKS="${2:?}"; shift 2 ;;
    --crabwerk) CRABWERK="${2:?}"; shift 2 ;;
    --file) SINGLE_FILE="${2:?}"; shift 2 ;;
    --output) OUTPUT="${2:?}"; shift 2 ;;
    --skip-ruby) SKIP_RUBY=1; shift ;;
    --skip-parity) SKIP_PARITY=1; shift ;;
    -h|--help) usage; exit 0 ;;
    -*) die "unknown option: $1" ;;
    *) APP="$1"; shift ;;
  esac
done

[[ -n "$APP" ]] || { usage; exit 1; }
[[ -d "$APP" ]] || die "not a directory: $APP"
APP="$(cd "$APP" && pwd)"
APP_NAME="$(basename "$APP")"
[[ -f "$APP/packwerk.yml" ]] || die "$APP has no packwerk.yml"

command -v hyperfine >/dev/null || die "hyperfine is not installed (brew install hyperfine)"
command -v jq >/dev/null || die "jq is not installed (brew install jq)"
command -v mise >/dev/null || die "mise is not installed"

# --- resolve the three binaries -----------------------------------------------

if [[ -z "$CRABWERK" ]]; then
  CRABWERK="$REPO_ROOT/target/release/crabwerk"
  if [[ ! -x "$CRABWERK" ]]; then
    note "building crabwerk"
    (cd "$REPO_ROOT" && cargo build --release >/dev/null)
  fi
fi
[[ -x "$CRABWERK" ]] || die "crabwerk binary not found: $CRABWERK"

cd "$APP"

# The app pins its own Ruby. Load its mise environment so bin/packwerk resolves
# the same interpreter a developer would get in that directory.
if mise ls --current >/dev/null 2>&1; then
  eval "$(mise env -s bash)"
fi

# The app is expected to pin packs itself. `mise which` is used instead of a
# PATH lookup because an app that vendors the Ruby packs gem also has a
# bin/packs binstub, which is a different program with the same name.
if [[ -z "$PACKS" ]]; then
  PACKS="$(mise which packs 2>/dev/null || true)"
  [[ -n "$PACKS" ]] || die "$APP_NAME does not provide packs. Add it to its mise config, for example:

  [tools]
  \"github:alexevanczuk/packs\" = \"v0.2.40\"

then run \`mise install\` there. Pass --packs PATH to use a binary from elsewhere."
fi
[[ -x "$PACKS" ]] || die "packs binary is not executable: $PACKS"

if [[ $SKIP_RUBY -eq 0 ]]; then
  if [[ ! -x bin/packwerk ]]; then
    note "no bin/packwerk in $APP, skipping the Ruby rows"
    SKIP_RUBY=1
  elif ! bin/packwerk version >/dev/null 2>&1; then
    note "bin/packwerk does not run (bundle install?), skipping the Ruby rows"
    SKIP_RUBY=1
  fi
fi

# Spring keeps a Rails process alive between runs, which would measure the
# preloader rather than the analysis.
RUBY_PREFIX=""
[[ -f bin/spring ]] && RUBY_PREFIX="DISABLE_SPRING=1 "

CRABWERK_VERSION="$("$CRABWERK" --version | awk '{print $2}')"
PACKS_BIN_VERSION="$("$PACKS" --version | awk '{print $2}')"
RUBY_VERSION_STR=""
if [[ $SKIP_RUBY -eq 0 ]]; then
  RUBY_VERSION_STR="$(bin/packwerk version 2>/dev/null | tr -d '\n' | sed 's/[^0-9.]*//' | awk '{print $1}')"
  [[ -n "$RUBY_VERSION_STR" ]] || RUBY_VERSION_STR="unknown"
fi

WORK="$(mktemp -d)"
# The cache directory is this script's doing unless the app already had one.
CACHE_DIR="$APP/tmp/cache/packwerk"
CACHE_PREEXISTING=0
[[ -d "$CACHE_DIR" ]] && CACHE_PREEXISTING=1
cleanup() {
  rm -rf "$WORK" "${PARITY_DIR:-}"
  [[ $CACHE_PREEXISTING -eq 0 ]] && rm -rf "$CACHE_DIR"
  return 0
}
trap cleanup EXIT

# --- what each tool sees ------------------------------------------------------

note "collecting file counts and violation counts"

CRABWERK_FILES="$("$CRABWERK" list-included-files | wc -l | tr -d ' ')"
PACKS_FILES="$("$PACKS" --no-cache list-included-files | wc -l | tr -d ' ')"

# `check` exits non-zero when it finds violations, which is not a script error.
run_check() {
  local out="$1"; shift
  set +e
  "$@" >"$out" 2>&1
  set -e
}

rust_violations() {
  local n
  n="$(grep -oE '[0-9]+ violation\(s\) found' "$1" | head -1 | awk '{print $1}' || true)"
  echo "${n:-0}"
}

rm -rf tmp/cache/packwerk
run_check "$WORK/crabwerk.out" "$CRABWERK" check
CRABWERK_VIOLATIONS="$(rust_violations "$WORK/crabwerk.out")"

rm -rf tmp/cache/packwerk
run_check "$WORK/packs.out" "$PACKS" --no-cache check
PACKS_VIOLATIONS="$(rust_violations "$WORK/packs.out")"

RUBY_FILES="n/a"
RUBY_VIOLATIONS="n/a"
if [[ $SKIP_RUBY -eq 0 ]]; then
  rm -rf tmp/cache/packwerk
  run_check "$WORK/packwerk.out" env DISABLE_SPRING=1 bin/packwerk check
  RUBY_FILES="$(grep -oE 'inspecting [0-9]+ file' "$WORK/packwerk.out" | head -1 | awk '{print $2}' || true)"
  [[ -n "$RUBY_FILES" ]] || RUBY_FILES="?"
  RUBY_VIOLATIONS="$(grep -oE '[0-9]+ offenses? detected' "$WORK/packwerk.out" | head -1 | awk '{print $1}' || true)"
  [[ -n "$RUBY_VIOLATIONS" ]] || RUBY_VIOLATIONS=0
fi

# --- agreement on package_todo.yml -------------------------------------------
#
# The strongest cross-tool check available: all three write the same file
# format, so running `update` on identical copies and diffing the result shows
# whether they resolve constants the same way.

PARITY_DIR=""
PARITY_CRABWERK="baseline"
PARITY_PACKS="skipped"
PARITY_RUBY="skipped"

clone_app() {
  local dest="$1"
  cp -Rc "$APP" "$dest" 2>/dev/null || cp -R "$APP" "$dest"
}

dump_todos() {
  local dir="$1" out="$2" f
  : >"$out"
  while IFS= read -r f; do
    echo "### $f" >>"$out"
    cat "$dir/$f" >>"$out"
  done < <(cd "$dir" && find . -name package_todo.yml -not -path './.git/*' | sort)
}

if [[ $SKIP_PARITY -eq 0 ]]; then
  note "checking package_todo.yml agreement (this copies the app)"
  # Kept beside the app so APFS can clone the tree instead of copying it.
  PARITY_DIR="$(dirname "$APP")/.crabwerk-compare.$$"
  mkdir -p "$PARITY_DIR"

  clone_app "$PARITY_DIR/crabwerk"
  (cd "$PARITY_DIR/crabwerk" && "$CRABWERK" update >/dev/null 2>&1) || true
  dump_todos "$PARITY_DIR/crabwerk" "$WORK/todo.crabwerk"

  clone_app "$PARITY_DIR/packs"
  (cd "$PARITY_DIR/packs" && "$PACKS" --no-cache update >/dev/null 2>&1) || true
  dump_todos "$PARITY_DIR/packs" "$WORK/todo.packs"
  if diff -q "$WORK/todo.crabwerk" "$WORK/todo.packs" >/dev/null; then
    PARITY_PACKS="identical"
  else
    PARITY_PACKS="**differs**"
    diff -u "$WORK/todo.crabwerk" "$WORK/todo.packs" >"$WORK/diff.packs" || true
  fi

  if [[ $SKIP_RUBY -eq 0 ]]; then
    clone_app "$PARITY_DIR/packwerk"
    (cd "$PARITY_DIR/packwerk" && DISABLE_SPRING=1 bin/packwerk update >/dev/null 2>&1) || true
    dump_todos "$PARITY_DIR/packwerk" "$WORK/todo.packwerk"
    if diff -q "$WORK/todo.crabwerk" "$WORK/todo.packwerk" >/dev/null; then
      PARITY_RUBY="identical"
    else
      PARITY_RUBY="**differs**"
      diff -u "$WORK/todo.crabwerk" "$WORK/todo.packwerk" >"$WORK/diff.packwerk" || true
    fi
  fi
fi

# --- benchmarks ---------------------------------------------------------------

if [[ -z "$SINGLE_FILE" ]]; then
  if [[ -f config/initializers/inflections.rb ]]; then
    SINGLE_FILE="config/initializers/inflections.rb"
  else
    SINGLE_FILE="$("$CRABWERK" list-included-files | head -1)"
  fi
fi

# hyperfine reports a name as `command` when one is given with -n. Speedups are
# quoted against the Ruby original, which is the number that matters; without a
# Ruby row there is nothing to improve on, so the fastest row becomes the base.
render_table() {
  jq -r '
    def f: (. * 1000 | round) / 1000;
    def s: (. * 100 | round) / 100;
    ([.results[] | select(.command | test("^packwerk"))] | .[0].mean) as $ruby
    | ([.results[].mean] | min) as $min
    | ($ruby // $min) as $base
    | (if $ruby then "vs packwerk" else "vs fastest" end) as $label
    | "| Tool | Mean [s] | Min [s] | Max [s] | \($label) |",
      "|:---|---:|---:|---:|---:|",
      (.results[]
        | "| \(.command) | \(.mean|f) ± \((.stddev // 0)|f) | \(.min|f) | \(.max|f) | \(($base / .mean)|s)× |")
  ' "$1"
}

# Each command carries its own --prepare, because the rows differ precisely in
# what they leave in the cache directory. hyperfine matches --prepare options to
# commands in order.
CLEAR_CACHE='rm -rf tmp/cache/packwerk'
KEEP_CACHE='true'

# The warm row depends on a preceding run having filled the cache.
if [[ $WARMUP -lt 1 ]]; then
  note "raising warmup to 1 so the warm-cache row is actually warm"
  WARMUP=1
fi

bench() {
  local json="$1" suffix="${2:-}"
  local args=(--warmup "$WARMUP" --runs "$RUNS" --ignore-failure --export-json "$json")

  if [[ $SKIP_RUBY -eq 0 ]]; then
    args+=(--prepare "$CLEAR_CACHE" -n "packwerk $RUBY_VERSION_STR (Ruby)" \
      "${RUBY_PREFIX}bin/packwerk check$suffix")
  fi
  args+=(--prepare "$CLEAR_CACHE" -n "packs $PACKS_BIN_VERSION, no cache" \
    "$PACKS --no-cache check$suffix")
  args+=(--prepare "$CLEAR_CACHE" -n "packs $PACKS_BIN_VERSION, cold cache" \
    "$PACKS check$suffix")
  args+=(--prepare "$KEEP_CACHE" -n "packs $PACKS_BIN_VERSION, warm cache" \
    "$PACKS check$suffix")
  args+=(--prepare "$KEEP_CACHE" -n "crabwerk $CRABWERK_VERSION" \
    "$CRABWERK check$suffix")

  hyperfine "${args[@]}" >&2
}

note "benchmarking the full check"
bench "$WORK/full.json"

note "benchmarking a single file: $SINGLE_FILE"
bench "$WORK/one.json" " $SINGLE_FILE"

# --- report -------------------------------------------------------------------

[[ -n "$OUTPUT" ]] || OUTPUT="$REPO_ROOT/tmp/compare-$APP_NAME.md"
mkdir -p "$(dirname "$OUTPUT")"

APP_REV="$(git -C "$APP" rev-parse --short HEAD 2>/dev/null || echo "not a git repo")"
RUBY_ACTUAL="$(ruby -e 'print RUBY_VERSION' 2>/dev/null || echo "?")"
CACHE_NOTE="off"
grep -qE '^cache:[[:space:]]*true' packwerk.yml && CACHE_NOTE="on (tmp/cache/packwerk)" || true

{
  echo "# packwerk vs packs vs crabwerk — $APP_NAME"
  echo
  echo "- App: \`$APP\` @ \`$APP_REV\`"
  echo "- Host: $(uname -sm), $(sysctl -n hw.ncpu 2>/dev/null || nproc) cores"
  echo "- Ruby: $RUBY_ACTUAL, packwerk cache in packwerk.yml: $CACHE_NOTE"
  echo "- packs: \`$PACKS\`"
  echo "- Runs: $RUNS (warmup $WARMUP)"
  echo
  echo "Cache handling per row: every row except \"warm cache\" deletes"
  echo "\`tmp/cache/packwerk\` before each run. crabwerk has no cache at all, so its"
  echo "row is always a cold run."
  echo
  echo "## Agreement"
  echo
  echo "| Tool | Version | Files inspected | Violations | package_todo.yml |"
  echo "|:---|---:|---:|---:|:---|"
  if [[ $SKIP_RUBY -eq 0 ]]; then
    echo "| packwerk (Ruby) | $RUBY_VERSION_STR | $RUBY_FILES | $RUBY_VIOLATIONS | $PARITY_RUBY |"
  fi
  echo "| packs (upstream) | $PACKS_BIN_VERSION | $PACKS_FILES | $PACKS_VIOLATIONS | $PARITY_PACKS |"
  echo "| crabwerk | $CRABWERK_VERSION | $CRABWERK_FILES | $CRABWERK_VIOLATIONS | $PARITY_CRABWERK |"
  echo
  echo "## Full check"
  echo
  render_table "$WORK/full.json"
  echo
  echo "## Single file: \`$SINGLE_FILE\`"
  echo
  render_table "$WORK/one.json"
  for d in packs packwerk; do
    if [[ -s "$WORK/diff.$d" ]]; then
      echo
      echo "## package_todo.yml diff vs $d"
      echo
      echo '```diff'
      head -60 "$WORK/diff.$d"
      echo '```'
    fi
  done
} >"$OUTPUT"

cat "$OUTPUT"
note "written to $OUTPUT"
