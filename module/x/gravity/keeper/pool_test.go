package keeper

import (
	"fmt"
	"testing"

	sdk "github.com/cosmos/cosmos-sdk/types"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/onomyprotocol/cosmos-gravity-bridge/module/x/gravity/types"
)

// Tests that the pool is populated with the created transactions before any batch is created
func TestAddToOutgoingPool(t *testing.T) {
	input := CreateTestEnv(t)
	ctx := input.Context
	var (
		mySender, _         = sdk.AccAddressFromBech32("gravity1ahx7f8wyertuus9r20284ej0asrs085ceqtfnm")
		myReceiver          = "0xd041c41EA1bf0F006ADBb6d2c9ef9D425dE5eaD7"
		myTokenContractAddr = "0x429881672B9AE42b8EbA0E26cD9C73711b891Ca5"
	)
	receiver, err := types.NewEthAddress(myReceiver)
	require.NoError(t, err)
	tokenContract, err := types.NewEthAddress(myTokenContractAddr)
	require.NoError(t, err)
	// mint some voucher first
	allVouchersToken, err := types.NewInternalERC20Token(sdk.NewInt(99999), myTokenContractAddr)
	require.NoError(t, err)
	allVouchers := sdk.Coins{allVouchersToken.GravityCoin()}
	allCoins := allVouchers.Add(sdk.NewInt64Coin(sdk.DefaultBondDenom, 10000))
	err = input.BankKeeper.MintCoins(ctx, types.ModuleName, allCoins)
	require.NoError(t, err)

	// set senders balance
	input.AccountKeeper.NewAccountWithAddress(ctx, mySender)
	err = input.BankKeeper.SendCoinsFromModuleToAccount(ctx, types.ModuleName, mySender, allCoins)
	require.NoError(t, err)

	// when
	for i, v := range []uint64{2, 3, 2, 1} {
		amountToken, err := types.NewInternalERC20Token(sdk.NewInt(int64(i+100)), myTokenContractAddr)
		require.NoError(t, err)
		amount := amountToken.GravityCoin()
		fee := sdk.NewInt64Coin(sdk.DefaultBondDenom, int64(v))

		r, err := input.GravityKeeper.AddToOutgoingPool(ctx, mySender, *receiver, amount, fee)
		require.NoError(t, err)
		t.Logf("___ response: %#v", r)
		// Should create:
		// 1: amount 100, fee 2
		// 2: amount 101, fee 3
		// 3: amount 102, fee 2
		// 4: amount 103, fee 1
	}
	// then
	got := input.GravityKeeper.GetUnbatchedTransactionsByContract(ctx, *tokenContract)

	receiverAddr, _ := types.NewEthAddress(myReceiver)
	oneTokFee := sdk.NewInt64Coin(sdk.DefaultBondDenom, 1)
	twoTokFee := sdk.NewInt64Coin(sdk.DefaultBondDenom, 2)
	threeTokFee := sdk.NewInt64Coin(sdk.DefaultBondDenom, 3)
	oneHundredTok, _ := types.NewInternalERC20Token(sdk.NewInt(100), myTokenContractAddr)
	oneHundredOneTok, _ := types.NewInternalERC20Token(sdk.NewInt(101), myTokenContractAddr)
	oneHundredTwoTok, _ := types.NewInternalERC20Token(sdk.NewInt(102), myTokenContractAddr)
	oneHundredThreeTok, _ := types.NewInternalERC20Token(sdk.NewInt(103), myTokenContractAddr)
	exp := []*types.InternalOutgoingTransferTx{
		{
			Id:          2,
			Fee:         &threeTokFee,
			Sender:      mySender,
			DestAddress: receiverAddr,
			Erc20Token:  oneHundredOneTok,
		},
		{
			Id:          3,
			Fee:         &twoTokFee,
			Sender:      mySender,
			DestAddress: receiverAddr,
			Erc20Token:  oneHundredTwoTok,
		},
		{
			Id:          1,
			Fee:         &twoTokFee,
			Sender:      mySender,
			DestAddress: receiverAddr,
			Erc20Token:  oneHundredTok,
		},
		{
			Id:          4,
			Fee:         &oneTokFee,
			Sender:      mySender,
			DestAddress: receiverAddr,
			Erc20Token:  oneHundredThreeTok,
		},
	}
	assert.Equal(t, exp, got)
}

