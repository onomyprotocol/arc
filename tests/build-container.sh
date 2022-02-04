#!/bin/bash
set -eux

# this directy of this script
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
DOCKERFOLDER=$DIR/dockerfile
REPOFOLDER=$DIR/..

#docker system prune -a -f

# By default we want to do a clean build, but for faster development `USE_LOCAL_ARTIFACTS=1` can
# be set in which case binaries that reuse local artifacts will be placed into the docker image
if [[ "${USE_LOCAL_ARTIFACTS:-0}" -eq "0" ]]; then
    # change our directory so that the git archive command works as expected
    pushd $REPOFOLDER

    # don't compress to `tar.gz`, because it adds more build time and
    # will be uncompressed anyway when added to the container
    git archive --format=tar -o $DOCKERFOLDER/gravity.tar --prefix=gravity/ HEAD
else
    # getting the `test-runner` binary
    pushd $REPOFOLDER/orchestrator/test_runner && PATH=$PATH:$HOME/.cargo/bin cargo build --all --release
    # getting the `gravity` binary. `GOBIN` is set so that it is placed under `/dockerfile`.
    # This will be moved to its normal place by the `Dockerfile`.
    pushd $REPOFOLDER/module/ &&
        PATH=$PATH:/usr/local/go/bin GOPROXY=https://proxy.golang.org make &&
        PATH=$PATH:/usr/local/go/bin GOBIN=$REPOFOLDER/tests/dockerfile make install

    # TODO figure out a way to reuse solidity artifacts and deploy contracts,
    # preferably without bringing all of `node_modules` to the container
    #pushd $REPOFOLDER/solidity/ &&
    #HUSKY_SKIP_INSTALL=1 npm install
    #npm run typechain

    pushd $REPOFOLDER

    # because `--add-file` is not available except in very recent versions of `git`,
    # we cannot add them through the archive command and need to add them to the tar
    # file ourselves
    git archive --format=tar -o $DOCKERFOLDER/gravity.tar --prefix=gravity/ HEAD
    tar --append --file=$DOCKERFOLDER/gravity.tar --transform='s,orchestrator/target/release/test-runner,gravity/orchestrator/target/release/test-runner,' $REPOFOLDER/orchestrator/target/release/test-runner
    # this is the go binary
    tar --append --file=$DOCKERFOLDER/gravity.tar --transform='s,tests/dockerfile/gravity,gravity/tests/dockerfile/gravity,' $REPOFOLDER/tests/dockerfile/gravity
    # solidity artifacts
    #tar --append --file=$DOCKERFOLDER/gravity.tar --transform='s,solidity/cache,gravity/solidity/cache,' $REPOFOLDER/solidity/cache
    #tar --append --file=$DOCKERFOLDER/gravity.tar --transform='s,solidity/artifacts,gravity/solidity/artifacts,' $REPOFOLDER/solidity/artifacts
    #tar --append --file=$DOCKERFOLDER/gravity.tar --transform='s,solidity/typechain,gravity/solidity/typechain,' $REPOFOLDER/solidity/typechain
    #tar --append --file=$DOCKERFOLDER/gravity.tar --transform='s,solidity/node_modules,gravity/solidity/node_modules,' $REPOFOLDER/solidity/node_modules
fi

pushd $DOCKERFOLDER

# setup for Mac M1 Compatibility 
PLATFORM_CMD=""
if [[ "$OSTYPE" == "darwin"* ]]; then
    if [[ -n $(sysctl -a | grep brand | grep "M1") ]]; then
       echo "Setting --platform=linux/amd64 for Mac M1 compatibility"
       PLATFORM_CMD="--platform=linux/amd64"; fi
fi
docker build -t gravity-base $PLATFORM_CMD . --build-arg use_local_artifacts=${USE_LOCAL_ARTIFACTS:-0}
