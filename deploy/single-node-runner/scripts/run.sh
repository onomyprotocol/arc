#!/bin/bash
set -eu

echo "running cosmos-gravity-bridge"

# Initial dir
CURRENT_WORKING_DIR=$(pwd)
# Name of the network to bootstrap
CHAINID="testchain"
# Name of the gravity artifact
GRAVITY=gravity
# The name of the gravity node
GRAVITY_NODE_NAME="gravity"
# The address to run gravity node
GRAVITY_HOST="0.0.0.0"
# The port of the gravity gRPC
GRAVITY_GRPC_PORT="9090"
# Home folder for gravity config
GRAVITY_HOME="$CURRENT_WORKING_DIR/$CHAINID/$GRAVITY_NODE_NAME"
# Home flag for home folder
GRAVITY_HOME_FLAG="--home $GRAVITY_HOME"
# Gravity chain demons
STAKE_DENOM="stake"

ETH_MINER_PUBLIC_KEY="0x239fA7623354eC26520dE878B52f13Fe84b06971"
# The host of ethereum node
ETH_HOST="0.0.0.0"
# Eth node rpc port
ETH_RPC_PORT="8545"

# ------------------ Run gravity ------------------

echo "Starting $GRAVITY_NODE_NAME"
$GRAVITY $GRAVITY_HOME_FLAG start --pruning=nothing &

echo "Waiting $GRAVITY_NODE_NAME to launch gRPC $GRAVITY_GRPC_PORT..."

while ! timeout 1 bash -c "</dev/tcp/$GRAVITY_HOST/$GRAVITY_GRPC_PORT"; do
  sleep 1
done

echo "$GRAVITY_NODE_NAME launched"

#-------------------- Run ethereum (geth) --------------------

geth --identity "GravityTestnet" \
    --nodiscover \
    --networkid 15 init assets/ETHGenesis.json

geth --identity "GravityTestnet" --nodiscover \
                               --networkid 15 \
                               --mine \
                               --http \
                               --http.port $ETH_RPC_PORT \
                               --http.addr "$ETH_HOST" \
                               --http.corsdomain "*" \
                               --http.vhosts "*" \
                               --miner.threads=1 \
                               --nousb \
                               --verbosity=5 \
                               --miner.etherbase="$ETH_MINER_PUBLIC_KEY" \
                               &

GETH_IPC_PATH="/root/.ethereum/geth.ipc"
GETH_CONSOLE="geth --rinkeby attach ipc:$GETH_IPC_PATH console --exec"

# 600 sec to run light node
set +e
for i in {1..600}; do
  sleep 1
  echo "attempt $i to start the eth node"

  ethSyncing=$($GETH_CONSOLE "eth.syncing")
  if [ "$ethSyncing" != "false"  ]; then
     echo "eth.syncing : $ethSyncing"
     continue
  fi

   if [ $i -eq 600 ]; then
     echo "timeout for ethereum light node exceed"
     exit
  fi

  break
done
set -e

echo "ethereum light node is ready"

sleep 60

#-------------------- Run orchestrator --------------------

CONTRACT_ADDRESS=$(cat $GRAVITY_HOME/eth_contract_address)
echo "Contract address: $CONTRACT_ADDRESS"

echo "Gathering keys for orchestrator"
COSMOS_GRPC="http://$GRAVITY_HOST:$GRAVITY_GRPC_PORT/"
COSMOS_PHRASE=$(jq -r .mnemonic $GRAVITY_HOME/orchestrator_key.json)
ETH_RPC=http://$ETH_HOST:$ETH_RPC_PORT
ETH_PRIVATE_KEY=$(jq -r .private_key $GRAVITY_HOME/eth_key.json)
echo "Run orchestrator"

gbt orchestrator --cosmos-phrase="$COSMOS_PHRASE" \
             --ethereum-key="$ETH_PRIVATE_KEY" \
             --cosmos-grpc="$COSMOS_GRPC" \
             --ethereum-rpc="$ETH_RPC" \
             --fees="1$STAKE_DENOM" \
             --gravity-contract-address="$CONTRACT_ADDRESS"