// Checks some common edge cases like invalid inputs, user doesn't have enough tokens, token doesn't exist, inconsistent entry
func TestAddToOutgoingPoolEdgeCases(t *testing.T) {
	input := CreateTestEnv(t)
	ctx := input.Context
	var (
		mySender, _         = sdk.AccAddressFromBech32("gravity1ahx7f8wyertuus9r20284ej0asrs085ceqtfnm")
		myReceiver          = "0xd041c41EA1bf0F006ADBb6d2c9ef9D425dE5eaD7"
		myTokenContractAddr = "0x429881672B9AE42b8EbA0E26cD9C73711b891Ca5"
	)
	receiver, err := types.NewEthAddress(myReceiver)
	require.NoError(t, err)
	amountToken, err := types.NewInternalERC20Token(sdk.NewInt(int64(100)), myTokenContractAddr)
	require.NoError(t, err)
	amount := amountToken.GravityCoin()
	fee := sdk.NewInt64Coin(sdk.DefaultBondDenom, int64(2))

	//////// Nonexistant Token ////////
	r, err := input.GravityKeeper.AddToOutgoingPool(ctx, mySender, *receiver, amount, fee)
	require.Error(t, err)
	require.Zero(t, r)

	// mint some voucher first
	allVouchersToken, err := types.NewInternalERC20Token(sdk.NewInt(99999), myTokenContractAddr)
	require.NoError(t, err)
	allVouchers := sdk.Coins{allVouchersToken.GravityCoin()}
	err = input.BankKeeper.MintCoins(ctx, types.ModuleName, allVouchers)
	require.NoError(t, err)

	// set senders balance
	input.AccountKeeper.NewAccountWithAddress(ctx, mySender)
	err = input.BankKeeper.SendCoinsFromModuleToAccount(ctx, types.ModuleName, mySender, allVouchers)
	require.NoError(t, err)

	//////// Insufficient Balance from Amount ////////
	badAmountToken, err := types.NewInternalERC20Token(sdk.NewInt(999999), myTokenContractAddr)
	require.NoError(t, err)
	badAmount := badAmountToken.GravityCoin()
	r, err = input.GravityKeeper.AddToOutgoingPool(ctx, mySender, *receiver, badAmount, fee)
	require.Error(t, err)
	require.Zero(t, r)

	//////// Insufficient Balance from Fee ////////
	badFee := sdk.NewInt64Coin(sdk.DefaultBondDenom, int64(999999))
	r, err = input.GravityKeeper.AddToOutgoingPool(ctx, mySender, *receiver, amount, badFee)
	require.Error(t, err)
	require.Zero(t, r)

	//////// Insufficient Balance from Amount and Fee ////////
	// Amount is 100, fee is the current balance - 99
	badFee = sdk.NewInt64Coin(sdk.DefaultBondDenom, int64(999999-99))
	r, err = input.GravityKeeper.AddToOutgoingPool(ctx, mySender, *receiver, amount, badFee)
	require.Error(t, err)
	require.Zero(t, r)

	//////// Zero inputs ////////
	mtCtx := new(sdk.Context)
	mtSend := new(sdk.AccAddress)
	var mtRecieve = types.ZeroAddress() // This address should not actually cause an issue
	mtCoin := new(sdk.Coin)
	r, err = input.GravityKeeper.AddToOutgoingPool(*mtCtx, *mtSend, mtRecieve, *mtCoin, *mtCoin)
	require.Error(t, err)
	require.Zero(t, r)

	//////// Inconsistent Entry ////////
	badFee = sdk.NewInt64Coin("none-existing", int64(100))
	r, err = input.GravityKeeper.AddToOutgoingPool(ctx, mySender, *receiver, amount, badFee)
	require.Error(t, err)
	require.Zero(t, r)
}

func TestTotalBatchFeeInPool(t *testing.T) {
	input := CreateTestEnv(t)
	ctx := input.Context

	// token1
	var (
		mySender, _         = sdk.AccAddressFromBech32("gravity1ahx7f8wyertuus9r20284ej0asrs085ceqtfnm")
		myReceiver          = "0xd041c41EA1bf0F006ADBb6d2c9ef9D425dE5eaD7"
		myTokenContractAddr = "0x429881672B9AE42b8EbA0E26cD9C73711b891Ca5"
	)
	receiver, err := types.NewEthAddress(myReceiver)
	require.NoError(t, err)
	// mint some voucher first
	allVouchersToken, err := types.NewInternalERC20Token(sdk.NewInt(99999), myTokenContractAddr)
	require.NoError(t, err)
	allVouchers := sdk.Coins{allVouchersToken.GravityCoin()}
	allCoins := allVouchers.Add(sdk.NewInt64Coin(sdk.DefaultBondDenom, 99999))
	err = input.BankKeeper.MintCoins(ctx, types.ModuleName, allCoins)
	require.NoError(t, err)

	// set senders balance
	input.AccountKeeper.NewAccountWithAddress(ctx, mySender)
	err = input.BankKeeper.SendCoinsFromModuleToAccount(ctx, types.ModuleName, mySender, allCoins)
	require.NoError(t, err)

	// create outgoing pool
	for i, v := range []uint64{2, 3, 2, 1} {
		amountToken, err := types.NewInternalERC20Token(sdk.NewInt(int64(i+100)), myTokenContractAddr)
		require.NoError(t, err)
		amount := amountToken.GravityCoin()
		fee := sdk.NewInt64Coin(sdk.DefaultBondDenom, int64(v))

		r, err2 := input.GravityKeeper.AddToOutgoingPool(ctx, mySender, *receiver, amount, fee)
		require.NoError(t, err2)
		t.Logf("___ response: %#v", r)
	}

	// token 2 - Only top 100
	var (
		myToken2ContractAddr = "0x7D1AfA7B718fb893dB30A3aBc0Cfc608AaCfeBB0"
	)
	// mint some voucher first
	allVouchersToken, err = types.NewInternalERC20Token(sdk.NewIntFromUint64(uint64(18446744073709551615)), myToken2ContractAddr)
	require.NoError(t, err)
	allVouchers = sdk.Coins{allVouchersToken.GravityCoin()}
	err = input.BankKeeper.MintCoins(ctx, types.ModuleName, allVouchers)
	require.NoError(t, err)

	// set senders balance
	input.AccountKeeper.NewAccountWithAddress(ctx, mySender)
	err = input.BankKeeper.SendCoinsFromModuleToAccount(ctx, types.ModuleName, mySender, allVouchers)
	require.NoError(t, err)

	// Add

	// create outgoing pool
	for i := 0; i < 110; i++ {
		amountToken, err := types.NewInternalERC20Token(sdk.NewInt(int64(i+100)), myToken2ContractAddr)
		require.NoError(t, err)
		amount := amountToken.GravityCoin()
		fee := sdk.NewInt64Coin(sdk.DefaultBondDenom, int64(5))

		r, err := input.GravityKeeper.AddToOutgoingPool(ctx, mySender, *receiver, amount, fee)
		require.NoError(t, err)
		t.Logf("___ response: %#v", r)
	}

	batchFees := input.GravityKeeper.GetAllBatchFees(ctx, OutgoingTxBatchSize)
	/*
		tokenFeeMap should be
		map[0x429881672B9AE42b8EbA0E26cD9C73711b891Ca5:8 0x7D1AfA7B718fb893dB30A3aBc0Cfc608AaCfeBB0:500]
		**/
	assert.Equal(t, batchFees[0].TotalFees, sdk.NewCoins(sdk.NewInt64Coin(sdk.DefaultBondDenom, 8)))
	assert.Equal(t, batchFees[0].TxCount, uint64(4))
	assert.Equal(t, batchFees[1].TotalFees, sdk.NewCoins(sdk.NewInt64Coin(sdk.DefaultBondDenom, 500)))
	assert.Equal(t, batchFees[1].TxCount, uint64(100))
}

