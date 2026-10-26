#!/usr/bin/env bash
# Benchmarks a full `check` on an application with packwerk, packs and this
# checkout's release build, and rewrites the README table between the bench
# markers.
#
# Usage: dev/bench.sh <app> [hyperfine options...]
#
#   PACKWERK_CMD  how to run the gem in the app (default: bin/packwerk)
#   PACKS_BIN     the packs binary (default: `mise which packs` in the app)
#   CRABWERK_BIN  the binary to measure (default: a fresh release build)
#
# Hyperfine options replace the default `--warmup 1 --runs 10`, e.g. `--runs 3`
# for a quick pass. Needs hyperfine 2 (for `--ignore-failure=<codes>`) and jq.
# For the agreement check between the three tools, use dev/compare.sh.
set -euo pipefail
export LC_ALL=en_US.UTF-8 # ${#s} must count "±" and "×" as one column

repo=$(cd "$(dirname "$0")/.." && pwd)
readme=$repo/README.md
app=$(cd "${1:?usage: dev/bench.sh <app> [hyperfine options...]}" && pwd)
shift

opts=("$@")
[[ $# -eq 0 ]] && opts=(--warmup 1 --runs 10)

if ! grep -q '^<!-- bench:start -->$' "$readme" || ! grep -q '^<!-- bench:end -->$' "$readme"; then
  echo "README.md has no bench:start/bench:end markers" >&2
  exit 2
fi
if ! grep -q '^(powered by .*; idle .*)$' "$readme"; then
  echo "README.md has no \"(powered by ...; idle ...)\" footnote" >&2
  exit 2
fi
[[ -f $app/packwerk.yml ]] || { echo "$app has no packwerk.yml" >&2; exit 2; }

if [[ -z ${CRABWERK_BIN:-} ]]; then
  (cd "$repo" && cargo build --release --quiet)
  CRABWERK_BIN=$repo/target/release/crabwerk
fi

cd "$app"

# The app pins its own Ruby and packs. Its mise environment gives bin/packwerk
# the interpreter a developer would get in that directory.
if command -v mise >/dev/null && mise ls --current >/dev/null 2>&1; then
  eval "$(mise env -s bash)"
fi

packwerk_cmd=${PACKWERK_CMD:-bin/packwerk}
# Spring keeps a Rails process alive between runs, which would measure the
# preloader rather than the analysis.
export DISABLE_SPRING=1

# `mise which` and not a PATH lookup: an app that vendors the Ruby packs gem
# has a bin/packs binstub, which is a different program with the same name.
PACKS_BIN=${PACKS_BIN:-$(mise which packs 2>/dev/null || true)}
if [[ ! -x $PACKS_BIN ]]; then
  echo "no packs binary; add \"github:alexevanczuk/packs\" to the app's mise config or set PACKS_BIN" >&2
  exit 2
fi
packs_version=$("$PACKS_BIN" --version | awk '{ print $2 }') ||
  { echo "$PACKS_BIN --version failed" >&2; exit 2; }

# crabwerk reads only crabwerk.yml; naming the gem's file keeps all three tools
# on one configuration.
crabwerk_cmd="$CRABWERK_BIN --config packwerk.yml"

tmp=$(mktemp -d)
cache=$app/tmp/cache/packwerk
cache_preexisting=0
[[ -d $cache ]] && cache_preexisting=1
cleanup() {
  rm -rf "$tmp"
  [[ $cache_preexisting -eq 0 ]] && rm -rf "$cache"
  return 0
}
trap cleanup EXIT

# `check` exits 1 for violations and for a crash alike, so hyperfine cannot
# tell them apart. One run each up front makes sure every number in the table
# is a real check, not a tool that failed fast.
sanity() {
  local name=$1 cmd=$2 out
  rm -rf "$cache"
  out=$(sh -c "$cmd check" 2>&1) || true
  if ! grep -qE 'violation\(s\) found|offenses? detected|No (violations|offenses) detected' <<<"$out"; then
    echo "$name did not complete a check:" >&2
    echo "$out" | tail -20 >&2
    exit 2
  fi
}
sanity packwerk "$packwerk_cmd"
sanity packs "$PACKS_BIN --no-cache"
sanity crabwerk "$crabwerk_cmd"

# Each command has its own --prepare, matched to the commands in order: every
# row but the warm one starts without a cache. The warm row needs no warmup of
# its own, because the cold row before it leaves the cache filled.
clear='rm -rf tmp/cache/packwerk'
hyperfine "${opts[@]}" --ignore-failure=1 --export-json "$tmp/check.json" \
  --prepare "$clear" -n packwerk "$packwerk_cmd check" \
  --prepare "$clear" -n "packs $packs_version, no cache" "$PACKS_BIN --no-cache check" \
  --prepare "$clear" -n "packs $packs_version, cold cache" "$PACKS_BIN check" \
  --prepare true -n "packs $packs_version, warm cache" "$PACKS_BIN check" \
  --prepare true -n crabwerk "$crabwerk_cmd check"

rows=()
while IFS=$'\t' read -r name mean stddev min max; do
  [[ -z ${base:-} ]] && base=$mean
  rows+=("$(awk -v n="$name" -v m="$mean" -v s="$stddev" -v lo="$min" -v hi="$max" -v b="$base" \
    'BEGIN { printf "%s|%.3f ± %.3f|%.3f|%.3f|%.1f×", n, m, s, lo, hi, b / m }')")
done < <(jq -r '.results[] | .summary.time_wall_clock as $t
  | [.name, $t.mean, ($t.stddev // 0), $t.min, $t.max] | @tsv' "$tmp/check.json")
((${#rows[@]} == 5)) || { echo "unexpected hyperfine JSON in $tmp/check.json" >&2; exit 2; }

# Padded as prettier would, so a later format pass leaves the table alone. The
# first column is left-aligned and the rest right-aligned.
header="Tool|Mean [s]|Min [s]|Max [s]|vs packwerk"
widths=(0 0 0 0 0)
for row in "$header" "${rows[@]}"; do
  IFS='|' read -r -a cells <<<"$row"
  for i in 0 1 2 3 4; do
    ((${#cells[i]} > widths[i])) && widths[i]=${#cells[i]}
  done
done

line() {
  local out="|" cell pad i
  IFS='|' read -r -a cells <<<"$1"
  for i in 0 1 2 3 4; do
    cell=${cells[i]}
    pad=$(printf '%*s' $((widths[i] - ${#cell})) '')
    if ((i == 0)); then out+=" $cell$pad |"; else out+=" $pad$cell |"; fi
  done
  echo "$out"
}

dashes() {
  printf '%*s' "$1" '' | tr ' ' '-'
}

if [[ $(uname) == Darwin ]]; then
  hw=$(system_profiler SPHardwareDataType)
  model=$(sed -n 's/^ *Model Name: //p' <<<"$hw")
  chip=$(sed -n 's/^ *Chip: Apple //p' <<<"$hw")
  machine="$model $chip"
else
  machine=$(uname -sm)
fi
ruby_version=$(ruby -e 'print RUBY_VERSION')
rust_version=$(cd "$repo" && rustc -V | awk '{ print $2 }')

{
  line "$header"
  sep="| :$(dashes $((widths[0] - 1))) |"
  for i in 1 2 3 4; do sep+=" $(dashes $((widths[i] - 1))): |"; done
  echo "$sep"
  for row in "${rows[@]}"; do line "$row"; done
  echo
  # The project description before "; idle" is hand-written; only the machine
  # and versions are measured.
  grep '^(powered by .*; idle .*)$' "$readme" |
    sed "s/; idle .*)\$/; idle $machine, Ruby $ruby_version, Rust $rust_version)/"
} >"$tmp/block"

awk -v block="$tmp/block" '
  /^<!-- bench:end -->$/ { while ((getline l < block) > 0) print l; skip = 0 }
  !skip { print }
  /^<!-- bench:start -->$/ { skip = 1 }
' "$readme" >"$tmp/README.md"
mv "$tmp/README.md" "$readme"
echo "Updated $readme"
