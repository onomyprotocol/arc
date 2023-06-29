package keeper

import (
	"fmt"
	"sort"

	sdk "github.com/cosmos/cosmos-sdk/types"
	stakingkeeper "github.com/cosmos/cosmos-sdk/x/staking/keeper"
	"github.com/cosmos/cosmos-sdk/x/staking/types"

	consumerkeeper "github.com/cosmos/interchain-security/x/ccv/consumer/keeper"
	consumertypes "github.com/cosmos/interchain-security/x/ccv/consumer/types"
)

// TODO msgs.go and send.rs need to use ValCons

/*
GetBondedValidatorsByPower => GetAllCCValidatorsSortedByPower
GetLastValidatorPower => GetLastValconsPower
GetLastTotalPower => GetLastTotalPower
IterateValidators => used only by standalone export
ValidatorQueueIterator => branch around
GetParams => leave unchanged TODO check all parameter usages
GetValidator => GetCCValidator
Validator => GetCCValidator
Slash => simple override (just need to use correct infraction type)
Jail => simple override (just need to use correct infraction type)
*/

// TODO check equal power sorting vs what contract does

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

func NewForwardingKeeper(sk *stakingkeeper.Keeper, ck *consumerkeeper.Keeper) Keeper {
	return Keeper{
		Keeper:         sk,
		ConsumerKeeper: ck,
	}
}

func (k Keeper) GetConsAddrFromCCV(ccv consumertypes.CrossChainValidator) sdk.ConsAddress {
	pk, err := ccv.ConsPubKey()
	if err != nil {
		panic(err)
	}
	return sdk.GetConsAddress(pk)
}

// Note: this uses the default power reduction
func (k Keeper) GetAllCCValidators(ctx sdk.Context) []consumertypes.CrossChainValidator {
	if k.ConsumerKeeper == nil {
		validators := k.Keeper.GetAllValidators(ctx)
		mapped := make([]consumertypes.CrossChainValidator, 0, len(validators))
		for _, v := range validators {
			pk, err := v.ConsPubKey()
			if err != nil {
				panic(err)
			}
			// no power reduction
			ccv, err := consumertypes.NewCCValidator(pk.Address(), v.ConsensusPower(sdk.DefaultPowerReduction), pk)
			if err != nil {
				panic(err)
			}
			mapped = append(mapped, ccv)
		}
		return mapped
	} else {
		return k.ConsumerKeeper.GetAllCCValidator(ctx)
	}
}

func (k Keeper) GetAllCCValidatorsSortedByPower(ctx sdk.Context) []consumertypes.CrossChainValidator {
	ccvs := k.GetAllCCValidators(ctx)
	sort.Slice(ccvs, func(i, j int) bool {
		return ccvs[i].Power > ccvs[j].Power
	})
	return ccvs
}

func (k Keeper) GetAllValcons(ctx sdk.Context) []sdk.ConsAddress {
	if k.ConsumerKeeper == nil {
		validators := k.Keeper.GetAllValidators(ctx)
		mapped := make([]sdk.ConsAddress, 0, len(validators))
		for _, v := range validators {
			consAddr, err := v.GetConsAddr()
			if err != nil {
				panic(err)
			}
			mapped = append(mapped, consAddr)
		}
		return mapped
	} else {
		ccvalidators := k.ConsumerKeeper.GetAllCCValidator(ctx)
		mapped := make([]sdk.ConsAddress, 0, len(ccvalidators))
		for _, v := range ccvalidators {
			pk, err := v.ConsPubKey()
			if err != nil {
				panic(err)
			}
			consAddr := sdk.GetConsAddress(pk)
			mapped = append(mapped, consAddr)
		}
		return mapped
	}
}

func (k Keeper) GetNumberOfValcons(ctx sdk.Context) int {
	if k.ConsumerKeeper == nil {
		return len(k.Keeper.GetAllValidators(ctx))
	} else {
		return len(k.ConsumerKeeper.GetAllCCValidator(ctx))
	}
}