func TestGetBatchFeeByTokenType(t *testing.T) {
	input := CreateTestEnv(t)
	ctx := input.Context

	// token1
	var (
		mySender1, _                        = sdk.AccAddressFromBech32("gravity1ahx7f8wyertuus9r20284ej0asrs085ceqtfnm")
		mySender2            sdk.AccAddress = []byte("gravity1ahx7f8wyertus")
		mySender3            sdk.AccAddress = []byte("gravity1ahx7f8wyertut")
		myReceiver                          = "0xd041c41EA1bf0F006ADBb6d2c9ef9D425dE5eaD7"
		myTokenContractAddr1                = "0x429881672B9AE42b8EbA0E26cD9C73711b891Ca5"
		myTokenContractAddr2                = "0x429881672B9AE42b8EbA0E26cD9C73711b891Ca6"
		myTokenContractAddr3                = "0x429881672B9AE42b8EbA0E26cD9C73711b891Ca7"
	)
	receiver, err := types.NewEthAddress(myReceiver)
	require.NoError(t, err)
	tokenContract1, err := types.NewEthAddress(myTokenContractAddr1)
	require.NoError(t, err)
	tokenContract2, err := types.NewEthAddress(myTokenContractAddr2)
	require.NoError(t, err)
	tokenContract3, err := types.NewEthAddress(myTokenContractAddr3)
	require.NoError(t, err)
	// mint some vouchers first
	allVouchersToken1, err := types.NewInternalERC20Token(sdk.NewInt(99999), myTokenContractAddr1)
	require.NoError(t, err)
	allVouchers1 := sdk.Coins{allVouchersToken1.GravityCoin()}
	allCoins1 := allVouchers1.Add(sdk.NewInt64Coin(sdk.DefaultBondDenom, 99999))
	allVouchersToken2, err := types.NewInternalERC20Token(sdk.NewInt(99999), myTokenContractAddr2)
	require.NoError(t, err)
	allVouchers2 := sdk.Coins{allVouchersToken2.GravityCoin()}
	allCoins2 := allVouchers2.Add(sdk.NewInt64Coin(sdk.DefaultBondDenom, 99999))
	allVouchersToken3, err := types.NewInternalERC20Token(sdk.NewInt(99999), myTokenContractAddr3)
	require.NoError(t, err)
	allVouchers3 := sdk.Coins{allVouchersToken3.GravityCoin()}
	allCoins3 := allVouchers3.Add(sdk.NewInt64Coin(sdk.DefaultBondDenom, 99999))

	err = input.BankKeeper.MintCoins(ctx, types.ModuleName, allCoins1)
	require.NoError(t, err)
	err = input.BankKeeper.MintCoins(ctx, types.ModuleName, allCoins2)
	require.NoError(t, err)
	err = input.BankKeeper.MintCoins(ctx, types.ModuleName, allCoins3)
	require.NoError(t, err)

	// set senders balance
	input.AccountKeeper.NewAccountWithAddress(ctx, mySender1)
	err = input.BankKeeper.SendCoinsFromModuleToAccount(ctx, types.ModuleName, mySender1, allCoins1)
	require.NoError(t, err)
	input.AccountKeeper.NewAccountWithAddress(ctx, mySender2)
	err = input.BankKeeper.SendCoinsFromModuleToAccount(ctx, types.ModuleName, mySender2, allCoins2)
	require.NoError(t, err)
	input.AccountKeeper.NewAccountWithAddress(ctx, mySender3)
	err = input.BankKeeper.SendCoinsFromModuleToAccount(ctx, types.ModuleName, mySender3, allCoins3)
	require.NoError(t, err)

	totalFee1 := int64(0)
	totalFee2 := int64(0)
	totalFee3 := int64(0)
	// create outgoing pool
	for i := 0; i < 110; i++ {
		amountToken1, err := types.NewInternalERC20Token(sdk.NewInt(int64(i+100)), myTokenContractAddr1)
		require.NoError(t, err)
		amount1 := amountToken1.GravityCoin()
		feeAmt1 := int64(i + 1) // fees can't be 0
		fee1 := sdk.NewInt64Coin(sdk.DefaultBondDenom, feeAmt1)
		amountToken2, err := types.NewInternalERC20Token(sdk.NewInt(int64(i+100)), myTokenContractAddr2)
		require.NoError(t, err)
		amount2 := amountToken2.GravityCoin()
		feeAmt2 := int64(2*i + 1) // fees can't be 0
		fee2 := sdk.NewInt64Coin(sdk.DefaultBondDenom, feeAmt2)
		amountToken3, err := types.NewInternalERC20Token(sdk.NewInt(int64(i+100)), myTokenContractAddr3)
		require.NoError(t, err)
		amount3 := amountToken3.GravityCoin()
		feeAmt3 := int64(3*i + 1) // fees can't be 0
		fee3 := sdk.NewInt64Coin(sdk.DefaultBondDenom, feeAmt3)

		if i >= 10 {
			totalFee1 += feeAmt1
		}
		r, err := input.GravityKeeper.AddToOutgoingPool(ctx, mySender1, *receiver, amount1, fee1)
		require.NoError(t, err)
		t.Logf("___ response: %d", r)

		if i >= 10 {
			totalFee2 += feeAmt2
		}
		r, err = input.GravityKeeper.AddToOutgoingPool(ctx, mySender2, *receiver, amount2, fee2)
		require.NoError(t, err)
		t.Logf("___ response: %d", r)

		if i >= 10 {
			totalFee3 += feeAmt3
		}
		r, err = input.GravityKeeper.AddToOutgoingPool(ctx, mySender3, *receiver, amount3, fee3)
		require.NoError(t, err)
		t.Logf("___ response: %d", r)
	}

	batchFee1 := input.GravityKeeper.GetBatchFeeByTokenType(ctx, *tokenContract1, 100)
	require.Equal(t, batchFee1.Token, myTokenContractAddr1)
	require.Equal(t, batchFee1.TotalFees, sdk.NewCoins(sdk.NewInt64Coin(sdk.DefaultBondDenom, totalFee1)), fmt.Errorf("expected total fees %v but got %d", batchFee1.TotalFees, uint64(totalFee1)))
	require.Equal(t, batchFee1.TxCount, uint64(100), fmt.Errorf("expected tx count %d but got %d", batchFee1.TxCount, uint64(100)))
	batchFee2 := input.GravityKeeper.GetBatchFeeByTokenType(ctx, *tokenContract2, 100)
	require.Equal(t, batchFee2.Token, myTokenContractAddr2)
	require.Equal(t, batchFee2.TotalFees, sdk.NewCoins(sdk.NewInt64Coin(sdk.DefaultBondDenom, totalFee2)), fmt.Errorf("expected total fees %v but got %d", batchFee2.TotalFees, uint64(totalFee2)))
	require.Equal(t, batchFee2.TxCount, uint64(100), fmt.Errorf("expected tx count %d but got %d", batchFee2.TxCount, uint64(100)))
	batchFee3 := input.GravityKeeper.GetBatchFeeByTokenType(ctx, *tokenContract3, 100)
	require.Equal(t, batchFee3.Token, myTokenContractAddr3)
	require.Equal(t, batchFee3.TotalFees, sdk.NewCoins(sdk.NewInt64Coin(sdk.DefaultBondDenom, totalFee3)), fmt.Errorf("expected total fees %v but got %d", batchFee3.TotalFees, uint64(totalFee3)))
	require.Equal(t, batchFee3.TxCount, uint64(100), fmt.Errorf("expected tx count %d but got %d", batchFee3.TxCount, uint64(100)))
}

