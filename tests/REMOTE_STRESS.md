# Run the remote stress test on the running chain


* Export the minter private key for the test

```
export MINER_PRIVATE_KEY="eth-private-key" # the address used as faucet for test, usually we need about
export COSMOS_NODE_GRPC="http://host:port"  # the port is 9090 by default
export ETH_NODE="http://your-eth-node"
export GRAVITY_ADDRESS="deployed-gravity-address"
```

* Run the contract deployer to deploy ERC20 contracts only

```
cd solidity
npm ci
npm run typechain

cp artifacts/contracts/Gravity.sol/Gravity.json Gravity.json
cp artifacts/contracts/TestERC20A.sol/TestERC20A.json TestERC20A.json
cp artifacts/contracts/TestERC20B.sol/TestERC20B.json TestERC20B.json
cp artifacts/contracts/TestERC20C.sol/TestERC20C.json TestERC20C.json

npx ts-node \
contract-deployer.ts \
--eth-node=$ETH_NODE \
--eth-privkey=$MINER_PRIVATE_KEY \
--contract=Gravity.json \
--test-mode=true \
--remote-mode=true

# remove redundant files
rm *.json

```

Copy the addresses from the output, example:

```
ERC20 deployed at Address -  address1
ERC20 deployed at Address -  address2
ERC20 deployed at Address -  address3
```
You can run this step multiple times if you want.

Set the deployed contract 
```
export ERC20_ADDRESSES=address1,address2,address3
```

* Build and run test-runner

```
cd ../orchestrator
cargo build --all --release
chmod +x target/release/test-runner

# Run test for 50 users and 2 sends for each user 100 eth -> onomy and 100 onomy -> eth

RUST_BACKTRACE=full \
RUST_LOG="INFO" \
TEST_TYPE=REMOTE_STRESS \
DEPLOY_CONTRACTS=false \
MINER_PRIVATE_KEY=$MINER_PRIVATE_KEY \
COSMOS_NODE_GRPC=$COSMOS_NODE_GRPC \
ERC20_ADDRESSES=$ERC20_ADDRESSES \
NUM_USERS=10 \
ADDRESS_PREFIX=onomy \
ETH_NODE=$ETH_NODE \
GRAVITY_ADDRESS=$GRAVITY_ADDRESS \
WEI_PER_USER=1000000000000000 \
NUM_OF_SEND_ITERATIONS=5 \
./target/release/test-runner
 
```

The last "run" step might be launched almost unlimited numer of times, since when we `deploy`ed ERC20 contracts
with 100000000000000000000000000 minted coins.

At that step, we can also build the new testrunner docker and run multiple instances in parallel with retry.
We need the retry because at the first steps the minter sends ETH and ERC20 tokens to the generated addresses, and
it might cause an issue in case we execute it in parallel.

Also pay attention that the WEI_PER_USER should cover NUM_OF_SEND_ITERATIONS * ERC20_ADDRESSES operations,
so if you increase those params you must increase the WEI_PER_USER as well. The symptom of the misconfiguration of
that param is the error with the wait tx timeouts.