package keeper

import (
	"encoding/binary"
	"fmt"
	"sort"
	"strconv"

	sdk "github.com/cosmos/cosmos-sdk/types"
	sdkerrors "github.com/cosmos/cosmos-sdk/types/errors"

	"github.com/onomyprotocol/cosmos-gravity-bridge/module/x/gravity/types"
)

// AddToOutgoingPool creates a transaction and adds it to the pool, returns the id of the unbatched transaction
// - locks amount and fees in gravity module
// - persists an OutgoingTx
// - adds the TX to the `available` TX pool
func (k Keeper) AddToOutgoingPool(
	ctx sdk.Context,
	sender sdk.AccAddress,
	counterpartReceiver types.EthAddress,
	amount sdk.Coin,
	fee sdk.Coin,
) (uint64, error) {
	if ctx.IsZero() {
		return 0, sdkerrors.Wrap(types.ErrInvalid, "ctx")
	}
	if sdk.VerifyAddressFormat(sender) != nil {
		return 0, sdkerrors.Wrap(types.ErrInvalid, "sender")
	}
	if counterpartReceiver.ValidateBasic() != nil {
		return 0, sdkerrors.Wrap(types.ErrInvalid, "counterpartReceiver")
	}
	if !amount.IsValid() {
		return 0, sdkerrors.Wrap(types.ErrInvalid, "amount")
	}
	if !fee.IsValid() {
		return 0, sdkerrors.Wrap(types.ErrInvalid, "fee")
	}

	// FIXME try to remove and run tests
	batchFeeDenom := k.GetParams(ctx).BatchFeeDenom
	if batchFeeDenom != "" && fee.Denom != batchFeeDenom {
		return 0, sdkerrors.Wrap(types.ErrInvalid, fmt.Sprintf("It's allowed to pay fee in %q denom only", batchFeeDenom))
	}

	// lock coins in module
	totalLocked := sdk.NewCoins(amount).Add(fee)
	if err := k.bankKeeper.SendCoinsFromAccountToModule(ctx, sender, types.ModuleName, totalLocked); err != nil {
		return 0, err
	}

	// get next tx id from keeper
	nextID := k.autoIncrementID(ctx, []byte(types.KeyLastTXPoolID))

	_, tokenContract, err := k.DenomToERC20Lookup(ctx, amount.Denom)
	if err != nil {
		return 0, err
	}

	erc20Token, err := types.NewInternalERC20Token(amount.Amount, tokenContract.GetAddress())
	if err != nil {
		return 0, sdkerrors.Wrapf(err, "invalid ERC20Token from amount %d and contract %v",
			amount.Amount, tokenContract)
	}
	// construct outgoing tx, as part of this process we represent
	// the token as an ERC20 token since it is preparing to go to ETH
	// rather than the denom that is the input to this function.
	outgoing, err := types.OutgoingTransferTx{
		Id:          nextID,
		Sender:      sender.String(),
		DestAddress: counterpartReceiver.GetAddress(),
		Erc20Token:  erc20Token.ToExternal(),
		Fee:         fee,
	}.ToInternal()
	if err != nil { // This should never happen since all the components are validated
		panic(sdkerrors.Wrap(err, "unable to create InternalOutgoingTransferTx"))
	}

	// add a second index with the fee
	err = k.addUnbatchedTX(ctx, outgoing)
	if err != nil {
		panic(err)
	}

	poolEvent := sdk.NewEvent(
		types.EventTypeBridgeWithdrawalReceived,
		sdk.NewAttribute(sdk.AttributeKeyModule, types.ModuleName),
		sdk.NewAttribute(types.AttributeKeyContract, k.GetBridgeContractAddress(ctx).GetAddress()),
		sdk.NewAttribute(types.AttributeKeyBridgeChainID, strconv.Itoa(int(k.GetBridgeChainID(ctx)))),
		sdk.NewAttribute(types.AttributeKeyOutgoingTXID, strconv.Itoa(int(nextID))),
		sdk.NewAttribute(types.AttributeKeyNonce, fmt.Sprint(nextID)),
	)
	ctx.EventManager().EmitEvent(poolEvent)

	return nextID, nil
}

