use std::{collections::HashSet, time::Duration};

use cosmos_gravity::{
    query::get_pending_send_to_eth,
    send::{cancel_send_to_eth, send_request_batch, send_to_eth},
};
use ethereum_gravity::{send_to_cosmos::send_to_cosmos, utils::get_tx_batch_nonce};
use futures::future::join_all;
use gravity_proto::gravity::query_client::QueryClient as GravityQueryClient;
use gravity_utils::{
    clarity::Address as EthAddress,
    deep_space::{coin::Coin, Contact},
    web30::{client::Web3, types::SendTxOption},
    TESTS_BATCH_NUM_USERS, TEST_FEE_AMOUNT,
};
use rand::seq::SliceRandom;
use tokio::time::sleep;
use tonic::transport::Channel;

use crate::{get_fee, get_fee_amount, utils::*, ONE_ETH, ONE_HUNDRED_ETH, TOTAL_TIMEOUT};

const TIMEOUT: Duration = Duration::from_secs(120);

/// The number of users we will be simulating for this test, each user
/// will get one token from each token type in erc20_addresses and send it
/// across the bridge to Cosmos as a deposit and then send it back to a different
/// Ethereum address in a transaction batch
/// So the total number of
/// Ethereum sends = (2 * NUM_USERS)
/// ERC20 sends = (erc20_addresses.len() * NUM_USERS)
/// Gravity Deposits = (erc20_addresses.len() * NUM_USERS)
/// Batches executed = erc20_addresses.len() * (NUM_USERS / 100)
const NUM_USERS: usize = TESTS_BATCH_NUM_USERS;

