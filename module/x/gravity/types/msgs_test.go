package types

import (
	"bytes"
	"fmt"
	"testing"

	sdk "github.com/cosmos/cosmos-sdk/types"
	"github.com/stretchr/testify/assert"
)

func TestValidateMsgSetOrchestratorAddress(t *testing.T) {
	var (
		ethAddress                    = "0xb462864E395d88d6bc7C5dd5F3F5eb4cc2599255"
		cosmosAddress sdk.AccAddress  = bytes.Repeat([]byte{0x1}, 20)
		consAddress   sdk.ConsAddress = bytes.Repeat([]byte{0x1}, 20)
	)
	specs := map[string]struct {
		srcCosmosAddr sdk.AccAddress
		srcConsAddr   sdk.ConsAddress
		srcETHAddr    string
		expErr        bool
	}{
		"all good": {
			srcCosmosAddr: cosmosAddress,
			srcConsAddr:   consAddress,
			srcETHAddr:    ethAddress,
		},
		"empty validator address": {
			srcETHAddr:    ethAddress,
			srcCosmosAddr: cosmosAddress,
			expErr:        true,
		},
		"short validator address": {
			srcConsAddr:   []byte{0x1},
			srcCosmosAddr: cosmosAddress,
			srcETHAddr:    ethAddress,
			expErr:        false,
		},
		"empty cosmos address": {
			srcConsAddr: consAddress,
			srcETHAddr:  ethAddress,
			expErr:      true,
		},
		"short cosmos address": {
			srcCosmosAddr: []byte{0x1},
			srcConsAddr:   consAddress,
			srcETHAddr:    ethAddress,
			expErr:        false,
		},
	}
	for msg, spec := range specs {
		t.Run(msg, func(t *testing.T) {
			println(fmt.Sprintf("Spec is %v", msg))
			ethAddr, err := NewEthAddress(spec.srcETHAddr)
			assert.NoError(t, err)
			msg := NewMsgSetOrchestratorAddress(spec.srcConsAddr, spec.srcCosmosAddr, *ethAddr)
			// when
			err = msg.ValidateBasic()
			if spec.expErr {
				assert.Error(t, err)
				return
			}
			assert.NoError(t, err)
		})
	}

}
