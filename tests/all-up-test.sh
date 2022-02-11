#!/bin/bash
set -eux
# the directory of this script, useful for allowing this script
# to be run with any PWD
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
REPOFOLDER=$DIR/..

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
set +u
TEST_TYPE=$1
ALCHEMY_ID=$2
set -u

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

# Run new test container instance
docker run --name gravity_all_up_test_instance $VOLUME_ARGS --env USE_LOCAL_ARTIFACTS=${USE_LOCAL_ARTIFACTS:-0} $PLATFORM_CMD --cap-add=NET_ADMIN -t gravity-base /bin/bash /gravity/tests/container-scripts/all-up-test-internal.sh $NODES $TEST_TYPE $ALCHEMY_ID
