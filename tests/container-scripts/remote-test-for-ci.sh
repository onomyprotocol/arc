#!/bin/bash
set -ex
# This is not how the real remote testing will be done, this is for the CI to test
# if the needed environment variable configuration and REMOTE_STRESS work.

if [[ "${USE_LOCAL_ARTIFACTS:-0}" -eq "0" ]]; then
    # Prepare the contracts for later deployment
    pushd /gravity/solidity/
    HUSKY_SKIP_INSTALL=1 npm ci
    npm run typechain
    RUN_ARGS="cargo run --release --bin test-runner"
else
    RUN_ARGS=/gravity/tests/dockerfile/test-runner
fi

# manually set up the nodes so we can simulate interacting with already running testnet nodes
RUN_ARGS=$RUN_ARGS bash /gravity/tests/container-scripts/setup-validators.sh 4
bash /gravity/tests/container-scripts/run-gravity.sh 4
sleep 10
bash /gravity/tests/container-scripts/run-eth.sh
sleep 10

# predeploy Gravity contract
pushd /gravity/orchestrator/test_runner
DEPLOY_CONTRACTS=1 RUST_BACKTRACE=full RUST_LOG="INFO,relayer=DEBUG,orchestrator=DEBUG" PATH=$PATH:$HOME/.cargo/bin $RUN_ARGS

GRAVITY_ADDRESS=$(cat /contracts | sed -n -e 's/^Gravity deployed at Address -  //p') COSMOS_NODE_GRPC=http://localhost:9090 COSMOS_NODE_ABCI=http://localhost:26657 ETH_NODE=http://localhost:8545 bash /gravity/tests/container-scripts/all-up-test-internal.sh REMOTE_STRESS
