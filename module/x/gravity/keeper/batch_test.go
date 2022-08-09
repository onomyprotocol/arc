package keeper

import (
	"fmt"
	"math/rand"
	"testing"
	"time"

	sdk "github.com/cosmos/cosmos-sdk/types"
	"github.com/stretchr/testify/require"

	"github.com/onomyprotocol/cosmos-gravity-bridge/module/x/gravity/types"
)

//nolint: exhaustivestruct
func TestBatchesTxsExecutionOrder(t *testing.T) {
	var (
		now = time.Now().UTC()

		cosmosSender, _    = sdk.AccAddressFromBech32("gravity1ahx7f8wyertuus9r20284ej0asrs085ceqtfnm")
		rewardRecipient, _ = sdk.AccAddressFromBech32("gravity16ahjkfqxpp6lvfy9fpfnfjg39xr96qet0l08hu")
		ethReceiver, _     = types.NewEthAddress("0xd041c41EA1bf0F006ADBb6d2c9ef9D425dE5eaD7")
		erc20Address, _    = types.NewEthAddress("0x429881672B9AE42b8EbA0E26cD9C73711b891Ca5") // Pickle
		erc20Token, _      = types.NewInternalERC20Token(sdk.NewInt(99999), erc20Address.GetAddress())

		cosmosSenderBalance = sdk.NewCoins(erc20Token.GravityCoin(), sdk.NewCoin(sdk.DefaultBondDenom, erc20Token.Amount))

		input = CreateTestEnv(t)
		ctx   = input.Context
	)

	// mint some voucher first
	require.NoError(t, input.BankKeeper.MintCoins(ctx, types.ModuleName, cosmosSenderBalance))
	// set senders balance
	input.AccountKeeper.NewAccountWithAddress(ctx, cosmosSender)
	require.NoError(t, input.BankKeeper.SendCoinsFromModuleToAccount(ctx, types.ModuleName, cosmosSender, cosmosSenderBalance))

	// CREATE FIRST BATCH
	// ==================

	// add some TX to the pool
	outgoingTransferTx := make(map[uint64]types.OutgoingTransferTx, 0)
	outgoingTransferTxInternal := make(map[uint64]*types.InternalOutgoingTransferTx, 0)
	for i, fee := range []uint64{2, 3, 2, 1} {
		addTxsToOutgoingPool(t, ctx, input, 100+i, fee, erc20Address, cosmosSender, ethReceiver, outgoingTransferTx, outgoingTransferTxInternal)
	}
	// Should create:
	// id: 1, amount: 100, fee is 2 stake
	// id: 2, amount: 101, fee is 3 stake
	// id: 3, amount: 102, fee is 2 stake
	// id: 4, amount: 103, fee is 1 stake

	// when
	ctx = ctx.WithBlockTime(now)

	// tx batch size is 2, so that some of them stay behind
	firstBatch, err := input.GravityKeeper.BuildOutgoingTXBatch(ctx, *erc20Address, 2)
	require.NoError(t, err)

	// then batch is persisted
	gotFirstBatch := input.GravityKeeper.GetOutgoingTXBatch(ctx, firstBatch.TokenContract, firstBatch.BatchNonce)
	require.NotNil(t, gotFirstBatch)

	// we expect 2 and 3 since we expect 2 highest fees
	expFirstBatch := &types.InternalOutgoingTxBatch{
		BatchNonce: 1,
		Transactions: []*types.InternalOutgoingTransferTx{
			outgoingTransferTxInternal[2],
			outgoingTransferTxInternal[3],
		},
		TokenContract: *erc20Address,
		Block:         1234567,
	}

	require.Equal(t, expFirstBatch, gotFirstBatch)

	confirmBatch(t, ctx, input, firstBatch)

	// verify that confirms are persisted
	firstBatchConfirms := input.GravityKeeper.GetBatchConfirmByNonceAndTokenContract(ctx, firstBatch.BatchNonce, firstBatch.TokenContract)
	require.Equal(t, len(OrchAddrs), len(firstBatchConfirms))

	// and verify remaining available Tx in the pool
	// Should still have 1: and 4: above
	gotUnbatchedTx := input.GravityKeeper.GetUnbatchedTransactionsByContract(ctx, *erc20Address)

	expUnbatchedTx := []*types.InternalOutgoingTransferTx{
		outgoingTransferTxInternal[1],
		outgoingTransferTxInternal[4],
	}
	require.Equal(t, expUnbatchedTx, gotUnbatchedTx)

	// CREATE SECOND, MORE PROFITABLE BATCH
	// ====================================

	// add some more TX to the pool to create a more profitable batch
	for i, fee := range []uint64{4, 5} {
		addTxsToOutgoingPool(t, ctx, input, 100+i, fee, erc20Address, cosmosSender, ethReceiver, outgoingTransferTx, outgoingTransferTxInternal)
	}
	// id: 5, amount: 100, fee is 4 stake
	// id: 6, amount: 101, fee is 5 stake

	// create the more profitable batch
	ctx = ctx.WithBlockTime(now)
	// tx batch size is 2, so that some of them stay behind
	secondBatch, err := input.GravityKeeper.BuildOutgoingTXBatch(ctx, *erc20Address, 2)
	require.NoError(t, err)

	// check that the more profitable batch has the right txs in it
	// Should only have 5: and 6: above
	expSecondBatch := &types.InternalOutgoingTxBatch{
		BatchNonce: 2,
		Transactions: []*types.InternalOutgoingTransferTx{
			outgoingTransferTxInternal[6],
			outgoingTransferTxInternal[5],
		},
		TokenContract: *erc20Address,
		Block:         1234567,
	}

	require.Equal(t, expSecondBatch, secondBatch)

	confirmBatch(t, ctx, input, secondBatch)

	// verify that confirms are persisted
	secondBatchConfirms := input.GravityKeeper.GetBatchConfirmByNonceAndTokenContract(ctx, secondBatch.BatchNonce, secondBatch.TokenContract)
	require.Equal(t, len(OrchAddrs), len(secondBatchConfirms))

	//check that last added batch is the one with the biggest nonce
	lastOutgoingBatch := input.GravityKeeper.GetLastOutgoingBatchByTokenType(ctx, *erc20Address)
	require.NotNil(t, lastOutgoingBatch)
	require.Equal(t, lastOutgoingBatch.BatchNonce, secondBatch.BatchNonce)

	// EXECUTE THE MORE PROFITABLE BATCH WITH THE REWARD RECIPIENT
	// =================================
	// Execute the batch
	input.GravityKeeper.OutgoingTxBatchExecuted(ctx, secondBatch.TokenContract, secondBatch.BatchNonce, rewardRecipient.String())

	// check batch has been deleted
	gotSecondBatch := input.GravityKeeper.GetOutgoingTXBatch(ctx, secondBatch.TokenContract, secondBatch.BatchNonce)
	require.Nil(t, gotSecondBatch)
	// check batch confirmations have been deleted
	secondBatchConfirms = input.GravityKeeper.GetBatchConfirmByNonceAndTokenContract(ctx, secondBatch.BatchNonce, secondBatch.TokenContract)
	require.Equal(t, 0, len(secondBatchConfirms))

	// check that txs from first batch have been freed
	gotUnbatchedTx = input.GravityKeeper.GetUnbatchedTransactionsByContract(ctx, *erc20Address)
	expUnbatchedTx = []*types.InternalOutgoingTransferTx{
		outgoingTransferTxInternal[2],
		outgoingTransferTxInternal[3],
		outgoingTransferTxInternal[1],
		outgoingTransferTxInternal[4],
	}
	require.Equal(t, expUnbatchedTx, gotUnbatchedTx)

	// check that first batch has been deleted
	gotFirstBatch = input.GravityKeeper.GetOutgoingTXBatch(ctx, firstBatch.TokenContract, firstBatch.BatchNonce)
	require.Nil(t, gotFirstBatch)
	// check that first batch confirmations have been deleted
	firstBatchConfirms = input.GravityKeeper.GetBatchConfirmByNonceAndTokenContract(ctx, firstBatch.BatchNonce, firstBatch.TokenContract)
	require.Equal(t, 0, len(firstBatchConfirms))

	// Check that the reward was received
	rewardRecipientBalance := input.BankKeeper.GetBalance(ctx, rewardRecipient, sdk.DefaultBondDenom).Amount
	require.Equal(t, secondBatch.ToExternal().GetFees().AmountOf(sdk.DefaultBondDenom), rewardRecipientBalance)
}

