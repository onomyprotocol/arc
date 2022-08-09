import chai from "chai";
import {ethers} from "hardhat";
import {solidity} from "ethereum-waffle";

import {deployContracts, sortValidators} from "../test-utils";
import {EmptyDenom, examplePowers, getSignerAddresses, makeCheckpoint, parseEvent, signHash,} from "../test-utils/pure";
import {BigNumber} from "ethers";

chai.use(solidity);
const {expect} = chai;

describe("Gravity happy path valset update + batch submit", function () {
    it("Happy path", async function () {

        // DEPLOY CONTRACTS
        // ================

        const signers = await ethers.getSigners();
        const gravityId = ethers.utils.formatBytes32String("foo");
        let validators = sortValidators(signers.slice(0, examplePowers().length))
        const cosmosAddress = "cosmos1zkl8g9vd62x0ykvwq4mdcaehydvwc8ylh6panp"

        const valset0 = {
            // This is the power distribution on the Cosmos hub as of 7/14/2020
            powers: examplePowers(),
            validators,
            valsetNonce: 0,
            rewardAmount: 0,
            rewardDenom: EmptyDenom
        }


        const {
            gravity,
            testERC20,
            checkpoint: deployCheckpoint
        } = await deployContracts(gravityId, valset0.validators, valset0.powers);


        // UDPATEVALSET
        // ============

        const valset1 = (() => {
            // Make new valset by modifying some powers
            let powers = examplePowers();
            powers[0] -= 3;
            powers[1] += 3;
            let validators = sortValidators(signers.slice(0, powers.length));

            return {
                powers: powers,
                validators: validators,
                valsetNonce: 1,
                rewardAmount: 0,
                rewardDenom: EmptyDenom
            }
        })()

        // redefine valset0 and 1 with strings for 'validators'
        const valset0_str = {
            powers: valset0.powers,
            validators: await getSignerAddresses(valset0.validators),
            valsetNonce: valset0.valsetNonce,
            rewardAmount: valset0.rewardAmount,
            rewardDenom: valset0.rewardDenom
        }
        const valset1_str = {
            powers: valset1.powers,
            validators: await getSignerAddresses(valset1.validators),
            valsetNonce: valset1.valsetNonce,
            rewardAmount: valset1.rewardAmount,
            rewardDenom: valset1.rewardDenom
        }

        const checkpoint1 = makeCheckpoint(
            valset1_str.validators,
            valset1_str.powers,
            valset1_str.valsetNonce,
            valset1_str.rewardAmount,
            valset1_str.rewardDenom,
            gravityId
        );

        let sigs1 = await signHash(valset0.validators, checkpoint1);

        await gravity.updateValset(
            valset1_str,
            valset0_str,
            sigs1,
            ""
        );

        expect((await gravity.functions.state_lastValsetCheckpoint())[0]).to.equal(checkpoint1);

        // SUBMITBATCH
        // ==========================

        // Transfer out to Cosmos, locking coins
        await testERC20.functions.approve(gravity.address, 1000);
        await gravity.functions.sendToCosmos(
            testERC20.address,
            ethers.utils.formatBytes32String("myCosmosAddress"),
            1000
        );

        // Transferring into ERC20 from Cosmos
        const numTxs = 100;
        const txDestinationsInt = new Array(numTxs);

        const txAmounts = new Array(numTxs);
        for (let i = 0; i < numTxs; i++) {
            txAmounts[i] = 1;
            txDestinationsInt[i] = signers[i + 5];
        }

        const txDestinations = await getSignerAddresses(txDestinationsInt);

        const batchNonce = 1
        const batchTimeout = 10000000

        const methodName = ethers.utils.formatBytes32String(
            "transactionBatch"
        );

        let abiEncoded = ethers.utils.defaultAbiCoder.encode(
            [
                "bytes32",
                "bytes32",
                "uint256[]",
                "address[]",
                "uint256",
                "address",
                "uint256"
            ],
            [
                gravityId,
                methodName,
                txAmounts,
                txDestinations,
                batchNonce,
                testERC20.address,
                batchTimeout
            ]
        );

        let digest = ethers.utils.keccak256(abiEncoded);

        let sigs = await signHash(valset1.validators, digest);

        const batchSubmitEventArgs = await parseEvent(gravity, gravity.submitBatch(
            valset1_str,
            sigs,
            txAmounts,
            txDestinations,
            batchNonce,
            testERC20.address,
            batchTimeout,
            cosmosAddress,
        ), numTxs)

        // check event content
        expect(batchSubmitEventArgs).to.deep.equal({
            _batchNonce: BigNumber.from(1),
            _token: testERC20.address,
            _eventNonce: BigNumber.from(4),
            _rewardRecipient: cosmosAddress
        })

        // check that the transfer was successful
        expect((await testERC20.functions.balanceOf(await signers[6].getAddress()))[0].toBigInt())
            .to.equal(BigInt(1));

    });
});