func TestRemoveFromOutgoingPoolAndRefund(t *testing.T) {
	input := CreateTestEnv(t)
	ctx := input.Context
	var (
		mySender, _         = sdk.AccAddressFromBech32("gravity1ahx7f8wyertuus9r20284ej0asrs085ceqtfnm")
		myReceiver          = "0xd041c41EA1bf0F006ADBb6d2c9ef9D425dE5eaD7"
		myTokenContractAddr = "0x429881672B9AE42b8EbA0E26cD9C73711b891Ca5"
		myTokenDenom        = "gravity" + myTokenContractAddr
	)
	receiver, err := types.NewEthAddress(myReceiver)
	require.NoError(t, err)
	// mint some voucher first
	originalBal := uint64(99999)

	allVouchersToken, err := types.NewInternalERC20Token(sdk.NewIntFromUint64(originalBal), myTokenContractAddr)
	require.NoError(t, err)
	allVouchers := sdk.Coins{allVouchersToken.GravityCoin()}
	allCoins := allVouchers.Add(sdk.NewInt64Coin(sdk.DefaultBondDenom, int64(originalBal)))
	err = input.BankKeeper.MintCoins(ctx, types.ModuleName, allCoins)
	require.NoError(t, err)

	// set senders balance
	input.AccountKeeper.NewAccountWithAddress(ctx, mySender)
	err = input.BankKeeper.SendCoinsFromModuleToAccount(ctx, types.ModuleName, mySender, allCoins)
	require.NoError(t, err)

	// Create unbatched transactions
	require.Empty(t, input.GravityKeeper.GetUnbatchedTransactions(ctx))
	spentAmounts := uint64(0)
	feesAmounts := uint64(0)
	ids := make([]uint64, 4)
	fees := []uint64{2, 3, 2, 1}
	amounts := []uint64{100, 101, 102, 103}
	for i, v := range fees {
		amountToken, err := types.NewInternalERC20Token(sdk.NewIntFromUint64(amounts[i]), myTokenContractAddr)
		require.NoError(t, err)
		amount := amountToken.GravityCoin()
		fee := sdk.NewInt64Coin(sdk.DefaultBondDenom, int64(v))
		spentAmounts += amounts[i]
		feesAmounts += v
		r, err := input.GravityKeeper.AddToOutgoingPool(ctx, mySender, *receiver, amount, fee)
		require.NoError(t, err)
		t.Logf("___ response: %#v", r)
		ids[i] = r
		// Should create:
		// 1: amount 100, fee 2
		// 2: amount 101, fee 3
		// 3: amount 102, fee 2
		// 4: amount 103, fee 1

	}
	// Check balance
	currentBals := input.BankKeeper.GetAllBalances(ctx, mySender)
	require.Equal(t, currentBals.AmountOf(myTokenDenom).Uint64(), originalBal-spentAmounts)
	require.Equal(t, currentBals.AmountOf(sdk.DefaultBondDenom).Uint64(), originalBal-feesAmounts)

	// Check that removing a transaction refunds the costs and the tx no longer exists in the pool
	checkRemovedTx(t, input, ctx, ids[2], fees[2], amounts[2], &spentAmounts, &feesAmounts, originalBal, mySender, myTokenContractAddr, myTokenDenom)
	checkRemovedTx(t, input, ctx, ids[3], fees[3], amounts[3], &spentAmounts, &feesAmounts, originalBal, mySender, myTokenContractAddr, myTokenDenom)
	checkRemovedTx(t, input, ctx, ids[1], fees[1], amounts[1], &spentAmounts, &feesAmounts, originalBal, mySender, myTokenContractAddr, myTokenDenom)
	checkRemovedTx(t, input, ctx, ids[0], fees[0], amounts[0], &spentAmounts, &feesAmounts, originalBal, mySender, myTokenContractAddr, myTokenDenom)
	require.Empty(t, input.GravityKeeper.GetUnbatchedTransactions(ctx))
}