// tests that batches work with large token amounts, mostly a duplicate of the above
// tests but using much bigger numbers
//nolint: exhaustivestruct
func TestBatchesFullCoins(t *testing.T) {
	input := CreateTestEnv(t)
	ctx := input.Context
	var (
		now                  = time.Now().UTC()
		mySender, _          = sdk.AccAddressFromBech32("gravity1ahx7f8wyertuus9r20284ej0asrs085ceqtfnm")
		myRewardRecipient, _ = sdk.AccAddressFromBech32("gravity16ahjkfqxpp6lvfy9fpfnfjg39xr96qet0l08hu")
		myReceiver           = "0xd041c41EA1bf0F006ADBb6d2c9ef9D425dE5eaD7"
		receiverAddr, _      = types.NewEthAddress(myReceiver)
		myTokenContractAddr  = "0x429881672B9AE42b8EbA0E26cD9C73711b891Ca5"   // Pickle
		totalCoins, _        = sdk.NewIntFromString("1500000000000000000000") // 1,500 ETH worth
		oneEth, _            = sdk.NewIntFromString("1000000000000000000")
		token, err           = types.NewInternalERC20Token(totalCoins, myTokenContractAddr)
		testCoins            = sdk.NewCoins(token.GravityCoin()).Add(sdk.NewCoin(sdk.DefaultBondDenom, totalCoins))
	)
	require.NoError(t, err)
	tokenContract, err := types.NewEthAddress(myTokenContractAddr)
	require.NoError(t, err)

	// mint some voucher first
	require.NoError(t, input.BankKeeper.MintCoins(ctx, types.ModuleName, testCoins))
	// set senders balance
	input.AccountKeeper.NewAccountWithAddress(ctx, mySender)
	require.NoError(t, input.BankKeeper.SendCoinsFromModuleToAccount(ctx, types.ModuleName, mySender, testCoins))

	// CREATE FIRST BATCH
	// ==================

	// add some TX to the pool
	for _, v := range []uint64{20, 300, 25, 10} {
		vAsSDKInt := sdk.NewIntFromUint64(v)
		amountToken, err := types.NewInternalERC20Token(oneEth.Mul(vAsSDKInt), myTokenContractAddr)
		require.NoError(t, err)
		amount := amountToken.GravityCoin()
		fee := sdk.NewCoin(sdk.DefaultBondDenom, oneEth.Mul(vAsSDKInt))

		_, err = input.GravityKeeper.AddToOutgoingPool(ctx, mySender, *receiverAddr, amount, fee)
		require.NoError(t, err)
	}

	// when
	ctx = ctx.WithBlockTime(now)

	// tx batch size is 2, so that some of them stay behind
	firstBatch, err := input.GravityKeeper.BuildOutgoingTXBatch(ctx, *tokenContract, 2)
	require.NoError(t, err)

	// then batch is persisted
	gotFirstBatch := input.GravityKeeper.GetOutgoingTXBatch(ctx, firstBatch.TokenContract, firstBatch.BatchNonce)
	require.NotNil(t, gotFirstBatch)

	expFirstBatch := &types.OutgoingTxBatch{
		BatchNonce: 1,
		Transactions: []types.OutgoingTransferTx{
			{
				Id:          2,
				Fee:         sdk.NewCoin(sdk.DefaultBondDenom, oneEth.Mul(sdk.NewIntFromUint64(300))),
				Sender:      mySender.String(),
				DestAddress: myReceiver,
				Erc20Token:  types.NewSDKIntERC20Token(oneEth.Mul(sdk.NewIntFromUint64(300)), myTokenContractAddr),
			},
			{
				Id:          3,
				Fee:         sdk.NewCoin(sdk.DefaultBondDenom, oneEth.Mul(sdk.NewIntFromUint64(25))),
				Sender:      mySender.String(),
				DestAddress: myReceiver,
				Erc20Token:  types.NewSDKIntERC20Token(oneEth.Mul(sdk.NewIntFromUint64(25)), myTokenContractAddr),
			},
		},
		TokenContract: myTokenContractAddr,
		Block:         1234567,
	}
	require.Equal(t, expFirstBatch.BatchTimeout, gotFirstBatch.BatchTimeout)
	require.Equal(t, expFirstBatch.BatchNonce, gotFirstBatch.BatchNonce)
	require.Equal(t, expFirstBatch.Block, gotFirstBatch.Block)
	require.Equal(t, expFirstBatch.TokenContract, gotFirstBatch.TokenContract.GetAddress())
	require.Equal(t, len(expFirstBatch.Transactions), len(gotFirstBatch.Transactions))
	for i := 0; i < len(expFirstBatch.Transactions); i++ {
		require.Equal(t, expFirstBatch.Transactions[i], gotFirstBatch.Transactions[i].ToExternal())
	}

	// and verify remaining available Tx in the pool
	gotUnbatchedTx := input.GravityKeeper.GetUnbatchedTransactionsByContract(ctx, *tokenContract)
	twentyTok, _ := types.NewInternalERC20Token(oneEth.Mul(sdk.NewIntFromUint64(20)), myTokenContractAddr)
	twentyFeeTok := sdk.NewCoin(sdk.DefaultBondDenom, oneEth.Mul(sdk.NewIntFromUint64(20)))
	tenTok, _ := types.NewInternalERC20Token(oneEth.Mul(sdk.NewIntFromUint64(10)), myTokenContractAddr)
	tenFeeTok := sdk.NewCoin(sdk.DefaultBondDenom, oneEth.Mul(sdk.NewIntFromUint64(10)))

	expUnbatchedTx := []*types.InternalOutgoingTransferTx{
		{
			Id:          1,
			Fee:         &twentyFeeTok,
			Sender:      mySender,
			DestAddress: receiverAddr,
			Erc20Token:  twentyTok,
		},
		{
			Id:          4,
			Fee:         &tenFeeTok,
			Sender:      mySender,
			DestAddress: receiverAddr,
			Erc20Token:  tenTok,
		},
	}
	require.Equal(t, expUnbatchedTx, gotUnbatchedTx)

	// CREATE SECOND, MORE PROFITABLE BATCH
	// ====================================

	// add some more TX to the pool to create a more profitable batch
	for _, v := range []uint64{200, 150} {
		vAsSDKInt := sdk.NewIntFromUint64(v)
		amountToken, err := types.NewInternalERC20Token(oneEth.Mul(vAsSDKInt), myTokenContractAddr)
		require.NoError(t, err)
		amount := amountToken.GravityCoin()
		fee := sdk.NewCoin(sdk.DefaultBondDenom, oneEth.Mul(vAsSDKInt))

		_, err = input.GravityKeeper.AddToOutgoingPool(ctx, mySender, *receiverAddr, amount, fee)
		require.NoError(t, err)
	}

	// create the more profitable batch
	ctx = ctx.WithBlockTime(now)
	// tx batch size is 2, so that some of them stay behind
	secondBatch, err := input.GravityKeeper.BuildOutgoingTXBatch(ctx, *tokenContract, 2)
	require.NoError(t, err)

	// check that the more profitable batch has the right txs in it
	expSecondBatch := &types.OutgoingTxBatch{
		BatchNonce: 2,
		Transactions: []types.OutgoingTransferTx{
			{
				Id:          5,
				Fee:         sdk.NewCoin(sdk.DefaultBondDenom, oneEth.Mul(sdk.NewIntFromUint64(200))),
				Sender:      mySender.String(),
				DestAddress: myReceiver,
				Erc20Token:  types.NewSDKIntERC20Token(oneEth.Mul(sdk.NewIntFromUint64(200)), myTokenContractAddr),
			},
			{
				Id:          6,
				Fee:         sdk.NewCoin(sdk.DefaultBondDenom, oneEth.Mul(sdk.NewIntFromUint64(150))),
				Sender:      mySender.String(),
				DestAddress: myReceiver,
				Erc20Token:  types.NewSDKIntERC20Token(oneEth.Mul(sdk.NewIntFromUint64(150)), myTokenContractAddr),
			},
		},
		TokenContract: myTokenContractAddr,
		Block:         1234567,
	}

	require.Equal(t, expSecondBatch.BatchTimeout, secondBatch.BatchTimeout)
	require.Equal(t, expSecondBatch.BatchNonce, secondBatch.BatchNonce)
	require.Equal(t, expSecondBatch.Block, secondBatch.Block)
	require.Equal(t, expSecondBatch.TokenContract, secondBatch.TokenContract.GetAddress())
	require.Equal(t, len(expSecondBatch.Transactions), len(secondBatch.Transactions))
	for i := 0; i < len(expSecondBatch.Transactions); i++ {
		require.Equal(t, expSecondBatch.Transactions[i], secondBatch.Transactions[i].ToExternal())
	}

	// EXECUTE THE MORE PROFITABLE BATCH
	// =================================

	// Execute the batch
	input.GravityKeeper.OutgoingTxBatchExecuted(ctx, secondBatch.TokenContract, secondBatch.BatchNonce, myRewardRecipient.String())

	// received fee from the batch
	totalFeel := sdk.NewCoins()
	for i := range expSecondBatch.Transactions {
		totalFeel = totalFeel.Add(expSecondBatch.Transactions[i].Fee)
	}
	require.Equal(t, totalFeel, input.BankKeeper.GetAllBalances(ctx, myRewardRecipient))

	// check batch has been deleted
	gotSecondBatch := input.GravityKeeper.GetOutgoingTXBatch(ctx, secondBatch.TokenContract, secondBatch.BatchNonce)
	require.Nil(t, gotSecondBatch)

	// check that txs from first batch have been freed
	gotUnbatchedTx = input.GravityKeeper.GetUnbatchedTransactionsByContract(ctx, *tokenContract)
	threeHundredTok, _ := types.NewInternalERC20Token(oneEth.Mul(sdk.NewIntFromUint64(300)), myTokenContractAddr)
	threeHundredFeeTok := sdk.NewCoin(sdk.DefaultBondDenom, oneEth.Mul(sdk.NewIntFromUint64(300)))
	twentyFiveTok, _ := types.NewInternalERC20Token(oneEth.Mul(sdk.NewIntFromUint64(25)), myTokenContractAddr)
	twentyFiveFeeTok := sdk.NewCoin(sdk.DefaultBondDenom, oneEth.Mul(sdk.NewIntFromUint64(25)))
	expUnbatchedTx = []*types.InternalOutgoingTransferTx{
		{
			Id:          2,
			Fee:         &threeHundredFeeTok,
			Sender:      mySender,
			DestAddress: receiverAddr,
			Erc20Token:  threeHundredTok,
		},
		{
			Id:          3,
			Fee:         &twentyFiveFeeTok,
			Sender:      mySender,
			DestAddress: receiverAddr,
			Erc20Token:  twentyFiveTok,
		},
		{
			Id:          1,
			Fee:         &twentyFeeTok,
			Sender:      mySender,
			DestAddress: receiverAddr,
			Erc20Token:  twentyTok,
		},
		{
			Id:          4,
			Fee:         &tenFeeTok,
			Sender:      mySender,
			DestAddress: receiverAddr,
			Erc20Token:  tenTok,
		},
	}
	require.Equal(t, expUnbatchedTx, gotUnbatchedTx)

}