// RemoveFromOutgoingPoolAndRefund
// - checks that the provided tx actually exists
// - deletes the unbatched tx from the pool
// - issues the tokens back to the sender
func (k Keeper) RemoveFromOutgoingPoolAndRefund(ctx sdk.Context, txId uint64, sender sdk.AccAddress) error {
	if ctx.IsZero() || txId < 1 || sdk.VerifyAddressFormat(sender) != nil {
		return sdkerrors.Wrap(types.ErrInvalid, "arguments")
	}
	// check that we actually have a tx with that id and what it's details are
	tx, err := k.GetUnbatchedTxById(ctx, txId)
	if err != nil {
		return sdkerrors.Wrapf(err, "unknown transaction with id %d from sender %s", txId, sender.String())
	}

	// Check that this user actually sent the transaction, this prevents someone from refunding someone
	// elses transaction to themselves.
	if !tx.Sender.Equals(sender) {
		return sdkerrors.Wrapf(types.ErrInvalid, "Sender %s did not send Id %d", sender, txId)
	}

	// delete this tx from the pool
	err = k.removeUnbatchedTX(ctx, tx.Erc20Token.Contract.GetAddress(), tx.Fee.Amount, txId)
	if err != nil {
		return sdkerrors.Wrapf(types.ErrInvalid, "txId %d not in unbatched index! Must be in a batch!", txId)
	}
	// Make sure the tx was removed
	oldTx, oldTxErr := k.GetUnbatchedTxErc20TokenAndId(ctx, tx.Erc20Token.Contract.GetAddress(), tx.Fee.Amount, tx.Id)
	if oldTx != nil || oldTxErr == nil {
		return sdkerrors.Wrapf(types.ErrInvalid, "tx with id %d was not fully removed from the pool, a duplicate must exist", txId)
	}

	// Calculate refund
	totalToRefund := sdk.NewCoins(tx.Erc20Token.GravityCoin()).Add(*tx.Fee)

	// Perform refund
	if err = k.bankKeeper.SendCoinsFromModuleToAccount(ctx, types.ModuleName, sender, totalToRefund); err != nil {
		return sdkerrors.Wrap(err, "transfer vouchers")
	}

	poolEvent := sdk.NewEvent(
		types.EventTypeBridgeWithdrawCanceled,
		sdk.NewAttribute(sdk.AttributeKeyModule, types.ModuleName),
		sdk.NewAttribute(types.AttributeKeyContract, k.GetBridgeContractAddress(ctx).GetAddress()),
		sdk.NewAttribute(types.AttributeKeyBridgeChainID, strconv.Itoa(int(k.GetBridgeChainID(ctx)))),
	)
	ctx.EventManager().EmitEvent(poolEvent)

	return nil
}

// addUnbatchedTx creates a new transaction in the pool
// WARNING: Do not make this function public
func (k Keeper) addUnbatchedTX(ctx sdk.Context, val *types.InternalOutgoingTransferTx) error {
	store := ctx.KVStore(k.storeKey)
	idxKey := []byte(types.GetOutgoingTxPoolKey(val.Erc20Token.Contract.GetAddress(), val.Fee.Amount, val.Id))
	if store.Has(idxKey) {
		return sdkerrors.Wrap(types.ErrDuplicate, "transaction already in pool")
	}

	extVal := val.ToExternal()

	bz, err := k.cdc.Marshal(&extVal)
	if err != nil {
		return err
	}

	store.Set(idxKey, bz)
	return err
}

// removeUnbatchedTXIndex removes the tx from the pool
// WARNING: Do not make this function public
func (k Keeper) removeUnbatchedTX(ctx sdk.Context, erc20Address string, feeAmount sdk.Int, txID uint64) error {
	store := ctx.KVStore(k.storeKey)
	idxKey := []byte(types.GetOutgoingTxPoolKey(erc20Address, feeAmount, txID))
	if !store.Has(idxKey) {
		return sdkerrors.Wrap(types.ErrUnknown, "pool transaction")
	}
	store.Delete(idxKey)
	return nil
}

