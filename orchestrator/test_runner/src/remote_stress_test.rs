use std::{collections::HashSet, env, time::Duration};

use cosmos_gravity::send::send_to_eth;
use ethereum_gravity::send_to_cosmos::send_to_cosmos;
use futures::future::join_all;
use gravity_utils::{
    clarity::{u256, Address as EthAddress, Uint256},
    deep_space::{coin::Coin, Contact},
    num_conversion::print_eth,
    u64_array_bigints,
    web30::{client::Web3, types::SendTxOption},
    TESTS_BATCH_NUM_USERS,
};
use lazy_static::lazy_static;
use tokio::time::sleep;

use crate::{utils::*, ONE_ETH, TOTAL_TIMEOUT};

const TIMEOUT: Duration = Duration::from_secs(60);

lazy_static! {
    /// The number of users we will be simulating for this test, each user
    /// will get one token from each token type in erc20_addresses and send it
    /// across the bridge to Cosmos as a deposit and then send it back to a different
    /// Ethereum address in a transaction batch
    /// So the total number of
    /// Ethereum sends = (2 * NUM_USERS)
    /// ERC20 sends = (erc20_addresses.len() * NUM_USERS)
    /// Gravity Deposits = (erc20_addresses.len() * NUM_USERS)
    /// Batches executed = erc20_addresses.len() * (NUM_USERS / 100)
    static ref NUM_USERS: usize =
        env::var("NUM_USERS").map(|s| s.parse().unwrap()).unwrap_or_else(|_| TESTS_BATCH_NUM_USERS);
    /// default is 0.001 ETH per user
    static ref WEI_PER_USER: Uint256 =
    Uint256::from_u128(env::var("WEI_PER_USER").unwrap_or_else(|_| "1000000000000000".to_string()).parse::<u128>().unwrap());
    /// The number of the iteration to send per user for both eth->cosmos and cosmos->eth
    static ref NUM_OF_SEND_ITERATIONS: usize =
        env::var("NUM_OF_SEND_ITERATIONS").unwrap_or_else(|_| "5".to_string()).parse().unwrap();
}

