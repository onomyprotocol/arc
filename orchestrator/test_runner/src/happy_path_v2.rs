//! This is the happy path test for Cosmos to Ethereum asset transfers, meaning assets originated on Cosmos

use std::{collections::HashMap, panic, time::Duration};

use cosmos_gravity::send::send_to_eth;
use ethereum_gravity::{deploy_erc20::deploy_erc20, utils::get_valset_nonce};
use gravity_proto::{
    cosmos_sdk_proto::cosmos::bank::v1beta1::Metadata,
    gravity::{query_client::QueryClient as GravityQueryClient, QueryDenomToErc20Request},
};
use gravity_utils::{
    clarity::{u256, Address as EthAddress, Uint256},
    deep_space::{coin::Coin, Address as CosmosAddress, Contact},
    u64_array_bigints,
    web30::{client::Web3, types::SendTxOption},
};
use tokio::time::{sleep, timeout};
use tonic::transport::Channel;

use crate::{
    await_validators_first_cosmos_deposit, get_fee, get_stake_token_name,
    get_validators_cosmos_balances, send_cosmos_coins,
    utils::{
        create_default_test_config, footoken_metadata, get_decimals, get_erc20_balance_safe,
        get_event_nonce_safe, get_user_key, send_one_eth, start_orchestrators, ValidatorKeys,
    },
    MINER_ADDRESS, MINER_PRIVATE_KEY, TOTAL_TIMEOUT,
};

pub async fn happy_path_test_v2(
    web30: &Web3,
    grpc_client: GravityQueryClient<Channel>,
    contact: &Contact,
    keys: Vec<ValidatorKeys>,
    gravity_address: EthAddress,
    validator_out: bool,
) {
    let mut grpc_client = grpc_client;

    let erc20_contract = deploy_cosmos_representing_erc20_and_check_adoption(
        gravity_address,
        web30,
        Some(keys.clone()),
        &mut grpc_client,
        validator_out,
        footoken_metadata(contact).await,
    )
    .await;

    let token_to_send_to_eth = footoken_metadata(contact).await.base;

    let coin_to_bridge = Coin {
        denom: token_to_send_to_eth.clone(),
        amount: u256!(1_000),
    };
    let bridge_fee = Coin {
        denom: get_stake_token_name(),
        amount: u256!(100_000),
    };
    let cosmos_tx_fee = get_fee();
    let user = get_user_key();

    // send the foo token to bridge and stake token to pay for cosmos fee and bridge fee
    send_cosmos_coins(
        contact,
        keys[0].validator_key,
        vec![user.cosmos_address],
        vec![
            coin_to_bridge.clone(),
            bridge_fee.clone(),
            cosmos_tx_fee.clone(),
        ],
    )
    .await;

    // send the user some eth, they only need this to check their
    // erc20 balance, so a pretty minor use case
    send_one_eth(user.eth_address, web30).await;
    info!("Sent 1 eth to user address {}", user.eth_address);

    let initial_reward_token_balance: HashMap<CosmosAddress, Uint256> =
        get_validators_cosmos_balances(contact, &keys, bridge_fee.denom.to_string()).await;

    let res = send_to_eth(
        user.cosmos_key,
        user.eth_address,
        coin_to_bridge.clone(),
        bridge_fee.clone(), // bridge fee
        cosmos_tx_fee,      // cosmos tx fee
        contact,
    )
    .await
    .unwrap();
    info!("Send to eth res {:?}", res);
    info!(
        "Locked up {} {} to send to Cosmos",
        coin_to_bridge.denom, coin_to_bridge.amount
    );

    let verify_asset_is_bridged_to_eth = async {
        loop {
            match get_erc20_balance_safe(erc20_contract, web30, user.eth_address).await {
                Err(_) => {}
                Ok(balance) => {
                    if balance == coin_to_bridge.amount {
                        info!(
                            "Successfully bridged {} Cosmos asset {} to Ethereum!",
                            coin_to_bridge.amount, token_to_send_to_eth
                        );
                        assert!(balance == coin_to_bridge.amount);
                        break;
                    } else if !balance.is_zero() {
                        panic!(
                            "Expected {} {} but got {} instead",
                            coin_to_bridge.amount, token_to_send_to_eth, balance
                        );
                    }
                }
            }

            sleep(Duration::from_secs(1)).await;
        }
    };

    info!("Waiting for batch to be signed and relayed to Ethereum");
    if timeout(TOTAL_TIMEOUT, verify_asset_is_bridged_to_eth)
        .await
        .is_err()
    {
        panic!("failed to verify asset is bridged to ethereum: timed out")
    }

    info!("Waiting for batch reward deposit.");
    let expected_reward_increase = Coin {
        denom: bridge_fee.denom,
        // the orchestrator spends extra 3stake to pay for the signing after we captured the initial_reward_token_balance
        amount: bridge_fee.amount.checked_sub(u256!(3)).unwrap(),
    };

    if timeout(
        TOTAL_TIMEOUT,
        await_validators_first_cosmos_deposit(
            contact,
            keys,
            expected_reward_increase,
            initial_reward_token_balance,
        ),
    )
    .await
    .is_err()
    {
        panic!("Failed to perform the valset reward deposits to Cosmos!");
    }
    info!("The batch reward is received.");
}

