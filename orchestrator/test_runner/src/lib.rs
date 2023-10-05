//! this crate, namely runs all up integration tests of the Gravity code against
//! several scenarios, happy path and non happy path. This is essentially meant
//! to be executed in our specific CI docker container and nowhere else. If you
//! find some function useful pull it up into the more general gravity_utils or the like

#[macro_use]
extern crate log;

use std::{env, time::Duration};

use evidence_based_slashing::evidence_based_slashing;
use gravity_proto::gravity::query_client::QueryClient as GravityQueryClient;
use gravity_utils::{
    clarity::{u256, Address as EthAddress, PrivateKey as EthPrivateKey, Uint256},
    deep_space::{coin::Coin, Contact},
    get_block_delay,
    get_with_retry::get_net_version_with_retry,
    u64_array_bigints, DEFAULT_ADDRESS_PREFIX, GRAVITY_DENOM_PREFIX,
    TEST_DEFAULT_ETH_NODE_ENDPOINT, TEST_DEFAULT_MINER_KEY, TEST_ETH_CHAIN_ID, TEST_FEE_AMOUNT,
    TEST_GAS_LIMIT, TEST_RUN_BLOCK_STIMULATOR, USE_FINALIZATION,
};
use happy_path::happy_path_test;
use happy_path_v2::happy_path_test_v2;
use lazy_static::lazy_static;
use orch_keys::orch_keys;
use relay_market::relay_market_test;
use remote_stress_test::remote_stress_test;
use transaction_stress_test::transaction_stress_test;
use unhalt_bridge::unhalt_bridge_test;
use valset_stress::validator_set_stress_test;

pub use crate::{
    airdrop_proposal::airdrop_proposal_test, bootstrapping::*,
    deposit_overflow::deposit_overflow_test, ethereum_blacklist_test::ethereum_blacklist_test,
    ibc_metadata::ibc_metadata_proposal_test, invalid_events::invalid_events,
    pause_bridge::pause_bridge_test, signature_slashing::signature_slashing_test,
    slashing_delegation::slashing_delegation_test, tx_cancel::send_to_eth_and_cancel, utils::*,
    valset_rewards::valset_rewards_test,
};

mod airdrop_proposal;
mod bootstrapping;
mod deposit_overflow;
mod ethereum_blacklist_test;
mod evidence_based_slashing;
mod happy_path;
mod happy_path_v2;
mod ibc_metadata;
mod invalid_events;
mod orch_keys;
mod pause_bridge;
mod relay_market;
mod remote_stress_test;
mod signature_slashing;
mod slashing_delegation;
mod transaction_stress_test;
mod tx_cancel;
mod unhalt_bridge;
mod utils;
mod valset_rewards;
mod valset_stress;

/// the timeout for individual requests
const OPERATION_TIMEOUT: Duration = Duration::from_secs(120);
/// the timeout for the total system
const TOTAL_TIMEOUT: Duration = Duration::from_secs(3600);

// Retrieve values from runtime ENV vars
lazy_static! {
    pub static ref ADDRESS_PREFIX: String =
        env::var("ADDRESS_PREFIX").unwrap_or_else(|_| DEFAULT_ADDRESS_PREFIX.to_owned());
    pub static ref STAKING_TOKEN: String =
        env::var("STAKING_TOKEN").unwrap_or_else(|_| "stake".to_owned());
    pub static ref COSMOS_NODE_GRPC: String =
        env::var("COSMOS_NODE_GRPC").unwrap_or_else(|_| "http://localhost:9090".to_owned());
    pub static ref COSMOS_NODE_ABCI: String =
        env::var("COSMOS_NODE_ABCI").unwrap_or_else(|_| "http://localhost:26657".to_owned());
    pub static ref ETH_NODE: String =
        env::var("ETH_NODE").unwrap_or_else(|_| TEST_DEFAULT_ETH_NODE_ENDPOINT.to_owned());
    // this key is the private key for the public key defined in tests/assets/ETHGenesis.json
    // where the full node / miner sends its rewards. Therefore it's always going
    // to have a lot of ETH to pay for things like contract deployments
    pub static ref MINER_PRIVATE_KEY: EthPrivateKey = env::var("MINER_PRIVATE_KEY").unwrap_or_else(|_|
        TEST_DEFAULT_MINER_KEY.to_owned()
            ).parse()
            .unwrap();
    pub static ref MINER_ADDRESS: EthAddress = MINER_PRIVATE_KEY.to_address();
}