// TestManyBatches handles test cases around batch execution, specifically executing multiple batches
// out of sequential order, which is exactly what happens on the
//nolint: exhaustivestruct
func TestManyBatches(t *testing.T) {
	input := CreateTestEnv(t)
	ctx := input.Context
	var (
		now                = time.Now().UTC()
		mySender, _        = sdk.AccAddressFromBech32("gravity1ahx7f8wyertuus9r20284ej0asrs085ceqtfnm")
		myReceiver         = "0xd041c41EA1bf0F006ADBb6d2c9ef9D425dE5eaD7"
		tokenContractAddr1 = "0x429881672B9AE42b8EbA0E26cD9C73711b891Ca5"
		tokenContractAddr2 = "0xF815240800ddf3E0be80e0d848B13ecaa504BF37"
		tokenContractAddr3 = "0xd086dDA7BccEB70e35064f540d07E4baED142cB3"
		tokenContractAddr4 = "0x384981B9d133701c4bD445F77bF61C3d80e79D46"
		totalCoins, _      = sdk.NewIntFromString("1500000000000000000000000")
		oneEth, _          = sdk.NewIntFromString("1000000000000000000")
		token1, err1       = types.NewInternalERC20Token(totalCoins, tokenContractAddr1)
		token2, err2       = types.NewInternalERC20Token(totalCoins, tokenContractAddr2)
		token3, err3       = types.NewInternalERC20Token(totalCoins, tokenContractAddr3)
		token4, err4       = types.NewInternalERC20Token(totalCoins, tokenContractAddr4)
		testCoins          = sdk.NewCoins(
			token1.GravityCoin(),
			token2.GravityCoin(),
			token3.GravityCoin(),
			token4.GravityCoin(),
			sdk.NewCoin(sdk.DefaultBondDenom, totalCoins),
		)
	)
	require.NoError(t, err1)
	require.NoError(t, err2)
	require.NoError(t, err3)
	require.NoError(t, err4)
	receiver, err := types.NewEthAddress(myReceiver)
	require.NoError(t, err)

	// mint vouchers first
	require.NoError(t, input.BankKeeper.MintCoins(ctx, types.ModuleName, testCoins))
	// set senders balance
	input.AccountKeeper.NewAccountWithAddress(ctx, mySender)
	require.NoError(t, input.BankKeeper.SendCoinsFromModuleToAccount(ctx, types.ModuleName, mySender, testCoins))

	// CREATE FIRST BATCH
	// ==================

	tokens := [4]string{tokenContractAddr1, tokenContractAddr2, tokenContractAddr3, tokenContractAddr4}

	// when
	ctx = ctx.WithBlockTime(now)
	var batches []types.OutgoingTxBatch

	for _, contract := range tokens {
		contractAddr, err := types.NewEthAddress(contract)
		require.NoError(t, err)
		for v := 1; v < 500; v++ {
			vAsSDKInt := sdk.NewIntFromUint64(uint64(v))
			amountToken, err := types.NewInternalERC20Token(oneEth.Mul(vAsSDKInt), contract)
			require.NoError(t, err)
			amount := amountToken.GravityCoin()
			fee := sdk.NewCoin(sdk.DefaultBondDenom, oneEth.Mul(vAsSDKInt))

			_, err = input.GravityKeeper.AddToOutgoingPool(ctx, mySender, *receiver, amount, fee)
			require.NoError(t, err)
			//create batch after every 100 txs to be able to create more profitable batches
			if (v+1)%100 == 0 {
				batch, err := input.GravityKeeper.BuildOutgoingTXBatch(ctx, *contractAddr, 100)
				require.NoError(t, err)
				batches = append(batches, batch.ToExternal())
			}
		}
	}

	for _, batch := range batches {
		// then batch is persisted
		contractAddr, err := types.NewEthAddress(batch.TokenContract)
		require.NoError(t, err)
		gotBatch := input.GravityKeeper.GetOutgoingTXBatch(ctx, *contractAddr, batch.BatchNonce)
		require.NotNil(t, gotBatch)
	}

	// EXECUTE BOTH BATCHES
	// =================================

	// shuffle batches to simulate out of order execution on Ethereum
	rand.Seed(time.Now().UnixNano())
	rand.Shuffle(len(batches), func(i, j int) { batches[i], batches[j] = batches[j], batches[i] })

	// Execute the batches, if there are any problems OutgoingTxBatchExecuted will panic
	totalFees := sdk.NewCoins()
	for _, batch := range batches {
		contractAddr, err := types.NewEthAddress(batch.TokenContract)
		require.NoError(t, err)
		gotBatch := input.GravityKeeper.GetOutgoingTXBatch(ctx, *contractAddr, batch.BatchNonce)
		// we may have already deleted some of the batches in this list by executing later ones
		if gotBatch != nil {
			totalFees = totalFees.Add(gotBatch.ToExternal().GetFees()...)
			input.GravityKeeper.OutgoingTxBatchExecuted(ctx, *contractAddr, batch.BatchNonce, "")
		}
	}

	// the fee goes to community pool
	feePool, _ := input.DistKeeper.GetFeePool(ctx).CommunityPool.TruncateDecimal()
	require.Equal(t, totalFees, feePool)
}

