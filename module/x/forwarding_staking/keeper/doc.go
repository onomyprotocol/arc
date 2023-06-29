/*
This defines a wrapper around around the Cosmos SDK native x/staking/keeper
that embeds and provides the exact same functionality, except that certain methods are overridden to "forward" valset and slashing information from the provider.
*/
package keeper
