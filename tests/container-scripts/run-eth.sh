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
    moonbeam --dev --rpc-port 8545 &> /moonbeam.log &
    echo "waiting for moonbeam to come online"
    until $(curl --output /dev/null --fail --silent --header "content-type: application/json" --data '{"method":"eth_blockNumber","params":[],"id":93,"jsonrpc":"2.0"}' http://localhost:8545); do
        printf '.'
        sleep 1
    done

    # transfer funds from Alith account to account used by bridge
    curl -s --header "content-type: application/json" --data "{\"id\":10,\"jsonrpc\":\"2.0\",\"method\":\"eth_sendRawTransaction\",\"params\":[\"0xf870808506fc23ac00825dc094bf660843528035a5a4921534e156a27e64b231fe8ae8ef1e96ae389780000080820a25a03c8d2c425d0b408b4b9084de247f9051854598dc4a3ab0803ee0aa4fe20a8c1aa06e12623f17b9c830c696a538cad8af562ec750e4fd9bdc94302b29fe871495cf\"]}" http://localhost:8545
fi