//nolint: exhaustivestruct
func TestPoolTxRefund(t *testing.T) {
	input := CreateTestEnv(t)
	ctx := input.Context
	var (
		now                 = time.Now().UTC()
		mySender, _         = sdk.AccAddressFromBech32("gravity1ahx7f8wyertuus9r20284ej0asrs085ceqtfnm")
		notMySender, _      = sdk.AccAddressFromBech32("gravity1ahx7f8wyertuus9r20284ej0asrs085case3km")
		myReceiver          = "0xd041c41EA1bf0F006ADBb6d2c9ef9D425dE5eaD7"
		myTokenContractAddr = "0x429881672B9AE42b8EbA0E26cD9C73711b891Ca5" // Pickle
		token, err          = types.NewInternalERC20Token(sdk.NewInt(500), myTokenContractAddr)
		testCoins           = sdk.NewCoins(token.GravityCoin()).Add(sdk.NewCoin(sdk.DefaultBondDenom, token.Amount))
		denomToken, dErr    = types.NewInternalERC20Token(sdk.NewInt(500), myTokenContractAddr)
		myDenom             = denomToken.GravityCoin().Denom
	)
	require.NoError(t, err)
	require.NoError(t, dErr)
	contract, err := types.NewEthAddress(myTokenContractAddr)
	require.NoError(t, err)
	receiver, err := types.NewEthAddress(myReceiver)
	require.NoError(t, err)

	// mint some voucher first
	require.NoError(t, input.BankKeeper.MintCoins(ctx, types.ModuleName, testCoins))
	// set senders balance
	input.AccountKeeper.NewAccountWithAddress(ctx, mySender)
	require.NoError(t, input.BankKeeper.SendCoinsFromModuleToAccount(ctx, types.ModuleName, mySender, testCoins))

	// CREATE FIRST BATCH
	// ==================

	// add some TX to the pool
	for i, v := range []uint64{2, 3, 2, 1} {
		amountToken, err := types.NewInternalERC20Token(sdk.NewInt(int64(i+100)), myTokenContractAddr)
		require.NoError(t, err)
		amount := amountToken.GravityCoin()
		fee := sdk.NewInt64Coin(sdk.DefaultBondDenom, int64(v))

		_, err = input.GravityKeeper.AddToOutgoingPool(ctx, mySender, *receiver, amount, fee)
		require.NoError(t, err)
		// Should have created:
		// 1: amount 100, fee 2
		// 2: amount 101, fee 3
		// 3: amount 102, fee 2
		// 4: amount 103, fee 1
	}

	// when
	ctx = ctx.WithBlockTime(now)

	// tx batch size is 2, so that some of them stay behind
	// Should have 2: and 3: from above
	_, err = input.GravityKeeper.BuildOutgoingTXBatch(ctx, *contract, 2)
	require.NoError(t, err)

	// try to refund a tx that's in a batch
	err1 := input.GravityKeeper.RemoveFromOutgoingPoolAndRefund(ctx, 3, mySender)
	require.Error(t, err1)

	// try to refund somebody else's tx
	err2 := input.GravityKeeper.RemoveFromOutgoingPoolAndRefund(ctx, 4, notMySender)
	require.Error(t, err2)

	// try to refund a tx that's in the pool
	err3 := input.GravityKeeper.RemoveFromOutgoingPoolAndRefund(ctx, 4, mySender)
	require.NoError(t, err3)

	// make sure refund was issued
	balances := input.BankKeeper.GetAllBalances(ctx, mySender)
	require.Equal(t, sdk.NewInt(197), balances.AmountOf(myDenom))
	require.Equal(t, sdk.NewInt(493), balances.AmountOf(sdk.DefaultBondDenom))
}

