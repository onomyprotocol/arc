# Test scripts

The scripts directly under `tests/` are for running from the command line locally.
`tests/container-scripts` are for running within the container, but can be run externally with
scripts like `run-tests.sh` after the container has been built with `build-container.sh` and
run with `start-chains.sh`. `all-up-test.sh` and `run-all-test.sh` automatically run the whole
process from building the container to executing scripts in it.

NOTE: `git archive ... HEAD` is used so that only committed changes will be used, both in the
default case and with USE_LOCAL_ARTIFACTS.

## USE_LOCAL_ARTIFACTS

For CI or other rigorous testing, the scripts should be run without setting `USE_LOCAL_ARTIFACTS`,
in which case the default behavior will build needed artifacts from scratch in a clean container.

The default process takes several minutes which makes development cycles slow. Instead,
`USE_LOCAL_ARTIFACTS=1` can be prepended (e.x.
`USE_LOCAL_ARTIFACTS=1 bash all-up-test.sh HAPPY_PATH_V2`). This will cause
`build-container.sh` to use locally built artifacts. The first build in a clean repository will be
as slow as the default case, but every build afterwards will reuse the local incremental compilation
data on the Rust and Go sides.
