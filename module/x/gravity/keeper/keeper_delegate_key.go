package keeper

import (
	sdkerrors "github.com/cosmos/cosmos-sdk/types/errors"

	sdk "github.com/cosmos/cosmos-sdk/types"

	"github.com/onomyprotocol/arc/module/x/gravity/types"
)

/////////////////////////////
//    ADDRESS DELEGATION   //
/////////////////////////////

// SetOrchestratorValidator sets the Orchestrator key for a given validator
func (k Keeper) SetOrchestratorValcons(ctx sdk.Context, consAddr sdk.ConsAddress, orch sdk.AccAddress) {
	if err := sdk.VerifyAddressFormat(consAddr); err != nil {
		panic(sdkerrors.Wrap(err, "invalid valcons address"))
	}
	if err := sdk.VerifyAddressFormat(orch); err != nil {
		panic(sdkerrors.Wrap(err, "invalid orch address"))
	}
	store := ctx.KVStore(k.storeKey)
	store.Set([]byte(types.GetOrchestratorAddressKey(orch)), consAddr.Bytes())
}

// GetOrchestratorValidator returns the valcons key associated with an orchestrator key
func (k Keeper) GetOrchestratorValcons(ctx sdk.Context, orch sdk.AccAddress) (consAddr sdk.ConsAddress, found bool) {
	if err := sdk.VerifyAddressFormat(orch); err != nil {
		ctx.Logger().Error("invalid orch address")
		return consAddr, false
	}
	store := ctx.KVStore(k.storeKey)
	consAddr = store.Get([]byte(types.GetOrchestratorAddressKey(orch)))
	if consAddr == nil {
		return consAddr, false
	}

	return consAddr, true
}

/////////////////////////////
//       ETH ADDRESS       //
/////////////////////////////

// SetEthAddress sets the ethereum address for a given validator consensus address
func (k Keeper) SetEthAddressForValcons(ctx sdk.Context, consAddr sdk.ConsAddress, ethAddr types.EthAddress) {
	if err := sdk.VerifyAddressFormat(consAddr); err != nil {
		panic(sdkerrors.Wrap(err, "invalid valcons address"))
	}
	store := ctx.KVStore(k.storeKey)
	store.Set([]byte(types.GetEthAddressByValconsKey(consAddr)), []byte(ethAddr.GetAddress()))
	store.Set([]byte(types.GetValconsByEthAddressKey(ethAddr)), []byte(consAddr))
}

// GetEthAddressByValidator returns the eth address for a given gravity validator
func (k Keeper) GetEthAddressByValcons(ctx sdk.Context, consAddr sdk.ConsAddress) (ethAddress *types.EthAddress, found bool) {
	if err := sdk.VerifyAddressFormat(consAddr); err != nil {
		panic(sdkerrors.Wrap(err, "invalid valcons address"))
	}
	store := ctx.KVStore(k.storeKey)
	ethAddr := store.Get([]byte(types.GetEthAddressByValconsKey(consAddr)))
	if ethAddr == nil {
		return nil, false
	}

	addr, err := types.NewEthAddress(string(ethAddr))
	if err != nil {
		return nil, false
	}
	return addr, true
}

// GetValconsByEthAddress returns the validator consensus for a given eth address
func (k Keeper) GetValconsByEthAddress(ctx sdk.Context, ethAddr types.EthAddress) (consAddr sdk.ConsAddress, found bool) {
	store := ctx.KVStore(k.storeKey)
	consAddr = store.Get([]byte(types.GetValconsByEthAddressKey(ethAddr)))
	if consAddr == nil {
		return sdk.ConsAddress{}, false
	}
	return consAddr, true
}
