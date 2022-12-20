#!/bin/bash
# Starts the Ethereum testnet chain in the background

set -ex
TEST_TYPE=$1
ALCHEMY_ID=$2
# GETH and TEST_TYPE may be unbound, don't `set -u`

# Starts a hardhat RPC backend that is based off of a fork of Ethereum mainnet. This is useful in that we take
# over the account of a major Uniswap liquidity provider and from there we can test many things that are infeasible
# to do with a Geth backend, simply becuase reproducting that state on our testnet would be far too complex to consider
# The tradeoff here is that hardhat is an ETH dev environment and not an actual ETH implementation, as such the outputs
# may be different. These two tests have different fork block heights they rely on
if [[ $TEST_TYPE == *"ARBITRARY_LOGIC"* ]]; then
    export ALCHEMY_ID=$ALCHEMY_ID
    pushd /gravity/solidity
    npm run solidity_test_fork &
    popd
elif [[ $TEST_TYPE == *"RELAY_MARKET"* ]]; then
    export ALCHEMY_ID=$ALCHEMY_ID
    pushd /gravity/solidity
    npm run evm_fork &
    popd
# This starts a hardhat test environment with no pre-seeded state, faster to run, not accurate
elif [[ ! -z "$HARDHAT" ]]; then
    pushd /gravity/solidity
    npm run evm &
    popd
# This starts the Geth backed testnet with no pre-seeded in state.
# Geth is what we run in CI and in general, but developers frequently
# perfer a faster experience provided by HardHat, also Mac's do not
# work correctly with the Geth backend, there is some issue where the Docker VM on Mac platforms can't get
# the right number of cpu cores and Geth goes crazy consuming all the processing power, on the other hand
# hardhat doesn't work for some tests that depend on transactions waiting for blocks, so Geth is the default
else
    echo "waiting for neon to come online"
    until $(curl --output /dev/null --fail --silent --header "content-type: application/json" --data '{"method":"eth_blockNumber","params":[],"id":1,"jsonrpc":"2.0"}' http://proxy:9090/solana); do
        sleep 1
    done
    echo "waiting for neon to sync"
    until [ "$(curl -s --header "content-type: application/json" --data '{"id":1,"jsonrpc":"2.0","method":"eth_syncing","params":[]}' http://proxy:9090/solana)" == '{"jsonrpc": "2.0", "id": 1, "result": false}' ]; do
        sleep 1
    done
    # request funds for the test account
    echo "waiting for faucet"
    until $(curl --output /dev/null --fail --silent -X POST -d '{"wallet": "0xBf660843528035a5A4921534E156a27e64B231fE", "amount": 100000000}' 'http://faucet:3333/request_neon'); do
        sleep 1
    done
    # request funds for block stimulator
    curl -X POST -d '{"wallet": "0x3Cd0A705a2DC65e5b1E1205896BaA2be8A07c6e0", "amount": 100000000}' 'http://faucet:3333/request_neon'
fi
