use cosmos_gravity::{
    query::get_pending_send_to_eth,
    send::{cancel_send_to_eth, send_to_eth},
};
use gravity_proto::gravity::query_client::QueryClient as GravityQueryClient;
use gravity_utils::{
    clarity::{u256, Address as EthAddress},
    deep_space::{coin::Coin, Contact},
    u64_array_bigints,
    web30::client::Web3,
};
use tonic::transport::Channel;

use crate::{get_fee, utils::*, validator_out::test_erc20_deposit_panic, ONE_ETH};

// Justin: Here's the method I set up to test out sending and cancelling, but I have not been able to get any transaction ids
// So I have not been able to generate the cancel request
pub async fn send_to_eth_and_cancel(
    contact: &Contact,
    grpc_client: GravityQueryClient<Channel>,
    web30: &Web3,
    keys: Vec<ValidatorKeys>,
    gravity_address: EthAddress,
    erc20_address: EthAddress,
) {
    let mut grpc_client = grpc_client;

    let no_relay_market_config = create_default_test_config();
    start_orchestrators(keys.clone(), gravity_address, false, no_relay_market_config).await;

    // a pair of cosmos and Ethereum keys + addresses to use for this test
    let user_keys = get_user_key();

    test_erc20_deposit_panic(
        web30,
        contact,
        &mut grpc_client,
        user_keys.cosmos_address,
        gravity_address,
        erc20_address,
        ONE_ETH,
        None,
        None,
    )
    .await;

    let coin_to_bridge = Coin {
        denom: convert_to_erc20_denom(erc20_address),
        amount: ONE_ETH.checked_sub(u256!(1_500)).unwrap(),
    };
    let bridge_fee = get_fee();
    let cosmos_tx_fee = get_fee();
    // send some coins to pay fees
    send_cosmos_coins(
        contact,
        keys[0].validator_key,
        vec![user_keys.cosmos_address],
        vec![
            bridge_fee.clone(),
            cosmos_tx_fee.clone(),
            cosmos_tx_fee.clone(),
        ],
    )
    .await;

    info!(
        "Sending {}{} from {} on Cosmos back to Ethereum",
        coin_to_bridge.amount, coin_to_bridge.denom, user_keys.cosmos_address
    );

    let res = send_to_eth(
        user_keys.cosmos_key,
        user_keys.eth_address,
        coin_to_bridge,
        bridge_fee,
        cosmos_tx_fee.clone(),
        contact,
    )
    .await
    .unwrap();
    info!("{:?}", res);
    for thing in res.logs {
        for event in thing.events {
            info!("attribute for {:?}", event.attributes);
        }
    }

    let res = get_pending_send_to_eth(&mut grpc_client, user_keys.cosmos_address)
        .await
        .unwrap();

    let send_to_eth_id = res.unbatched_transfers[0].id;

    cancel_send_to_eth(user_keys.cosmos_key, cosmos_tx_fee, contact, send_to_eth_id)
        .await
        .unwrap();

    let res = get_pending_send_to_eth(&mut grpc_client, user_keys.cosmos_address)
        .await
        .unwrap();

    assert!(res.unbatched_transfers.is_empty());
    info!("Successfully canceled SendToEth!")
}
