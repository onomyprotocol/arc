package app_test

import (
	"testing"

	// use the interchain-security provider for the provider
	appProvider "github.com/cosmos/interchain-security/app/provider"
	e2e "github.com/cosmos/interchain-security/tests/e2e"
	icstestingutils "github.com/cosmos/interchain-security/testutil/ibc_testing"
	appConsumerDemocracy "github.com/onomyprotocol/arc/module/app/consumer-democracy"
	"github.com/stretchr/testify/suite"
)

// Executes a standard suite of tests, against a democracy consumer app.go implementation.
func TestConsumerDemocracyCCVTestSuite(t *testing.T) {
	// Pass in concrete app type that implement the interface defined in /testutil/e2e/interfaces.go
	// IMPORTANT: the concrete app types passed in as type parameters here must match the
	// concrete app types returned by the relevant app initers.
	democSuite := e2e.NewCCVTestSuite[*appProvider.App, *appConsumerDemocracy.App](
		// Pass in ibctesting.AppIniter for provider and democracy consumer.
		// TestRewardsDistribution needs to be skipped since the democracy specific distribution test is in ConsumerDemocracyTestSuite,
		// while this one tests consumer app without minter
		icstestingutils.ProviderAppIniter, DemocracyConsumerAppIniter, []string{"TestRewardsDistribution"})

	// Run tests
	suite.Run(t, democSuite)
}

// Executes a specialized group of tests specific to a democracy consumer,
// against a democracy consumer app.go implementation.
func TestConsumerDemocracyTestSuite(t *testing.T) {
	// Pass in concrete app type that implement the interface defined in /testutil/e2e/interfaces.go
	// IMPORTANT: the concrete app type passed in as a type parameter here must match the
	// concrete app type returned by the relevant app initer.
	democSuite := e2e.NewConsumerDemocracyTestSuite[*appConsumerDemocracy.App](
		// Pass in ibctesting.AppIniter for democracy consumer.
		DemocracyConsumerAppIniter)

	// Run tests
	suite.Run(t, democSuite)
}