//nolint: exhaustivestruct
func TestBatchesNotCreatedWhenBridgePaused(t *testing.T) {
	input := CreateTestEnv(t)
	ctx := input.Context

	// pause the bridge
	params := input.GravityKeeper.GetParams(ctx)
	params.BridgeActive = false
	input.GravityKeeper.SetParams(ctx, params)

	var (
		now                    = time.Now().UTC()
		mySender, _            = sdk.AccAddressFromBech32("gravity1ahx7f8wyertuus9r20284ej0asrs085ceqtfnm")
		myReceiver, _          = types.NewEthAddress("0xd041c41EA1bf0F006ADBb6d2c9ef9D425dE5eaD7")
		myTokenContractAddr, _ = types.NewEthAddress("0x429881672B9AE42b8EbA0E26cD9C73711b891Ca5") // Pickle
		token, err             = types.NewInternalERC20Token(sdk.NewInt(99999), myTokenContractAddr.GetAddress())
		testCoins              = sdk.NewCoins(token.GravityCoin()).Add(sdk.NewInt64Coin(sdk.DefaultBondDenom, 99999))
	)
	require.NoError(t, err)

	// mint some voucher first
	require.NoError(t, input.BankKeeper.MintCoins(ctx, types.ModuleName, testCoins))
	// set senders balance
	input.AccountKeeper.NewAccountWithAddress(ctx, mySender)
	require.NoError(t, input.BankKeeper.SendCoinsFromModuleToAccount(ctx, types.ModuleName, mySender, testCoins))

	// CREATE FIRST BATCH
	// ==================

	// add some TX to the pool
	for i, v := range []uint64{2, 3, 2, 1} {
		amountToken, err := types.NewInternalERC20Token(sdk.NewInt(int64(i+100)), myTokenContractAddr.GetAddress())
		require.NoError(t, err)
		amount := amountToken.GravityCoin()
		fee := sdk.NewInt64Coin(sdk.DefaultBondDenom, int64(v))

		_, err = input.GravityKeeper.AddToOutgoingPool(ctx, mySender, *myReceiver, amount, fee)
		require.NoError(t, err)
		ctx.Logger().Info(fmt.Sprintf("Created transaction %v with amount %v and fee %v", i, amount, fee))
		// Should create:
		// 1: tx amount is 100, fee is 2, id is 1
		// 2: tx amount is 101, fee is 3, id is 2
		// 3: tx amount is 102, fee is 2, id is 3
		// 4: tx amount is 103, fee is 1, id is 4
	}

	// when
	ctx = ctx.WithBlockTime(now)

	// tx batch size is 2, so that some of them stay behind
	_, err = input.GravityKeeper.BuildOutgoingTXBatch(ctx, *myTokenContractAddr, 2)
	require.Error(t, err)

	// then batch is persisted
	gotFirstBatch := input.GravityKeeper.GetOutgoingTXBatch(ctx, *myTokenContractAddr, 1)
	require.Nil(t, gotFirstBatch)

	// resume the bridge
	params.BridgeActive = true
	input.GravityKeeper.SetParams(ctx, params)

	// when
	ctx = ctx.WithBlockTime(now)

	// tx batch size is 2, so that some of them stay behind
	firstBatch, err := input.GravityKeeper.BuildOutgoingTXBatch(ctx, *myTokenContractAddr, 2)
	require.NoError(t, err)

	// then batch is persisted
	gotFirstBatch = input.GravityKeeper.GetOutgoingTXBatch(ctx, firstBatch.TokenContract, firstBatch.BatchNonce)
	require.NotNil(t, gotFirstBatch)
}

