#!/bin/bash
set -eux
# your gaiad binary name
BIN=gravity

CHAIN_ID="gravity-test"

NODES=$1

ALLOCATION="100000000000000000000000stake,100000000000000000000000footoken,100000000000000000000000ibc/nometadatatoken"
pushd /gravity/orchestrator/test_runner
# note that the gravity address here is the sha256 hash of 'distribution'
ADDRESS=$(GET_TEST_ADDRESS=1 PATH=$PATH:$HOME/.cargo/bin $RUN_ARGS)
popd

# need to init all validators so that we can get the tendermint keys
for i in $(seq 1 $NODES);
do
$BIN init --home /validator$i --chain-id=$CHAIN_ID validator$i
done

# first we start a genesis.json with validator 1
# validator 1 will also collect the gentx's once gnerated
STARTING_VALIDATOR=1
STARTING_VALIDATOR_HOME="--home /validator$STARTING_VALIDATOR"

# set the minimum gas price so that it isn't an empty string
# note that this enforces `footoken` as the gas denom
sed -c -i "s/\(minimum-gas-prices *= *\).*/\1\"1footoken\"/" /validator1/config/app.toml
# reset the chain id for cases where Cosmos-SDK does not do it properly
sed -c -i "s/\(chain-id *= *\).*/\1\"$CHAIN_ID\"/" /validator1/config/client.toml

## Modify generated genesis.json to our liking by editing fields using jq
## we could keep a hardcoded genesis file around but that would prevent us from
## testing the generated one with the default values provided by the module.

jq ".chain_id = \"$CHAIN_ID\"" /validator$STARTING_VALIDATOR/config/genesis.json > /edited-genesis.json

# add in denom metadata for both native tokens
jq '.app_state.bank.denom_metadata += [{"name": "Foo Token", "symbol": "FOO", "base": "footoken", display: "mfootoken", "description": "A non-staking test token", "denom_units": [{"denom": "footoken", "exponent": 0}, {"denom": "mfootoken", "exponent": 6}]},{"name": "Stake Token", "symbol": "STEAK", "base": "stake", display: "mstake", "description": "A staking test token", "denom_units": [{"denom": "stake", "exponent": 0}, {"denom": "mstake", "exponent": 6}]}]' /edited-genesis.json > /metadata-genesis.json

# a 30 second voting period to allow us to pass governance proposals in the tests
jq '.app_state.gov.voting_params.voting_period = "30s"' /metadata-genesis.json > /community-pool-genesis.json

# Add some funds to the community pool to test Airdrops
jq '.app_state.distribution.fee_pool.community_pool = [{"denom": "stake", "amount": "10000000000.0"}]' /community-pool-genesis.json > /community-pool2-genesis.json
jq '.app_state.auth.accounts += [{"@type": "/cosmos.auth.v1beta1.ModuleAccount", "base_account": { "account_number": "0", "address": "'$ADDRESS'","pub_key": null,"sequence": "0"},"name": "distribution","permissions": ["basic"]}]' /community-pool2-genesis.json > /community-pool3-genesis.json
jq '.app_state.bank.balances += [{"address": "'$ADDRESS'", "coins": [{"amount": "10000000000", "denom": "stake"}]}]' /community-pool3-genesis.json > /edited-genesis.json

mv /edited-genesis.json /genesis.json

# Sets up an arbitrary number of validators on a single machine by manipulating
# the --home parameter on gaiad
for i in $(seq 1 $NODES);
do
GAIA_HOME="--home /validator$i"
GENTX_HOME="--home-client /validator$i"
ARGS="$GAIA_HOME --keyring-backend test"

# Generate a validator key, orchestrator key, and eth key for each validator
$BIN keys add $ARGS validator$i 2>> /validator-phrases
$BIN keys add $ARGS orchestrator$i 2>> /orchestrator-phrases
$BIN eth_keys add >> /validator-eth-keys

VALIDATOR_KEY=$($BIN keys show validator$i -a $ARGS)
ORCHESTRATOR_KEY=$($BIN keys show orchestrator$i -a $ARGS)
# move the genesis in
mkdir -p /validator$i/config/
mv /genesis.json /validator$i/config/genesis.json
$BIN add-genesis-account $ARGS $VALIDATOR_KEY $ALLOCATION
$BIN add-genesis-account $ARGS $ORCHESTRATOR_KEY $ALLOCATION
# move the genesis back out
mv /validator$i/config/genesis.json /genesis.json
done


for i in $(seq 1 $NODES);
do
cp /genesis.json /validator$i/config/genesis.json
GAIA_HOME="--home /validator$i"
ARGS="$GAIA_HOME --keyring-backend test"
CONSADDR=$($BIN $GAIA_HOME tendermint show-address)
#ORCHESTRATOR_KEY=$($BIN keys show orchestrator$i -a $ARGS)
ETHEREUM_KEY=$(grep address /validator-eth-keys | sed -n "$i"p | sed 's/.*://')
# the /8 containing 7.7.7.7 is assigned to the DOD and never routable on the public internet
# we're using it in private to prevent gaia from blacklisting it as unroutable
# and allow local pex
$BIN gentx $ARGS $GAIA_HOME --moniker orchestrator$i --chain-id=$CHAIN_ID --ip 7.7.7.$i "$(seq 400 500 | sort -R | head -n 1)""000000000000000000stake" $CONSADDR $ETHEREUM_KEY orchestrator$i
# obviously we don't need to copy validator1's gentx to itself
if [ $i -gt 1 ]; then
cp /validator$i/config/gentx/* /validator1/config/gentx/
fi
done


$BIN collect-gentxs $STARTING_VALIDATOR_HOME
GENTXS=$(ls /validator1/config/gentx | wc -l)
cp /validator1/config/genesis.json /genesis.json
cp /genesis.json /gravity/tests/assets/gravity_genesis.json
echo "Collected $GENTXS gentx"

# put the now final genesis.json into the correct folders
for i in $(seq 1 $NODES);
do
cp /genesis.json /validator$i/config/genesis.json
done