// Helper method to:
// 1. Remove the transaction specified by `id`, `myTokenContractAddr` and `fee`
// 2. Update the spentAmounts tracker by subtracting the refunded `fee` and `amount`
// 3. Require that `mySender` has been refunded the correct amount for the cancelled transaction
// 4. Require that the unbatched transaction pool does not contain the refunded transaction via iterating its elements
func checkRemovedTx(t *testing.T, input TestInput, ctx sdk.Context, id uint64, fee uint64, amount uint64,
	spentAmounts *uint64, feesAmounts *uint64, originalBal uint64, mySender sdk.AccAddress, myTokenContractAddr string, myTokenDenom string) {
	err := input.GravityKeeper.RemoveFromOutgoingPoolAndRefund(ctx, id, mySender)
	require.NoError(t, err)

	*spentAmounts -= amount // user should have regained the locked amounts from tx
	*feesAmounts -= fee
	currentBals := input.BankKeeper.GetAllBalances(ctx, mySender)
	require.Equal(t, currentBals.AmountOf(myTokenDenom).Uint64(), originalBal-*spentAmounts)
	require.Equal(t, currentBals.AmountOf(sdk.DefaultBondDenom).Uint64(), originalBal-*feesAmounts)
	expectedKey := myTokenContractAddr + fmt.Sprint(fee) + fmt.Sprint(id)
	input.GravityKeeper.IterateUnbatchedTransactions(ctx, []byte(types.OutgoingTXPoolKey), func(key []byte, tx *types.InternalOutgoingTransferTx) bool {
		require.NotEqual(t, []byte(expectedKey), key)
		found := id == tx.Id &&
			fee == tx.Fee.Amount.Uint64() &&
			amount == tx.Erc20Token.Amount.Uint64()
		require.False(t, found)
		return false
	})
}

// ======================== Edge case tests for RemoveFromOutgoingPoolAndRefund() =================================== //

// Checks some common edge cases like invalid inputs, user didn't submit the transaction, tx doesn't exist, inconsistent entry
func TestRefundInconsistentTx(t *testing.T) {
	input := CreateTestEnv(t)
	ctx := input.Context
	var (
		mySender, _            = sdk.AccAddressFromBech32("gravity1ahx7f8wyertuus9r20284ej0asrs085ceqtfnm")
		myReceiver, _          = types.NewEthAddress("0xd041c41EA1bf0F006ADBb6d2c9ef9D425dE5eaD7")
		myTokenContractAddr, _ = types.NewEthAddress("0x429881672B9AE42b8EbA0E26cD9C73711b891Ca5")
	)

	//////// Refund an inconsistent tx ////////
	amountToken, err := types.NewInternalERC20Token(sdk.NewInt(100), myTokenContractAddr.GetAddress())
	require.NoError(t, err)
	badFeeToken := sdk.NewInt64Coin("bad", 100) // the fee deno is wrong
	// This way should fail
	r, err := input.GravityKeeper.AddToOutgoingPool(ctx, mySender, *myReceiver, amountToken.GravityCoin(), badFeeToken)
	require.Zero(t, r)
	require.Error(t, err)
	// But this unsafe override won't fail
	err = input.GravityKeeper.addUnbatchedTX(ctx, &types.InternalOutgoingTransferTx{
		Id:          uint64(5),
		Sender:      mySender,
		DestAddress: myReceiver,
		Erc20Token:  amountToken,
		Fee:         &badFeeToken,
	})
	origBalances := input.BankKeeper.GetAllBalances(ctx, mySender)
	require.NoError(t, err, "someone added validation to addUnbatchedTx")
	err = input.GravityKeeper.RemoveFromOutgoingPoolAndRefund(ctx, uint64(5), mySender)
	require.Error(t, err)
	newBalances := input.BankKeeper.GetAllBalances(ctx, mySender)
	require.Equal(t, origBalances, newBalances)
}

func TestRefundNonexistentTx(t *testing.T) {
	input := CreateTestEnv(t)
	ctx := input.Context
	var (
		mySender, _ = sdk.AccAddressFromBech32("gravity1ahx7f8wyertuus9r20284ej0asrs085ceqtfnm")
	)

	//////// Refund a tx which never existed ////////
	origBalances := input.BankKeeper.GetAllBalances(ctx, mySender)
	err := input.GravityKeeper.RemoveFromOutgoingPoolAndRefund(ctx, uint64(1), mySender)
	require.Error(t, err)
	newBalances := input.BankKeeper.GetAllBalances(ctx, mySender)
	require.Equal(t, origBalances, newBalances)
}

func TestRefundTwice(t *testing.T) {
	input := CreateTestEnv(t)
	ctx := input.Context
	var (
		mySender, _         = sdk.AccAddressFromBech32("gravity1ahx7f8wyertuus9r20284ej0asrs085ceqtfnm")
		myReceiver          = "0xd041c41EA1bf0F006ADBb6d2c9ef9D425dE5eaD7"
		myTokenContractAddr = "0x429881672B9AE42b8EbA0E26cD9C73711b891Ca5"
	)
	receiver, err := types.NewEthAddress(myReceiver)
	require.NoError(t, err)

	//////// Refund a tx twice ////////

	// mint some voucher first
	originalBal := uint64(99999)
	allVouchersToken, err := types.NewInternalERC20Token(sdk.NewIntFromUint64(originalBal), myTokenContractAddr)
	require.NoError(t, err)
	allVouchers := sdk.Coins{allVouchersToken.GravityCoin()}
	allCoins := allVouchers.Add(sdk.NewInt64Coin(sdk.DefaultBondDenom, int64(originalBal)))
	err = input.BankKeeper.MintCoins(ctx, types.ModuleName, allCoins)
	require.NoError(t, err)

	// set senders balance
	input.AccountKeeper.NewAccountWithAddress(ctx, mySender)
	err = input.BankKeeper.SendCoinsFromModuleToAccount(ctx, types.ModuleName, mySender, allCoins)
	require.NoError(t, err)

	amountToken, err := types.NewInternalERC20Token(sdk.NewInt(100), myTokenContractAddr)
	require.NoError(t, err)
	fee := sdk.NewInt64Coin(sdk.DefaultBondDenom, 2)
	origBalances := input.BankKeeper.GetAllBalances(ctx, mySender)

	txId, err := input.GravityKeeper.AddToOutgoingPool(ctx, mySender, *receiver, amountToken.GravityCoin(), fee)
	require.NoError(t, err)
	afterAddBalances := input.BankKeeper.GetAllBalances(ctx, mySender)

	// First refund goes through
	err = input.GravityKeeper.RemoveFromOutgoingPoolAndRefund(ctx, txId, mySender)
	require.NoError(t, err)
	afterRefundBalances := input.BankKeeper.GetAllBalances(ctx, mySender)

	// Second fails
	err = input.GravityKeeper.RemoveFromOutgoingPoolAndRefund(ctx, txId, mySender)
	require.Error(t, err)
	afterSecondRefundBalances := input.BankKeeper.GetAllBalances(ctx, mySender)

	require.NotEqual(t, origBalances, afterAddBalances)
	require.Equal(t, origBalances, afterRefundBalances)
	require.Equal(t, origBalances, afterSecondRefundBalances)
}