// note: this uses the default power reduction
func (k Keeper) GetCCValidator(ctx sdk.Context, consAddr sdk.ConsAddress) (consaddr consumertypes.CrossChainValidator, found bool) {
	if k.ConsumerKeeper == nil {
		val, found := k.Keeper.GetValidatorByConsAddr(ctx, consAddr)
		pk, err := val.ConsPubKey()
		if err != nil {
			panic(err)
		}
		// no power reduction
		ccv, err := consumertypes.NewCCValidator(consAddr.Bytes(), val.ConsensusPower(sdk.DefaultPowerReduction), pk)
		if err != nil {
			panic(err)
		}
		if found {
			return ccv, true
		} else {
			return consumertypes.CrossChainValidator{}, false
		}
	} else {
		ccvalidator, found := k.ConsumerKeeper.GetCCValidator(ctx, consAddr.Bytes())
		return ccvalidator, found
	}
}

// For the consumer case this checks if the validator has a corresponding ICS valcons address
func (k Keeper) DoesValconExist(ctx sdk.Context, consAddr sdk.ConsAddress) bool {
	_, found := k.GetCCValidator(ctx, consAddr)
	return found
}

// For the consumer case, this is the same as `DoesValidatorExist` because there isn't a separate notion of being bonded
func (k Keeper) DoesValconExistAndIsBonded(ctx sdk.Context, consAddr sdk.ConsAddress) bool {
	if k.ConsumerKeeper == nil {
		val, found := k.Keeper.GetValidatorByConsAddr(ctx, consAddr)
		if found {
			return val.IsBonded()
		} else {
			return false
		}
	} else {
		_, found := k.GetCCValidator(ctx, consAddr)
		return found
	}
}

// Load the last total validator power.
func (k Keeper) GetLastTotalPower(ctx sdk.Context) sdk.Int {
	if k.ConsumerKeeper == nil {
		return k.Keeper.GetLastTotalPower(ctx)
	} else {
		vals := k.ConsumerKeeper.GetAllCCValidator(ctx)
		total := sdk.ZeroInt()
		for _, v := range vals {
			total = total.AddRaw(v.Power)
		}
		return total
	}
}

// Note: this does not have a well defined notion of
// "Returns zero if the operator was not a validator last block",
// but the Gravity module does not seem to check for this kind of case
// and just passes any zeroes through
func (k Keeper) GetLastValconsPower(ctx sdk.Context, consAddr sdk.ConsAddress) (power int64, found bool) {
	if k.ConsumerKeeper == nil {
		val, found := k.Keeper.GetValidatorByConsAddr(ctx, consAddr)
		if found {
			return k.Keeper.GetLastValidatorPower(ctx, val.GetOperator()), true
		} else {
			return 0, false
		}
	} else {
		consAddr, found := k.GetCCValidator(ctx, consAddr)
		if found {
			return consAddr.Power, true
		} else {
			return 0, false
		}
	}
}

func (k Keeper) GetValconsSortedByPower(ctx sdk.Context) []sdk.ConsAddress {
	if k.ConsumerKeeper == nil {
		validators := k.Keeper.GetAllValidators(ctx)
		mapped := make([]sdk.ConsAddress, 0, len(validators))
		for _, v := range validators {
			consAddr, err := v.GetConsAddr()
			if err != nil {
				panic(err)
			}
			mapped = append(mapped, consAddr)
		}
		return mapped
	} else {
		ccvalidators := k.ConsumerKeeper.GetAllCCValidator(ctx)
		mapped := make([]sdk.ConsAddress, 0, len(ccvalidators))
		for _, v := range ccvalidators {
			pk, err := v.ConsPubKey()
			if err != nil {
				panic(err)
			}
			consAddr := sdk.GetConsAddress(pk)
			mapped = append(mapped, consAddr)
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

func (k Keeper) IsJailed(ctx sdk.Context, consAddr sdk.ConsAddress) bool {
	if k.ConsumerKeeper == nil {
		val, found := k.Keeper.GetValidatorByConsAddr(ctx, consAddr)
		if found {
			return val.IsJailed()
		} else {
			return false
		}
	} else {
		return k.ConsumerKeeper.IsValidatorJailed(ctx, consAddr)
	}
}

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
