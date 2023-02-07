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
    # init the genesis block
    geth --identity "GravityTestnet" \
    --nodiscover \
    --networkid 1337 init /gravity/tests/assets/ETHGenesis.json

    # etherbase is where rewards get sent
    # private key for this address is 0xb1bab011e03a9862664706fc3bbaa1b16651528e5f0e7fbfcbfdd8be302a13e7
    geth --identity "GravityTestnet" --nodiscover \
    --networkid 1337 \
    --mine \
    --http \
    --http.addr="0.0.0.0" \
    --http.vhosts="*" \
    --http.corsdomain="*" \
    --miner.threads=1 \
    --nousb \
    --verbosity=5 \
    --miner.etherbase=0xBf660843528035a5A4921534E156a27e64B231fE &> /geth.log &

    echo "waiting for geth to come online"
    until $(curl --output /dev/null --fail --silent --header "content-type: application/json" --data '{"method":"eth_blockNumber","params":[],"id":1,"jsonrpc":"2.0"}' http://localhost:8545); do
        sleep 1
    done
    echo "waiting for geth to sync"
    until [ "$(curl -s --header "content-type: application/json" --data '{"id":1,"jsonrpc":"2.0","method":"eth_syncing","params":[]}' http://localhost:8545)" == '{"jsonrpc":"2.0","id":1,"result":false}' ]; do
        sleep 1
    done
fi
