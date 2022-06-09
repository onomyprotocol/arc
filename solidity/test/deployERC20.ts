import chai from "chai";
import {ethers} from "hardhat";
import {solidity} from "ethereum-waffle";

import {deployContracts, sortValidators} from "../test-utils";
import {examplePowers, getSignerAddresses, parseEvent, signHash, ZeroAddress,} from "../test-utils/pure";
import {BigNumber} from "ethers";

chai.use(solidity);
const {expect} = chai;

export const MintedForDeployer = BigInt("100000000000000000000000000")

async function runTest(opts: { duplicateValidator?: boolean; sortValidators?: boolean; }) {
    // Prep and deploy Gravity contract
    // ========================
    const signers = await ethers.getSigners();
    const gravityId = ethers.utils.formatBytes32String("foo");
    // This is the power distribution on the Cosmos hub as of 7/14/2020
    let powers = examplePowers();
    let validators = signers.slice(0, powers.length);

    // arbitrarily set a duplicate validator
    if (opts.duplicateValidator) {
        let firstValidator = validators[0];
        validators[22] = firstValidator;
    }

    // before deploying sort the validators
    if (opts.sortValidators) {
        sortValidators(validators)
    }

    const {gravity} = await deployContracts(gravityId, validators, powers);

    // Deploy ERC20 contract representing Cosmos asset
    // ===============================================
    const eventArgs = await parseEvent(gravity, gravity.deployERC20('uatom', 'Atom', 'ATOM', 6), 1)

    expect(eventArgs).to.deep.equal({
        _cosmosDenom: 'uatom',
        _tokenContract: eventArgs._tokenContract, // We don't know this ahead of time
        _name: 'Atom',
        _symbol: 'ATOM',
        _decimals: 6,
        _eventNonce: BigNumber.from(2)
    })

    // Connect to deployed contract for testing
    // ========================================
    let ERC20contract = new ethers.Contract(eventArgs._tokenContract, [
        "function balanceOf(address account) view returns (uint256 balance)"
    ], gravity.provider);

    const maxUint256 = BigNumber.from(2).pow(256).sub(1)
    // Check that gravity balance is correct
    expect((await ERC20contract.functions.balanceOf(gravity.address)).toString()).to.equal(maxUint256.toString())


    // Prepare batch
    // ===============================
    const numTxs = 100;
    const txDestinationsInt = new Array(numTxs);
    const txFees = new Array(numTxs);

    const txAmounts = new Array(numTxs);
    for (let i = 0; i < numTxs; i++) {
        txFees[i] = 1;
        txAmounts[i] = 1;
        txDestinationsInt[i] = signers[i + 5];
    }
    const txDestinations = await getSignerAddresses(txDestinationsInt);
    let batchNonce = 1
    let batchTimeout = 10000

    // Call method
    // ===========
    const methodName = ethers.utils.formatBytes32String(
        "transactionBatch"
    );
    let abiEncoded = ethers.utils.defaultAbiCoder.encode(
        [
            "bytes32",
            "bytes32",
            "uint256[]",
            "address[]",
            "uint256[]",
            "uint256",
            "address",
            "uint256"
        ],
        [
            gravityId,
            methodName,
            txAmounts,
            txDestinations,
            txFees,
            batchNonce,
            eventArgs._tokenContract,
            batchTimeout
        ]
    );
    let digest = ethers.utils.keccak256(abiEncoded);
    let sigs = await signHash(validators, digest);
    let currentValsetNonce = 0;

    let valset = {
        validators: await getSignerAddresses(validators),
        powers,
        valsetNonce: currentValsetNonce,
        rewardAmount: 0,
        rewardToken: ZeroAddress
    }

    await gravity.submitBatch(
        valset,

        sigs,

        txAmounts,
        txDestinations,
        txFees,
        batchNonce,
        eventArgs._tokenContract,
        batchTimeout
    );

    // Check that Gravity's balance is correct
    expect((await ERC20contract.functions.balanceOf(gravity.address)).toString()).to.equal(maxUint256.sub(200).toString())

    // Check that one of the recipient's balance is correct
    expect((await ERC20contract.functions.balanceOf(await signers[6].getAddress())).toString()).to.equal('1')
}

describe("deployERC20 tests", function () {

    // Non-duplicate & sorted validators must work
    it("runs with non-duplicate, sorted validators", async function () {
        await runTest({sortValidators: true})
    });

    // Non-duplicate yet unsorted validators must fail
    it("throws MalformedNewValidatorSet on non-duplicate, unsorted validators", async function () {
        await expect(runTest({})).to.be.revertedWith(
            "MalformedNewValidatorSet()"
        );
    });

    // Duplicate yet unsorted validators must fail
    it("throws MalformedNewValidatorSet on duplicate, unsorted validators", async function () {
        await expect(runTest({duplicateValidator: true})).to.be.revertedWith(
            "MalformedNewValidatorSet()"
        );
    });

    // Duplicate validators and already sorted must fail
    it("throws MalformedNewValidatorSet on duplicate, sorted validators", async function () {
        await expect(runTest({duplicateValidator: true, sortValidators: true})).to.be.revertedWith(
            "MalformedNewValidatorSet()"
        );
    });
});