// Check the various getter methods for the pool
func TestGetUnbatchedTransactions(t *testing.T) {
	input := CreateTestEnv(t)
	ctx := input.Context

	// token1
	var (
		mySender1, _                        = sdk.AccAddressFromBech32("gravity1ahx7f8wyertuus9r20284ej0asrs085ceqtfnm")
		mySender2            sdk.AccAddress = []byte("gravity1ahx7f8wyertus")
		myReceiver                          = "0xd041c41EA1bf0F006ADBb6d2c9ef9D425dE5eaD7"
		myTokenContractAddr1                = "0x429881672B9AE42b8EbA0E26cD9C73711b891Ca5"
		myTokenContractAddr2                = "0x429881672B9AE42b8EbA0E26cD9C73711b891Ca6"
	)
	receiver, err := types.NewEthAddress(myReceiver)
	require.NoError(t, err)
	tokenContract1, err := types.NewEthAddress(myTokenContractAddr1)
	require.NoError(t, err)
	tokenContract2, err := types.NewEthAddress(myTokenContractAddr2)
	require.NoError(t, err)
	// mint some vouchers first
	allVouchersToken1, err := types.NewInternalERC20Token(sdk.NewInt(99999), myTokenContractAddr1)
	require.NoError(t, err)
	allVouchers1 := sdk.Coins{allVouchersToken1.GravityCoin()}
	allCoins1 := allVouchers1.Add(sdk.NewInt64Coin(sdk.DefaultBondDenom, 99999))
	err = input.BankKeeper.MintCoins(ctx, types.ModuleName, allCoins1)
	require.NoError(t, err)
	allVouchersToken2, err := types.NewInternalERC20Token(sdk.NewInt(99999), myTokenContractAddr2)
	require.NoError(t, err)
	allVouchers2 := sdk.Coins{allVouchersToken2.GravityCoin()}
	allCoins2 := allVouchers2.Add(sdk.NewInt64Coin(sdk.DefaultBondDenom, 99999))
	require.NoError(t, err)
	err = input.BankKeeper.MintCoins(ctx, types.ModuleName, allCoins2)
	require.NoError(t, err)

	// set senders balance
	input.AccountKeeper.NewAccountWithAddress(ctx, mySender1)
	err = input.BankKeeper.SendCoinsFromModuleToAccount(ctx, types.ModuleName, mySender1, allCoins1)
	require.NoError(t, err)
	input.AccountKeeper.NewAccountWithAddress(ctx, mySender2)
	err = input.BankKeeper.SendCoinsFromModuleToAccount(ctx, types.ModuleName, mySender2, allCoins2)
	require.NoError(t, err)

	ids1 := make([]uint64, 4)
	ids2 := make([]uint64, 4)
	fees := []uint64{2, 3, 2, 1}
	amounts := []uint64{100, 101, 102, 103}
	idToTxMap := make(map[uint64]*types.OutgoingTransferTx)
	for i, v := range fees {
		amountToken1, err := types.NewInternalERC20Token(sdk.NewIntFromUint64(amounts[i]), myTokenContractAddr1)
		require.NoError(t, err)
		amount1 := amountToken1.GravityCoin()
		fee1 := sdk.NewInt64Coin(sdk.DefaultBondDenom, int64(v))

		r, err := input.GravityKeeper.AddToOutgoingPool(ctx, mySender1, *receiver, amount1, fee1)
		require.NoError(t, err)
		ids1[i] = r
		idToTxMap[r] = &types.OutgoingTransferTx{
			Id:          r,
			Sender:      mySender1.String(),
			DestAddress: myReceiver,
			Erc20Token:  amountToken1.ToExternal(),
			Fee:         fee1,
		}
		amountToken2, err := types.NewInternalERC20Token(sdk.NewIntFromUint64(amounts[i]), myTokenContractAddr2)
		require.NoError(t, err)
		amount2 := amountToken2.GravityCoin()
		fee2 := sdk.NewInt64Coin(sdk.DefaultBondDenom, int64(v))

		r, err = input.GravityKeeper.AddToOutgoingPool(ctx, mySender2, *receiver, amount2, fee2)
		require.NoError(t, err)
		ids2[i] = r
		idToTxMap[r] = &types.OutgoingTransferTx{
			Id:          r,
			Sender:      mySender2.String(),
			DestAddress: myReceiver,
			Erc20Token:  amountToken2.ToExternal(),
			Fee:         fee2,
		}
	}

	// GetUnbatchedTxErc20TokenAndId
	token1Fee := sdk.NewInt64Coin(sdk.DefaultBondDenom, int64(fees[0]))
	token1Amount, err := types.NewInternalERC20Token(sdk.NewIntFromUint64(amounts[0]), myTokenContractAddr1)
	require.NoError(t, err)
	token1Id := ids1[0]
	tx1, err1 := input.GravityKeeper.GetUnbatchedTxErc20TokenAndId(ctx, myTokenContractAddr1, token1Fee.Amount, token1Id)
	require.NoError(t, err1)
	expTx1, err1 := types.NewInternalOutgoingTransferTx(token1Id, mySender1.String(), myReceiver, token1Amount.ToExternal(), token1Fee)
	require.NoError(t, err1)
	require.Equal(t, *expTx1, *tx1)

	token2Fee := sdk.NewInt64Coin(sdk.DefaultBondDenom, int64(fees[3]))
	require.NoError(t, err)
	token2Amount, err := types.NewInternalERC20Token(sdk.NewIntFromUint64(amounts[3]), myTokenContractAddr2)
	require.NoError(t, err)

	token2Id := ids2[3]
	tx2, err2 := input.GravityKeeper.GetUnbatchedTxErc20TokenAndId(ctx, myTokenContractAddr2, token2Fee.Amount, token2Id)
	require.NoError(t, err2)
	expTx2, err2 := types.NewInternalOutgoingTransferTx(token2Id, mySender2.String(), myReceiver, token2Amount.ToExternal(), token2Fee)
	require.NoError(t, err2)
	require.Equal(t, *expTx2, *tx2)

	// GetUnbatchedTxById
	tx1, err1 = input.GravityKeeper.GetUnbatchedTxById(ctx, token1Id)
	require.NoError(t, err1)
	require.Equal(t, *expTx1, *tx1)

	tx2, err2 = input.GravityKeeper.GetUnbatchedTxById(ctx, token2Id)
	require.NoError(t, err2)
	require.Equal(t, *expTx2, *tx2)

	// GetUnbatchedTransactionsByContract
	token1Txs := input.GravityKeeper.GetUnbatchedTransactionsByContract(ctx, *tokenContract1)
	for _, v := range token1Txs {
		expTx := idToTxMap[v.Id]
		require.NotNil(t, expTx)
		require.Equal(t, sdk.DefaultBondDenom, v.Fee.Denom)
		require.Equal(t, myTokenContractAddr1, v.Erc20Token.Contract.GetAddress())
		require.Equal(t, expTx.DestAddress, v.DestAddress.GetAddress())
		require.Equal(t, expTx.Sender, v.Sender.String())
	}
	token2Txs := input.GravityKeeper.GetUnbatchedTransactionsByContract(ctx, *tokenContract2)
	for _, v := range token2Txs {
		expTx := idToTxMap[v.Id]
		require.NotNil(t, expTx)
		require.Equal(t, sdk.DefaultBondDenom, v.Fee.Denom)
		require.Equal(t, myTokenContractAddr2, v.Erc20Token.Contract.GetAddress())
		require.Equal(t, expTx.DestAddress, v.DestAddress.GetAddress())
		require.Equal(t, expTx.Sender, v.Sender.String())
	}
	// GetUnbatchedTransactions
	allTxs := input.GravityKeeper.GetUnbatchedTransactions(ctx)
	for _, v := range allTxs {
		expTx := idToTxMap[v.Id]
		require.NotNil(t, expTx)
		require.Equal(t, expTx.DestAddress, v.DestAddress.GetAddress())
		require.Equal(t, expTx.Sender, v.Sender.String())
		require.Equal(t, sdk.DefaultBondDenom, v.Fee.Denom)
		require.Equal(t, expTx.Erc20Token.Contract, v.Erc20Token.Contract.GetAddress())
	}
}

