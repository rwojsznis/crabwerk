#!/usr/bin/env bash
#
# Check that three implementations agree on one application:
#
#   1. packwerk  - the Ruby original, from the app's own bundle (bin/packwerk)
#   2. packs     - the upstream Rust binary, fetched with mise
#   3. crabwerk  - this repo's build
#
# Each tool runs `check` on the app, and `update` on its own copy of it. The
# copies' package_todo.yml files are then diffed against crabwerk's. Exits 1
# when anything differs. For timings, use dev/bench.sh.
#
# Usage: dev/compare.sh [OPTIONS] <APP_DIR>

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

APP=""
PACKS=""
CRABWERK=""
OUTPUT=""
SKIP_RUBY=0

die() {
  echo "error: $*" >&2
  exit 2
}

note() {
  echo "==> $*" >&2
}

usage() {
  sed -n '2,13p' "${BASH_SOURCE[0]}" | sed 's/^#\s\?//'
  cat <<'EOF'

Options:
  --packs PATH         packs binary (default: `mise which packs` in the app)
  --crabwerk PATH      crabwerk binary (default: a fresh release build)
  --output PATH        Markdown destination (default: <repo>/tmp/compare-<app>.md)
  --skip-ruby          Leave out bin/packwerk
  -h, --help           Show this message
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --packs) PACKS="${2:?}"; shift 2 ;;
    --crabwerk) CRABWERK="${2:?}"; shift 2 ;;
    --output) OUTPUT="${2:?}"; shift 2 ;;
    --skip-ruby) SKIP_RUBY=1; shift ;;
    -h|--help) usage; exit 0 ;;
    -*) die "unknown option: $1" ;;
    *) APP="$1"; shift ;;
  esac
done

[[ -n "$APP" ]] || { usage; exit 2; }
[[ -d "$APP" ]] || die "not a directory: $APP"
APP="$(cd "$APP" && pwd)"
APP_NAME="$(basename "$APP")"
[[ -f "$APP/packwerk.yml" ]] || die "$APP has no packwerk.yml"

command -v mise >/dev/null || die "mise is not installed"

# --- resolve the three binaries -----------------------------------------------

# Always rebuilt: a parity result from a stale binary describes old code.
if [[ -z "$CRABWERK" ]]; then
  note "building crabwerk"
  (cd "$REPO_ROOT" && cargo build --release --quiet)
  CRABWERK="$REPO_ROOT/target/release/crabwerk"
fi
[[ -x "$CRABWERK" ]] || die "crabwerk binary not found: $CRABWERK"

# crabwerk reads `crabwerk.yml`, not `packwerk.yml`. Naming the gem's own file
# keeps all three tools on one configuration, so a difference in the output
# cannot be blamed on a difference in what they read. The path is relative, so
# it also resolves inside the app copies.
CRABWERK_CONFIG=(--config packwerk.yml)

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

# Spring keeps a Rails process alive between runs, which can serve a check from
# a stale preloaded app.
export DISABLE_SPRING=1

if [[ $SKIP_RUBY -eq 0 ]]; then
  if [[ ! -x bin/packwerk ]]; then
    note "no bin/packwerk in $APP, skipping packwerk"
    SKIP_RUBY=1
  elif ! bin/packwerk version >/dev/null 2>&1; then
    note "bin/packwerk does not run (bundle install?), skipping packwerk"
    SKIP_RUBY=1
  fi
fi

CRABWERK_VERSION="$("$CRABWERK" --version | awk '{print $2}')" || die "$CRABWERK --version failed"
PACKS_VERSION="$("$PACKS" --version | awk '{print $2}')" || die "$PACKS --version failed"
PACKWERK_VERSION="n/a"
[[ $SKIP_RUBY -eq 1 ]] || PACKWERK_VERSION="$(bin/packwerk version 2>/dev/null | tr -d '\n')"

WORK="$(mktemp -d)"
PARITY_DIR=""
# The cache directory is this script's doing unless the app already had one.
CACHE_DIR="$APP/tmp/cache/packwerk"
CACHE_PREEXISTING=0
[[ -d "$CACHE_DIR" ]] && CACHE_PREEXISTING=1
# shellcheck disable=SC2329 # invoked by the trap
cleanup() {
  rm -rf "$WORK" "$PARITY_DIR"
  [[ $CACHE_PREEXISTING -eq 0 ]] && rm -rf "$CACHE_DIR"
  return 0
}
trap cleanup EXIT

# --- what each tool sees ------------------------------------------------------

note "collecting file counts and violation counts"

# `check` exits 1 for violations and for a crash alike. The summary line tells
# them apart: a run without one did not finish, and its count would be a lie.
check_count() {
  local out="$1" n
  if grep -qE '^No (violations|offenses) detected' "$out"; then
    echo 0
  elif n="$(grep -oE '[0-9]+ (violation\(s\) found|offenses? detected)' "$out" | head -1)"; then
    echo "${n%% *}"
  else
    echo "$2 did not complete a check:" >&2
    tail -20 "$out" >&2
    exit 2
  fi
}

rm -rf "$CACHE_DIR"
"$CRABWERK" "${CRABWERK_CONFIG[@]}" check >"$WORK/check.crabwerk" 2>&1 || true
CRABWERK_VIOLATIONS="$(check_count "$WORK/check.crabwerk" crabwerk)"
CRABWERK_FILES="$("$CRABWERK" "${CRABWERK_CONFIG[@]}" list-included-files | wc -l | tr -d ' ')"

"$PACKS" --no-cache check >"$WORK/check.packs" 2>&1 || true
PACKS_VIOLATIONS="$(check_count "$WORK/check.packs" packs)"
PACKS_FILES="$("$PACKS" --no-cache list-included-files | wc -l | tr -d ' ')"

