/*
This defines a wrapper around around the Cosmos SDK native x/staking/keeper
that embeds and provides the exact same functionality, except that certain methods are overridden
to support the gravity module on a consumer chain. Primarily, the graviy module has been changed
to have its powers based on the ValCons address instead of the main validator address.

NOTE: this module might be unsafe to use for the non-consumer case because of usages of
`GetValidatorByConsAddr` and redundant information

# MsgSetOrchestratorAddress uses have been changed to use the ValCons address

The `gentx` command has been changed to use the valcons address and orchestrator key name.
setup-validators.sh and bootstrapping.rs have been modified to use the orchestrator key only
*/
package keeper
