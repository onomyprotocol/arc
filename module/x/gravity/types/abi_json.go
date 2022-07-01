package types

import (
	"fmt"
	"strings"

	"github.com/ethereum/go-ethereum/accounts/abi"
)

// The go-ethereum ABI encoder *only* encodes function calls and then it only encodes
// function calls for which you provide an ABI json just like you would get out of the
// solidity compiler with your compiled contract.
// You are supposed to compile your contract, use abigen to generate an ABI , import
// this generated go module and then use for that for all testing and development.
// This abstraction layer is more trouble than it's worth, because we don't want to
// encode a function call at all, but instead we want to emulate a Solidity encode operation
// which has no equal available from go-ethereum.
//
// In order to work around this absurd series of problems we have to manually write the below
// 'function specification' that will encode the same arguments into a function call. We can then
// truncate the first several bytes where the call name is encoded to finally get the equal of the

var (
	OutgoingBatchTxCheckpointABI, ValsetCheckpointABI, OutgoingLogicCallABI abi.ABI
)

func init() {
	var err error

	// OutgoingBatchTxCheckpointABIJSON checks the ETH ABI for compatability of the OutgoingBatchTx message
	OutgoingBatchTxCheckpointABI, err = abi.JSON(strings.NewReader(`[{
		"name": "submitBatch",
		"stateMutability": "pure",
		"type": "function",
		"inputs": [
			{ "internalType": "bytes32",   "name": "_gravityId",       "type": "bytes32" },
			{ "internalType": "bytes32",   "name": "_methodName",    "type": "bytes32" },
			{ "internalType": "uint256[]", "name": "_amounts",       "type": "uint256[]" },
			{ "internalType": "address[]", "name": "_destinations",  "type": "address[]" },
			{ "internalType": "uint256[]", "name": "_fees",          "type": "uint256[]" },
			{ "internalType": "uint256",   "name": "_batchNonce",    "type": "uint256" },
			{ "internalType": "address",   "name": "_tokenContract", "type": "address" },
			{ "internalType": "uint256",   "name": "_batchTimeout",  "type": "uint256" }
		],
		"outputs": [
			{ "internalType": "bytes32", "name": "", "type": "bytes32" }
		]
	}]`))

	if err != nil {
		panic(fmt.Sprintf("can't decode OutgoingBatchTxCheckpointABI, %s", err))
	}

	// ValsetCheckpointABIJSON checks the ETH ABI for compatability of the Valset update message
	ValsetCheckpointABI, err = abi.JSON(strings.NewReader(`[{
		"name": "checkpoint",
		"stateMutability": "pure",
		"type": "function",
		"inputs": [
			{ "internalType": "bytes32",   "name": "_gravityId",   "type": "bytes32"   },
			{ "internalType": "bytes32",   "name": "_checkpoint",  "type": "bytes32"   },
			{ "internalType": "uint256",   "name": "_valsetNonce", "type": "uint256"   },
			{ "internalType": "address[]", "name": "_validators",  "type": "address[]" },
			{ "internalType": "uint256[]", "name": "_powers",      "type": "uint256[]" },
			{ "internalType": "uint256",   "name": "_rewardAmount", "type": "uint256"  },
			{ "internalType": "string",    "name": "_rewardDenom",  "type": "string"   }
		],
		"outputs": [
			{ "internalType": "bytes32", "name": "", "type": "bytes32" }
		]
	}]`))

	if err != nil {
		panic(fmt.Sprintf("can't decode ValsetCheckpointABI, %s", err))
	}

	// OutgoingLogicCallABIJSON checks the ETH ABI for compatability of the logic call message
	OutgoingLogicCallABI, err = abi.JSON(strings.NewReader(`[{
	  "name": "checkpoint",
      "outputs": [],
      "stateMutability": "pure",
      "type": "function",
      "inputs": [
			{ "internalType": "bytes32",   "name": "_gravityId",                "type": "bytes32"   },
			{ "internalType": "bytes32",   "name": "_methodName",             "type": "bytes32"   },
			{ "internalType": "uint256[]", "name": "_transferAmounts",        "type": "uint256[]" },
			{ "internalType": "address[]", "name": "_transferTokenContracts", "type": "address[]" },
			{ "internalType": "uint256[]", "name": "_feeAmounts",             "type": "uint256[]" },
			{ "internalType": "address[]", "name": "_feeTokenContracts",      "type": "address[]" },
			{ "internalType": "address",   "name": "_logicContractAddress",   "type": "address"   },
			{ "internalType": "bytes",     "name": "_payload",                "type": "bytes"     },
			{ "internalType": "uint256",   "name": "_timeout",                "type": "uint256"   },
			{ "internalType": "bytes32",   "name": "_invalidationId",         "type": "bytes32"   },
			{ "internalType": "uint256",   "name": "_invalidationNonce",      "type": "uint256"   }
      ]
    }]`))

	if err != nil {
		panic(fmt.Sprintf("can't decode OutgoingLogicCallABI, %s", err))
	}

}
