package keeper

import (
	"bytes"
	"fmt"
	"sort"

	sdk "github.com/cosmos/cosmos-sdk/types"
	stakingkeeper "github.com/cosmos/cosmos-sdk/x/staking/keeper"
	"github.com/cosmos/cosmos-sdk/x/staking/types"
	abci "github.com/tendermint/tendermint/abci/types"

	consumerkeeper "github.com/cosmos/interchain-security/x/ccv/consumer/keeper"
)

// Implements ValidatorSet interface
var _ types.ValidatorSet = Keeper{}

// Implements DelegationSet interface
var _ types.DelegationSet = Keeper{}

// keeper of the staking store
type Keeper struct {
	// the embedded Cosmos SDK x/staking keeper
	*stakingkeeper.Keeper

	ConsumerKeeper *consumerkeeper.Keeper
}

// need this to avoid changing a lot of uses of `GetOperator`
type CustomValAddress struct {
	address sdk.ValAddress
	power   int64
}

func (v CustomValAddress) GetOperator() sdk.ValAddress {
	return v.address
}

func NewForwardingKeeper(sk *stakingkeeper.Keeper, ck *consumerkeeper.Keeper) Keeper {
	return Keeper{
		Keeper:         sk,
		ConsumerKeeper: ck,
	}
}

// Load the last total validator power.
func (k Keeper) GetLastTotalPower(ctx sdk.Context) sdk.Int {
	if k.ConsumerKeeper == nil {
		return k.Keeper.GetLastTotalPower(ctx)
	} else {
		vals := k.ConsumerKeeper.MustGetCurrentValidatorsAsABCIUpdates(ctx)
		total := sdk.ZeroInt()
		for _, v := range vals {
			total = total.AddRaw(v.Power)
		}
		return total
	}
}

func (k Keeper) SetLastTotalPower(ctx sdk.Context, power sdk.Int) {
	if k.ConsumerKeeper == nil {
		k.Keeper.SetLastTotalPower(ctx, power)
	} else {
		panic("tried to call SetLastTotalPower on a forwarding staking keeper")
	}
}

func (k Keeper) SetValidatorUpdates(ctx sdk.Context, valUpdates []abci.ValidatorUpdate) {
	if k.ConsumerKeeper == nil {
		k.Keeper.SetValidatorUpdates(ctx, valUpdates)
	} else {
		panic("tried to call SetValidatorUpdates on a forwarding staking keeper")
	}
}

// GetValidatorUpdates returns the ABCI validator power updates within the current block.
func (k Keeper) GetValidatorUpdates(ctx sdk.Context) []abci.ValidatorUpdate {
	if k.ConsumerKeeper == nil {
		return k.Keeper.GetValidatorUpdates(ctx)
	} else {
		return k.ConsumerKeeper.MustGetCurrentValidatorsAsABCIUpdates(ctx)
	}
}

// GetBondedValidatorsByPower cannot be supported because types.Validator could never be propery supported on a consumer chain, use the functions below instead

func (k Keeper) GetValidatorUpdatesSortedByPower(ctx sdk.Context) []abci.ValidatorUpdate {
	// MustGetCurrentValidatorsAsABCIUpdates and GetAllCCValidator are sorted by address, we need to resort by power
	validators := k.GetValidatorUpdates(ctx)
	sort.Slice(validators, func(i, j int) bool {
		return validators[i].Power > validators[j].Power
	})
	return validators
}

func (k Keeper) GetCCValidatorUpdatesSortedByPower(ctx sdk.Context) []CustomValAddress {
	if k.ConsumerKeeper == nil {
		validators := k.Keeper.GetBondedValidatorsByPower(ctx)
		mapped := make([]CustomValAddress, 0, len(validators))
		for _, v := range validators {
			mapped = append(mapped, CustomValAddress{address: v.GetOperator(), power: v.ConsensusPower(sdk.DefaultPowerReduction)})
		}
		return mapped
	} else {
		// MustGetCurrentValidatorsAsABCIUpdates and GetAllCCValidator are sorted by address, we need to resort by power
		tmp := k.ConsumerKeeper.GetAllCCValidator(ctx)
		sort.Slice(tmp, func(i, j int) bool {
			return tmp[i].Power > tmp[j].Power
		})
		mapped := make([]CustomValAddress, 0, len(tmp))
		for _, v := range tmp {
			mapped = append(mapped, CustomValAddress{address: v.GetAddress(), power: v.Power})
		}
		return mapped
	}
}

