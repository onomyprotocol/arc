package types

import (
	"github.com/cosmos/cosmos-sdk/codec"
	"github.com/cosmos/cosmos-sdk/codec/types"
	sdk "github.com/cosmos/cosmos-sdk/types"
	"github.com/cosmos/cosmos-sdk/types/msgservice"
	govtypes "github.com/cosmos/cosmos-sdk/x/gov/types"
)

// ModuleCdc is the codec for the module
var ModuleCdc = codec.NewLegacyAmino()

func init() {
	RegisterCodec(ModuleCdc)
}

// nolint: exhaustivestruct
// RegisterInterfaces registers the interfaces for the proto stuff
func RegisterInterfaces(registry types.InterfaceRegistry) {
	registry.RegisterImplementations((*sdk.Msg)(nil),
		&MsgValsetConfirm{},
		&MsgSendToEth{},
		&MsgRequestBatch{},
		&MsgConfirmBatch{},
		&MsgConfirmLogicCall{},
		&MsgSendToCosmosClaim{},
		&MsgBatchSendToEthClaim{},
		&MsgERC20DeployedClaim{},
		&MsgSetOrchestratorAddress{},
		&MsgLogicCallExecutedClaim{},
		&MsgValsetUpdatedClaim{},
		&MsgCancelSendToEth{},
		&MsgSubmitBadSignatureEvidence{},
	)

	registry.RegisterInterface(
		"arcbnb.v1beta1.EthereumClaim",
		(*EthereumClaim)(nil),
		&MsgSendToCosmosClaim{},
		&MsgBatchSendToEthClaim{},
		&MsgERC20DeployedClaim{},
		&MsgLogicCallExecutedClaim{},
		&MsgValsetUpdatedClaim{},
	)

	registry.RegisterImplementations((*govtypes.Content)(nil), &UnhaltBridgeProposal{}, &AirdropProposal{}, &IBCMetadataProposal{})

	registry.RegisterInterface("arcbnb.v1beta1.EthereumSigned", (*EthereumSigned)(nil), &Valset{}, &OutgoingTxBatch{}, &OutgoingLogicCall{})

	msgservice.RegisterMsgServiceDesc(registry, &_Msg_serviceDesc)
}

// nolint: exhaustivestruct
// RegisterCodec registers concrete types on the Amino codec
func RegisterCodec(cdc *codec.LegacyAmino) {
	cdc.RegisterInterface((*EthereumClaim)(nil), nil)
	cdc.RegisterConcrete(&MsgSetOrchestratorAddress{}, "arcbnb/MsgSetOrchestratorAddress", nil)
	cdc.RegisterConcrete(&MsgValsetConfirm{}, "arcbnb/MsgValsetConfirm", nil)
	cdc.RegisterConcrete(&MsgSendToEth{}, "arcbnb/MsgSendToEth", nil)
	cdc.RegisterConcrete(&MsgRequestBatch{}, "arcbnb/MsgRequestBatch", nil)
	cdc.RegisterConcrete(&MsgConfirmBatch{}, "arcbnb/MsgConfirmBatch", nil)
	cdc.RegisterConcrete(&MsgConfirmLogicCall{}, "arcbnb/MsgConfirmLogicCall", nil)
	cdc.RegisterConcrete(&Valset{}, "arcbnb/Valset", nil)
	cdc.RegisterConcrete(&MsgSendToCosmosClaim{}, "arcbnb/MsgSendToCosmosClaim", nil)
	cdc.RegisterConcrete(&MsgBatchSendToEthClaim{}, "arcbnb/MsgBatchSendToEthClaim", nil)
	cdc.RegisterConcrete(&MsgERC20DeployedClaim{}, "arcbnb/MsgERC20DeployedClaim", nil)
	cdc.RegisterConcrete(&MsgLogicCallExecutedClaim{}, "arcbnb/MsgLogicCallExecutedClaim", nil)
	cdc.RegisterConcrete(&MsgValsetUpdatedClaim{}, "arcbnb/MsgValsetUpdatedClaim", nil)
	cdc.RegisterConcrete(&OutgoingTxBatch{}, "arcbnb/OutgoingTxBatch", nil)
	cdc.RegisterConcrete(&MsgCancelSendToEth{}, "arcbnb/MsgCancelSendToEth", nil)
	cdc.RegisterConcrete(&OutgoingTransferTx{}, "arcbnb/OutgoingTransferTx", nil)
	cdc.RegisterConcrete(&ERC20Token{}, "arcbnb/ERC20Token", nil)
	cdc.RegisterConcrete(&IDSet{}, "arcbnb/IDSet", nil)
	cdc.RegisterConcrete(&Attestation{}, "arcbnb/Attestation", nil)
	cdc.RegisterConcrete(&MsgSubmitBadSignatureEvidence{}, "arcbnb/MsgSubmitBadSignatureEvidence", nil)
}
