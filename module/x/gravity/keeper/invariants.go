package keeper

import (
	"fmt"

	sdk "github.com/cosmos/cosmos-sdk/types"

	"github.com/onomyprotocol/cosmos-gravity-bridge/module/x/gravity/types"
)

// ModuleBalanceInvariant checks that the module account's balance is equal to the balance of unbatched transactions and unobserved batches
// Note that the returned bool should be true if there is an error, e.g. an unexpected module balance
func ModuleBalanceInvariant(k Keeper) sdk.Invariant {
	return func(ctx sdk.Context) (string, bool) {
		modAcc := k.accountKeeper.GetModuleAddress(types.ModuleName)
		actualBalances := k.bankKeeper.GetAllBalances(ctx, modAcc)
		expectedBalances := sdk.NewCoins()

		// The module is given the balance of all unobserved batches
		k.IterateOutgoingTXBatches(ctx, func(_ []byte, batch types.InternalOutgoingTxBatch) bool {
			_, denom := k.ERC20ToDenomLookup(ctx, batch.TokenContract)
			for _, tx := range batch.Transactions {
				expectedBalances = expectedBalances.Add(sdk.NewCoin(denom, tx.Erc20Token.Amount))
				expectedBalances = expectedBalances.Add(*tx.Fee)
			}

			return false
		})
		// It is also given the balance of all unbatched txs in the pool
		k.IterateUnbatchedTransactions(ctx, []byte(types.OutgoingTXPoolKey), func(_ []byte, tx *types.InternalOutgoingTransferTx) bool {
			_, denom := k.ERC20ToDenomLookup(ctx, tx.Erc20Token.Contract)
			expectedBalances = expectedBalances.Add(sdk.NewCoin(denom, tx.Erc20Token.Amount))
			expectedBalances = expectedBalances.Add(*tx.Fee)

			return false
		})

		diff, _ := expectedBalances.SafeSub(actualBalances)
		if len(diff) != 0 {
			return fmt.Sprintf("Invalid balance invariant, exp: %s, actual: %s", expectedBalances.String(), actualBalances.String()), true
		}

		return "", false
	}
}
