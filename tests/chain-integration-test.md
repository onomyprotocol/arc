# How to run happy path integration test with the remote chain

1. send 2 eth to the minter account "0xBf660843528035a5A4921534E156a27e64B231fE"

2. run happy path tests
```
ETH_NODE="http://rpc-address" USE_LOCAL_ARTIFACTS=1 bash all-up-test.sh HAPPY_PATH_V2
```
