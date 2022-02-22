#!/bin/bash
# the script run inside the container for all-up-test.sh
NODES=$1
TEST_TYPE=$2
ALCHEMY_ID=$3
set -eux

if [[ "${USE_LOCAL_ARTIFACTS:-0}" -eq "0" ]]; then
    # Prepare the contracts for later deployment
    pushd /gravity/solidity/
    HUSKY_SKIP_INSTALL=1 npm ci
    npm run typechain
    RUN_ARGS="cargo run --release --bin test-runner"
else
    RUN_ARGS=/gravity/tests/dockerfile/test-runner
fi

bash /gravity/tests/container-scripts/setup-validators.sh $NODES
bash /gravity/tests/container-scripts/run-testnet.sh $NODES $TEST_TYPE $ALCHEMY_ID &

echo "Running test using ETH_NODE:$ETH_NODE"

# deploy the ethereum contracts
pushd /gravity/orchestrator/test_runner
DEPLOY_CONTRACTS=1 RUST_BACKTRACE=full RUST_LOG="INFO,relayer=DEBUG,orchestrator=DEBUG" PATH=$PATH:$HOME/.cargo/bin $RUN_ARGS

bash /gravity/tests/container-scripts/integration-tests.sh $NODES $TEST_TYPE
