//! This is a test for validator set relaying rewards

use std::collections::HashMap;

use cosmos_gravity::query::get_gravity_params;
use gravity_proto::{
    cosmos_sdk_proto::cosmos::params::v1beta1::ParamChange,
    gravity::query_client::QueryClient as GravityQueryClient,
};
use gravity_utils::{
    clarity::{u256, Address as EthAddress, Uint256},
    deep_space::{coin::Coin, Address as CosmosAddress, Contact},
    u64_array_bigints,
    web30::client::Web3,
};
use tokio::time::timeout;
use tonic::transport::Channel;

use crate::{
    airdrop_proposal::wait_for_proposals_to_execute,
    await_validators_first_cosmos_deposit, get_validators_cosmos_balances,
    utils::{
        create_default_test_config, create_parameter_change_proposal, footoken_metadata,
        start_orchestrators, vote_yes_on_proposals, ValidatorKeys,
    },
    validator_out::test_valset_update,
    TOTAL_TIMEOUT,
};

pub async fn valset_rewards_test(
    web30: &Web3,
    grpc_client: GravityQueryClient<Channel>,
    contact: &Contact,
    keys: Vec<ValidatorKeys>,
    gravity_address: EthAddress,
) {
    let mut grpc_client = grpc_client;
    let token_to_send_to_eth = footoken_metadata(contact).await.base;

    let no_relay_market_config = create_default_test_config();
    start_orchestrators(keys.clone(), gravity_address, false, no_relay_market_config).await;

    let valset_reward = Coin {
        denom: token_to_send_to_eth.clone(),
        amount: u256!(1_000_000),
    };

    let mut params_to_change = Vec::new();
    let gravity_address_param = ParamChange {
        subspace: "gravity".to_string(),
        key: "BridgeEthereumAddress".to_string(),
        value: format!("\"{}\"", gravity_address),
    };
    params_to_change.push(gravity_address_param);
    let json_value = serde_json::to_string(&valset_reward).unwrap().to_string();
    let valset_reward_param = ParamChange {
        subspace: "gravity".to_string(),
        key: "ValsetReward".to_string(),
        value: json_value.clone(),
    };
    params_to_change.push(valset_reward_param);
    let chain_id = ParamChange {
        subspace: "gravity".to_string(),
        key: "BridgeChainID".to_string(),
        value: format!("\"{}\"", 1),
    };
    params_to_change.push(chain_id);

    // next we create a governance proposal to use the newly bridged asset as the reward
    // and vote to pass the proposal
    info!("Creating parameter change governance proposal.");
    create_parameter_change_proposal(contact, keys[0].validator_key, params_to_change).await;

    vote_yes_on_proposals(contact, &keys, None).await;

    // wait for the voting period to pass
    wait_for_proposals_to_execute(contact).await;

    let params = get_gravity_params(&mut grpc_client).await.unwrap();
    // check that params have changed
    assert_eq!(params.bridge_chain_id, 1);
    assert_eq!(params.bridge_ethereum_address, gravity_address.to_string());

    let initial_reward_token_balance: HashMap<CosmosAddress, Uint256> =
        get_validators_cosmos_balances(contact, &keys, token_to_send_to_eth.to_string()).await;

    info!("Trigger a valset update.");
    test_valset_update(web30, contact, &mut grpc_client, &keys, gravity_address).await;

    info!("Waiting for valset reward deposit.");
    if timeout(
        TOTAL_TIMEOUT,
        await_validators_first_cosmos_deposit(
            contact,
            keys,
            valset_reward,
            initial_reward_token_balance,
        ),
    )
    .await
    .is_err()
    {
        panic!("Failed to perform the valset reward deposits to Cosmos!");
    }
    info!("Successfully issued validator set reward!");
}
