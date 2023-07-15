#!/bin/bash
set -eux
# your gaiad binary name
BIN=gravity

NODES=$1

for i in $(seq 1 $NODES);
do
    # add this ip for loopback dialing
    ip addr add 7.7.7.$i/32 dev eth0 || true # allowed to fail

    GAIA_HOME="--home /validator$i"
    # this implicitly caps us at ~6000 nodes for this sim
    # note that we start on 26656 the idea here is that the first
    # node (node 1) is at the expected contact address from the gentx
    # faciliating automated peer exchange
    if [[ "$i" -eq 1 ]]; then
        # node one gets localhost so we can easily shunt these ports
        # to the docker host
        RPC_ADDRESS="--rpc.laddr tcp://0.0.0.0:26657"
        GRPC_ADDRESS="--grpc.address 0.0.0.0:9090"
        GRPC_WEB_ADDRESS="--grpc-web.address 0.0.0.0:9092"
    else
        # move these to another port and address, not becuase they will
        # be used there, but instead to prevent them from causing problems
        # you also can't duplicate the port selection against localhost
        # for reasons that are not clear to me right now.
        RPC_ADDRESS="--rpc.laddr tcp://7.7.7.$i:26658"
        GRPC_ADDRESS="--grpc.address 7.7.7.$i:9091"
        GRPC_WEB_ADDRESS="--grpc-web.address 7.7.7.$i:9093"
    fi
    LISTEN_ADDRESS="--address tcp://7.7.7.$i:26655"
    P2P_ADDRESS="--p2p.laddr tcp://7.7.7.$i:26656"
    LOG_LEVEL="--log_level info"
    INVARIANTS_CHECK="--inv-check-period 1"
    ARGS="$GAIA_HOME $LISTEN_ADDRESS $RPC_ADDRESS $GRPC_ADDRESS $GRPC_WEB_ADDRESS $LOG_LEVEL $INVARIANTS_CHECK $P2P_ADDRESS"
    $BIN $ARGS start &> /gravity/tests/assets/gravity_validator$i.log &
done