func (k Keeper) Slash(ctx sdk.Context, consAddr sdk.ConsAddress, infractionHeight int64, power int64, slashFactor sdk.Dec, infraction types.InfractionType) {
	if k.ConsumerKeeper == nil {
		k.Keeper.Slash(ctx, consAddr, infractionHeight, power, slashFactor, infraction)
	} else {
		k.ConsumerKeeper.Slash(ctx, consAddr, infractionHeight, power, slashFactor, infraction)
	}
}

// Note: `k.ConsumerKeeper.IsValidatorJailed()` has the correct outstanding slashing checking

// currently a no-op, but note to use `k.ConsumerKeeper.IsValidatorJailed()` in place of `IsJailed()`
func (k Keeper) Jail(ctx sdk.Context, consAddr sdk.ConsAddress) {
	if k.ConsumerKeeper == nil {
		k.Keeper.Jail(ctx, consAddr)
	} else {
		// The slash packets handle the provider slashing and jailing processes,
		// although TODO should we be doing something locally?
		fmt.Printf("Warning: Jail(...) on the forwarding staking module was called for consAddr %s", consAddr)
	}
}

// A replacement for `GetValidator`
func (k Keeper) GetCCValidator(ctx sdk.Context, addr sdk.ValAddress) (validator CustomValAddress, found bool) {
	if k.ConsumerKeeper == nil {
		validator, found := k.Keeper.GetValidator(ctx, addr)
		if found {
			ccvalidator := CustomValAddress{
				address: validator.GetOperator(),
				power:   validator.ConsensusPower(sdk.DefaultPowerReduction),
			}
			return ccvalidator, true
		} else {
			ccvalidator := CustomValAddress{}
			return ccvalidator, false
		}
	} else {
		validators := k.ConsumerKeeper.GetAllCCValidator(ctx)
		for _, v := range validators {
			if bytes.Equal(v.GetAddress(), addr.Bytes()) {
				/*pk, err := v.ConsPubKey()
				if err != nil {
					// This should never happen as the pubkey is assumed
					// to be stored correctly in ApplyCCValidatorChanges.
					panic(err)
				}
				tmPK, err := cryptocodec.ToTmProtoPublicKey(pk)
				if err != nil {
					// This should never happen as the pubkey is assumed
					// to be stored correctly in ApplyCCValidatorChanges.
					panic(err)
				}*/
				ccvalidator := CustomValAddress{
					address: addr.Bytes(),
					power:   v.Power,
				}
				return ccvalidator, true
			}
		}
		return CustomValAddress{}, false
	}
}

/*type Params {
	UnbondingTime,
	HistoricalEntries,
}*/

func (k Keeper) GetParams(ctx sdk.Context) types.Params {
	if k.ConsumerKeeper == nil {
		return k.Keeper.GetParams(ctx)
	} else {
		//return k.ConsumerKeeper.GetParams(ctx)
		panic("TODO")
	}
}

// for `ValidatorQueueIterator` it isn't possible to override with a branch that returns an empty iterator since we can't get an empty `sdk.Iterator` it seems, it has to be branched around where it is called

// Note: this does not have a well defined notion of
// "Returns zero if the operator was not a validator last block",
// but the Gravity module does not seem to check for this kind of case
// and just passes any zeroes through
func (k Keeper) GetLastValidatorPower(ctx sdk.Context, operator sdk.ValAddress) (power int64) {
	if k.ConsumerKeeper == nil {
		return k.Keeper.GetLastValidatorPower(ctx, operator)
	} else {
		validators := k.ConsumerKeeper.GetAllCCValidator(ctx)
		for _, v := range validators {
			if bytes.Equal(v.GetAddress(), operator.Bytes()) {
				return v.Power
			}
		}
		return 0
	}
}

// there are two places where `Validator` is called outside of tests, but fortunately we only need to check for existence and if the validator is bonded

func (k Keeper) DoesValidatorExist(ctx sdk.Context, address sdk.ValAddress) bool {
	if k.ConsumerKeeper == nil {
		val := k.Keeper.Validator(ctx, address)
		return val != nil
	} else {
		validators := k.ConsumerKeeper.GetAllCCValidator(ctx)
		for _, v := range validators {
			if bytes.Equal(v.GetAddress(), address.Bytes()) {
				return true
			}
		}
		return false
	}
}

// note: we don't differentiate a bonded state for the consumer case
func (k Keeper) DoesValidatorExistAndIsBonded(ctx sdk.Context, address sdk.ValAddress) bool {
	if k.ConsumerKeeper == nil {
		val := k.Keeper.Validator(ctx, address)
		return (val != nil) && val.IsBonded()
	} else {
		validators := k.ConsumerKeeper.GetAllCCValidator(ctx)
		for _, v := range validators {
			if bytes.Equal(v.GetAddress(), address.Bytes()) {
				return true
			}
		}
		return false
	}
}