/// This segment is broken out because it's used in two different tests
/// once here where we verify that tokens bridge correctly and once in valset_rewards
/// where we do a governance update to enable rewards
pub async fn deploy_cosmos_representing_erc20_and_check_adoption(
    gravity_address: EthAddress,
    web30: &Web3,
    keys: Option<Vec<ValidatorKeys>>,
    grpc_client: &mut GravityQueryClient<Channel>,
    validator_out: bool,
    token_metadata: Metadata,
) -> EthAddress {
    get_valset_nonce(gravity_address, *MINER_ADDRESS, web30)
        .await
        .expect("Incorrect Gravity Address or otherwise unable to contact Gravity");

    let starting_event_nonce = get_event_nonce_safe(gravity_address, web30, *MINER_ADDRESS)
        .await
        .unwrap();

    let cosmos_decimals = get_decimals(&token_metadata);
    deploy_erc20(
        token_metadata.base.clone(),
        token_metadata.name.clone(),
        token_metadata.symbol.clone(),
        cosmos_decimals,
        gravity_address,
        web30,
        Some(TOTAL_TIMEOUT),
        *MINER_PRIVATE_KEY,
        vec![
            SendTxOption::GasLimitMultiplier(2.0),
            SendTxOption::GasPriceMultiplier(2.0),
        ],
    )
    .await
    .unwrap();
    let ending_event_nonce = get_event_nonce_safe(gravity_address, web30, *MINER_ADDRESS)
        .await
        .unwrap();

    assert!(starting_event_nonce != ending_event_nonce);
    info!(
        "Successfully deployed new ERC20 representing FooToken on Cosmos with event nonce {}",
        ending_event_nonce
    );

    // if no keys are provided we assume the caller does not want to spawn
    // orchestrators as part of the test
    if let Some(keys) = keys {
        let no_relay_market_config = create_default_test_config();
        start_orchestrators(
            keys.clone(),
            gravity_address,
            validator_out,
            no_relay_market_config,
        )
        .await;
    }

    let get_cosmos_asset_on_eth = async {
        loop {
            // the erc20 representing the cosmos asset on Ethereum
            if let Ok(res) = grpc_client
                .denom_to_erc20(QueryDenomToErc20Request {
                    denom: token_metadata.base.clone(),
                })
                .await
            {
                let erc20 = res.into_inner().erc20;
                info!(
                    "Successfully adopted {} token contract of {}",
                    token_metadata.base, erc20
                );
                return erc20;
            }

            sleep(Duration::from_secs(1)).await;
        }
    };

    let erc20_contract = match tokio::time::timeout(TOTAL_TIMEOUT, get_cosmos_asset_on_eth).await {
        Err(_) => panic!(
            "Cosmos did not adopt the ERC20 contract for {} it must be invalid in some way",
            token_metadata.base
        ),
        Ok(erc20_contract) => erc20_contract.parse().unwrap(),
    };

    // now that we have the contract, validate that it has the properties we want
    let got_decimals = web30
        .get_erc20_decimals(erc20_contract, *MINER_ADDRESS)
        .await
        .unwrap();
    assert_eq!(Uint256::from_u32(cosmos_decimals), got_decimals);

    let got_name = web30
        .get_erc20_name(erc20_contract, *MINER_ADDRESS)
        .await
        .unwrap();
    assert_eq!(got_name, token_metadata.name);

    let got_symbol = web30
        .get_erc20_symbol(erc20_contract, *MINER_ADDRESS)
        .await
        .unwrap();
    assert_eq!(got_symbol, token_metadata.symbol);

    let got_supply = web30
        .get_erc20_supply(erc20_contract, *MINER_ADDRESS)
        .await
        .unwrap();
    assert_eq!(got_supply, u256!(0));

    erc20_contract
}