/// Perform a stress test by sending thousands of
/// transactions and producing large batches
#[allow(clippy::too_many_arguments)]
pub async fn remote_stress_test(
    web30: &Web3,
    contact: &Contact,
    keys: Vec<ValidatorKeys>,
    gravity_address: EthAddress,
    erc20_addresses: Vec<EthAddress>,
) {
    if !keys.is_empty() {
        start_orchestrators(
            keys.clone(),
            gravity_address,
            false,
            create_no_batch_requests_config(),
        )
        .await;
    }

    // Generate user keys to send ETH and multiple types of tokens
    let mut user_keys = Vec::new();
    for _ in 0..*NUM_USERS {
        user_keys.push(get_user_key());
    }

    // the sending eth addresses need Ethereum to send ERC20 tokens to the bridge
    let sending_eth_addresses: Vec<EthAddress> = user_keys.iter().map(|i| i.eth_address).collect();
    // the destination eth addresses need Ethereum to perform a contract call and get their erc20 balances
    let dest_eth_addresses: Vec<EthAddress> =
        user_keys.iter().map(|i| i.eth_dest_address).collect();
    let mut eth_destinations = Vec::new();
    eth_destinations.extend(sending_eth_addresses.clone());
    eth_destinations.extend(dest_eth_addresses);
    info!(
        "Sending {} addresses {}ETH",
        *NUM_USERS,
        print_eth(*WEI_PER_USER)
    );
    send_eth_bulk(*WEI_PER_USER, &eth_destinations, web30).await;
    info!(
        "Sent {} addresses {}ETH",
        *NUM_USERS,
        print_eth(*WEI_PER_USER)
    );

    let total_send_count = *NUM_USERS * erc20_addresses.len() * *NUM_OF_SEND_ITERATIONS;

    let send_to_cosmos_amount = ONE_ETH;
    let send_to_cosmos_amount_total = send_to_cosmos_amount
        .checked_mul(Uint256::from_usize(*NUM_OF_SEND_ITERATIONS))
        .unwrap();
    for token in erc20_addresses.iter() {
        send_erc20_bulk(
            send_to_cosmos_amount_total,
            *token,
            &sending_eth_addresses,
            web30,
        )
        .await;
        info!(
            "Sent {} addresses {}{}",
            *NUM_USERS, send_to_cosmos_amount_total, token
        );
    }
    info!("Waiting for next Ethereum block");
    web30.wait_for_next_block(TOTAL_TIMEOUT).await.unwrap();

    info!("Sending from Ethereum to Cosmos");

    for _ in 0..*NUM_OF_SEND_ITERATIONS {
        for token in erc20_addresses.iter() {
            let mut sends = Vec::new();
            for keys in user_keys.iter() {
                let fut = send_to_cosmos(
                    *token,
                    gravity_address,
                    send_to_cosmos_amount,
                    keys.cosmos_address,
                    keys.eth_key,
                    TIMEOUT,
                    web30,
                    vec![SendTxOption::GasPriceMultiplier(1.5)],
                );
                info!(
                    "Locked {}{} from {} user into the Gravity Ethereum Contract",
                    send_to_cosmos_amount, token, *NUM_USERS
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
                result.unwrap().block_number.unwrap();
            }
        }
        info!("Waiting for next Ethereum block");
        web30.wait_for_next_block(TOTAL_TIMEOUT).await.unwrap();
    }

    let check_all_deposists_bridged_to_cosmos = async {
        loop {
            let mut good = true;
            for keys in user_keys.iter() {
                let c_addr = keys.cosmos_address;
                let balances = contact.get_balances(c_addr).await.unwrap();

                for token in erc20_addresses.iter() {
                    for balance in balances.iter() {
                        if balance.denom.contains(&token.to_string())
                            && balance.amount != send_to_cosmos_amount_total
                        {
                            good = false;
                            info!(
                                "ERC20 denom: {}, user: {}, now: {}, expect: {}",
                                &token.to_string(),
                                c_addr,
                                balance.amount,
                                send_to_cosmos_amount_total
                            );
                        }
                    }
                }
                if balances.len() != erc20_addresses.len() {
                    good = false
                }
            }

            if good {
                break;
            }

            sleep(Duration::from_secs(5)).await;
        }
    };

    info!("Waiting for cosmos deposit");
    if tokio::time::timeout(TOTAL_TIMEOUT, check_all_deposists_bridged_to_cosmos)
        .await
        .is_err()
    {
        panic!(
            "Failed to perform all {} deposits to Cosmos!",
            user_keys.len() * erc20_addresses.len()
        );
    }

    info!(
        "All {} deposits bridged to Cosmos successfully!",
        user_keys.len() * erc20_addresses.len() * *NUM_OF_SEND_ITERATIONS
    );

    info!("Sending from Cosmos to Ethereum");
    let send_to_eth_amount = send_to_cosmos_amount.checked_sub(u256!(500)).unwrap(); // a bit less to keep some for fee
    let send_to_eth_amount_total = send_to_eth_amount
        .checked_mul(Uint256::from_usize(*NUM_OF_SEND_ITERATIONS))
        .unwrap();
    let mut denoms = HashSet::new();
    for _ in 0..*NUM_OF_SEND_ITERATIONS {
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
                send_coin.amount = send_to_eth_amount;
                let send_fee = Coin {
                    denom: send_coin.denom.clone(),
                    amount: u256!(1),
                };
                let res = send_to_eth(
                    c_key,
                    e_dest_addr,
                    send_coin,
                    send_fee.clone(),
                    send_fee,
                    contact,
                );
                futs.push(res);
            }
            let results = join_all(futs).await;
            for result in results {
                result.unwrap();
            }
            info!(
                "Successfully placed {} {} into the tx pool to be send to Ethereum",
                *NUM_USERS, token
            );
        }
    }

    info!("Waiting for next Cosmos block");
    contact.wait_for_next_block(TIMEOUT).await.unwrap();

    let check_withdraws_from_ethereum = async {
        loop {
            let mut good = true;
            for keys in user_keys.iter() {
                let e_dest_addr = keys.eth_dest_address;
                for token in erc20_addresses.iter() {
                    let bal = get_erc20_balance_safe(*token, web30, e_dest_addr)
                        .await
                        .unwrap();
                    if bal != send_to_eth_amount_total {
                        good = false;
                        info!(
                            "ERC20 denom: {}, user: {}, now: {}, expect: {}",
                            *token, e_dest_addr, bal, send_to_eth_amount_total
                        );
                    }
                }

                if good {
                    return;
                }

                sleep(Duration::from_secs(5)).await;
            }
        }
    };

    info!("Waiting for Ethereum deposit");
    if tokio::time::timeout(TOTAL_TIMEOUT, check_withdraws_from_ethereum)
        .await
        .is_err()
    {
        panic!(
            "Failed to perform all {} withdraws to Ethereum!",
            total_send_count
        );
    }
    info!(
        "All {} withdraws to Ethereum bridged successfully!",
        total_send_count
    );
}
