package keeper

import (
	"testing"

	codectypes "github.com/cosmos/cosmos-sdk/codec/types"
	"github.com/cosmos/cosmos-sdk/crypto/keys/ed25519"
	sdk "github.com/cosmos/cosmos-sdk/types"
	distypes "github.com/cosmos/cosmos-sdk/x/distribution/types"
	"github.com/stretchr/testify/require"

	"github.com/onomyprotocol/cosmos-gravity-bridge/module/x/gravity/types"
)

// Sets up 10 attestations and checks that they are returned in the correct order
func TestGetMostRecentAttestations(t *testing.T) {
	input := CreateTestEnv(t)
	k := input.GravityKeeper
	ctx := input.Context

	lenth := 10
	msgs := make([]types.MsgSendToCosmosClaim, 0, lenth)
	anys := make([]codectypes.Any, 0, lenth)
	for i := 0; i < lenth; i++ {
		nonce := uint64(1 + i)
		msg := types.MsgSendToCosmosClaim{
			EventNonce:     nonce,
			BlockHeight:    1,
			TokenContract:  "0x00000000000000000001",
			Amount:         sdk.NewInt(10000000000 + int64(i)),
			EthereumSender: "0x00000000000000000002",
			CosmosReceiver: "0x00000000000000000003",
			Orchestrator:   "0x00000000000000000004",
		}
		msgs = append(msgs, msg)

		any, _ := codectypes.NewAnyWithValue(&msg)
		anys = append(anys, *any)
		att := &types.Attestation{
			Observed: false,
			Height:   uint64(ctx.BlockHeight()),
			Claim:    any,
		}
		hash, err := msg.ClaimHash()
		require.NoError(t, err)
		k.SetAttestation(ctx, nonce, hash, att)
	}

	recentAttestations := k.GetMostRecentAttestations(ctx, uint64(10))
	require.True(t, len(recentAttestations) == lenth,
		"recentAttestations should have len %v but instead has %v", lenth, len(recentAttestations))
	for n, attest := range recentAttestations {
		require.Equal(t, attest.Claim.GetCachedValue(), anys[n].GetCachedValue(),
			"The %vth claim does not match our message: claim %v\n message %v", n, attest.Claim, msgs[n])
	}
}

func TestHandleMsgValsetUpdatedClaim(t *testing.T) {
	rewardAmount := sdk.NewInt(100)

	rewardRecipient := sdk.AccAddress(ed25519.GenPrivKey().PubKey().Address())

	testEnv := CreateTestEnv(t)

	accountKeeper := testEnv.AccountKeeper
	bankKeeper := testEnv.BankKeeper
	gravityKeeper := testEnv.GravityKeeper
	stakingKeeper := testEnv.StakingKeeper

	ctx := testEnv.Context

	rewardDenom := stakingKeeper.BondDenom(ctx)
	distAccount := accountKeeper.GetModuleAddress(distypes.ModuleName)
	initialDistBalanceAmount := bankKeeper.GetBalance(ctx, distAccount, rewardDenom).Amount

	// empty message
	msg := &types.MsgValsetUpdatedClaim{}
	err := gravityKeeper.AttestationHandler.Handle(ctx, types.Attestation{}, msg)
	require.NoError(t, err)

	// with valid reward and recipient
	msg = &types.MsgValsetUpdatedClaim{
		RewardAmount:    rewardAmount,
		RewardDenom:     rewardDenom,
		RewardRecipient: rewardRecipient.String(),
	}
	err = gravityKeeper.AttestationHandler.Handle(ctx, types.Attestation{}, msg)
	require.NoError(t, err)
	recipientBalanceAmount := bankKeeper.GetBalance(ctx, rewardRecipient, rewardDenom).Amount
	require.Equal(t, rewardAmount, recipientBalanceAmount)

	// with valid reward and invalid recipient (goes to community pool)
	msg = &types.MsgValsetUpdatedClaim{
		RewardAmount:    rewardAmount,
		RewardDenom:     rewardDenom,
		RewardRecipient: "invalid-recipient-address",
	}
	err = gravityKeeper.AttestationHandler.Handle(ctx, types.Attestation{}, msg)
	require.NoError(t, err)

	distBalanceAmount := bankKeeper.GetBalance(ctx, distAccount, rewardDenom).Amount
	require.Equal(t, rewardAmount, distBalanceAmount.Sub(initialDistBalanceAmount))
}
