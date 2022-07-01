package types

import (
	"bytes"
	"encoding/hex"
	"fmt"
	mrand "math/rand"
	"testing"

	sdk "github.com/cosmos/cosmos-sdk/types"
	gethcommon "github.com/ethereum/go-ethereum/common"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestValsetConfirmHash(t *testing.T) {
	const (
		gravityID = "foo"
		denom     = "foo_denom"
	)
	var (
		reward       = sdk.NewInt(1000)
		powers       = []uint64{3333, 3333, 3333}
		ethAddresses = []string{
			"0xc783df8a850f42e7F7e57013759C285caa701eB6",
			"0xE5904695748fe4A84b40b3fc79De2277660BD1D3",
			"0xeAD9C93b79Ae7C1591b1FB5323BD777E86e150d4",
		}
		members = make(InternalBridgeValidators, len(powers))
	)

	for i := range powers {
		bv := BridgeValidator{
			Power:           powers[i],
			EthereumAddress: ethAddresses[i],
		}
		ibv, err := NewInternalBridgeValidator(bv)
		require.NoError(t, err)
		members[i] = ibv
	}

	// check with filled reward

	v, err := NewValset(0, 0, members, reward, denom)

	fmt.Printf("%+v", v)

	require.NoError(t, err)

	hash := v.GetCheckpoint(gravityID)
	hexHash := hex.EncodeToString(hash)
	// you can find the same value in the orchestrator tests, so if you update it update there as well
	correctHash := "0x2751e9f1cdef7c6f1365e81a42707c0ecff75e6cd7cecd6c456e571234548a1e"[2:]
	assert.Equal(t, correctHash, hexHash)

	// check with empty reward

	v, err = NewValset(0, 0, members, sdk.ZeroInt(), "")
	require.NoError(t, err)

	hash = v.GetCheckpoint(gravityID)
	hexHash = hex.EncodeToString(hash)
	// you can find the same value in the orchestrator tests, so if you update it update there as well
	correctHash = "0xa2c8dc58c06fa959763bffd4c8fe8668869b7b5c866a7b0f0f1739b92a6cd5d1"[2:]
	assert.Equal(t, correctHash, hexHash)

	// check with invalid nonce and nonce reward

	v, err = NewValset(1, 0, members, sdk.ZeroInt(), "")
	require.NoError(t, err)

	hash = v.GetCheckpoint(gravityID)
	hexHash = hex.EncodeToString(hash)
	// you can find the same value in the orchestrator tests, so if you update it update there as well
	correctHash = "0xa2c8dc58c06fa959763bffd4c8fe8668869b7b5c866a7b0f0f1739b92a6cd5d1"[2:]
	// the nonce is not 0 hence the hash is invalid
	assert.NotEqual(t, correctHash, hexHash)

}

func TestValsetCheckpointGold1(t *testing.T) {
	bridgeValidators, err := BridgeValidators{{
		Power:           6667,
		EthereumAddress: "0xc783df8a850f42e7F7e57013759C285caa701eB6",
	}}.ToInternal()
	require.NoError(t, err)
	src, err := NewValset(0, 0, *bridgeValidators, sdk.NewInt(0), "")
	require.NoError(t, err)

	// normally we would load the GravityID from the store, but for this test we use
	// the same hardcoded value in the solidity tests
	ourHash := src.GetCheckpoint("foo")

	// hash from bridge contract
	goldHash := "0xe3d534594d4a3cf357de3b07a7b26dbc31daab10edb881cb3eef0292cf0669c0"[2:]
	assert.Equal(t, goldHash, hex.EncodeToString(ourHash))
}

func TestValsetPowerDiff(t *testing.T) {
	specs := map[string]struct {
		start BridgeValidators
		diff  BridgeValidators
		exp   float64
	}{
		"no diff": {
			start: BridgeValidators{
				{Power: 1, EthereumAddress: "0x479FFc856Cdfa0f5D1AE6Fa61915b01351A7773D"},
				{Power: 2, EthereumAddress: "0x8E91960d704Df3fF24ECAb78AB9df1B5D9144140"},
				{Power: 3, EthereumAddress: "0xF14879a175A2F1cEFC7c616f35b6d9c2b0Fd8326"},
			},
			diff: BridgeValidators{
				{Power: 1, EthereumAddress: "0x479FFc856Cdfa0f5D1AE6Fa61915b01351A7773D"},
				{Power: 2, EthereumAddress: "0x8E91960d704Df3fF24ECAb78AB9df1B5D9144140"},
				{Power: 3, EthereumAddress: "0xF14879a175A2F1cEFC7c616f35b6d9c2b0Fd8326"},
			},
			exp: 0.0,
		},
		"one": {
			start: BridgeValidators{
				{Power: 1073741823, EthereumAddress: "0x479FFc856Cdfa0f5D1AE6Fa61915b01351A7773D"},
				{Power: 1073741823, EthereumAddress: "0x8E91960d704Df3fF24ECAb78AB9df1B5D9144140"},
				{Power: 2147483646, EthereumAddress: "0xF14879a175A2F1cEFC7c616f35b6d9c2b0Fd8326"},
			},
			diff: BridgeValidators{
				{Power: 858993459, EthereumAddress: "0x479FFc856Cdfa0f5D1AE6Fa61915b01351A7773D"},
				{Power: 858993459, EthereumAddress: "0x8E91960d704Df3fF24ECAb78AB9df1B5D9144140"},
				{Power: 2576980377, EthereumAddress: "0xF14879a175A2F1cEFC7c616f35b6d9c2b0Fd8326"},
			},
			exp: 0.2,
		},
		"real world": {
			start: BridgeValidators{
				{Power: 678509841, EthereumAddress: "0x6db48cBBCeD754bDc760720e38E456144e83269b"},
				{Power: 671724742, EthereumAddress: "0x8E91960d704Df3fF24ECAb78AB9df1B5D9144140"},
				{Power: 685294939, EthereumAddress: "0x479FFc856Cdfa0f5D1AE6Fa61915b01351A7773D"},
				{Power: 671724742, EthereumAddress: "0x0A7254b318dd742A3086882321C27779B4B642a6"},
				{Power: 671724742, EthereumAddress: "0x454330deAaB759468065d08F2b3B0562caBe1dD1"},
				{Power: 617443955, EthereumAddress: "0x3511A211A6759d48d107898302042d1301187BA9"},
				{Power: 6785098, EthereumAddress: "0x37A0603dA2ff6377E5C7f75698dabA8EE4Ba97B8"},
				{Power: 291759231, EthereumAddress: "0xF14879a175A2F1cEFC7c616f35b6d9c2b0Fd8326"},
			},
			diff: BridgeValidators{
				{Power: 642345266, EthereumAddress: "0x479FFc856Cdfa0f5D1AE6Fa61915b01351A7773D"},
				{Power: 678509841, EthereumAddress: "0x6db48cBBCeD754bDc760720e38E456144e83269b"},
				{Power: 671724742, EthereumAddress: "0x0A7254b318dd742A3086882321C27779B4B642a6"},
				{Power: 671724742, EthereumAddress: "0x454330deAaB759468065d08F2b3B0562caBe1dD1"},
				{Power: 671724742, EthereumAddress: "0x8E91960d704Df3fF24ECAb78AB9df1B5D9144140"},
				{Power: 617443955, EthereumAddress: "0x3511A211A6759d48d107898302042d1301187BA9"},
				{Power: 291759231, EthereumAddress: "0xF14879a175A2F1cEFC7c616f35b6d9c2b0Fd8326"},
				{Power: 6785098, EthereumAddress: "0x37A0603dA2ff6377E5C7f75698dabA8EE4Ba97B8"},
			},
			exp: 0.010000000011641532,
		},
	}
	for msg, spec := range specs {
		t.Run(msg, func(t *testing.T) {
			startInternal, _ := spec.start.ToInternal()
			diffInternal, _ := spec.diff.ToInternal()
			assert.Equal(t, spec.exp, startInternal.PowerDiff(*diffInternal))
		})
	}
}

func TestValsetSort(t *testing.T) {
	specs := map[string]struct {
		src BridgeValidators
		exp BridgeValidators
	}{
		"by eth addres": {
			src: BridgeValidators{
				{Power: 1, EthereumAddress: gethcommon.BytesToAddress(bytes.Repeat([]byte{byte(2)}, 20)).String()},
				{Power: 1, EthereumAddress: gethcommon.BytesToAddress(bytes.Repeat([]byte{byte(1)}, 20)).String()},
				{Power: 1, EthereumAddress: gethcommon.BytesToAddress(bytes.Repeat([]byte{byte(3)}, 20)).String()},
			},
			exp: BridgeValidators{
				{Power: 1, EthereumAddress: gethcommon.BytesToAddress(bytes.Repeat([]byte{byte(1)}, 20)).String()},
				{Power: 1, EthereumAddress: gethcommon.BytesToAddress(bytes.Repeat([]byte{byte(2)}, 20)).String()},
				{Power: 1, EthereumAddress: gethcommon.BytesToAddress(bytes.Repeat([]byte{byte(3)}, 20)).String()},
			},
		},
		// we ignore the power and sort by the address only
		"real world": {
			src: BridgeValidators{
				{Power: 617443955, EthereumAddress: "0x3511A211A6759d48d107898302042d1301187BA9"},
				{Power: 671724742, EthereumAddress: "0x0A7254b318dd742A3086882321C27779B4B642a6"},
				{Power: 291759231, EthereumAddress: "0xa14879a175A2F1cEFC7c616f35b6d9c2b0Fd8326"},
				{Power: 291759231, EthereumAddress: "0xA24879a175A2F1cEFC7c616f35b6d9c2b0Fd8326"},
				{Power: 291759231, EthereumAddress: "0xF14879a175A2F1cEFC7c616f35b6d9c2b0Fd8326"},
				{Power: 671724742, EthereumAddress: "0x454330deAaB759468065d08F2b3B0562caBe1dD1"},
				{Power: 6785098, EthereumAddress: "0x37A0603dA2ff6377E5C7f75698dabA8EE4Ba97B8"},
				{Power: 685294939, EthereumAddress: "0x479FFc856Cdfa0f5D1AE6Fa61915b01351A7773D"},
			},
			exp: BridgeValidators{
				{Power: 671724742, EthereumAddress: "0x0A7254b318dd742A3086882321C27779B4B642a6"},
				{Power: 617443955, EthereumAddress: "0x3511A211A6759d48d107898302042d1301187BA9"},
				{Power: 6785098, EthereumAddress: "0x37A0603dA2ff6377E5C7f75698dabA8EE4Ba97B8"},
				{Power: 671724742, EthereumAddress: "0x454330deAaB759468065d08F2b3B0562caBe1dD1"},
				{Power: 685294939, EthereumAddress: "0x479FFc856Cdfa0f5D1AE6Fa61915b01351A7773D"},
				{Power: 291759231, EthereumAddress: "0xa14879a175A2F1cEFC7c616f35b6d9c2b0Fd8326"},
				{Power: 291759231, EthereumAddress: "0xA24879a175A2F1cEFC7c616f35b6d9c2b0Fd8326"},
				{Power: 291759231, EthereumAddress: "0xF14879a175A2F1cEFC7c616f35b6d9c2b0Fd8326"},
			},
		},
	}
	for msg, spec := range specs {
		t.Run(msg, func(t *testing.T) {
			srcInternal, _ := spec.src.ToInternal()
			expInternal, _ := spec.exp.ToInternal()
			srcInternal.Sort()
			assert.Equal(t, srcInternal, expInternal)
			shuffled := shuffled(*srcInternal)
			shuffled.Sort()
			assert.Equal(t, shuffled, *expInternal)
		})
	}
}

func shuffled(v InternalBridgeValidators) InternalBridgeValidators {
	mrand.Shuffle(len(v), func(i, j int) {
		v[i], v[j] = v[j], v[i]
	})
	return v
}