PACKWERK_FILES="n/a"
PACKWERK_VIOLATIONS="n/a"
if [[ $SKIP_RUBY -eq 0 ]]; then
  rm -rf "$CACHE_DIR"
  bin/packwerk check >"$WORK/check.packwerk" 2>&1 || true
  PACKWERK_VIOLATIONS="$(check_count "$WORK/check.packwerk" packwerk)"
  PACKWERK_FILES="$(grep -oE 'inspecting [0-9]+ file' "$WORK/check.packwerk" | head -1 | awk '{print $2}' || true)"
  [[ -n "$PACKWERK_FILES" ]] || PACKWERK_FILES="?"
fi

# --- agreement on package_todo.yml -------------------------------------------
#
# The strongest cross-tool check available: all three write the same file
# format, so running `update` on identical copies and diffing the result shows
# whether they resolve constants the same way.

clone_app() {
  local dest="$1"
  cp -Rc "$APP" "$dest" 2>/dev/null || cp -R "$APP" "$dest"
  # The copy must not hand packwerk the cache of an earlier run in the app.
  rm -rf "$dest/tmp/cache/packwerk"
}

dump_todos() {
  local dir="$1" out="$2" f
  : >"$out"
  while IFS= read -r f; do
    echo "### $f" >>"$out"
    cat "$dir/$f" >>"$out"
  done < <(cd "$dir" && find . -name package_todo.yml -not -path './.git/*' -not -path './node_modules/*' | sort)
}

# A failed update leaves the copy's files as they were, which can look like
# agreement; it must stop the run instead.
update_copy() {
  local name="$1"; shift
  note "running update with $name"
  clone_app "$PARITY_DIR/$name"
  if ! (cd "$PARITY_DIR/$name" && "$@") >"$WORK/update.$name" 2>&1; then
    echo "$name update failed:" >&2
    tail -20 "$WORK/update.$name" >&2
    exit 2
  fi
  dump_todos "$PARITY_DIR/$name" "$WORK/todo.$name"
}

# Kept beside the app so APFS can clone the tree instead of copying it.
PARITY_DIR="$(dirname "$APP")/.crabwerk-compare.$$"
mkdir -p "$PARITY_DIR"

[[ -n "$OUTPUT" ]] || OUTPUT="$REPO_ROOT/tmp/compare-$APP_NAME.md"
mkdir -p "$(dirname "$OUTPUT")"
DIFF_BASE="${OUTPUT%.md}"

DIFFERS=0
PARITY_PACKS="skipped"
PARITY_PACKWERK="skipped"

# Full diffs are kept beside the report, where the report only quotes the start.
# Called in a command substitution, so it reports through its output only.
compare_todos() {
  local name="$1" diff_file="$DIFF_BASE.$1.diff"
  rm -f "$diff_file"
  if diff -u "$WORK/todo.crabwerk" "$WORK/todo.$name" >"$WORK/diff.$name"; then
    echo "identical"
  else
    cp "$WORK/diff.$name" "$diff_file"
    echo "**differs**"
  fi
}

update_copy crabwerk "$CRABWERK" "${CRABWERK_CONFIG[@]}" update
update_copy packs "$PACKS" --no-cache update
PARITY_PACKS="$(compare_todos packs)"
[[ $PARITY_PACKS == identical ]] || DIFFERS=1

if [[ $SKIP_RUBY -eq 0 ]]; then
  update_copy packwerk bin/packwerk update
  PARITY_PACKWERK="$(compare_todos packwerk)"
  [[ $PARITY_PACKWERK == identical ]] || DIFFERS=1
fi

if [[ $SKIP_RUBY -eq 0 && "$PACKWERK_VIOLATIONS" != "$CRABWERK_VIOLATIONS" ]] ||
  [[ "$PACKS_VIOLATIONS" != "$CRABWERK_VIOLATIONS" ]]; then
  DIFFERS=1
fi

# --- report -------------------------------------------------------------------

APP_REV="$(git -C "$APP" rev-parse --short HEAD 2>/dev/null || echo "not a git repo")"
TODO_COUNT="$(grep -c '^### ' "$WORK/todo.crabwerk" || true)"

{
  echo "# packwerk vs packs vs crabwerk — $APP_NAME"
  echo
  echo "- App: \`$APP\` @ \`$APP_REV\`"
  echo "- Ruby: $(ruby -e 'print RUBY_VERSION' 2>/dev/null || echo "?")"
  echo "- packs: \`$PACKS\`"
  echo "- package_todo.yml files after crabwerk's update: $TODO_COUNT"
  echo
  echo "| Tool | Version | Files inspected | Violations | package_todo.yml |"
  echo "|:---|---:|---:|---:|:---|"
  if [[ $SKIP_RUBY -eq 0 ]]; then
    echo "| packwerk (Ruby) | $PACKWERK_VERSION | $PACKWERK_FILES | $PACKWERK_VIOLATIONS | $PARITY_PACKWERK |"
  fi
  echo "| packs (upstream) | $PACKS_VERSION | $PACKS_FILES | $PACKS_VIOLATIONS | $PARITY_PACKS |"
  echo "| crabwerk | $CRABWERK_VERSION | $CRABWERK_FILES | $CRABWERK_VIOLATIONS | baseline |"
  for d in packs packwerk; do
    if [[ -s "$WORK/diff.$d" ]]; then
      echo
      echo "## package_todo.yml diff vs $d"
      echo
      echo "First 60 lines; the full diff is \`$DIFF_BASE.$d.diff\`."
      echo
      echo '```diff'
      head -60 "$WORK/diff.$d"
      echo '```'
    fi
  done
} >"$OUTPUT"

cat "$OUTPUT"
note "written to $OUTPUT"
exit "$DIFFERS"