// GetUnbatchedTxErc20TokenAndId grabs a tx from the pool given its fee and txID
func (k Keeper) GetUnbatchedTxErc20TokenAndId(ctx sdk.Context, erc20Address string, feeAmount sdk.Int, txID uint64) (*types.InternalOutgoingTransferTx, error) {
	store := ctx.KVStore(k.storeKey)
	bz := store.Get([]byte(types.GetOutgoingTxPoolKey(erc20Address, feeAmount, txID)))
	if bz == nil {
		return nil, sdkerrors.Wrap(types.ErrUnknown, "pool transaction")
	}
	var r types.OutgoingTransferTx
	err := k.cdc.Unmarshal(bz, &r)
	if err != nil {
		panic(sdkerrors.Wrapf(err, "invalid unbatched tx in store: %v", r))
	}
	intR, err := r.ToInternal()
	if err != nil {
		panic(sdkerrors.Wrapf(err, "invalid unbatched tx in store: %v", r))
	}
	return intR, nil
}

// GetUnbatchedTxById grabs a tx from the pool given only the txID
// note that due to the way unbatched txs are indexed, the GetUnbatchedTxErc20TokenAndId method is much faster
func (k Keeper) GetUnbatchedTxById(ctx sdk.Context, txID uint64) (*types.InternalOutgoingTransferTx, error) {
	var r *types.InternalOutgoingTransferTx = nil
	k.IterateUnbatchedTransactions(ctx, []byte(types.OutgoingTXPoolKey), func(_ []byte, tx *types.InternalOutgoingTransferTx) bool {
		if tx.Id == txID {
			r = tx
			return true
		}
		return false // iterating DESC, exit early
	})

	if r == nil {
		// We have no return tx, it was either batched or never existed
		return nil, sdkerrors.Wrap(types.ErrUnknown, "pool transaction")
	}
	return r, nil
}

// GetUnbatchedTransactionsByContract, grabs all unbatched transactions from the tx pool for the given contract
// unbatched transactions are sorted by fee amount in DESC order
func (k Keeper) GetUnbatchedTransactionsByContract(ctx sdk.Context, contractAddress types.EthAddress) []*types.InternalOutgoingTransferTx {
	return k.collectUnbatchedTransactions(ctx, []byte(types.GetOutgoingTxPoolContractPrefix(contractAddress)))
}

// GetPoolTransactions, grabs all transactions from the tx pool, useful for queries or genesis save/load
func (k Keeper) GetUnbatchedTransactions(ctx sdk.Context) []*types.InternalOutgoingTransferTx {
	return k.collectUnbatchedTransactions(ctx, []byte(types.OutgoingTXPoolKey))
}

// Aggregates all unbatched transactions in the store with a given prefix
func (k Keeper) collectUnbatchedTransactions(ctx sdk.Context, prefixKey []byte) (out []*types.InternalOutgoingTransferTx) {
	k.IterateUnbatchedTransactions(ctx, prefixKey, func(_ []byte, tx *types.InternalOutgoingTransferTx) bool {
		out = append(out, tx)
		return false
	})
	return
}

// IterateUnbatchedTransactionsByContract, iterates through unbatched transactions from the tx pool for the given contract
// unbatched transactions are sorted by fee amount in DESC order
func (k Keeper) IterateUnbatchedTransactionsByContract(ctx sdk.Context, contractAddress types.EthAddress, cb func(key []byte, tx *types.InternalOutgoingTransferTx) bool) {
	k.IterateUnbatchedTransactions(ctx, []byte(types.GetOutgoingTxPoolContractPrefix(contractAddress)), cb)
}

// IterateUnbatchedTransactions iterates through all unbatched transactions whose keys begin with prefixKey in DESC order
func (k Keeper) IterateUnbatchedTransactions(ctx sdk.Context, prefixKey []byte, cb func(key []byte, tx *types.InternalOutgoingTransferTx) bool) {
	prefixStore := ctx.KVStore(k.storeKey)
	iter := prefixStore.ReverseIterator(prefixRange(prefixKey))
	defer iter.Close()
	for ; iter.Valid(); iter.Next() {
		var transact types.OutgoingTransferTx
		k.cdc.MustUnmarshal(iter.Value(), &transact)
		intTx, err := transact.ToInternal()
		if err != nil {
			panic(sdkerrors.Wrapf(err, "invalid unbatched transaction in store: %v", transact))
		}
		// cb returns true to stop early
		if cb(iter.Key(), intTx) {
			break
		}
	}
}