/// this value reflects the contents of /tests/container-scripts/setup-validator.sh
/// and is used to compute if a stake change is big enough to trigger a validator set
/// update since we want to make several such changes intentionally
pub const STAKE_SUPPLY_PER_VALIDATOR: Uint256 = u256!(1000000000000000000000);
/// this is the amount each validator bonds at startup
pub const STARTING_STAKE_PER_VALIDATOR: Uint256 = STAKE_SUPPLY_PER_VALIDATOR.shr1();

/// Gets the standard non-token fee for the testnet. We deploy the test chain with STAKE
/// and FOOTOKEN balances by default, one footoken is sufficient for any Cosmos tx fee except
/// fees for send_to_eth messages which have to be of the same bridged denom so that the relayers
/// on the Ethereum side can be paid in that token.
///
/// In cases where we are sending back and forth, and we need to send more than we send back, the argument allows multiplying the returned amount
pub fn get_fee() -> Coin {
    Coin {
        denom: get_test_token_name(),
        amount: TEST_FEE_AMOUNT,
    }
}

pub fn get_fee_amount(multiplier: u64) -> Uint256 {
    TEST_FEE_AMOUNT
        .checked_mul(Uint256::from_u64(multiplier))
        .unwrap()
}

pub fn get_deposit() -> Coin {
    Coin {
        denom: STAKING_TOKEN.to_string(),
        amount: u256!(1000000000000000000), // 10^18
    }
}
pub fn get_test_token_name() -> String {
    "footoken".to_string()
}

pub fn get_chain_id() -> String {
    "gravity-test".to_string()
}

pub const ONE_ETH: Uint256 = u256!(1000000000000000000);
pub const ONE_HUNDRED_ETH: Uint256 = u256!(100000000000000000000);

pub fn should_deploy_contracts() -> bool {
    match env::var("DEPLOY_CONTRACTS") {
        Ok(s) => s == "1" || s.to_lowercase() == "yes" || s.to_lowercase() == "true",
        _ => false,
    }
}

