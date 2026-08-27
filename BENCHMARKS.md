I use https://github.com/sharkdp/hyperfine to benchmark, which makes it easy to get consistent benchmarks. Note that crabwerk itself does not cache, so every crabwerk run is a cold run; the `bin/packwerk` rows still benefit from packwerk's own cache.
To run these benchmarks on your application, you can place this repo next to your rails application and run bash ../crabwerk/dev/run_benchmarks.sh from the root of your application

> The numbers below were measured at 0.2.40, when the binary was still named `pks` and before caching was removed. They have not been re-taken.

## Hot Cache, with and without spring, entire codebase
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `../crabwerk/target/release/crabwerk update` | 2.219 ± 0.221 | 2.049 | 2.469 | 1.00 |
| `../crabwerk/target/release/crabwerk --experimental-parser update` | 2.506 ± 0.260 | 2.316 | 2.803 | 1.13 ± 0.16 |
| `DISABLE_SPRING=1 bin/packwerk update` | 29.653 ± 2.329 | 27.122 | 31.706 | 13.37 ± 1.70 |
| `bin/packwerk update` | 21.439 ± 2.535 | 19.080 | 24.120 | 9.66 ± 1.49 |

## Hot Cache, with and without spring, single file
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `../crabwerk/target/release/crabwerk check config/initializers/inflections.rb` | 579.9 ± 22.7 | 564.6 | 606.0 | 1.00 |
| `../crabwerk/target/release/crabwerk --experimental-parser check config/initializers/inflections.rb` | 1041.3 ± 10.6 | 1031.7 | 1052.7 | 1.80 ± 0.07 |
| `DISABLE_SPRING=1 bin/packwerk check config/initializers/inflections.rb` | 16693.2 ± 455.8 | 16361.6 | 17213.0 | 28.79 ± 1.37 |
| `bin/packwerk check config/initializers/inflections.rb` | 6749.6 ± 106.0 | 6658.2 | 6865.8 | 11.64 ± 0.49 |