//nolint: exhaustivestruct
// test that tokens on the blacklist do not enter batches
func TestEthereumBlacklistBatches(t *testing.T) {
	input := CreateTestEnv(t)
	ctx := input.Context
	var (
		now                    = time.Now().UTC()
		mySender, _            = sdk.AccAddressFromBech32("gravity1ahx7f8wyertuus9r20284ej0asrs085ceqtfnm")
		myReceiver, _          = types.NewEthAddress("0xd041c41EA1bf0F006ADBb6d2c9ef9D425dE5eaD7")
		blacklistedReceiver, _ = types.NewEthAddress("0x4d16b9E4a27c3313440923fEfCd013178149A5bD")
		myTokenContractAddr, _ = types.NewEthAddress("0x429881672B9AE42b8EbA0E26cD9C73711b891Ca5") // Pickle
		token, err             = types.NewInternalERC20Token(sdk.NewInt(99999), myTokenContractAddr.GetAddress())
		testCoins              = sdk.NewCoins(token.GravityCoin()).Add(sdk.NewInt64Coin(sdk.DefaultBondDenom, 99999))
	)
	require.NoError(t, err)

	// add the blacklisted address to the blacklist
	params := input.GravityKeeper.GetParams(ctx)
	params.EthereumBlacklist = append(params.EthereumBlacklist, blacklistedReceiver.GetAddress())
	input.GravityKeeper.SetParams(ctx, params)

	// mint some voucher first
	require.NoError(t, input.BankKeeper.MintCoins(ctx, types.ModuleName, testCoins))
	// set senders balance
	input.AccountKeeper.NewAccountWithAddress(ctx, mySender)
	require.NoError(t, input.BankKeeper.SendCoinsFromModuleToAccount(ctx, types.ModuleName, mySender, testCoins))

	// CREATE FIRST BATCH
	// ==================

	// add some TX to the pool
	for i, v := range []uint64{2, 3, 2, 1, 5} {
		amountToken, err := types.NewInternalERC20Token(sdk.NewInt(int64(i+100)), myTokenContractAddr.GetAddress())
		require.NoError(t, err)
		amount := amountToken.GravityCoin()
		fee := sdk.NewInt64Coin(sdk.DefaultBondDenom, int64(v))

		// one of the transactions should go to the blacklisted address
		if i == 4 {
			_, err = input.GravityKeeper.AddToOutgoingPool(ctx, mySender, *blacklistedReceiver, amount, fee)
		} else {
			_, err = input.GravityKeeper.AddToOutgoingPool(ctx, mySender, *myReceiver, amount, fee)
		}
		require.NoError(t, err)
		ctx.Logger().Info(fmt.Sprintf("Created transaction %v with amount %v and fee %v", i, amount, fee))
		// Should create:
		// 1: tx amount is 100, fee is 2, id is 1
		// 2: tx amount is 101, fee is 3, id is 2
		// 3: tx amount is 102, fee is 2, id is 3
		// 4: tx amount is 103, fee is 1, id is 4
		// 5: tx amount is 104, fee is 5, id is 5
	}

	//check that blacklisted tx fee is not insluded in profitability calculation
	currentFees := input.GravityKeeper.GetBatchFeeByTokenType(ctx, *myTokenContractAddr, 10)
	require.NotNil(t, currentFees)
	require.Equal(t, sdk.NewInt(8), currentFees.TotalFees.AmountOf(sdk.DefaultBondDenom))

	// when
	ctx = ctx.WithBlockTime(now)

	// tx batch size is 10
	firstBatch, err := input.GravityKeeper.BuildOutgoingTXBatch(ctx, *myTokenContractAddr, 10)
	require.NoError(t, err)

	// then batch is persisted
	gotFirstBatch := input.GravityKeeper.GetOutgoingTXBatch(ctx, firstBatch.TokenContract, firstBatch.BatchNonce)
	require.NotNil(t, gotFirstBatch)
	// Should have all from above except the banned dest
	ctx.Logger().Info(fmt.Sprintf("found batch %+v", gotFirstBatch))

	// should be 4 not 5 transactions
	require.Equal(t, 4, len(gotFirstBatch.Transactions))
	// should not contain id 5
	for i := 0; i < len(gotFirstBatch.Transactions); i++ {
		require.NotEqual(t, gotFirstBatch.Transactions[i].Id, 5)
	}

	// and verify remaining available Tx in the pool
	// should only be 5
	gotUnbatchedTx := input.GravityKeeper.GetUnbatchedTransactionsByContract(ctx, *myTokenContractAddr)
	require.Equal(t, gotUnbatchedTx[0].Id, uint64(5))

}