pub async fn run_test(
    cosmos_node_grpc: &str,
    cosmos_node_abci: &str,
    eth_node: &str,
    keys: Vec<ValidatorKeys>,
    test_type: &str,
) {
    info!("Starting Gravity test-runner with test_type {test_type}");
    let contact =
        Contact::new(cosmos_node_grpc, OPERATION_TIMEOUT, ADDRESS_PREFIX.as_str()).unwrap();

    info!("Waiting for Cosmos chain to come online");
    wait_for_cosmos_online(&contact, TOTAL_TIMEOUT).await;

    let grpc_client = GravityQueryClient::connect(cosmos_node_grpc.to_owned())
        .await
        .unwrap();
    let web30 = gravity_utils::web30::client::Web3::new(eth_node, OPERATION_TIMEOUT);

    let net_version = get_net_version_with_retry(&web30).await;
    let block_delay = get_block_delay(&web30).await;
    info!(
        "Eth chain ID is {}, Cosmos prefix is {}, denom prefix is {}",
        net_version, *ADDRESS_PREFIX, GRAVITY_DENOM_PREFIX
    );
    if net_version != TEST_ETH_CHAIN_ID {
        warn!("Chain ID is not equal to TEST_ETH_CHAIN_ID");
    }
    if USE_FINALIZATION {
        info!("Using finalization for block delays");
    } else {
        info!(
            "Using probabilistic finality with block delay {}",
            block_delay
        );
    }

    if TEST_RUN_BLOCK_STIMULATOR {
        // if `should_deploy_contracts()` this needs to be running beforehand,
        // because some chains have really strong quiescence
        info!("Starting block stimulator workaround");
        let eth_node = web30.get_url();
        tokio::spawn(async move {
            use std::str::FromStr;
            // we need a duplicate `send_eth_bulk` that uses a different
            // private key and does not wait on transactions, otherwise we
            // conflict with the main runner's nonces and calculations
            async fn send_eth_bulk2(
                amount: Uint256,
                destinations: &[EthAddress],
                web3: &gravity_utils::web30::client::Web3,
            ) {
                let private_key: EthPrivateKey =
                    "0x8075991ce870b93a8870eca0c0f91913d12f47948ca0fd25b49c6fa7cdbeee8b"
                        .to_owned()
                        .parse()
                        .unwrap();
                let pub_key: EthAddress = private_key.to_address();
                let net_version = web3.net_version().await.unwrap();
                let mut nonce = web3.eth_get_transaction_count(pub_key).await.unwrap();
                let mut transactions = Vec::new();
                let gas_price: Uint256 = web3.eth_gas_price().await.unwrap();
                let double = gas_price.checked_mul(u256!(2)).unwrap();
                for address in destinations {
                    let t = gravity_utils::clarity::Transaction {
                        to: *address,
                        nonce,
                        gas_price: double,
                        gas_limit: TEST_GAS_LIMIT,
                        value: amount,
                        data: Vec::new(),
                        signature: None,
                    };
                    let t = t.sign(&private_key, Some(net_version));
                    transactions.push(t);
                    nonce = nonce.checked_add(u256!(1)).unwrap();
                }
                for tx in transactions {
                    // The problem that this is trying to solve, is that if we try and wait
                    // for the transaction in this thread, there are race conditions such
                    // that we can softlock. There are also problems with fluctuating gas
                    // prices and long block production times from batch tests that cause
                    // replacement errors. What we do is simply ignore the transaction ids
                    // and just send a warning if there is an error.
                    if let Err(e) = web3.eth_send_raw_transaction(tx.to_bytes().unwrap()).await {
                        warn!("Block stimulator encountered transaction error: {}", e);
                    }
                }
            }

            // repeatedly send to unrelated addresses
            let web3 = gravity_utils::web30::client::Web3::new(&eth_node, OPERATION_TIMEOUT);
            for i in 0u64.. {
                send_eth_bulk2(
                    u256!(1),
                    // alternate to reduce replacement errors
                    &if (i & 1) == 0 {
                        [
                            EthAddress::from_str("0x798d4Ba9baf0064Ec19eB4F0a1a45785ae9D6DFc")
                                .unwrap(),
                        ]
                    } else {
                        [
                            EthAddress::from_str("0xFf64d3F6efE2317EE2807d223a0Bdc4c0c49dfDB")
                                .unwrap(),
                        ]
                    },
                    &web3,
                )
                .await;
                tokio::time::sleep(Duration::from_secs(4)).await;
            }
        });
    }

    // if we detect this env var we are only deploying contracts, do that then exit.
    if should_deploy_contracts() {
        // prevents the node deployer from failing (rarely) when the chain has not
        // yet produced the next block after submitting each eth address
        contact.wait_for_next_block(TOTAL_TIMEOUT).await.unwrap();
        info!("test-runner in contract deploying mode, deploying contracts, then exiting");
        deploy_contracts(cosmos_node_abci, eth_node, env::var("GRAVITY_ADDRESS").ok()).await;
        return;
    }

    let contracts = parse_contract_addresses();
    // the address of the deployed Gravity contract
    let gravity_address = contracts.gravity_contract;
    // addresses of deployed ERC20 token contracts to be used for testing
    let erc20_addresses = contracts.erc20_addresses;

    if !keys.is_empty() {
        // before we start the orchestrators send them some funds so they can pay
        // for things
        send_eth_to_orchestrators(&keys, &web30).await;

        assert!(contact
            .get_balance(
                keys[0]
                    .validator_key
                    .to_address(&contact.get_prefix())
                    .unwrap(),
                get_test_token_name(),
            )
            .await
            .unwrap()
            .is_some());
    }

    // This segment contains optional tests, by default we run a happy path test
    // this tests all major functionality of Gravity once or twice.
    // VALSET_STRESS sends in 1k valsets to sign and update
    // BATCH_STRESS fills several batches and executes an out of order batch
    // VALIDATOR_OUT simulates a validator not participating in the happy path test
    // V2_HAPPY_PATH runs the happy path tests but focusing on moving Cosmos assets to Ethereum
    // ARBITRARY_LOGIC tests the arbitrary logic functionality, where an arbitrary contract call
    //                 is created and deployed vai the bridge.
    match test_type {
        "HAPPY_PATH" => {
            info!("Starting Happy path test");
            happy_path_test(
                &web30,
                grpc_client,
                &contact,
                keys,
                gravity_address,
                erc20_addresses[0],
                false,
            )
            .await;
        }
        "VALIDATOR_OUT" => {
            info!("Starting Validator out test");
            happy_path_test(
                &web30,
                grpc_client,
                &contact,
                keys,
                gravity_address,
                erc20_addresses[0],
                true,
            )
            .await;
            return;
        }
        "BATCH_STRESS" => {
            let contact =
                Contact::new(cosmos_node_grpc, TOTAL_TIMEOUT, ADDRESS_PREFIX.as_str()).unwrap();
            transaction_stress_test(
                &web30,
                &contact,
                grpc_client,
                keys,
                gravity_address,
                erc20_addresses,
            )
            .await;
            return;
        }
        "REMOTE_STRESS" => {
            let contact =
                Contact::new(cosmos_node_grpc, TOTAL_TIMEOUT, ADDRESS_PREFIX.as_str()).unwrap();
            remote_stress_test(&web30, &contact, keys, gravity_address, erc20_addresses).await;
            return;
        }
        "VALSET_STRESS" => {
            info!("Starting Valset update stress test");
            validator_set_stress_test(&web30, grpc_client, &contact, keys, gravity_address).await;
            return;
        }
        "VALSET_REWARDS" => {
            info!("Starting Valset rewards test");
            valset_rewards_test(&web30, grpc_client, &contact, keys, gravity_address).await;
            return;
        }
        "HAPPY_PATH_V2" => {
            info!("Starting happy path for Gravity v2");
            happy_path_test_v2(&web30, grpc_client, &contact, keys, gravity_address, false).await;
            return;
        }
        "RELAY_MARKET" => {
            info!("Starting relay market tests!");
            relay_market_test(&web30, grpc_client, &contact, keys, gravity_address).await;
            return;
        }
        "ORCHESTRATOR_KEYS" => {
            info!("Starting orchestrator key update tests!");
            orch_keys(grpc_client, &contact, keys).await;
            return;
        }
        "EVIDENCE" => {
            info!("Starting evidence based slashing tests!");
            evidence_based_slashing(&web30, &contact, keys, gravity_address).await;
            return;
        }
        "TXCANCEL" => {
            info!("Starting SendToEth cancellation test!");
            send_to_eth_and_cancel(
                &contact,
                grpc_client,
                &web30,
                keys,
                gravity_address,
                erc20_addresses[0],
            )
            .await;
            return;
        }
        "INVALID_EVENTS" => {
            info!("Starting invalid events test!");
            invalid_events(
                &web30,
                &contact,
                keys,
                gravity_address,
                erc20_addresses[0],
                grpc_client,
            )
            .await;
            return;
        }
        "UNHALT_BRIDGE" => {
            info!("Starting unhalt bridge tests");
            unhalt_bridge_test(
                &web30,
                grpc_client,
                &contact,
                keys,
                gravity_address,
                erc20_addresses[0],
            )
            .await;
            return;
        }
        "PAUSE_BRIDGE" => {
            info!("Starting pause bridge tests");
            pause_bridge_test(
                &web30,
                grpc_client,
                &contact,
                keys,
                gravity_address,
                erc20_addresses[0],
            )
            .await;
            return;
        }
        "DEPOSIT_OVERFLOW" => {
            info!("Starting deposit overflow test!");
            deposit_overflow_test(&web30, &contact, keys, erc20_addresses, grpc_client).await;
            return;
        }
        "ETHEREUM_BLACKLIST" => {
            info!("Starting ethereum blacklist test");
            ethereum_blacklist_test(grpc_client, &contact, keys).await;
            return;
        }
        "AIRDROP_PROPOSAL" => {
            info!("Starting airdrop governance proposal test");
            airdrop_proposal_test(&contact, keys).await;
            return;
        }
        "SIGNATURE_SLASHING" => {
            info!("Starting Signature Slashing test");
            signature_slashing_test(&web30, grpc_client, &contact, keys, gravity_address).await;
            return;
        }
        "SLASHING_DELEGATION" => {
            info!("Starting Slashing Delegation test");
            slashing_delegation_test(&web30, grpc_client, &contact, keys, gravity_address).await;
            return;
        }
        "IBC_METADATA" => {
            info!("Starting IBC metadata proposal test");
            ibc_metadata_proposal_test(gravity_address, keys, grpc_client, &contact, &web30).await;
            return;
        }
        _ => panic!("Err Unknown test type"),
    }
}