// GetBatchFeeByTokenType gets the fee the next batch of a given token type would
// have if created right now. This info is both presented to relayers for the purpose of determining
// when to request batches and also used by the batch creation process to decide not to create
// a new batch (fees must be increasing)
func (k Keeper) GetBatchFeeByTokenType(ctx sdk.Context, tokenContractAddr types.EthAddress, maxElements uint) *types.BatchFees {
	batchFee := types.BatchFees{
		Token:     tokenContractAddr.GetAddress(),
		TotalFees: sdk.NewCoins(),
		TxCount:   0,
	}

	k.IterateUnbatchedTransactions(ctx, []byte(types.GetOutgoingTxPoolContractPrefix(tokenContractAddr)), func(_ []byte, tx *types.InternalOutgoingTransferTx) bool {
		if k.IsOnBlacklist(ctx, *tx.DestAddress) {
			return false
		}
		batchFee.TotalFees = batchFee.TotalFees.Add(*tx.Fee)
		batchFee.TxCount += 1

		return batchFee.TxCount == uint64(maxElements)
	})

	return &batchFee
}

// GetAllBatchFees creates a fee entry for every batch type currently in the store
// this can be used by relayers to determine what batch types are desireable to request
func (k Keeper) GetAllBatchFees(ctx sdk.Context, maxElements uint) (batchFees []types.BatchFees) {
	batchFeesMap := k.createBatchFees(ctx, maxElements)
	// create array of batchFees
	for _, batchFee := range batchFeesMap {
		batchFees = append(batchFees, batchFee)
	}

	// quick sort by token to make this function safe for use
	// in consensus computations
	sort.Slice(batchFees, func(i, j int) bool {
		return batchFees[i].Token < batchFees[j].Token
	})

	return batchFees
}

// createBatchFees iterates over the unbatched transaction pool and creates batch token fee map
// Implicitly creates batches with the highest potential fee because the transaction keys enforce an order which goes
// fee contract address -> fee amount -> transaction nonce
func (k Keeper) createBatchFees(ctx sdk.Context, maxElements uint) map[string]types.BatchFees {
	batchFeesMap := make(map[string]types.BatchFees)
	k.IterateUnbatchedTransactions(ctx, []byte(types.OutgoingTXPoolKey), func(_ []byte, tx *types.InternalOutgoingTransferTx) bool {
		token := tx.Erc20Token.Contract.GetAddress()
		batchFee, ok := batchFeesMap[token]
		if !ok {
			batchFee = types.BatchFees{
				Token:     token,
				TotalFees: sdk.NewCoins(),
				TxCount:   0,
			}
		}

		// we should iterate the entire store in order to get all possible fee denoms
		if batchFee.TxCount < uint64(maxElements) {
			batchFee.TotalFees = batchFee.TotalFees.Add(*tx.Fee)
			batchFee.TxCount += 1
			batchFeesMap[token] = batchFee
		}

		return false
	})

	return batchFeesMap
}

// a specialized function used for iterating store counters, handling
// returning, initializing and incrementing all at once. This is particularly
// used for the transaction pool and batch pool where each batch or transaction is
// assigned a unique ID.
func (k Keeper) autoIncrementID(ctx sdk.Context, idKey []byte) uint64 {
	id := k.getID(ctx, idKey)
	id += 1
	k.setID(ctx, id, idKey)
	return id
}

// gets a generic uint64 counter from the store, initializing to 1 if no value exists
func (k Keeper) getID(ctx sdk.Context, idKey []byte) uint64 {
	store := ctx.KVStore(k.storeKey)
	bz := store.Get(idKey)
	id := binary.BigEndian.Uint64(bz)
	return id
}

// sets a generic uint64 counter in the store
func (k Keeper) setID(ctx sdk.Context, id uint64, idKey []byte) {
	store := ctx.KVStore(k.storeKey)
	bz := sdk.Uint64ToBigEndian(id)
	store.Set(idKey, bz)
}