//tests total batch fee collected from all of the txs in the batch
func TestGetFees(t *testing.T) {

	txs := []types.OutgoingTransferTx{
		{Fee: sdk.NewInt64Coin(sdk.DefaultBondDenom, 1)},
		{Fee: sdk.NewInt64Coin(sdk.DefaultBondDenom, 2)},
		{Fee: sdk.NewInt64Coin(sdk.DefaultBondDenom, 3)},
	}

	type batchFeesTuple struct {
		batch        types.OutgoingTxBatch
		expectedFees sdk.Int
	}

	batches := []batchFeesTuple{
		{types.OutgoingTxBatch{
			Transactions: []types.OutgoingTransferTx{}},
			sdk.NewInt(0),
		},
		{types.OutgoingTxBatch{
			Transactions: []types.OutgoingTransferTx{txs[0]}},
			sdk.NewInt(1),
		},
		{types.OutgoingTxBatch{
			Transactions: []types.OutgoingTransferTx{txs[0], txs[1], txs[2]}},
			sdk.NewInt(6),
		},
	}

	for _, val := range batches {
		if !val.batch.GetFees().AmountOf(sdk.DefaultBondDenom).Equal(val.expectedFees) {
			t.Errorf("Invalid total batch fees!")
		}
	}
}

func addTxsToOutgoingPool(
	t *testing.T,
	ctx sdk.Context,
	input TestInput,
	txAmount int,
	feeAmount uint64,
	tokenContractAddr *types.EthAddress,
	sender sdk.AccAddress,
	receiver *types.EthAddress,
	outgoingTransferTx map[uint64]types.OutgoingTransferTx,
	outgoingTransferTxInternal map[uint64]*types.InternalOutgoingTransferTx,
) {
	amountToken, err := types.NewInternalERC20Token(sdk.NewInt(int64(txAmount)), tokenContractAddr.GetAddress())
	require.NoError(t, err)
	fee := sdk.NewCoin(sdk.DefaultBondDenom, sdk.NewInt(int64(feeAmount)))

	_, err = input.GravityKeeper.AddToOutgoingPool(ctx, sender, *receiver, amountToken.GravityCoin(), fee)
	require.NoError(t, err)
	id := len(outgoingTransferTx) + 1
	ctx.Logger().Info(fmt.Sprintf("Created transaction %v with txAmount %v and fee %v", id, txAmount, fee))

	tx := types.OutgoingTransferTx{
		Id:          uint64(id),
		Fee:         fee,
		Sender:      sender.String(),
		DestAddress: receiver.GetAddress(),
		Erc20Token:  amountToken.ToExternal(),
	}
	outgoingTransferTx[tx.Id] = tx
	txInternal, err := tx.ToInternal()
	require.NoError(t, err)
	outgoingTransferTxInternal[tx.Id] = txInternal
}

func confirmBatch(t *testing.T, ctx sdk.Context, input TestInput, batch *types.InternalOutgoingTxBatch) {
	for i, orch := range OrchAddrs {
		ethAddr, err := types.NewEthAddress(EthAddrs[i].String())
		require.NoError(t, err)

		conf := &types.MsgConfirmBatch{
			Nonce:         batch.BatchNonce,
			TokenContract: batch.TokenContract.GetAddress(),
			EthSigner:     ethAddr.GetAddress(),
			Orchestrator:  orch.String(),
			Signature:     "dummysig",
		}

		input.GravityKeeper.SetBatchConfirm(ctx, conf)
	}
}
