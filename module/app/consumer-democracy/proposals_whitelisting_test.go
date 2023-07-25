package app_test

import (
	"encoding/json"
	"testing"

	"github.com/cosmos/cosmos-sdk/simapp"
	ibctesting "github.com/cosmos/interchain-security/legacy_ibc_testing/testing"
	appConsumer "github.com/onomyprotocol/arc/module/app/consumer-democracy"
	"github.com/stretchr/testify/require"
	"github.com/tendermint/spm/cosmoscmd"
	"github.com/tendermint/tendermint/libs/log"
	tmdb "github.com/tendermint/tm-db"
)

// DemocracyConsumerAppIniter implements ibctesting.AppIniter for a democracy consumer app
func DemocracyConsumerAppIniter() (ibctesting.TestingApp, map[string]json.RawMessage) {
	encoding := cosmoscmd.MakeEncodingConfig(appConsumer.ModuleBasics)
	testApp := appConsumer.New(log.NewNopLogger(), tmdb.NewMemDB(), nil, true, map[int64]bool{},
		simapp.DefaultNodeHome, 5, encoding, simapp.EmptyAppOptions{}).(ibctesting.TestingApp)
	return testApp, appConsumer.NewDefaultGenesisState(encoding.Marshaler)
}

func TestDemocracyGovernanceWhitelistingKeys(t *testing.T) {
	chain := ibctesting.NewTestChain(t, ibctesting.NewCoordinator(t, 0),
		DemocracyConsumerAppIniter, "test")
	paramKeeper := chain.App.(*appConsumer.App).ParamsKeeper
	for paramKey := range appConsumer.WhitelistedParams {
		ss, ok := paramKeeper.GetSubspace(paramKey.Subspace)
		require.True(t, ok, "Unknown subspace %s", paramKey.Subspace)
		hasKey := ss.Has(chain.GetContext(), []byte(paramKey.Key))
		require.True(t, hasKey, "Invalid key %s for subspace %s", paramKey.Key, paramKey.Subspace)
	}
}