// Check the various iteration methods for the pool
func TestIterateUnbatchedTransactions(t *testing.T) {
	input := CreateTestEnv(t)
	ctx := input.Context

	// token1
	var (
		mySender1, _                        = sdk.AccAddressFromBech32("gravity1ahx7f8wyertuus9r20284ej0asrs085ceqtfnm")
		mySender2            sdk.AccAddress = []byte("gravity1ahx7f8wyertus")
		myReceiver                          = "0xd041c41EA1bf0F006ADBb6d2c9ef9D425dE5eaD7"
		myTokenContractAddr1                = "0x429881672B9AE42b8EbA0E26cD9C73711b891Ca5"
		myTokenContractAddr2                = "0x429881672B9AE42b8EbA0E26cD9C73711b891Ca6"
	)
	receiver, err := types.NewEthAddress(myReceiver)
	require.NoError(t, err)
	tokenContract1, err := types.NewEthAddress(myTokenContractAddr1)
	require.NoError(t, err)
	tokenContract2, err := types.NewEthAddress(myTokenContractAddr2)
	require.NoError(t, err)
	// mint some vouchers first
	token1, err := types.NewInternalERC20Token(sdk.NewInt(99999), myTokenContractAddr1)
	require.NoError(t, err)
	allVouchers1 := sdk.Coins{token1.GravityCoin()}
	allCoins1 := allVouchers1.Add(sdk.NewInt64Coin(sdk.DefaultBondDenom, 10000))
	err = input.BankKeeper.MintCoins(ctx, types.ModuleName, allCoins1)
	require.NoError(t, err)

	token2, err := types.NewInternalERC20Token(sdk.NewInt(99999), myTokenContractAddr2)
	require.NoError(t, err)
	allVouchers2 := sdk.Coins{token2.GravityCoin()}
	allCoins2 := allVouchers2.Add(sdk.NewInt64Coin(sdk.DefaultBondDenom, 10000))
	err = input.BankKeeper.MintCoins(ctx, types.ModuleName, allCoins2)
	require.NoError(t, err)

	// set senders balance
	input.AccountKeeper.NewAccountWithAddress(ctx, mySender1)
	err = input.BankKeeper.SendCoinsFromModuleToAccount(ctx, types.ModuleName, mySender1, allCoins1)
	require.NoError(t, err)
	input.AccountKeeper.NewAccountWithAddress(ctx, mySender2)
	err = input.BankKeeper.SendCoinsFromModuleToAccount(ctx, types.ModuleName, mySender2, allCoins2)
	require.NoError(t, err)

	ids1 := make([]uint64, 4)
	ids2 := make([]uint64, 4)
	fees := []uint64{2, 3, 2, 1}
	amounts := []uint64{100, 101, 102, 103}
	idToTxMap := make(map[uint64]*types.OutgoingTransferTx)
	for i, v := range fees {
		amount1, err := types.NewInternalERC20Token(sdk.NewIntFromUint64(amounts[i]), myTokenContractAddr1)
		require.NoError(t, err)
		fee1 := sdk.NewInt64Coin(sdk.DefaultBondDenom, int64(v))
		r, err := input.GravityKeeper.AddToOutgoingPool(ctx, mySender1, *receiver, amount1.GravityCoin(), fee1)
		require.NoError(t, err)
		ids1[i] = r
		idToTxMap[r] = &types.OutgoingTransferTx{
			Id:          r,
			Sender:      mySender1.String(),
			DestAddress: myReceiver,
			Erc20Token:  amount1.ToExternal(),
			Fee:         fee1,
		}
		amount2, err := types.NewInternalERC20Token(sdk.NewIntFromUint64(amounts[i]), myTokenContractAddr2)
		require.NoError(t, err)
		fee2 := sdk.NewInt64Coin(sdk.DefaultBondDenom, int64(v))
		r, err = input.GravityKeeper.AddToOutgoingPool(ctx, mySender2, *receiver, amount2.GravityCoin(), fee2)
		require.NoError(t, err)

		ids2[i] = r
		idToTxMap[r] = &types.OutgoingTransferTx{
			Id:          r,
			Sender:      mySender2.String(),
			DestAddress: myReceiver,
			Erc20Token:  amount2.ToExternal(),
			Fee:         fee2,
		}
	}
	// IterateUnbatchedTransactionsByContract
	foundMap := make(map[uint64]bool)
	input.GravityKeeper.IterateUnbatchedTransactionsByContract(ctx, *tokenContract1, func(key []byte, tx *types.InternalOutgoingTransferTx) bool {
		require.NotNil(t, tx)
		fTx := idToTxMap[tx.Id]
		require.NotNil(t, fTx)
		require.Equal(t, fTx.Fee.Denom, sdk.DefaultBondDenom)
		require.Equal(t, fTx.Erc20Token.Contract, myTokenContractAddr1)
		require.Equal(t, fTx.DestAddress, myReceiver)
		require.Equal(t, mySender1.String(), fTx.Sender)
		foundMap[fTx.Id] = true
		return false
	})
	input.GravityKeeper.IterateUnbatchedTransactionsByContract(ctx, *tokenContract2, func(key []byte, tx *types.InternalOutgoingTransferTx) bool {
		require.NotNil(t, tx)
		fTx := idToTxMap[tx.Id]
		require.NotNil(t, fTx)
		require.Equal(t, fTx.Fee.Denom, sdk.DefaultBondDenom)
		require.Equal(t, fTx.Erc20Token.Contract, myTokenContractAddr2)
		require.Equal(t, fTx.DestAddress, myReceiver)
		require.Equal(t, mySender2.String(), fTx.Sender)
		foundMap[fTx.Id] = true
		return false
	})

	for i := 1; i <= 8; i++ {
		require.True(t, foundMap[uint64(i)])
	}
	// IterateUnbatchedTransactions
	anotherFoundMap := make(map[uint64]bool)
	input.GravityKeeper.IterateUnbatchedTransactions(ctx, []byte(types.OutgoingTXPoolKey), func(key []byte, tx *types.InternalOutgoingTransferTx) bool {
		require.NotNil(t, tx)
		fTx := idToTxMap[tx.Id]
		require.NotNil(t, fTx)
		require.Equal(t, fTx.DestAddress, tx.DestAddress.GetAddress())

		anotherFoundMap[fTx.Id] = true
		return false
	})

	for i := 1; i <= 8; i++ {
		require.True(t, anotherFoundMap[uint64(i)])
	}
}