/// Perform a stress test by sending thousands of
/// transactions and producing large batches
#[allow(clippy::too_many_arguments)]
pub async fn transaction_stress_test(
    web30: &Web3,
    contact: &Contact,
    grpc_client: GravityQueryClient<Channel>,
    keys: Vec<ValidatorKeys>,
    gravity_address: EthAddress,
    erc20_addresses: Vec<EthAddress>,
) {
    let mut grpc_client = grpc_client;

    let no_relay_market_config = create_no_batch_requests_config();
    start_orchestrators(keys.clone(), gravity_address, false, no_relay_market_config).await;

    // Generate 100 user keys to send ETH and multiple types of tokens
    let mut user_keys = Vec::new();
    for _ in 0..NUM_USERS {
        user_keys.push(get_user_key());
    }
    // send what will be used for fees
    for user in &user_keys {
        contact
            .send_coins(
                get_fee(),
                Some(get_fee()),
                user.cosmos_address,
                Some(TOTAL_TIMEOUT),
                keys[0].validator_key,
            )
            .await
            .unwrap();
    }
    // the sending eth addresses need Ethereum to send ERC20 tokens to the bridge
    let sending_eth_addresses: Vec<EthAddress> = user_keys.iter().map(|i| i.eth_address).collect();
    // the destination eth addresses need Ethereum to perform a contract call and get their erc20 balances
    let dest_eth_addresses: Vec<EthAddress> =
        user_keys.iter().map(|i| i.eth_dest_address).collect();
    let mut eth_destinations = Vec::new();
    eth_destinations.extend(sending_eth_addresses.clone());
    eth_destinations.extend(dest_eth_addresses);
    send_eth_bulk(ONE_ETH, &eth_destinations, web30).await;
    info!("Sent {} addresses 1 ETH", NUM_USERS);

    // now we need to send all the sending eth addresses erc20's to send
    for token in erc20_addresses.iter() {
        send_erc20_bulk(ONE_HUNDRED_ETH, *token, &sending_eth_addresses, web30).await;
        info!("Sent {} addresses 100 {}", NUM_USERS, token);
    }
    web30.wait_for_next_block(TOTAL_TIMEOUT).await.unwrap();
    for token in erc20_addresses.iter() {
        let mut sends = Vec::new();
        for keys in user_keys.iter() {
            let fut = send_to_cosmos(
                *token,
                gravity_address,
                ONE_HUNDRED_ETH,
                keys.cosmos_address,
                keys.eth_key,
                TIMEOUT,
                web30,
                vec![SendTxOption::GasPriceMultiplier(5.0)],
            );
            sends.push(fut);
        }
        let txids = join_all(sends).await;
        let mut wait_for_txid = Vec::new();
        for txid in txids {
            let wait = web30.wait_for_transaction(txid.unwrap(), TIMEOUT, None);
            wait_for_txid.push(wait);
        }
        let results = join_all(wait_for_txid).await;
        for result in results {
            let result = result.unwrap();
            result.block_number.unwrap();
        }
        info!(
            "Locked 100 {} from {} into the Gravity Ethereum Contract",
            token, NUM_USERS
        );
        web30.wait_for_next_block(TOTAL_TIMEOUT).await.unwrap();
    }

    let check_all_deposists_bridged_to_cosmos = async {
        loop {
            let mut good = true;

            for keys in user_keys.iter() {
                let c_addr = keys.cosmos_address;
                let balances = contact.get_balances(c_addr).await.unwrap();

                for token in erc20_addresses.iter() {
                    let mut found = false;
                    for balance in balances.iter() {
                        if balance.denom.contains(&token.to_string())
                            && balance.amount == ONE_HUNDRED_ETH
                        {
                            found = true;
                        }
                    }
                    if !found {
                        good = false;
                    }
                }
            }

            if good {
                break;
            }

            sleep(Duration::from_secs(5)).await;
        }
    };

    if tokio::time::timeout(TOTAL_TIMEOUT, check_all_deposists_bridged_to_cosmos)
        .await
        .is_err()
    {
        panic!(
            "Failed to perform all {} deposits to Cosmos!",
            user_keys.len() * erc20_addresses.len()
        );
    } else {
        info!(
            "All {} deposits bridged to Cosmos successfully!",
            user_keys.len() * erc20_addresses.len()
        );
    }

    // send back less than we sent over to account for fees we need to pay
    let send_amount = ONE_HUNDRED_ETH.checked_sub(get_fee_amount(9)).unwrap();

    let mut denoms = HashSet::new();
    for token in erc20_addresses.iter() {
        let mut futs = Vec::new();
        for keys in user_keys.iter() {
            let c_addr = keys.cosmos_address;
            let c_key = keys.cosmos_key;
            let e_dest_addr = keys.eth_dest_address;
            let balances = contact.get_balances(c_addr).await.unwrap();
            // this way I don't have to hardcode a denom and we can change the way denoms are formed
            // without changing this test.
            let mut send_coin = None;
            for balance in balances {
                if balance.denom.contains(&token.to_string()) {
                    send_coin = Some(balance.clone());
                    denoms.insert(balance.denom);
                }
            }
            let mut send_coin = send_coin.unwrap();
            send_coin.amount = send_amount;
            let bridge_fee = Coin {
                denom: send_coin.denom.clone(),
                amount: TEST_FEE_AMOUNT,
            };
            let res = send_to_eth(
                c_key,
                e_dest_addr,
                send_coin,
                bridge_fee,
                get_fee(),
                contact,
            );
            futs.push(res);
        }
        let results = join_all(futs).await;
        for result in results {
            let result = result.unwrap();
            trace!("SendToEth result {:?}", result);
        }
        info!(
            "Successfully placed {} {} into the tx pool",
            NUM_USERS, token
        );
    }

    // randomly select a user to cancel their transaction, as part of this test
    // we make sure that this user withdraws absolutely zero tokens
    let mut rng = rand::thread_rng();
    let user_who_cancels = user_keys.choose(&mut rng).unwrap();
    let pending = get_pending_send_to_eth(&mut grpc_client, user_who_cancels.cosmos_address)
        .await
        .unwrap();
    // if batch creation is made automatic this becomes a race condition we'll have to consider
    assert!(pending.transfers_in_batches.is_empty());
    assert!(!pending.unbatched_transfers.is_empty());

    // cancel all outgoing transactions for this user
    for tx in pending.unbatched_transfers {
        let res = cancel_send_to_eth(user_who_cancels.cosmos_key, get_fee(), contact, tx.id)
            .await
            .unwrap();
        info!("{:?}", res);
    }

    contact.wait_for_next_block(TIMEOUT).await.unwrap();

    // check that the cancelation worked
    let pending = get_pending_send_to_eth(&mut grpc_client, user_who_cancels.cosmos_address)
        .await
        .unwrap();
    info!("{:?}", pending);
    assert!(pending.transfers_in_batches.is_empty());
    assert!(pending.unbatched_transfers.is_empty());

    // this user will have someone else attempt to cancel their transaction
    let mut victim = None;
    for key in user_keys.iter() {
        if key != user_who_cancels {
            victim = Some(key);
            break;
        }
    }
    let pending = get_pending_send_to_eth(&mut grpc_client, victim.unwrap().cosmos_address)
        .await
        .unwrap();
    // try to cancel the victims transactions and ensure failure
    for tx in pending.unbatched_transfers {
        let res = cancel_send_to_eth(user_who_cancels.cosmos_key, get_fee(), contact, tx.id).await;
        info!("{:?}", res);
    }

    for denom in denoms {
        info!("Requesting batch for {}", denom);
        let res = send_request_batch(keys[0].validator_key, denom, Some(get_fee()), contact)
            .await
            .unwrap();
        info!("batch request response is {:?}", res);
    }

    let check_withdraws_from_ethereum = async {
        loop {
            let mut good = true;
            let mut found_canceled = false;

            for keys in user_keys.iter() {
                let e_dest_addr = keys.eth_dest_address;
                for token in erc20_addresses.iter() {
                    let bal = get_erc20_balance_safe(*token, web30, e_dest_addr)
                        .await
                        .unwrap();
                    if bal != send_amount {
                        if e_dest_addr == user_who_cancels.eth_address && bal.is_zero() {
                            info!("We successfully found the user who canceled their sends!");
                            found_canceled = true;
                        } else {
                            good = false;
                        }
                    }
                }
            }

            if good && found_canceled {
                info!(
                    "All {} withdraws to Ethereum bridged successfully!",
                    NUM_USERS * erc20_addresses.len()
                );
                break;
            }

            sleep(Duration::from_secs(5)).await;
        }
    };

    if tokio::time::timeout(TOTAL_TIMEOUT, check_withdraws_from_ethereum)
        .await
        .is_err()
    {
        panic!(
            "Failed to perform all {} withdraws to Ethereum!",
            NUM_USERS * erc20_addresses.len()
        );
    }

    // we should find a batch nonce greater than zero since all the batches
    // executed
    for token in erc20_addresses {
        assert!(
            get_tx_batch_nonce(gravity_address, token, keys[0].eth_key.to_address(), web30)
                .await
                .unwrap()
                > 0
        )
    }
}
