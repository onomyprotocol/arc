# How to run happy path integration test with the remote chain

1. send 2 eth to the minter account "0xBf660843528035a5A4921534E156a27e64B231fE"

2. set the "ETH_NODE" variable
```
export ETH_NODE=https://rpc-mumbai.maticvigil.com
```
Here the values is from teh polygon/mumbai, but you must paste the value for the chain you need to test. 

3. run happy path tests
```
./all-up-test.sh
```
