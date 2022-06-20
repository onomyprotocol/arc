#!/bin/bash
set -ex
# the directory of this script, useful for allowing this script
# to be run with any PWD
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
REPOFOLDER=$DIR/..

pushd $DIR

# builds the container containing various system deps
# also builds Gravity once in order to cache Go deps, this container
# must be rebuilt every time you run this test if you want a faster
# solution use start chains and then run tests
# note, this container does not need to be rebuilt to test the same code
# twice, docker will automatically detect and cache this case, no need
# for that logic here
bash $DIR/build-container.sh

# Remove existing container instance
set +e
docker rm -f gravity_all_up_test_instance
set -e

NODES=4
TEST_TYPE=$1
ALCHEMY_ID=$2

# setup for Mac M1 Compatibility 
PLATFORM_CMD=""
if [[ "$OSTYPE" == "darwin"* ]]; then
    if [[ -n $(sysctl -a | grep brand | grep "M1") ]]; then
       echo "Setting --platform=linux/amd64 for Mac M1 compatibility"
       PLATFORM_CMD="--platform=linux/amd64"; fi
fi

# use value instead of git archive in case of USE_LOCAL_ARTIFACTS tests
if [[ "${USE_LOCAL_ARTIFACTS:-0}" -eq "0" ]]; then
   VOLUME_ARGS=""
else
   VOLUME_ARGS="-v $REPOFOLDER:/gravity"
fi

# we cannot simply pass `--env COSMOS_NODE_ABCI=${COSMOS_NODE_ABCI:-}` to `docker run`,
# docker will set the environment variables with empty strings (different than being unset) if the
# local variable is unset
REPLICATED_VARS=""
if [[ -n "${COSMOS_NODE_GRPC}" ]]; then
   REPLICATED_VARS=REPLICATED_VARS:"--env COSMOS_NODE_GRPC=${COSMOS_NODE_GRPC} "
fi
if [[ -n "${COSMOS_NODE_ABCI}" ]]; then
   REPLICATED_VARS=REPLICATED_VARS:"--env COSMOS_NODE_ABCI=${COSMOS_NODE_ABCI} "
fi
if [[ -n "${ETH_NODE}" ]]; then
   REPLICATED_VARS=REPLICATED_VARS:"--env ETH_NODE=${ETH_NODE} "
fi
if [[ -n "${MINER_PRIVATE_KEY}" ]]; then
   REPLICATED_VARS=REPLICATED_VARS:"--env MINER_PRIVATE_KEY=${MINER_PRIVATE_KEY} "
fi
if [[ -n "${GRAVITY_ADDRESS}" ]]; then
   REPLICATED_VARS=REPLICATED_VARS:"--env GRAVITY_ADDRESS=${GRAVITY_ADDRESS} "
fi
# used by manual remote stress testing
if [[ -n "${NUM_USERS}" ]]; then
   REPLICATED_VARS=REPLICATED_VARS:"--env NUM_USERS=${NUM_USERS} "
fi
if [[ -n "${WEI_PER_USER}" ]]; then
   REPLICATED_VARS=REPLICATED_VARS:"--env WEI_PER_USER=${WEI_PER_USER} "
fi

RUN_ARGS=""
if [[ "${TEST_TYPE:-}" == "NO_SCRIPTS" ]]; then
   echo "Running container instance without starting scripts"
elif [[ "${TEST_TYPE:-}" == "REMOTE_STRESS_FOR_CI" ]]; then
   RUN_ARGS="/bin/bash /gravity/tests/container-scripts/remote-test-for-ci.sh"
else
   RUN_ARGS="/bin/bash /gravity/tests/container-scripts/all-up-test-internal.sh ${NODES} ${TEST_TYPE:-} ${ALCHEMY_ID:-}"
fi

docker-compose -f dockerfile/docker-compose.yml down

set +e
docker network rm net
set -e
# insure everything is self contained
docker network create --internal net

export NEON_EVM_COMMIT="v0.8.1"
export REVISION="v0.8.1"
export FAUCET_COMMIT=19a661e04545f3a880efc04f9b7924ba7c0d92cb
export USE_LOCAL_ARTIFACTS=${USE_LOCAL_ARTIFACTS:-0}
export VOLUME_ARGS
export RUN_ARGS
docker-compose -f dockerfile/docker-compose.yml build

set +e
docker-compose -f dockerfile/docker-compose.yml up -d --force-recreate
set -e

# the test container is run separately from the ones in `docker-compose` because of problems related
# to passing the replicated variables, volumes, and adding IPs for the gravity nodes
docker run --name gravity_all_up_test_instance --network net --hostname test $VOLUME_ARGS --env USE_LOCAL_ARTIFACTS=${USE_LOCAL_ARTIFACTS:-0} $REPLICATED_VARS $PLATFORM_CMD --cap-add=NET_ADMIN -t gravity-base $RUN_ARGS

docker-compose -f dockerfile/docker-compose.yml down
set +e
docker network rm net
set -e
