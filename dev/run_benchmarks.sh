#!/bin/bash

# Check if the file exists before removing it
if [ -f "tmp/crabwerk_benchmarks.md" ]; then
  rm tmp/crabwerk_benchmarks.md
fi

echo "I use https://github.com/sharkdp/hyperfine to benchmark, which makes it easy to get consistent benchmarks. Note that crabwerk itself does not cache, so every crabwerk run is a cold run; the \`bin/packwerk\` rows still benefit from packwerk's own cache." >> tmp/crabwerk_benchmarks.md
echo "To run these benchmarks on your application, you can place this repo next to your rails application and run bash ../crabwerk/dev/run_benchmarks.sh from the root of your application" >> tmp/crabwerk_benchmarks.md

echo -e "\n## Hot Cache, with and without spring, entire codebase" >> tmp/crabwerk_benchmarks.md

hyperfine --warmup=2 --runs=3 --export-markdown tmp/bm.md \
  '../crabwerk/target/release/crabwerk update' \
  '../crabwerk/target/release/crabwerk --experimental-parser update' \
  'DISABLE_SPRING=1 bin/packwerk update' \
  'bin/packwerk update'

cat tmp/bm.md >> tmp/crabwerk_benchmarks.md

echo -e "\n## Hot Cache, with and without spring, single file" >> tmp/crabwerk_benchmarks.md

hyperfine --warmup=2 --runs=3 --export-markdown tmp/bm.md \
  '../crabwerk/target/release/crabwerk check config/initializers/inflections.rb' \
  '../crabwerk/target/release/crabwerk --experimental-parser check config/initializers/inflections.rb' \
  'DISABLE_SPRING=1 bin/packwerk check config/initializers/inflections.rb' \
  'bin/packwerk check config/initializers/inflections.rb'

cat tmp/bm.md >> tmp/crabwerk_benchmarks.md

mv tmp/crabwerk_benchmarks.md ../crabwerk/BENCHMARKS.md
