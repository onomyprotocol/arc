#!/bin/bash
TEST_TYPE=$1
set -eu

FILE=/contracts
if test -f "$FILE"; then
echo "Contracts already deployed, running tests"
else
echo "Testnet is not started yet, please wait before running tests"
exit 0
fi

set +e
killall -9 test-runner
set -e

pushd /gravity/orchestrator/test_runner

if [[ "${USE_LOCAL_ARTIFACTS:-0}" -eq "0" ]]; then
    RUN_ARGS="cargo run --release --bin test-runner"
else
    RUN_ARGS=/gravity/tests/dockerfile/test-runner
fi

USE_FINALIZATION=false RUST_BACKTRACE=full TEST_TYPE=$TEST_TYPE RUST_LOG=INFO PATH=$PATH:$HOME/.cargo/bin $RUN_ARGS