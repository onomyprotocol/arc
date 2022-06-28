![black](https://user-images.githubusercontent.com/76499838/176236578-604faf74-3260-42dd-83bd-2717a5226cb5.png)

Onoym Arch is a bridge extended from AltheaNet's Gravity Bridge that was designed to run on the [Cosmos SDK blockchains](https://github.com/cosmos/cosmos-sdk) like the [Cosmos Hub](https://github.com/cosmos/gaia) focused on maximum design simplicity and efficiency. While initially a Cosmos <-> Ethereum bridge, Onomy has extended Gravity Bridge functionality with Arch by integrating additional chains to create a more inclusive cross-chain DeFi hub. Specifically, Arch pairs with the Onoym ecosystem of applications including the Onomy Exchange (hybrid orderbook + AMM DEX) and Onomy Access (multi-chain mobile wallet application) + more in future. 

Additional functionality and integrations are audited by NCC Group. 

## Documentation

### High level Explanation

Arch enables users to transfer tokens from an integrated chain to Onomy and back again by locking up tokens on integrated chain side, and minting equivalent tokens on the Onomy side. Arch is completely non-custodial, you only need to trust in the security of the Onomy chain itself - not some third party bridge administrators who could run off with your funds. The security of the Onomy chain, and thus Arch, is through the Onomy Validator Guild (OVG) which is comprised of a decentralized network of globally situated independent validator firms. 

### Code documentation

This documentation lives with the code it references and helps to understand the functions and data structures involved. This is useful if you are reviewing or working on the code.

[Solidity Ethereum contract documentation](https://github.com/onomyprotocol/onomy-arch/blob/main/solidity/contracts/contract-explanation.md)

[Go Cosmos module documentation](https://github.com/onomyprotocol/onomy-arch/tree/main/module/x/gravity/spec)

### Specs

These specs cover specific areas of the bridge that a lot of thought went into. They explore the tradeoffs involved and decisions made.

[slashing-spec](/spec/slashing-spec.md)

[batch-creation-spec](/spec/batch-creation-spec.md)

[valset-creation-spec](/spec/valset-creation-spec.md)

### Design docs

These are mid-level docs which go into the most detail on various topics relating to the bridge.

[design overview](/docs/design/overview.md)

[Bootstrapping the bridge](/docs/design/bootstrapping.md)

[Minting and locking tokens in Gravity](/docs/design/mint-lock.md)

[Oracle design](/docs/design/oracle.md)

[Ethereum signing](/docs/design/ethereum-signing.md)

[Messages](/docs/design/messages.md)

[Parameters](/docs/design/parameters.md)

[Incentives](/docs/design/incentives.md)

[arbitrary logic](/docs/design/arbitrary-logic.md)

[relaying semantics](/docs/design/relaying-semantics.md)

### Integrated Chains

[Moonbeam](https://github.com/onomyprotocol/onomy-arch/tree/moonbeam) | [Polygon](https://github.com/onomyprotocol/onomy-arch/tree/polygon) | [Fantom](https://github.com/onomyprotocol/onomy-arch/tree/fantom) | [Neon](https://github.com/onomyprotocol/onomy-arch/tree/neon) |  [Avalanche](https://github.com/onomyprotocol/onomy-arch/tree/avax) | [Aurora](https://github.com/onomyprotocol/near-aurora-bridge)

### Developer Guide

To contribute, refer to these guides.

[Development environment setup](/docs/developer/environment-setup.md)

[Code structure](/docs/developer/code-structure.md)

[Adding integration tests](/docs/developer/modifying-integration-tests.md)

[Security hotspots](/docs/developer/hotspots.md)

## Status

Arch is running on Onomy Testnet with integrations of Ethereum, Aurora/NEAR, Avalanche, Polygon and more. Audits have been completed by NCC Group. Additional chains to be integrated. 

It is your responsibility to understand the financial, legal, and other risks of using this software. There is no guarantee of functionality or safety. You use the Arch bridge entirely at your own risk.

## The design of Arch

- Trust in the integrity of Arch is anchored on the Onomy Network side. The signing of fraudulent validator set updates and transaction batches meant for the Ethereum contract, for example, is punished by slashing on the Cosmos chain. 
- It is mandatory for validators to maintain a trusted Ethereum node. This removes all trust and game theory implications that usually arise from independent relayers, once again dramatically simplifying the design.

## Key design Components

- A highly efficient way of mirroring Onomy validator voting onto Ethereum. The Arch solidity contract has validator set updates costing ~500,000 gas ($2 @ 20gwei). This was tested through Gravity with a snapshot of the Cosmos Hub validator set with 125 validators. Verifying the votes of the validator set is the most expensive on chain operation Gravity has to perform. Our highly optimized Solidity code provides enormous cost savings. Existing bridges incur more than double the gas costs for signature sets as small as 8 signers.
- Transactions from Onomy to other chains are batched, batches have a base cost of ~500,000 gas ($2 @ 20gwei). Batches may contain arbitrary numbers of transactions within the limits of sends per block, allowing for costs to be heavily amortized on high volume bridges.

## Operational parameters ensuring security

- There must be a validator set update made on the Ethereum contract by calling the `updateValset` method at least once every Cosmos unbonding period (usually 2 weeks). This is because if there has not been an update for longer than the unbonding period, the validator set stored by the Ethereum contract could contain validators who cannot be slashed for misbehavior.
- Onomy full nodes do not verify events coming from Ethereum. These events are accepted into the Cosmos state based purely on the signatures of the current validator set. It is possible for the validators with >2/3 of the stake to put events into the Cosmos state which never happened on Ethereum. In this case observers of both chains will need to "raise the alarm". We have built this functionality into the relayer.
