#!/bin/bash
# the script run inside the container for all-up-test.sh
NODES=$1
TEST_TYPE=$2
ALCHEMY_ID=$3
set -ex

if [[ "${USE_LOCAL_ARTIFACTS:-0}" -eq "0" ]]; then
    # Prepare the contracts for later deployment
    pushd /gravity/solidity/
    HUSKY_SKIP_INSTALL=1 npm ci
    npm run typechain
    RUN_ARGS="cargo run --release --bin test-runner"
else
    RUN_ARGS=/gravity/tests/dockerfile/test-runner
fi

# bash doesn't have booleans or a proper XOR operator
BOOL=0
if [[ -z "${COSMOS_NODE_GRPC}" ]] && [[ -n "${COSMOS_NODE_ABCI}" ]]; then
   BOOL=1
fi
if [[ -n "${COSMOS_NODE_GRPC}" ]] && [[ -z "${COSMOS_NODE_ABCI}" ]]; then
   BOOL=1
fi
# this is to catch accidentally forgetting one or the other
if [[ "$BOOL" -eq "1" ]]; then
   echo "both or none of the COSMOS_NODE_GRPC and COSMOS_NODE_ABCI environment variables need to be set"
   exit 1
fi

if [[ -z "${COSMOS_NODE_GRPC}" ]] ; then
    echo "Setting up cosmos side"
    bash /gravity/tests/container-scripts/setup-validators.sh $NODES
    bash /gravity/tests/container-scripts/run-gravity.sh $NODES
    # let the cosmos chain settle before starting eth as it
    # consumes a lot of processing power
    sleep 10
fi
if [[ -z "${ETH_NODE}" ]]; then
    echo "Setting up ethereum side"
    bash /gravity/tests/container-scripts/run-eth.sh $TEST_TYPE $ALCHEMY_ID
    sleep 10
fi
# running the test runner only to deploy the ethereum contracts
# note that variables like GRAVITY_ADDRESS affect the binary
pushd /gravity/orchestrator/test_runner
DEPLOY_CONTRACTS=1 RUST_BACKTRACE=full RUST_LOG="INFO,relayer=DEBUG,orchestrator=DEBUG" PATH=$PATH:$HOME/.cargo/bin $RUN_ARGS

# runs the test runner for real
bash /gravity/tests/container-scripts/integration-tests.sh $TEST_TYPE