// Ensures that any unbatched tx will make its way into the exported data from ExportGenesis
func TestAddToOutgoingPoolExportGenesis(t *testing.T) {
	input := CreateTestEnv(t)
	ctx := input.Context
	k := input.GravityKeeper
	var (
		mySender, _         = sdk.AccAddressFromBech32("gravity1ahx7f8wyertuus9r20284ej0asrs085ceqtfnm")
		myReceiver          = "0xd041c41EA1bf0F006ADBb6d2c9ef9D425dE5eaD7"
		myTokenContractAddr = "0x429881672B9AE42b8EbA0E26cD9C73711b891Ca5"
	)
	receiver, err := types.NewEthAddress(myReceiver)
	require.NoError(t, err)
	// mint some voucher first
	allVouchersToken, err := types.NewInternalERC20Token(sdk.NewInt(99999), myTokenContractAddr)
	require.NoError(t, err)
	allVouchers := sdk.Coins{allVouchersToken.GravityCoin()}
	allCoins := allVouchers.Add(sdk.NewInt64Coin(sdk.DefaultBondDenom, 10000))

	err = input.BankKeeper.MintCoins(ctx, types.ModuleName, allCoins)
	require.NoError(t, err)

	// set senders balance
	input.AccountKeeper.NewAccountWithAddress(ctx, mySender)
	err = input.BankKeeper.SendCoinsFromModuleToAccount(ctx, types.ModuleName, mySender, allCoins)
	require.NoError(t, err)

	unbatchedTxMap := make(map[uint64]types.OutgoingTransferTx)
	foundTxsMap := make(map[uint64]bool)
	// when
	for i, v := range []uint64{2, 3, 2, 1} {
		amountToken, err := types.NewInternalERC20Token(sdk.NewInt(int64(i+100)), myTokenContractAddr)
		require.NoError(t, err)
		amount := amountToken.GravityCoin()
		fee := sdk.NewInt64Coin(sdk.DefaultBondDenom, int64(v))

		r, err := input.GravityKeeper.AddToOutgoingPool(ctx, mySender, *receiver, amount, fee)
		require.NoError(t, err)

		unbatchedTxMap[r] = types.OutgoingTransferTx{
			Id:          r,
			Sender:      mySender.String(),
			DestAddress: myReceiver,
			Erc20Token:  amountToken.ToExternal(),
			Fee:         fee,
		}
		foundTxsMap[r] = false

	}
	// then
	got := ExportGenesis(ctx, k)
	require.NotNil(t, got)

	for _, tx := range got.UnbatchedTransfers {
		cached := unbatchedTxMap[tx.Id]
		require.NotNil(t, cached)
		require.Equal(t, cached, tx, "cached: %+v\nactual: %+v\n", cached, tx)
		foundTxsMap[tx.Id] = true
	}

	for _, v := range foundTxsMap {
		require.True(t, v)
	}
}
